// SPDX-License-Identifier: GPL-3.0-only

//! Adapters implementing the domain's port traits. Depends on `domain`
//! (to implement its traits) and `application` (to be wired at the
//! composition root in `interface::app`). Infra types (zbus proxies,
//! `freedesktop-desktop-entry` structs, raw paths) never leak past this
//! module boundary.

pub mod desktop_entry_repo;
pub mod icon_provider;
pub mod logind_power;
pub mod os_release;

pub use desktop_entry_repo::XdgDesktopEntryRepository;
pub use icon_provider::{BundledIcons, FreedesktopIconLookup};
pub use logind_power::LogindPowerControl;
pub use os_release::OsReleaseHostDetector;
