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

//! network abstracts away the manipulation of network device and
//! interface unit files. All that is left is to write the resulting string to
//! the necessary unit.

use crate::errors::*;
use error_chain::bail;
use ipnetwork::IpNetwork;
use pnet_base::MacAddr;
use std::net::IpAddr;
use std::string::String;
use std::string::ToString;

mod ip_cli;
pub mod utils;

pub const BONDING_MODE_BALANCE_RR: u32 = 0;
pub const BONDING_MODE_ACTIVE_BACKUP: u32 = 1;
pub const BONDING_MODE_BALANCE_XOR: u32 = 2;
pub const BONDING_MODE_BROADCAST: u32 = 3;
pub const BONDING_MODE_LACP: u32 = 4;
pub const BONDING_MODE_BALANCE_TLB: u32 = 5;
pub const BONDING_MODE_BALANCE_ALB: u32 = 6;

const BONDING_MODES: [(u32, &str); 7] = [
    (BONDING_MODE_BALANCE_RR, "balance-rr"),
    (BONDING_MODE_ACTIVE_BACKUP, "active-backup"),
    (BONDING_MODE_BALANCE_XOR, "balance-xor"),
    (BONDING_MODE_BROADCAST, "broadcast"),
    (BONDING_MODE_LACP, "802.3ad"),
    (BONDING_MODE_BALANCE_TLB, "balance-tlb"),
    (BONDING_MODE_BALANCE_ALB, "balance-alb"),
];

pub fn bonding_mode_to_string(mode: u32) -> Result<String> {
    for &(m, s) in &BONDING_MODES {
        if m == mode {
            return Ok(s.to_owned());
        }
    }
    Err(format!("no such bonding mode: {}", mode).into())
}

/// Try to parse an IP+netmask pair into a CIDR network.
pub fn try_parse_cidr(address: IpAddr, netmask: IpAddr) -> Result<IpNetwork> {
    let prefix = ipnetwork::ip_mask_to_prefix(netmask)?;
    IpNetwork::new(address, prefix).chain_err(|| "failed to parse network")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkRoute {
    pub destination: IpNetwork,
    pub gateway: IpAddr,
}

/// A network interface/link.
///
/// Depending on platforms, an interface may be identified by
/// name or by MAC address (at least one of those must be provided).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    /// Interface name.
    pub name: Option<String>,
    /// Interface MAC address.
    pub mac_address: Option<MacAddr>,
    /// Relative priority for interface configuration.
    pub priority: u8,
    pub nameservers: Vec<IpAddr>,
    pub ip_addresses: Vec<IpNetwork>,
    pub routes: Vec<NetworkRoute>,
    pub bond: Option<String>,
    pub unmanaged: bool,
}

/// A virtual network interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualNetDev {
    pub name: String,
    pub kind: NetDevKind,
    pub mac_address: MacAddr,
    pub priority: Option<u32>,
    pub sd_netdev_sections: Vec<SdSection>,
}

/// A free-form `systemd.netdev` section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdSection {
    pub name: String,
    pub attributes: Vec<(String, String)>,
}

/// Supported virtual network device kinds.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetDevKind {
    /// Parent aggregation for physically bonded devices.
    Bond,
    /// VLAN child interface for a physical device with 802.1Q.
    Vlan,
}

impl NetDevKind {
    /// Return device kind according to `systemd.netdev`.
    ///
    /// See [systemd documentation](kinds) for the full list.
    ///
    /// kinds: https://www.freedesktop.org/software/systemd/man/systemd.netdev.html#Supported%20netdev%20kinds
    fn sd_netdev_kind(&self) -> String {
        let kind = match *self {
            NetDevKind::Bond => "bond",
            NetDevKind::Vlan => "vlan",
        };
        kind.to_string()
    }
}

impl Interface {
    /// Return a deterministic `systemd.network` unit name for this device.
    pub fn sd_network_unit_name(&self) -> Result<String> {
        let iface_name = match (&self.name, &self.mac_address) {
            (Some(ref name), _) => name.clone(),
            (None, Some(ref addr)) => addr.to_string(),
            (None, None) => bail!("network interface without name nor MAC address"),
        };
        let unit_name = format!("{:02}-{}.network", self.priority, iface_name);
        Ok(unit_name)
    }

    pub fn config(&self) -> String {
        let mut config = String::new();

        // [Match] section
        config.push_str("[Match]\n");
        if let Some(name) = self.name.clone() {
            config.push_str(&format!("Name={}\n", name));
        }
        if let Some(mac) = self.mac_address {
            config.push_str(&format!("MACAddress={}\n", mac));
        }

        // [Network] section
        config.push_str("\n[Network]\n");
        for ns in &self.nameservers {
            config.push_str(&format!("DNS={}\n", ns))
        }
        if let Some(bond) = self.bond.clone() {
            config.push_str(&format!("Bond={}\n", bond));
        }

        // [Link] section
        if self.unmanaged {
            config.push_str("\n[Link]\nUnmanaged=yes\n");
        }

        // [Address] sections
        for addr in &self.ip_addresses {
            config.push_str(&format!("\n[Address]\nAddress={}\n", addr));
        }

        // [Route] sections
        for route in &self.routes {
            config.push_str(&format!(
                "\n[Route]\nDestination={}\nGateway={}\n",
                route.destination, route.gateway
            ));
        }

        config
    }

    /// Bring up interfaces and apply network configuration via `ip`.
    pub fn ip_apply(&self) -> Result<()> {
        let name = match self.name {
            Some(ref n) => n,
            None => bail!("missing interface name"),
        };

        // Bring up.
        ip_cli::ip_link_set_up(&name)?;

        // Add addresses.
        for ip_addr in &self.ip_addresses {
            ip_cli::ip_address_add(&name, ip_addr)?;
        }

        // Add routes
        for route in &self.routes {
            ip_cli::ip_route_add(route)?;
        }

        Ok(())
    }
}

impl VirtualNetDev {
    /// Return a deterministic netdev unit name for this device.
    pub fn netdev_unit_name(&self) -> String {
        format!("{:02}-{}.netdev", self.priority.unwrap_or(10), self.name)
    }

    /// Return the `systemd.netdev` configuration fragment for this device.
    pub fn sd_netdev_config(&self) -> String {
        let mut config = String::new();

        // [NetDev] section
        config.push_str("[NetDev]\n");
        config.push_str(&format!("Name={}\n", self.name));
        config.push_str(&format!("Kind={}\n", self.kind.sd_netdev_kind()));
        config.push_str(&format!("MACAddress={}\n", self.mac_address));

        // Custom sections.
        for section in &self.sd_netdev_sections {
            config.push_str(&format!("\n[{}]\n", section.name));
            for attr in &section.attributes {
                config.push_str(&format!("{}={}\n", attr.0, attr.1));
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipnetwork::{Ipv4Network, Ipv6Network};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn mac_addr_display() {
        let m = MacAddr(0xf4, 0x00, 0x34, 0x09, 0x73, 0xee);
        assert_eq!(m.to_string(), "f4:00:34:09:73:ee");
    }

    #[test]
    fn interface_unit_name() {
        let cases = vec![
            (
                Interface {
                    name: Some(String::from("lo")),
                    mac_address: Some(MacAddr(0, 0, 0, 0, 0, 0)),
                    priority: 20,
                    nameservers: vec![],
                    ip_addresses: vec![],
                    routes: vec![],
                    bond: None,
                    unmanaged: false,
                },
                "20-lo.network",
            ),
            (
                Interface {
                    name: Some(String::from("lo")),
                    mac_address: Some(MacAddr(0, 0, 0, 0, 0, 0)),
                    priority: 10,
                    nameservers: vec![],
                    ip_addresses: vec![],
                    routes: vec![],
                    bond: None,
                    unmanaged: false,
                },
                "10-lo.network",
            ),
            (
                Interface {
                    name: None,
                    mac_address: Some(MacAddr(0, 0, 0, 0, 0, 0)),
                    priority: 20,
                    nameservers: vec![],
                    ip_addresses: vec![],
                    routes: vec![],
                    bond: None,
                    unmanaged: false,
                },
                "20-00:00:00:00:00:00.network",
            ),
            (
                Interface {
                    name: Some(String::from("lo")),
                    mac_address: None,
                    priority: 20,
                    nameservers: vec![],
                    ip_addresses: vec![],
                    routes: vec![],
                    bond: None,
                    unmanaged: false,
                },
                "20-lo.network",
            ),
        ];

        for (iface, expected) in cases {
            let unit_name = iface.sd_network_unit_name().unwrap();
            assert_eq!(unit_name, expected);
        }
    }

    #[test]
    fn interface_unit_name_no_name_no_mac() {
        let i = Interface {
            name: None,
            mac_address: None,
            priority: 20,
            nameservers: vec![],
            ip_addresses: vec![],
            routes: vec![],
            bond: None,
            unmanaged: false,
        };
        i.sd_network_unit_name().unwrap_err();
    }

    #[test]
    fn virtual_netdev_unit_name() {
        let ds = vec![
            (
                VirtualNetDev {
                    name: String::from("vlan0"),
                    kind: NetDevKind::Vlan,
                    mac_address: MacAddr(0, 0, 0, 0, 0, 0),
                    priority: Some(20),
                    sd_netdev_sections: vec![],
                },
                "20-vlan0.netdev",
            ),
            (
                VirtualNetDev {
                    name: String::from("vlan0"),
                    kind: NetDevKind::Vlan,
                    mac_address: MacAddr(0, 0, 0, 0, 0, 0),
                    priority: None,
                    sd_netdev_sections: vec![],
                },
                "10-vlan0.netdev",
            ),
        ];

        for (d, s) in ds {
            assert_eq!(d.netdev_unit_name(), s);
        }
    }

    #[test]
    fn interface_config() {
        let is = vec![
            (
                Interface {
                    name: Some(String::from("lo")),
                    mac_address: Some(MacAddr(0, 0, 0, 0, 0, 0)),
                    priority: 20,
                    nameservers: vec![
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    ],
                    ip_addresses: vec![
                        IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 1), 8).unwrap()),
                        IpNetwork::V6(
                            Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 128).unwrap(),
                        ),
                    ],
                    routes: vec![NetworkRoute {
                        destination: IpNetwork::V4(
                            Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 1), 8).unwrap(),
                        ),
                        gateway: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    }],
                    bond: Some(String::from("james")),
                    unmanaged: false,
                },
                "[Match]
Name=lo
MACAddress=00:00:00:00:00:00

[Network]
DNS=127.0.0.1
DNS=::1
Bond=james

[Address]
Address=127.0.0.1/8

[Address]
Address=::1/128

[Route]
Destination=127.0.0.1/8
Gateway=127.0.0.1
",
            ),
            // this isn't really a valid interface object, but it's testing
            // the minimum possible configuration for all peices at the same
            // time, so I'll allow it. (sdemos)
            (
                Interface {
                    name: None,
                    mac_address: None,
                    priority: 10,
                    nameservers: vec![],
                    ip_addresses: vec![],
                    routes: vec![],
                    bond: None,
                    unmanaged: false,
                },
                "[Match]

[Network]
",
            ),
        ];

        for (i, s) in is {
            assert_eq!(i.config(), s);
        }
    }

    #[test]
    fn virtual_netdev_config() {
        let ds = vec![
            (
                VirtualNetDev {
                    name: String::from("vlan0"),
                    kind: NetDevKind::Vlan,
                    mac_address: MacAddr(0, 0, 0, 0, 0, 0),
                    priority: Some(20),
                    sd_netdev_sections: vec![
                        SdSection {
                            name: String::from("Test"),
                            attributes: vec![
                                (String::from("foo"), String::from("bar")),
                                (String::from("oingo"), String::from("boingo")),
                            ],
                        },
                        SdSection {
                            name: String::from("Empty"),
                            attributes: vec![],
                        },
                    ],
                },
                "[NetDev]
Name=vlan0
Kind=vlan
MACAddress=00:00:00:00:00:00

[Test]
foo=bar
oingo=boingo

[Empty]
",
            ),
            (
                VirtualNetDev {
                    name: String::from("vlan0"),
                    kind: NetDevKind::Vlan,
                    mac_address: MacAddr(0, 0, 0, 0, 0, 0),
                    priority: Some(20),
                    sd_netdev_sections: vec![],
                },
                "[NetDev]
Name=vlan0
Kind=vlan
MACAddress=00:00:00:00:00:00
",
            ),
        ];

        for (d, s) in ds {
            assert_eq!(d.sd_netdev_config(), s);
        }
    }
}
