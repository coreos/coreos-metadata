//! Provider for QEMU instances.

use crate::errors::*;
use crate::providers::MetadataProvider;
use std::path::Path;
use tokio::sync::oneshot;
use tokio::{runtime, time};

/// Default timeout (in seconds) before declaring the check-in attempt failed.
const DEFAULT_CHECKIN_TIMEOUT_SECS: u64 = 10;

/// Provider for QEMU platform.
#[derive(Clone, Debug)]
pub struct QemuProvider {
    /// Timeout (in seconds) before aborting check-in attempt.
    checkin_timeout: u64,
}

impl Default for QemuProvider {
    fn default() -> Self {
        Self {
            checkin_timeout: DEFAULT_CHECKIN_TIMEOUT_SECS,
        }
    }
}

impl QemuProvider {
    /// Timeout for shutting down the Tokio runtime (and any tasks blocked there).
    const TOKIO_TIMEOUT_SECS: u64 = 5;

    /// Create a provider with default settings.
    pub fn try_new() -> Result<Self> {
        Ok(Self::default())
    }

    /// Perform boot checkin over a VirtIO console.
    fn try_checkin(&self) -> Result<()> {
        let mut rt = runtime::Runtime::new()?;
        rt.block_on(self.ovirt_session_startup())?;
        rt.shutdown_timeout(time::Duration::from_secs(Self::TOKIO_TIMEOUT_SECS));
        Ok(())
    }

    async fn ovirt_session_startup(&self) -> Result<()> {
        // Build and initialize the client.
        let builder = tokio_oga::OgaBuilder::default()
            .initial_heartbeat(Some(true))
            .heartbeat_interval(Some(0));
        let mut client = builder.connect().await?;

        let term_chan = client.termination_chan();
        let cmd_chan = client.command_chan();

        // Run until core logic is completed, or client experiences a failure,
        // or timeout is reached. Tasks run concurrently, the quickest one
        // completes, all the others are cancelled.
        tokio::select! {
            res = self.send_startup(cmd_chan) => { res }
            client_err = self.watch_termination(term_chan) => { Err(client_err) }
            _ = self.abort_delayed() => {
                Err("failed to notify startup: timed out".into())
            }
        }
    }

    /// Send a `session-startup` command.
    async fn send_startup(&self, mut ch_outgoing: tokio_oga::OgaCommandSender) -> Result<()> {
        let startup_msg = tokio_oga::commands::SessionStartup::default();
        ch_outgoing.send(Box::new(startup_msg)).await?;
        Ok(())
    }

    /// Process oVirt-client termination errors.
    async fn watch_termination(&self, chan: oneshot::Receiver<tokio_oga::OgaError>) -> Error {
        chan.await
            .unwrap_or_else(|_| "termination event, sender aborted".into())
            .into()
    }

    /// Abort after configured delay.
    async fn abort_delayed(&self) {
        time::delay_for(time::Duration::from_secs(self.checkin_timeout)).await
    }
}

impl MetadataProvider for QemuProvider {
    fn boot_checkin(&self) -> Result<()> {
        let virtio_path = Path::new(tokio_oga::DEFAULT_VIRTIO_PATH);
        if !virtio_path.exists() {
            slog_scope::warn!(
                "skipping boot check-in, no virtual port found at '{}'",
                tokio_oga::DEFAULT_VIRTIO_PATH
            );
            return Ok(());
        }

        self.try_checkin()
    }
}
