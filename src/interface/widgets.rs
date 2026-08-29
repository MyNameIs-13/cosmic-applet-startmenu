// SPDX-License-Identifier: GPL-3.0-only

//! Small, pure view builders. `app.rs` owns state/message wiring; these
//! functions just render an `application` DTO into an `Element`.

use std::path::PathBuf;

use cosmic::{
    applet::menu_button,
    iced::{Alignment, Length},
    theme,
    widget::{self, icon},
    Element,
};

use crate::application::ProgramEntry;

use super::app::Message;

fn program_icon_handle(icon_name: Option<&str>) -> icon::Handle {
    match icon_name {
        Some(name) if name.starts_with('/') => icon::from_path(PathBuf::from(name)),
        Some(name) => icon::from_name(name).size(20).into(),
        None => icon::from_name("application-x-executable-symbolic").size(20).into(),
    }
}

/// The panel button: the detected-OS icon next to the word "Start". Built
/// by hand rather than via `core.applet.button_from_element`, which fixes
/// the button's width to the (small, square) suggested icon size — fine for
/// an icon-only button, but it clips any text content next to the icon.
pub fn start_button(
    core: &cosmic::Core,
    start_icon: icon::Handle,
    label: String,
    message: Message,
) -> Element<'static, Message> {
    let suggested = core.applet.suggested_size(true);
    let (major_axis, minor_axis) = core.applet.suggested_padding(true);
    let (horizontal_padding, vertical_padding) = if core.applet.is_horizontal() {
        (major_axis, minor_axis)
    } else {
        (minor_axis, major_axis)
    };

    let content = widget::Row::new()
        .push(icon::icon(start_icon).size(suggested.0))
        .push(widget::text::body(label).font(cosmic::font::bold()))
        .align_y(Alignment::Center)
        .spacing(4)
        .height(Length::Fill);

    // Height includes vertical_padding on both sides to match the button's
    // actual surface height (see `Context::window_settings`/`text_button`
    // in libcosmic) — a bare `suggested.1` leaves the button only half as
    // tall as the space the panel gives it.
    widget::button::custom(content)
        .on_press(message)
        .padding([0, horizontal_padding])
        .height(Length::Fixed(f32::from(suggested.1 + 2 * vertical_padding)))
        .class(theme::Button::AppletIcon)
        .into()
}

/// One row in the "All Programs" list.
pub fn program_row(entry: &ProgramEntry) -> Element<'static, Message> {
    let row = widget::Row::new()
        .push(icon::icon(program_icon_handle(entry.icon.as_deref())).size(20))
        .push(widget::text::body(entry.name.clone()))
        .align_y(Alignment::Center)
        .spacing(8)
        .width(Length::Fill);

    widget::button::custom(row)
        .on_press(Message::LaunchProgram(entry.clone()))
        .width(Length::Fill)
        .class(theme::Button::Text)
        .into()
}

/// A sidebar row: Files, Settings, Restart, Suspend — anything styled like
/// a menu entry rather than the primary Shut Down action.
pub fn sidebar_item(icon_name: &'static str, label: String, message: Message) -> Element<'static, Message> {
    let content = widget::Row::new()
        .push(icon::from_name(icon_name).size(16).icon())
        .push(widget::text::body(label))
        .align_y(Alignment::Center)
        .spacing(8);

    menu_button(content).on_press(message).into()
}

#[cfg(test)]
mod diagnose_panel_button_sizing {
    //! Reproduces (numerically, against the real `libcosmic` sizing
    //! constants — no compositor needed): "panel entry only fills half the
    //! height of the panel ... Start word not visible". `cosmic::applet::run`
    //! creates the panel-button's Wayland surface exactly once, non-resizable,
    //! sized by `Context::window_settings()` for a bare square icon (no text
    //! budget at all). `AppModel::view()` returns `start_button`'s icon+label
    //! Row without ever wrapping it in `core.applet.autosize_window(..)`, the
    //! API libcosmic ships for exactly this case, so the surface never grows
    //! to fit the label. This test pins down that precondition.
    #[test]
    fn fixed_window_has_no_room_for_the_label_and_is_taller_than_the_button() {
        let applet = cosmic::applet::Context::default(); // COSMIC_PANEL_SIZE unset -> PanelSize::S
        let suggested = applet.suggested_size(true);
        let (major_axis, minor_axis) = applet.suggested_padding(true);
        let (h_pad, v_pad) = if applet.is_horizontal() {
            (major_axis, minor_axis)
        } else {
            (minor_axis, major_axis)
        };

        // What `cosmic::applet::run` fixes the (non-resizable) panel window
        // to at startup — see `Context::window_settings`.
        let window_w = f32::from(suggested.0) + f32::from(h_pad) * 2.0;
        let window_h = f32::from(suggested.1) + f32::from(v_pad) * 2.0;

        // What `start_button` (this module) requests for its own height.
        let button_h = f32::from(suggested.1);

        // Width left inside the button's own padding for icon + Row spacing(4) + label.
        let content_w = window_w - f32::from(h_pad) * 2.0;
        let icon_and_spacing = f32::from(suggested.0) + 4.0;

        assert!(
            button_h < window_h,
            "expected the reported half-height gap: button_h={button_h} should be < window_h={window_h}"
        );
        assert!(
            content_w <= icon_and_spacing,
            "expected zero room left for the \"Start\" label: content_w={content_w}, icon+spacing={icon_and_spacing}"
        );
    }
}
