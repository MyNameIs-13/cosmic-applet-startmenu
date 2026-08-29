// SPDX-License-Identifier: GPL-3.0-only

use std::fs;

use crate::domain::{HostDetector, OsKind};

const OS_RELEASE_PATH: &str = "/etc/os-release";

pub struct OsReleaseHostDetector;

impl HostDetector for OsReleaseHostDetector {
    fn detect(&self) -> OsKind {
        let Ok(contents) = fs::read_to_string(OS_RELEASE_PATH) else {
            return OsKind::Other;
        };
        parse_id(&contents)
    }
}

fn parse_id(os_release: &str) -> OsKind {
    for line in os_release.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            return OsKind::from_os_release_id(value);
        }
    }
    OsKind::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pop_os_release() {
        let contents = "NAME=\"Pop!_OS\"\nID=pop\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(parse_id(contents), OsKind::PopOs);
    }

    #[test]
    fn parses_nixos_release() {
        let contents = "NAME=NixOS\nID=nixos\nVERSION=\"24.05\"\n";
        assert_eq!(parse_id(contents), OsKind::NixOs);
    }

    #[test]
    fn missing_id_falls_back_to_other() {
        assert_eq!(parse_id("NAME=Mystery\n"), OsKind::Other);
    }
}
