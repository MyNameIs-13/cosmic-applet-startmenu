// SPDX-License-Identifier: GPL-3.0-only

//! Turns raw key presses into edits of the search query.
//!
//! While the menu is open, keyboard focus usually sits on an invisible
//! helper surface rather than on the popup itself (see
//! `AppModel::capture_keyboard`), so the search field never sees those
//! keys. This module is how they still end up in the search query.

use cosmic::iced::keyboard::key::Named;
use cosmic::iced::keyboard::{Key, Modifiers};

/// What a key press should do to the open menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchKey {
    /// Append typed text to the query.
    Append(String),
    /// Delete the query's last character.
    Backspace,
    /// Close the menu.
    Dismiss,
}

/// Interprets one key press.
///
/// `captured` is true when a focused widget (the search field itself, once
/// the user has clicked it) already handled the key — editing the query
/// again here would type every character twice.
pub fn interpret(key: &Key, modifiers: Modifiers, text: Option<&str>, captured: bool) -> Option<SearchKey> {
    if matches!(key, Key::Named(Named::Escape)) {
        return Some(SearchKey::Dismiss);
    }

    if captured || !(modifiers.is_empty() || modifiers == Modifiers::SHIFT) {
        return None;
    }

    if matches!(key, Key::Named(Named::Backspace)) {
        return Some(SearchKey::Backspace);
    }

    let text = text?;
    (!text.is_empty() && !text.chars().any(char::is_control)).then(|| SearchKey::Append(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn typed_characters_are_appended() {
        assert_eq!(
            interpret(&character("f"), Modifiers::empty(), Some("f"), false),
            Some(SearchKey::Append("f".into()))
        );
        assert_eq!(
            interpret(&character("F"), Modifiers::SHIFT, Some("F"), false),
            Some(SearchKey::Append("F".into()))
        );
    }

    #[test]
    fn a_key_the_search_field_already_handled_is_not_applied_twice() {
        assert_eq!(interpret(&character("f"), Modifiers::empty(), Some("f"), true), None);
        assert_eq!(
            interpret(&Key::Named(Named::Backspace), Modifiers::empty(), None, true),
            None
        );
    }

    #[test]
    fn backspace_deletes() {
        assert_eq!(
            interpret(&Key::Named(Named::Backspace), Modifiers::empty(), Some("\u{8}"), false),
            Some(SearchKey::Backspace)
        );
    }

    #[test]
    fn escape_dismisses_even_when_captured() {
        assert_eq!(
            interpret(&Key::Named(Named::Escape), Modifiers::empty(), None, true),
            Some(SearchKey::Dismiss)
        );
    }

    #[test]
    fn shortcuts_and_control_characters_are_ignored() {
        assert_eq!(interpret(&character("c"), Modifiers::CTRL, Some("c"), false), None);
        assert_eq!(
            interpret(&Key::Named(Named::Tab), Modifiers::empty(), Some("\t"), false),
            None
        );
        assert_eq!(
            interpret(&Key::Named(Named::Shift), Modifiers::SHIFT, None, false),
            None
        );
    }
}
