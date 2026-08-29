// SPDX-License-Identifier: GPL-3.0-only

/// The host distribution, as far as this applet's icon selection cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    PopOs,
    NixOs,
    /// Anything else — falls back to the bundled Windows logo, per the
    /// "classic start menu" theme.
    Other,
}

impl OsKind {
    /// Map an `/etc/os-release` `ID` field to a kind.
    pub fn from_os_release_id(id: &str) -> Self {
        match id.trim().trim_matches('"') {
            "pop" => Self::PopOs,
            "nixos" => Self::NixOs,
            _ => Self::Other,
        }
    }

    /// XDG icon theme names to try, most specific first, before falling back
    /// to a bundled asset. Distro branding packages commonly ship
    /// `distributor-logo-<id>` (and symlink the generic `distributor-logo` to
    /// whichever is active), so both are worth trying.
    pub fn system_icon_candidates(self) -> &'static [&'static str] {
        match self {
            Self::PopOs => &["distributor-logo-pop-os", "distributor-logo"],
            Self::NixOs => &["distributor-logo-nixos", "nix-snowflake", "distributor-logo"],
            Self::Other => &["distributor-logo"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_pop_os_and_nixos() {
        assert_eq!(OsKind::from_os_release_id("pop"), OsKind::PopOs);
        assert_eq!(OsKind::from_os_release_id("nixos"), OsKind::NixOs);
        assert_eq!(OsKind::from_os_release_id("\"nixos\""), OsKind::NixOs);
    }

    #[test]
    fn falls_back_to_other() {
        assert_eq!(OsKind::from_os_release_id("ubuntu"), OsKind::Other);
        assert_eq!(OsKind::from_os_release_id(""), OsKind::Other);
    }
}
