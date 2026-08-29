// SPDX-License-Identifier: GPL-3.0-only

use zbus::{proxy, Connection};

use crate::domain::{PowerControl, PowerError};

/// Minimal proxy for the three `org.freedesktop.login1.Manager` methods this
/// applet needs. `PowerOff`/`Reboot`/`Suspend` go through the normal systemd
/// shutdown/sleep targets, which is what makes them graceful (running units
/// get `SIGTERM` and `InhibitDelayMaxSec` to exit cleanly) without this
/// applet having to orchestrate that itself.
#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LogindManager {
    /// `interactive: true` lets polkit prompt for authentication if the
    /// calling session isn't already permitted — it does not add a
    /// confirmation dialog or delay of its own.
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
}

pub struct LogindPowerControl;

async fn connect() -> Result<Connection, PowerError> {
    Connection::system()
        .await
        .map_err(|err| PowerError(format!("could not connect to the system bus: {err}")))
}

#[async_trait::async_trait]
impl PowerControl for LogindPowerControl {
    async fn shutdown(&self) -> Result<(), PowerError> {
        let connection = connect().await?;
        let manager = LogindManagerProxy::new(&connection)
            .await
            .map_err(|err| PowerError(format!("could not reach logind: {err}")))?;
        manager
            .power_off(true)
            .await
            .map_err(|err| PowerError(format!("shutdown failed: {err}")))
    }

    async fn restart(&self) -> Result<(), PowerError> {
        let connection = connect().await?;
        let manager = LogindManagerProxy::new(&connection)
            .await
            .map_err(|err| PowerError(format!("could not reach logind: {err}")))?;
        manager
            .reboot(true)
            .await
            .map_err(|err| PowerError(format!("restart failed: {err}")))
    }

    async fn suspend(&self) -> Result<(), PowerError> {
        let connection = connect().await?;
        let manager = LogindManagerProxy::new(&connection)
            .await
            .map_err(|err| PowerError(format!("could not reach logind: {err}")))?;
        manager
            .suspend(true)
            .await
            .map_err(|err| PowerError(format!("suspend failed: {err}")))
    }
}
