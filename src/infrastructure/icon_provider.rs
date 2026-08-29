// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use crate::domain::{EmbeddedIcons, IconLookup, OsKind};

/// Looks up an icon name in the live system icon theme (falling back to
/// `hicolor`, where most distro branding packages also install a copy).
pub struct FreedesktopIconLookup;

impl IconLookup for FreedesktopIconLookup {
    fn find_first(&self, candidates: &[&str]) -> Option<PathBuf> {
        let active_theme = freedesktop_icons::default_theme_gtk().unwrap_or_else(|| "hicolor".to_string());
        let themes: Vec<&str> = if active_theme == "hicolor" {
            vec!["hicolor"]
        } else {
            vec![active_theme.as_str(), "hicolor"]
        };

        for name in candidates {
            for theme in &themes {
                if let Some(path) = freedesktop_icons::lookup(name)
                    .with_theme(theme)
                    .with_cache()
                    .force_svg()
                    .find()
                {
                    return Some(path);
                }
            }
        }
        None
    }
}

const POP_OS: &[u8] = include_bytes!("../../resources/branding/pop-os.svg");
const NIXOS: &[u8] = include_bytes!("../../resources/branding/nixos.svg");
const WINDOWS: &[u8] = include_bytes!("../../resources/branding/windows.svg");

/// Bundled logos shipped in the binary, used when the live system has no
/// distro-logo icon installed.
pub struct BundledIcons;

impl EmbeddedIcons for BundledIcons {
    fn bytes(&self, kind: OsKind) -> &'static [u8] {
        match kind {
            OsKind::PopOs => POP_OS,
            OsKind::NixOs => NIXOS,
            OsKind::Other => WINDOWS,
        }
    }
}
