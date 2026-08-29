// SPDX-License-Identifier: GPL-3.0-only

/// A launchable program, as it should be presented in the "All Programs" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub name: String,
    /// The `Exec` value from the `.desktop` file, field codes (`%f`, `%U`, ...) intact.
    pub exec: String,
    /// Icon name or path from the `.desktop` file's `Icon` key, if any.
    pub icon: Option<String>,
}

impl AppEntry {
    /// Split `exec` into a program and argument list, dropping desktop-entry
    /// field codes (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`, ...) since this
    /// applet never passes files or startup metadata to the launched program.
    pub fn command(&self) -> Option<(String, Vec<String>)> {
        let mut parts = self
            .exec
            .split_whitespace()
            .filter(|token| !token.starts_with('%'))
            .map(str::to_string);
        let program = parts.next()?;
        Some((program, parts.collect()))
    }
}

/// Port: enumerate the launchable programs known to the system.
pub trait AppEntryRepository: Send + Sync {
    fn all(&self) -> Vec<AppEntry>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_strips_field_codes() {
        let entry = AppEntry {
            name: "Files".into(),
            exec: "cosmic-files %U".into(),
            icon: None,
        };
        assert_eq!(entry.command(), Some(("cosmic-files".into(), vec![])));
    }

    #[test]
    fn command_keeps_real_arguments() {
        let entry = AppEntry {
            name: "Editor".into(),
            exec: "code --new-window %F".into(),
            icon: None,
        };
        assert_eq!(
            entry.command(),
            Some(("code".into(), vec!["--new-window".into()]))
        );
    }
}
