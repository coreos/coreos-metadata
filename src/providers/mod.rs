// Copyright 2017 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Providers
//!
//! These are the providers which Afterburn knows how to retrieve metadata
//! from. Internally, they handle the ins and outs of each providers metadata
//! services, and externally, they provide a function to fetch that metadata in
//! a regular format.
//!
//! To add a provider, put a `pub mod provider;` line in this file, export a
//! function to fetch the metadata, and then add a match line in the top-level
//! `fetch_metadata()` function in metadata.rs.

pub mod aliyun;
pub mod aws;
pub mod azure;
pub mod cloudstack;
pub mod digitalocean;
pub mod exoscale;
pub mod gcp;
pub mod ibmcloud;
pub mod ibmcloud_classic;
pub mod openstack;
pub mod packet;
pub mod qemu;
#[cfg(feature = "cl-legacy")]
pub mod vagrant_virtualbox;
pub mod vmware;
pub mod vultr;

use crate::errors::*;
use crate::network;
use libsystemd::logging;
use openssh_keys::PublicKey;
use slog_scope::warn;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use users::{self, User};

#[cfg(not(feature = "cl-legacy"))]
const ENV_PREFIX: &str = "AFTERBURN_";
#[cfg(feature = "cl-legacy")]
const ENV_PREFIX: &str = "COREOS_";
/// Message ID marker for authorized-keys entries in journal.
const AFTERBURN_SSH_AUTHORIZED_KEYS_MESSAGEID: &str = "0f7d7a502f2d433caa1323440a6b4190";

fn create_file(filename: &str) -> Result<File> {
    let file_path = Path::new(&filename);
    // create the directories if they don't exist
    let folder = file_path
        .parent()
        .ok_or_else(|| format!("could not get parent directory of {:?}", file_path))?;
    fs::create_dir_all(&folder).chain_err(|| format!("failed to create directory {:?}", folder))?;
    // create (or truncate) the file we want to write to
    File::create(file_path).chain_err(|| format!("failed to create file {:?}", file_path))
}

/// Add a message to the journal logging SSH key additions; this
/// will be used by at least Fedora CoreOS to display in the console
/// if no ssh keys are present.
fn write_ssh_key_journal_entry(log: logging::Priority, name: &str, path: &str) {
    let message = format!("wrote ssh authorized keys file for user: {}", name);
    let map = maplit::hashmap! {
        "AFTERBURN_USER_NAME" => name.as_ref(),
        "AFTERBURN_PATH" => path.as_ref(),
        "MESSAGE_ID" => AFTERBURN_SSH_AUTHORIZED_KEYS_MESSAGEID,
    };
    if let Err(e) = logging::journal_send(log, &message, map.iter()) {
        warn!("failed to send information to journald: {}", e);
    }
}
#[cfg(feature = "cl-legacy")]
fn write_ssh_keys(user: User, ssh_keys: Vec<PublicKey>) -> Result<()> {
    use update_ssh_keys::{AuthorizedKeyEntry, AuthorizedKeys};

    // If we don't have any SSH keys, don't bother trying to write them as
    // update-ssh-keys will yell at us.
    if !ssh_keys.is_empty() {
        // open the user's authorized keys directory
        let user_name = user.name().to_string_lossy().into_owned();
        let mut authorized_keys_dir = AuthorizedKeys::open(user, true, None).chain_err(|| {
            format!(
                "failed to open authorized keys directory for user '{}'",
                user_name
            )
        })?;

        // add the ssh keys to the directory
        let entries = ssh_keys
            .into_iter()
            .map(|key| AuthorizedKeyEntry::Valid { key })
            .collect::<Vec<_>>();
        // legacy name for legacy mode
        authorized_keys_dir.add_keys("coreos-metadata", entries, true, true)?;

        // write the changes and sync the directory
        authorized_keys_dir
            .write()
            .chain_err(|| "failed to update authorized keys directory")?;
        authorized_keys_dir
            .sync()
            .chain_err(|| "failed to update authorized keys")?;
        let path = authorized_keys_dir.ssh_dir.display().to_string();
        write_ssh_key_journal_entry(logging::Priority::Info, &user_name, &path);
    }

    Ok(())
}

#[cfg(not(feature = "cl-legacy"))]
fn write_ssh_keys(user: User, ssh_keys: Vec<PublicKey>) -> Result<()> {
    use std::io::ErrorKind::NotFound;
    use users::os::unix::UserExt;

    // switch users
    let _guard = users::switch::switch_user_group(user.uid(), user.primary_group_id())
        .chain_err(|| "failed to switch user/group")?;

    // get paths
    let dir_path = user.home_dir().join(".ssh").join("authorized_keys.d");
    let file_name = "afterburn";
    let file_path = &dir_path.join(file_name);

    if !ssh_keys.is_empty() {
        // ensure directory exists
        fs::create_dir_all(&dir_path)
            .chain_err(|| format!("failed to create directory {:?}", &dir_path))?;

        // create temporary file
        let mut temp_file = tempfile::Builder::new()
            .prefix(&format!(".{}-", file_name))
            .tempfile_in(&dir_path)
            .chain_err(|| "failed to create temporary file")?;

        // write out keys
        for key in ssh_keys {
            writeln!(temp_file, "{}", key).chain_err(|| {
                format!("failed to write to file {:?}", temp_file.path().display())
            })?;
        }

        // sync to disk
        temp_file
            .as_file()
            .sync_all()
            .chain_err(|| format!("failed to sync file {:?}", temp_file.path().display()))?;

        // atomically rename to destination
        // don't leak temporary file on error
        temp_file
            .persist(&file_path)
            .map_err(|e| {
                e.file.close().ok();
                e.error
            })
            .chain_err(|| format!("failed to persist file {:?}", file_path.display()))?;
    } else {
        // delete the file
        match fs::remove_file(&file_path) {
            Err(ref e) if e.kind() == NotFound => Ok(()),
            other => other,
        }
        .chain_err(|| format!("failed to remove file {:?}", file_path.display()))?;
    }

    let username = user.name().to_string_lossy();
    let path = file_path.to_string_lossy();
    write_ssh_key_journal_entry(logging::Priority::Info, &username, &path);

    // sync parent dir to persist updates
    match File::open(&dir_path) {
        Ok(dir_file) => dir_file.sync_all(),
        Err(ref e) if e.kind() == NotFound => Ok(()),
        Err(e) => Err(e),
    }
    .chain_err(|| format!("failed to sync '{}'", dir_path.display()))?;

    // make clippy happy while fulfilling our interface
    drop(user);

    Ok(())
}

pub trait MetadataProvider {
    fn attributes(&self) -> Result<HashMap<String, String>> {
        Ok(HashMap::new())
    }

    fn hostname(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn ssh_keys(&self) -> Result<Vec<PublicKey>> {
        warn!("ssh-keys requested, but not supported on this platform");
        Ok(vec![])
    }

    fn networks(&self) -> Result<Vec<network::Interface>> {
        Ok(vec![])
    }

    fn boot_checkin(&self) -> Result<()> {
        warn!("boot check-in requested, but not supported on this platform");
        Ok(())
    }

    /// Return a list of virtual network devices for this machine.
    ///
    /// This is used to setup virtual interfaces, e.g. via [systemd.netdev][netdev]
    /// configuration fragments.
    ///
    /// netdev: https://www.freedesktop.org/software/systemd/man/systemd.netdev.html
    fn virtual_network_devices(&self) -> Result<Vec<network::VirtualNetDev>> {
        Ok(vec![])
    }

    /// Return custom initrd network kernel arguments, if any.
    fn rd_network_kargs(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn write_attributes(&self, attributes_file_path: String) -> Result<()> {
        let mut attributes_file = create_file(&attributes_file_path)?;
        for (k, v) in self.attributes()? {
            writeln!(&mut attributes_file, "{}{}={}", ENV_PREFIX, k, v).chain_err(|| {
                format!("failed to write attributes to file {:?}", attributes_file)
            })?;
        }
        Ok(())
    }

    fn write_ssh_keys(&self, ssh_keys_user: String) -> Result<()> {
        let ssh_keys = self.ssh_keys()?;
        let user = users::get_user_by_name(&ssh_keys_user)
            .ok_or_else(|| format!("could not find user with username {:?}", ssh_keys_user))?;

        write_ssh_keys(user, ssh_keys)?;

        Ok(())
    }

    fn write_hostname(&self, hostname_file_path: String) -> Result<()> {
        match self.hostname()? {
            Some(ref hostname) => {
                let mut hostname_file = create_file(&hostname_file_path)?;
                writeln!(&mut hostname_file, "{}", hostname).chain_err(|| {
                    format!(
                        "failed to write hostname {:?} to file {:?}",
                        hostname, hostname_file
                    )
                })
            }
            None => Ok(()),
        }
    }

    fn write_network_units(&self, network_units_dir: String) -> Result<()> {
        let dir_path = Path::new(&network_units_dir);
        fs::create_dir_all(&dir_path)
            .chain_err(|| format!("failed to create directory {:?}", dir_path))?;

        // Write `.network` fragments for network interfaces/links.
        for interface in &self.networks()? {
            let unit_name = interface.sd_network_unit_name()?;
            let file_path = dir_path.join(unit_name);
            let mut unit_file = File::create(&file_path)
                .chain_err(|| format!("failed to create file {:?}", file_path))?;
            write!(&mut unit_file, "{}", interface.config()).chain_err(|| {
                format!(
                    "failed to write network interface unit file {:?}",
                    unit_file
                )
            })?;
        }

        // Write `.netdev` fragments for virtual network devices.
        for device in &self.virtual_network_devices()? {
            let file_path = dir_path.join(device.netdev_unit_name());
            let mut unit_file = File::create(&file_path)
                .chain_err(|| format!("failed to create netdev unit file {:?}", file_path))?;
            write!(&mut unit_file, "{}", device.sd_netdev_config())
                .chain_err(|| format!("failed to write netdev unit file {:?}", unit_file))?;
        }
        Ok(())
    }
}
