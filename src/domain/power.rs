// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Shutdown,
    Restart,
    Suspend,
}

#[derive(Debug, Clone)]
pub struct PowerError(pub String);

impl fmt::Display for PowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PowerError {}

/// Port: trigger a system power action.
///
/// Implementations must be graceful (give running programs a chance to exit,
/// e.g. by going through systemd-logind rather than an immediate/forced
/// poweroff) and must not themselves prompt for confirmation — that UX
/// belongs to the caller, and this applet's caller deliberately skips it.
#[async_trait::async_trait]
pub trait PowerControl: Send + Sync {
    async fn shutdown(&self) -> Result<(), PowerError>;
    async fn restart(&self) -> Result<(), PowerError>;
    async fn suspend(&self) -> Result<(), PowerError>;
}
