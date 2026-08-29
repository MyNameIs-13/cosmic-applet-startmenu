// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

/// A launchable program, shaped for the UI layer: `Exec` field codes are
/// already stripped and the command is split into program + args so
/// `interface` never has to parse a desktop-entry `Exec` string itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntry {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    /// Icon name or path from the `.desktop` file, if any. Resolved to an
    /// actual icon by `interface` (falling back to a generic icon there),
    /// same as any other XDG icon name.
    pub icon: Option<String>,
}

/// Where to load the start-button/menu-header icon's bytes from.
#[derive(Debug, Clone)]
pub enum StartIcon {
    Path(PathBuf),
    Embedded(&'static [u8]),
}

/// Mirrors `domain::PowerAction` — a bare selector with no domain logic of
/// its own, but kept as a separate type so UI messages never carry a
/// `domain` type across the `interface` boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Shutdown,
    Restart,
    Suspend,
}

impl From<PowerAction> for crate::domain::PowerAction {
    fn from(action: PowerAction) -> Self {
        match action {
            PowerAction::Shutdown => Self::Shutdown,
            PowerAction::Restart => Self::Restart,
            PowerAction::Suspend => Self::Suspend,
        }
    }
}
