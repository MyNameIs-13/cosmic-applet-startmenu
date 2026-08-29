// SPDX-License-Identifier: GPL-3.0-only

use crate::domain::{EmbeddedIcons, HostDetector, IconLookup};

use super::dto::StartIcon;

/// Pick the icon for the panel button and menu header: the live system's
/// icon theme wins if it has anything for the detected OS, otherwise fall
/// back to the bundled logo (Pop!_OS, NixOS, or Windows as the generic
/// fallback).
pub fn resolve_start_icon(
    detector: &dyn HostDetector,
    icon_lookup: &dyn IconLookup,
    embedded: &dyn EmbeddedIcons,
) -> StartIcon {
    let kind = detector.detect();

    match icon_lookup.find_first(kind.system_icon_candidates()) {
        Some(path) => StartIcon::Path(path),
        None => StartIcon::Embedded(embedded.bytes(kind)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OsKind;
    use std::path::PathBuf;

    struct FixedDetector(OsKind);
    impl HostDetector for FixedDetector {
        fn detect(&self) -> OsKind {
            self.0
        }
    }

    struct NoSystemIcon;
    impl IconLookup for NoSystemIcon {
        fn find_first(&self, _candidates: &[&str]) -> Option<PathBuf> {
            None
        }
    }

    struct FoundSystemIcon(PathBuf);
    impl IconLookup for FoundSystemIcon {
        fn find_first(&self, _candidates: &[&str]) -> Option<PathBuf> {
            Some(self.0.clone())
        }
    }

    struct StubEmbedded;
    impl EmbeddedIcons for StubEmbedded {
        fn bytes(&self, kind: OsKind) -> &'static [u8] {
            match kind {
                OsKind::PopOs => b"pop",
                OsKind::NixOs => b"nix",
                OsKind::Other => b"windows",
            }
        }
    }

    #[test]
    fn prefers_system_icon_when_found() {
        let path = PathBuf::from("/usr/share/icons/hicolor/scalable/apps/distributor-logo.svg");
        let icon = resolve_start_icon(
            &FixedDetector(OsKind::PopOs),
            &FoundSystemIcon(path.clone()),
            &StubEmbedded,
        );
        match icon {
            StartIcon::Path(p) => assert_eq!(p, path),
            StartIcon::Embedded(_) => panic!("expected system icon path"),
        }
    }

    #[test]
    fn falls_back_to_embedded_asset_for_detected_kind() {
        let icon = resolve_start_icon(&FixedDetector(OsKind::NixOs), &NoSystemIcon, &StubEmbedded);
        match icon {
            StartIcon::Embedded(bytes) => assert_eq!(bytes, b"nix"),
            StartIcon::Path(_) => panic!("expected embedded fallback"),
        }
    }

    #[test]
    fn unknown_os_falls_back_to_windows_asset() {
        let icon = resolve_start_icon(&FixedDetector(OsKind::Other), &NoSystemIcon, &StubEmbedded);
        match icon {
            StartIcon::Embedded(bytes) => assert_eq!(bytes, b"windows"),
            StartIcon::Path(_) => panic!("expected embedded fallback"),
        }
    }
}
