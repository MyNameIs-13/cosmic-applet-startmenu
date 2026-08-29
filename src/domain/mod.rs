// SPDX-License-Identifier: GPL-3.0-only

//! Entities, value objects, and port traits. Zero dependencies on
//! application/infrastructure/interface or any UI/IPC crate.

pub mod app_entry;
pub mod host_icon;
pub mod os_kind;
pub mod power;

pub use app_entry::{AppEntry, AppEntryRepository};
pub use host_icon::{EmbeddedIcons, HostDetector, IconLookup};
pub use os_kind::OsKind;
pub use power::{PowerAction, PowerControl, PowerError};
