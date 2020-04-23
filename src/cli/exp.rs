//! `exp` CLI sub-command.

use crate::errors::*;
use crate::metadata;
use clap::ArgMatches;
use error_chain::bail;

#[derive(Debug)]
pub enum CliExp {
    NetBootstrap(CliNetBootstrap),
}

impl CliExp {
    /// Parse sub-command into configuration.
    pub(crate) fn parse(app_matches: &ArgMatches) -> Result<super::CliConfig> {
        if app_matches.subcommand_name().is_none() {
            bail!("missing exp subcommand");
        }

        let cfg = match app_matches.subcommand() {
            ("rd-net-bootstrap", Some(matches)) => CliNetBootstrap::parse(matches)?,
            (x, _) => unreachable!("unrecognized exp subcommand '{}'", x),
        };

        Ok(super::CliConfig::Exp(cfg))
    }

    // Run sub-command.
    pub(crate) fn run(&self) -> Result<()> {
        match self {
            CliExp::NetBootstrap(cmd) => cmd.run()?,
        };
        Ok(())
    }
}

/// Sub-command for network bootstrap.
#[derive(Debug)]
pub struct CliNetBootstrap {
    platform: String,
}

impl CliNetBootstrap {
    /// Parse sub-command into configuration.
    pub(crate) fn parse(matches: &ArgMatches) -> Result<CliExp> {
        let platform = super::parse_provider(matches)?;

        let cfg = Self { platform };
        Ok(CliExp::NetBootstrap(cfg))
    }

    /// Run the sub-command.
    pub(crate) fn run(&self) -> Result<()> {
        let provider = metadata::fetch_metadata(&self.platform)?;
        provider.rd_net_bootstrap()?;
        Ok(())
    }
}
