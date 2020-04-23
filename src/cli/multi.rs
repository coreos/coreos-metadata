//! `multi` CLI sub-command.

use crate::errors::*;
use crate::metadata;

#[derive(Debug)]
pub struct CliMulti {
    attributes_file: Option<String>,
    check_in: bool,
    hostname_file: Option<String>,
    network_units_dir: Option<String>,
    provider: String,
    ssh_keys_user: Option<String>,
}

impl CliMulti {
    /// Parse flags for the `multi` sub-command.
    pub(crate) fn parse(matches: &clap::ArgMatches) -> Result<super::CliConfig> {
        let provider = super::parse_provider(matches)?;

        let multi = Self {
            attributes_file: matches.value_of("attributes").map(String::from),
            check_in: matches.is_present("check-in"),
            hostname_file: matches.value_of("hostname").map(String::from),
            network_units_dir: matches.value_of("network-units").map(String::from),
            provider,
            ssh_keys_user: matches.value_of("ssh-keys").map(String::from),
        };

        if multi.attributes_file.is_none()
            && multi.network_units_dir.is_none()
            && !multi.check_in
            && multi.ssh_keys_user.is_none()
            && multi.hostname_file.is_none()
            && multi.network_units_dir.is_none()
        {
            slog_scope::warn!("multi: no action specified");
        }

        Ok(super::CliConfig::Multi(multi))
    }

    /// Run the `multi` sub-command.
    pub(crate) fn run(self) -> Result<()> {
        // fetch the metadata from the configured provider
        let metadata = metadata::fetch_metadata(&self.provider)
            .chain_err(|| "fetching metadata from provider")?;

        // write attributes if configured to do so
        self.attributes_file
            .map_or(Ok(()), |x| metadata.write_attributes(x))
            .chain_err(|| "writing metadata attributes")?;

        // write ssh keys if configured to do so
        self.ssh_keys_user
            .map_or(Ok(()), |x| metadata.write_ssh_keys(x))
            .chain_err(|| "writing ssh keys")?;

        // write hostname if configured to do so
        self.hostname_file
            .map_or(Ok(()), |x| metadata.write_hostname(x))
            .chain_err(|| "writing hostname")?;

        // write network units if configured to do so
        self.network_units_dir
            .map_or(Ok(()), |x| metadata.write_network_units(x))
            .chain_err(|| "writing network units")?;

        // perform boot check-in.
        if self.check_in {
            metadata
                .boot_checkin()
                .chain_err(|| "checking-in instance boot to cloud provider")?;
        }

        Ok(())
    }
}
