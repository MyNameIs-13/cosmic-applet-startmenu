// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use super::os_kind::OsKind;

/// Port: detect which distribution the applet is running on.
pub trait HostDetector: Send + Sync {
    fn detect(&self) -> OsKind;
}

/// Port: look up an icon by name in the live system icon theme.
pub trait IconLookup: Send + Sync {
    /// Returns the path to the first candidate name found in the theme, if any.
    fn find_first(&self, candidates: &[&str]) -> Option<PathBuf>;
}

/// Port: fetch the bundled fallback icon bytes for a kind.
pub trait EmbeddedIcons: Send + Sync {
    fn bytes(&self, kind: OsKind) -> &'static [u8];
}
