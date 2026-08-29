// SPDX-License-Identifier: GPL-3.0-only

mod application;
mod domain;
mod i18n;
mod infrastructure;
mod interface;

use std::sync::Arc;

/// Composition root: the only place concrete infrastructure adapters are
/// constructed and wired to the domain ports they implement. Everything
/// below (`application`, `interface`) only ever sees the resulting
/// `application::StartMenuService`.
fn main() -> cosmic::iced::Result {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    let service = Arc::new(application::StartMenuService::new(
        Arc::new(infrastructure::XdgDesktopEntryRepository),
        Arc::new(infrastructure::LogindPowerControl),
        Arc::new(infrastructure::OsReleaseHostDetector),
        Arc::new(infrastructure::FreedesktopIconLookup),
        Arc::new(infrastructure::BundledIcons),
    ));

    // Starts the applet's event loop, handing the wired service in as flags.
    cosmic::applet::run::<interface::app::AppModel>(service)
}
