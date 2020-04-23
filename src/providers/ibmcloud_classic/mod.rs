//! Metadata fetcher for IBMCloud (Classic) instances.
//!
//! This provider supports the "Classic" infrastructure type on IBMCloud.
//! It provides a config-drive as the only metadata source, whose layout
//! follows the `cloud-init ConfigDrive v2` [datasource][configdrive], with
//! the following details:
//!  - disk filesystem label is `config-2` (lowercase)
//!  - filesystem is `vfat`
//!  - drive contains a single directory at `/openstack/latest/`
//!  - content is exposed as JSON files called `meta_data.json`, `network_data.json`, and `vendor_data.json`.
//!
//! configdrive: https://cloudinit.readthedocs.io/en/latest/topics/datasources/configdrive.html

use error_chain::bail;
use openssh_keys::PublicKey;
use pnet_base::MacAddr;
use serde::Deserialize;
use slog_scope::warn;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::errors::*;
use crate::network;
use crate::providers::MetadataProvider;

// Filesystem label for the Config Drive.
static CONFIG_DRIVE_FS_LABEL: &str = "config-2";

// Filesystem type for the Config Drive.
static CONFIG_DRIVE_FS_TYPE: &str = "vfat";

/// IBMCloud provider (Classic).
#[derive(Debug)]
pub struct IBMClassicProvider {
    /// Path to the top directory of the mounted config-drive.
    drive_path: PathBuf,
    /// Temporary directory for own mountpoint.
    temp_dir: TempDir,
}

/// Partial object for `meta_data.json`
#[derive(Debug, Deserialize)]
pub struct MetaDataJSON {
    /// Fully-Qualified Domain Name (FQDN).
    #[serde(rename = "hostname")]
    pub fqdn: String,
    /// Local hostname.
    #[serde(rename = "name")]
    pub local_hostname: String,
    /// Instance ID (UUID).
    #[serde(rename = "uuid")]
    pub instance_id: String,
    /// SSH public keys.
    pub public_keys: HashMap<String, String>,
}

/// Partial object for `network_data.json`
#[derive(Debug, Deserialize)]
pub struct NetworkDataJSON {
    pub links: Vec<NetLinkJSON>,
    pub networks: Vec<NetNetworkJSON>,
    pub services: Vec<NetServiceJSON>,
}

/// JSON entry in `links` array.
#[derive(Debug, Deserialize)]
pub struct NetLinkJSON {
    pub name: String,
    pub id: String,
    #[serde(rename = "ethernet_mac_address")]
    pub mac_addr: String,
}

/// JSON entry in `networks` array.
#[derive(Debug, Deserialize)]
pub struct NetNetworkJSON {
    /// Unique network ID.
    pub id: String,
    /// Network type (e.g. `ipv4`)
    #[serde(rename = "type")]
    pub kind: String,
    /// Reference to the underlying interface (see `NetLinkJSON.id`)
    pub link: String,
    /// IP network address.
    pub ip_address: IpAddr,
    /// IP network mask.
    pub netmask: IpAddr,
    /// Routable networks.
    pub routes: Vec<NetRouteJSON>,
}

/// JSON entry in `networks.routes` array.
#[derive(Debug, Deserialize)]
pub struct NetRouteJSON {
    /// Route network address.
    pub network: IpAddr,
    /// Route netmask.
    pub netmask: IpAddr,
    /// Route gateway.
    pub gateway: IpAddr,
}

/// JSON entry in `services` array.
#[derive(Debug, Deserialize)]
pub struct NetServiceJSON {
    #[serde(rename = "type")]
    pub kind: String,
    pub address: IpAddr,
}

impl IBMClassicProvider {
    /// Try to build a new provider client.
    ///
    /// This internally tries to mount (and own) the config-drive.
    pub fn try_new() -> Result<Self> {
        let target = tempfile::Builder::new()
            .prefix("afterburn-")
            .tempdir()
            .chain_err(|| "failed to create temporary directory")?;
        crate::util::mount_ro(
            &Path::new("/dev/disk/by-label/").join(CONFIG_DRIVE_FS_LABEL),
            target.path(),
            CONFIG_DRIVE_FS_TYPE,
            3, // maximum retries
        )?;

        let provider = Self {
            drive_path: target.path().to_owned(),
            temp_dir: target,
        };
        Ok(provider)
    }

    /// Return the path to the metadata directory.
    fn metadata_dir(&self) -> PathBuf {
        let drive = self.drive_path.clone();
        drive.join("openstack").join("latest")
    }

    /// Read and parse metadata file.
    fn read_metadata(&self) -> Result<MetaDataJSON> {
        let filename = self.metadata_dir().join("meta_data.json");
        let file =
            File::open(&filename).chain_err(|| format!("failed to open file '{:?}'", filename))?;
        let bufrd = BufReader::new(file);
        Self::parse_metadata(bufrd)
    }

    /// Parse metadata attributes.
    ///
    /// Metadata file contains a JSON object, corresponding to `MetaDataJSON`.
    fn parse_metadata<T: Read>(input: BufReader<T>) -> Result<MetaDataJSON> {
        serde_json::from_reader(input).chain_err(|| "failed parse JSON metadata")
    }

    /// Extract supported metadata values and convert to Afterburn attributes.
    ///
    /// The `AFTERBURN_` prefix is added later on, so it is not part of the
    /// key-labels here.
    fn known_attributes(metadata: MetaDataJSON) -> Result<HashMap<String, String>> {
        if metadata.instance_id.is_empty() {
            bail!("empty instance ID");
        }

        if metadata.local_hostname.is_empty() {
            bail!("empty local hostname");
        }

        let attrs = maplit::hashmap! {
            "IBMCLOUD_CLASSIC_INSTANCE_ID".to_string() => metadata.instance_id,
            "IBMCLOUD_CLASSIC_LOCAL_HOSTNAME".to_string() => metadata.local_hostname,

        };
        Ok(attrs)
    }

    /// Read and parse network configuration.
    fn read_network_data(&self) -> Result<NetworkDataJSON> {
        let filename = self.metadata_dir().join("network_data.json");
        let file =
            File::open(&filename).chain_err(|| format!("failed to open file '{:?}'", filename))?;
        let bufrd = BufReader::new(file);
        Self::parse_network_data(bufrd)
    }

    /// Parse network configuration.
    ///
    /// Network configuration file contains a JSON object, corresponding to `NetworkDataJSON`.
    fn parse_network_data<T: Read>(input: BufReader<T>) -> Result<NetworkDataJSON> {
        serde_json::from_reader(input).chain_err(|| "failed parse JSON network data")
    }

    /// Transform network JSON data into a set of interface configurations.
    fn network_interfaces(input: NetworkDataJSON) -> Result<Vec<network::Interface>> {
        use std::str::FromStr;

        // Validate links and parse them into a map, keyed by id.
        let mut devices: HashMap<String, (String, MacAddr)> =
            HashMap::with_capacity(input.links.len());
        for dev in input.links {
            let mac = MacAddr::from_str(&dev.mac_addr)?;
            devices.insert(dev.id, (dev.name, mac));
        }

        // Parse resolvers.
        let nameservers: Vec<IpAddr> = input
            .services
            .into_iter()
            .filter_map(|svc| {
                if svc.kind == "dns" {
                    Some(svc.address)
                } else {
                    None
                }
            })
            .collect();

        let mut output = Vec::with_capacity(input.networks.len());
        for net in input.networks {
            // Ensure that the referenced link exists.
            let (name, mac_addr) = match devices.get(&net.link) {
                Some(dev) => (dev.0.clone(), dev.1),
                None => continue,
            };

            // Assemble network CIDR.
            let ip_net = network::try_parse_cidr(net.ip_address, net.netmask)?;

            // Parse network routes.
            let mut routes = Vec::with_capacity(net.routes.len());
            for entry in net.routes {
                let destination = network::try_parse_cidr(entry.network, entry.netmask)?;
                let route = network::NetworkRoute {
                    destination,
                    gateway: entry.gateway,
                };
                routes.push(route);
            }

            let iface = network::Interface {
                name: Some(name),
                mac_address: Some(mac_addr),
                priority: 10,
                nameservers: nameservers.clone(),
                ip_addresses: vec![ip_net],
                routes,
                bond: None,
                unmanaged: false,
            };
            output.push(iface);
        }

        output.shrink_to_fit();
        Ok(output)
    }
}

impl MetadataProvider for IBMClassicProvider {
    fn attributes(&self) -> Result<HashMap<String, String>> {
        let metadata = self.read_metadata()?;
        Self::known_attributes(metadata)
    }

    fn hostname(&self) -> Result<Option<String>> {
        let metadata = self.read_metadata()?;
        let hostname = if metadata.local_hostname.is_empty() {
            None
        } else {
            Some(metadata.local_hostname)
        };
        Ok(hostname)
    }

    fn ssh_keys(&self) -> Result<Vec<PublicKey>> {
        warn!("cloud SSH keys requested, but not supported on this platform");
        Ok(vec![])
    }

    fn networks(&self) -> Result<Vec<network::Interface>> {
        let data = self.read_network_data()?;
        let interfaces = Self::network_interfaces(data)?;
        Ok(interfaces)
    }

    fn virtual_network_devices(&self) -> Result<Vec<network::VirtualNetDev>> {
        warn!("virtual network devices metadata requested, but not supported on this platform");
        Ok(vec![])
    }

    fn boot_checkin(&self) -> Result<()> {
        warn!("boot check-in requested, but not supported on this platform");
        Ok(())
    }

    fn rd_net_bootstrap(&self) -> Result<()> {
        let net_ifaces = self.networks()?;
        let mut nameservers = vec![];

        // Configure network.
        for iface in net_ifaces {
            iface.ip_apply()?;

            // Collect nameservers for later.
            for ns in iface.nameservers {
                if !nameservers.contains(&ns) {
                    nameservers.push(ns);
                }
            }
        }

        // Configure DNS resolvers.
        let mut resolvconf = File::create("/etc/resolv.conf")?;
        crate::network::utils::write_resolvconf(&mut resolvconf, &nameservers)?;

        Ok(())
    }
}

impl Drop for IBMClassicProvider {
    fn drop(&mut self) {
        if let Err(e) = crate::util::unmount(
            self.temp_dir.path(),
            3, // maximum retries
        ) {
            slog_scope::error!("failed to unmount ibmcloud (Classic) config-drive: {}", e);
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_basic_attributes() {
        let metadata = r#"
{
  "hostname": "test_instance-classic.foo.cloud",
  "name": "test_instance-classic",
  "uuid": "3c9085db-3eba-4ef2-9d97-d3ffcff6fffe",
  "public_keys": {
    "1602320": "ssh-rsa AAAA foo@example.com"
  }
}
"#;

        let bufrd = BufReader::new(Cursor::new(metadata));
        let parsed = IBMClassicProvider::parse_metadata(bufrd).unwrap();
        assert_eq!(parsed.instance_id, "3c9085db-3eba-4ef2-9d97-d3ffcff6fffe",);
        assert_eq!(parsed.local_hostname, "test_instance-classic",);

        let attrs = IBMClassicProvider::known_attributes(parsed).unwrap();
        assert_eq!(attrs.len(), 2);
        assert_eq!(
            attrs.get("IBMCLOUD_CLASSIC_INSTANCE_ID"),
            Some(&"3c9085db-3eba-4ef2-9d97-d3ffcff6fffe".to_string())
        );
        assert_eq!(
            attrs.get("IBMCLOUD_CLASSIC_LOCAL_HOSTNAME"),
            Some(&"test_instance-classic".to_string())
        );
    }

    #[test]
    fn test_parse_metadata_json() {
        let fixture = File::open("./tests/fixtures/ibmcloud-classic/meta_data.json").unwrap();
        let bufrd = BufReader::new(fixture);
        let parsed = IBMClassicProvider::parse_metadata(bufrd).unwrap();

        assert!(!parsed.instance_id.is_empty());
        assert!(!parsed.local_hostname.is_empty());
        assert!(!parsed.public_keys.is_empty());
    }

    #[test]
    fn test_parse_network_data_json() {
        let fixture = File::open("./tests/fixtures/ibmcloud-classic/network_data.json").unwrap();
        let bufrd = BufReader::new(fixture);
        let parsed = IBMClassicProvider::parse_network_data(bufrd).unwrap();

        let interfaces = IBMClassicProvider::network_interfaces(parsed).unwrap();
        assert_eq!(interfaces.len(), 2);
        assert_eq!(interfaces[0].routes.len(), 3);
        assert_eq!(interfaces[1].routes.len(), 1);

        for entry in interfaces {
            assert_eq!(entry.nameservers.len(), 2);
        }
    }
}
