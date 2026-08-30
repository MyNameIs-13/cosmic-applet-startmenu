// SPDX-License-Identifier: GPL-3.0-only

//! Self-heals a gap in `cosmic-config`'s theme data that shows up when this
//! applet is built against a newer libcosmic than the rest of the system.
//!
//! `cosmic-config` resolves an unset user key by falling back first to an
//! older schema version's user override, then to a system-default file
//! shipped by the OS packaging for the current schema version. When
//! *neither* exists for a key — this applet's schema version case, since
//! the installed `cosmic-settings` never shipped a default for it — the
//! read is reported as `NoConfigDirectory`, which `cosmic_theme` treats as
//! "nothing to apply" rather than a real error. So `Theme::get_entry()`
//! reports success while silently leaving every field at `Theme::default()`
//! — `dark_default()` — regardless of the system's actual light/dark
//! preference (`ThemeMode::is_dark`, a separate, unaffected config schema
//! that reads correctly). The only externally visible symptom is the
//! resolved `Theme::is_dark` disagreeing with which config we asked for.
//!
//! Rather than overriding the theme in memory for this process only, this
//! writes the same values `Theme::light_default()`/`Theme::dark_default()`
//! would already produce as this user's config, once. From then on,
//! ordinary unmodified libcosmic reads succeed on their own, for this
//! applet or any other on the same schema version.
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::cosmic_theme::Theme;

/// Populates the light and dark theme config for the current schema
/// version if it's missing, so `Theme::get_active()` stops silently
/// resolving to a hardcoded dark palette. Safe to call on every launch: a
/// no-op once the config is present, whether we wrote it or the system did.
pub fn heal_missing_defaults() {
    heal(Theme::light_config, Theme::light_default, false);
    heal(Theme::dark_config, Theme::dark_default, true);
}

fn heal(
    config: fn() -> Result<cosmic::cosmic_config::Config, cosmic::cosmic_config::Error>,
    default: fn() -> Theme,
    expected_is_dark: bool,
) {
    let Ok(config) = config() else {
        return;
    };
    let resolved_is_dark = match Theme::get_entry(&config) {
        Ok(theme) | Err((_, theme)) => theme.is_dark,
    };
    if resolved_is_dark != expected_is_dark {
        if let Err(why) = default().write_entry(&config) {
            eprintln!("cosmic-applet-startmenu: failed to repair theme config: {why}");
        }
    }
}
