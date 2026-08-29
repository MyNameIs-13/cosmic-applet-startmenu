// SPDX-License-Identifier: GPL-3.0-only

use freedesktop_desktop_entry::{desktop_entries, get_languages_from_env};

use crate::domain::{AppEntry, AppEntryRepository};

/// Scans the standard XDG application directories for `.desktop` files.
/// Does I/O and type mapping only — filtering and sorting for display is an
/// `application`-layer concern (see `application::list_app_entries`).
pub struct XdgDesktopEntryRepository;

impl AppEntryRepository for XdgDesktopEntryRepository {
    fn all(&self) -> Vec<AppEntry> {
        let locales = get_languages_from_env();

        desktop_entries(&locales)
            .into_iter()
            .filter(|entry| {
                !entry.no_display() && !entry.hidden() && entry.type_() == Some("Application")
            })
            .filter_map(|entry| {
                let name = entry.name(&locales)?.into_owned();
                let exec = entry.exec()?.to_string();
                let icon = entry.icon().map(str::to_string);
                Some(AppEntry { name, exec, icon })
            })
            .collect()
    }
}
