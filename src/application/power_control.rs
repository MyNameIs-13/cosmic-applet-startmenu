// SPDX-License-Identifier: GPL-3.0-only

use crate::domain::{PowerAction, PowerControl};

/// Dispatch a power action to the port, flattening the domain error to a
/// `String` — the only thing `interface` does with it is show it, so there's
/// no reason to hand it a domain error type.
pub async fn perform_power_action(control: &dyn PowerControl, action: PowerAction) -> Result<(), String> {
    let result = match action {
        PowerAction::Shutdown => control.shutdown().await,
        PowerAction::Restart => control.restart().await,
        PowerAction::Suspend => control.suspend().await,
    };
    result.map_err(|err| err.to_string())
}
