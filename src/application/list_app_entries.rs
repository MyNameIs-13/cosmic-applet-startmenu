// SPDX-License-Identifier: GPL-3.0-only

use crate::domain::{AppEntry, AppEntryRepository};

use super::dto::ProgramEntry;

fn to_program_entry(entry: &AppEntry) -> Option<ProgramEntry> {
    let (program, args) = entry.command()?;
    Some(ProgramEntry {
        name: entry.name.clone(),
        program,
        args,
        icon: entry.icon.clone(),
    })
}

/// Load every launchable program for the "All Programs" list, sorted
/// alphabetically. Sorting is a use-case concern, not the repository
/// adapter's — the adapter only does I/O and type mapping.
pub fn load_programs(repo: &dyn AppEntryRepository) -> Vec<ProgramEntry> {
    let mut entries: Vec<ProgramEntry> = repo.all().iter().filter_map(to_program_entry).collect();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    entries
}

/// Filter an already-loaded, already-sorted program list by name. Kept
/// separate from `load_programs` so typing in the search box doesn't re-scan
/// the filesystem on every keystroke.
pub fn search_programs(programs: &[ProgramEntry], query: &str) -> Vec<ProgramEntry> {
    let query = query.trim().to_lowercase();
    programs
        .iter()
        .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRepo(Vec<AppEntry>);
    impl AppEntryRepository for FixedRepo {
        fn all(&self) -> Vec<AppEntry> {
            self.0.clone()
        }
    }

    fn entry(name: &str) -> AppEntry {
        AppEntry {
            name: name.into(),
            exec: format!("{name}-bin %U"),
            icon: None,
        }
    }

    #[test]
    fn load_sorts_alphabetically_case_insensitive() {
        let repo = FixedRepo(vec![entry("zed"), entry("Anki"), entry("blender")]);
        let names: Vec<_> = load_programs(&repo).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["Anki", "blender", "zed"]);
    }

    #[test]
    fn load_strips_exec_field_codes() {
        let repo = FixedRepo(vec![entry("Files")]);
        let programs = load_programs(&repo);
        assert_eq!(programs[0].program, "Files-bin");
        assert!(programs[0].args.is_empty());
    }

    #[test]
    fn search_filters_case_insensitively_by_substring() {
        let repo = FixedRepo(vec![entry("Firefox"), entry("Files"), entry("GIMP")]);
        let all = load_programs(&repo);
        let names: Vec<_> = search_programs(&all, "fi")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["Files", "Firefox"]);
    }

    #[test]
    fn empty_query_returns_everything() {
        let repo = FixedRepo(vec![entry("Firefox"), entry("Files")]);
        let all = load_programs(&repo);
        assert_eq!(search_programs(&all, "").len(), 2);
    }
}
