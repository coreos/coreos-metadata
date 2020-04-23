//! Helpers for shelling out to the `ip` command.

use crate::errors::*;
use error_chain::bail;
use ipnetwork::IpNetwork;
use slog_scope::trace;
use std::process::Command;

/// Create a new interface.
#[allow(dead_code)]
pub(crate) fn ip_link_add(dev_name: &str, mac_addr: &str) -> Result<()> {
    let link_type = "ether";
    let mut cmd = Command::new("ip");
    cmd.args(&["link", "add"])
        .arg(&dev_name)
        .arg("address")
        .arg(&mac_addr)
        .args(&["type", link_type]);
    try_exec(cmd).chain_err(|| "'ip link add' failed")
}

/// Bring up a named interface.
pub(crate) fn ip_link_set_up(dev_name: &str) -> Result<()> {
    let mut cmd = Command::new("ip");
    cmd.args(&["link", "set"])
        .args(&["dev", dev_name])
        .arg("up");
    try_exec(cmd).chain_err(|| "'ip link set up' failed")
}

/// Add an address to an interface.
pub(crate) fn ip_address_add(dev_name: &str, ip_addr: &IpNetwork) -> Result<()> {
    let mut cmd = Command::new("ip");
    cmd.args(&["address", "add"])
        .arg(ip_addr.to_string())
        .args(&["dev", dev_name]);
    try_exec(cmd).chain_err(|| "'ip address add' failed")
}

/// Add a route.
pub(crate) fn ip_route_add(route: &super::NetworkRoute) -> Result<()> {
    let mut cmd = Command::new("ip");
    cmd.args(&["route", "add"])
        .arg(&route.destination.to_string())
        .args(&["via", &route.gateway.to_string()]);
    try_exec(cmd).chain_err(|| "'ip route add' failed")
}

/// Try to execute, and log stderr on failure.
fn try_exec(cmd: Command) -> Result<()> {
    let mut cmd = cmd;
    trace!("{:?}", &cmd);

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{}", stderr);
    };

    Ok(())
}
