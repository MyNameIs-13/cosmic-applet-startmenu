// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use cosmic::{
    applet::token::subscription::{activation_token_subscription, TokenRequest, TokenUpdate},
    cctk::sctk::reexports::calloop,
    iced::{
        platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
        window::Id,
        Alignment, Length, Limits, Subscription,
    },
    prelude::*,
    theme,
    widget::{self, icon},
};

use crate::application::{PowerAction, ProgramEntry, StartIcon, StartMenuService};
use crate::fl;

use super::widgets::{program_row, sidebar_item, start_button};

/// The application model stores app-specific state used to describe its
/// interface and drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// The popup id, when the menu is open.
    popup: Option<Id>,
    /// Wired use cases; the only thing this layer depends on besides
    /// libcosmic itself.
    service: Arc<StartMenuService>,
    /// Resolved once at startup: the live system's distro icon, or a
    /// bundled fallback.
    start_icon: icon::Handle,
    /// Every launchable program, loaded once when the popup first opens and
    /// refreshed each time it reopens.
    programs: Vec<ProgramEntry>,
    search_query: String,
    /// Identifies the search field so it can be focused programmatically
    /// when the popup opens.
    search_id: widget::Id,
    /// Channel to request a Wayland activation token before launching a
    /// program — without one, activating an already-running single-instance
    /// app (like Settings) does nothing visible instead of raising it.
    token_tx: Option<calloop::channel::Sender<TokenRequest>>,
    /// The program waiting on a token from `token_tx`, if any.
    pending_launch: Option<ProgramEntry>,
    /// Whether the Restart/Suspend flyout (behind the little arrow next to
    /// Shut Down, same as the classic Windows menu) is open.
    power_menu_open: bool,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    /// The window manager has actually created a surface — libcosmic issues
    /// this only once the surface exists, unlike `TogglePopup`, so it's the
    /// first point at which focusing a widget inside it can work.
    SurfaceOpened(Id),
    Search(String),
    LaunchProgram(ProgramEntry),
    Token(TokenUpdate),
    TogglePowerMenu,
    Power(PowerAction),
    PowerResult(Result<(), String>),
}

fn start_icon_handle(icon: StartIcon) -> icon::Handle {
    match icon {
        StartIcon::Path(path) => icon::from_path(path),
        StartIcon::Embedded(bytes) => icon::from_svg_bytes(bytes),
    }
}

fn shortcut_entry(name: &str, program: &str) -> ProgramEntry {
    ProgramEntry {
        name: name.to_string(),
        program: program.to_string(),
        args: Vec::new(),
        icon: None,
    }
}

fn spawn_program(entry: &ProgramEntry, activation_token: Option<String>) {
    let mut command = std::process::Command::new(&entry.program);
    command.args(&entry.args);
    if let Some(token) = activation_token {
        command.env("XDG_ACTIVATION_TOKEN", &token);
        command.env("DESKTOP_STARTUP_ID", &token);
    }
    tokio::spawn(cosmic::process::spawn(command));
}

/// The scrollable program list, or a "no results" placeholder when the
/// search query matches nothing.
///
/// Both branches must claim `Length::Fill` for height: the search field is
/// stacked right after this pane in `view_window`'s fixed-height column, so
/// if this pane shrinks to its content (as a bare `text::body` does) instead
/// of filling the remaining space, the search field jumps up to sit right
/// under it instead of staying pinned near the bottom of the pane.
fn program_list_pane<'a>(visible: &[ProgramEntry]) -> Element<'a, Message> {
    if visible.is_empty() {
        widget::container(widget::text::body(fl!("no-results")))
            .height(Length::Fill)
            .into()
    } else {
        let mut list = widget::Column::new().spacing(2);
        for entry in visible {
            list = list.push(program_row(entry));
        }
        widget::scrollable(list).height(Length::Fill).into()
    }
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = Arc<StartMenuService>;
    type Message = Message;

    const APP_ID: &'static str = "io.github.MyNameIs-13.CosmicAppletStartMenu";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, service: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let start_icon = start_icon_handle(service.resolve_start_icon());
        let programs = service.load_programs();

        let app = AppModel {
            core,
            popup: None,
            service,
            start_icon,
            programs,
            search_query: String::new(),
            search_id: widget::Id::unique(),
            token_tx: None,
            pending_launch: None,
            power_menu_open: false,
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The panel button: the detected OS logo (or the bundled Windows logo)
    /// next to the word "Start".
    ///
    /// Wrapped in `autosize_window`: the panel-button's Wayland surface is
    /// created once, non-resizable, sized by libcosmic for a bare square
    /// icon — with no text budget at all. Without this wrapper the surface
    /// never grows to fit the icon+"Start" row, clipping the label entirely.
    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .autosize_window(start_button(
                &self.core,
                self.start_icon.clone(),
                fl!("start"),
                Message::TogglePopup,
            ))
            .into()
    }

    /// The classic two-pane start menu: a searchable program list on the
    /// left, a user/shortcuts sidebar on the right with Shut Down pinned to
    /// its bottom (Restart/Suspend tucked behind the little arrow next to
    /// it, same as the Windows 7 "Classic" menu).
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let visible = self.service.search_programs(&self.programs, &self.search_query);
        let program_list = program_list_pane(&visible);

        let search = widget::search_input(fl!("search-placeholder"), self.search_query.clone())
            .id(self.search_id.clone())
            .on_input(Message::Search)
            .width(Length::Fill);

        let programs_pane = widget::Column::new()
            .push(program_list)
            .push(search)
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fixed(420.0));

        let mut sidebar = widget::Column::new()
            .push(widget::text::body(
                std::env::var("USER").unwrap_or_else(|_| "User".to_string()),
            ))
            .push(widget::divider::horizontal::default())
            .push(sidebar_item(
                "folder-symbolic",
                fl!("files"),
                Message::LaunchProgram(shortcut_entry("Files", "cosmic-files")),
            ))
            .push(sidebar_item(
                "utilities-system-monitor-symbolic",
                fl!("system-monitor"),
                Message::LaunchProgram(shortcut_entry("System Monitor", "cosmic-monitor")),
            ))
            .push(sidebar_item(
                "preferences-system-symbolic",
                fl!("settings"),
                Message::LaunchProgram(shortcut_entry("Settings", "cosmic-settings")),
            ))
            .push(widget::space::vertical().height(Length::Fill));

        if self.power_menu_open {
            sidebar = sidebar
                .push(sidebar_item(
                    "system-reboot-symbolic",
                    fl!("restart"),
                    Message::Power(PowerAction::Restart),
                ))
                .push(sidebar_item(
                    "system-suspend-symbolic",
                    fl!("suspend"),
                    Message::Power(PowerAction::Suspend),
                ))
                .push(widget::divider::horizontal::default());
        }

        let shutdown_content = widget::Row::new()
            .push(icon::from_name("system-shutdown-symbolic").size(16).icon())
            .push(widget::text::body(fl!("shut-down")))
            .align_y(Alignment::Center)
            .spacing(8);

        let shutdown_row = widget::Row::new()
            .push(
                widget::button::custom(shutdown_content)
                    .on_press(Message::Power(PowerAction::Shutdown))
                    .width(Length::Fill)
                    .class(theme::Button::Suggested),
            )
            .push(
                widget::button::custom(
                    icon::from_name(if self.power_menu_open {
                        "go-down-symbolic"
                    } else {
                        "go-up-symbolic"
                    })
                    .size(12)
                    .icon(),
                )
                .on_press(Message::TogglePowerMenu)
                .class(theme::Button::Standard),
            )
            .spacing(4);

        let sidebar = sidebar.push(shutdown_row).spacing(4).width(Length::Fixed(160.0));

        let body = widget::Row::new()
            .push(programs_pane)
            .push(widget::divider::vertical::default())
            .push(sidebar)
            .spacing(8);

        let content = widget::Column::new().push(body).padding(8);

        self.core
            .applet
            .popup_container(content)
            .limits(Limits::NONE.min_height(1.).min_width(510.0).max_width(510.0).max_height(720.0))
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch([
            activation_token_subscription(0).map(Message::Token),
            cosmic::iced::event::listen_with(|event, _status, id| {
                if let cosmic::iced::Event::Window(cosmic::iced::window::Event::Opened { .. }) = event {
                    Some(Message::SurfaceOpened(id))
                } else {
                    None
                }
            }),
        ])
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    // The set of installed programs can change while the
                    // applet is running; refresh it each time the menu opens.
                    self.programs = self.service.load_programs();
                    self.search_query.clear();
                    self.power_menu_open = false;

                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(510.0)
                        .min_width(510.0)
                        .min_height(300.0)
                        .max_height(720.0);
                    // `get_popup_settings` defaults `grab` to `true`, which
                    // requests an `xdg_popup` grab (using the panel button's
                    // click serial) — that's meant to give the popup real
                    // keyboard focus without needing a click on it first.
                    //
                    // KNOWN LIMITATION (unresolved as of this comment): on
                    // cosmic-comp, that grab doesn't actually transfer
                    // keyboard focus to the popup — typing still requires a
                    // literal click on the search field first, reproducing
                    // the originally reported bug. Two things were tried and
                    // ruled out:
                    //
                    // 1. Also setting `KeyboardInteractivity::Exclusive` on
                    //    the panel button's own layer surface (thinking that
                    //    would help). It actively made things worse:
                    //    cosmic-comp's focus-fixup pass
                    //    (`focus_target_is_valid` in `shell/focus/mod.rs`)
                    //    treats an `Exclusive` layer surface as the *only*
                    //    valid keyboard-focus target on its layer, rejecting
                    //    any `Popup` target outright — even one belonging to
                    //    that very surface — so the fixup kept reverting
                    //    focus back to the panel button and force-unsetting
                    //    the popup's own grab.
                    // 2. Making the menu its own layer surface instead of a
                    //    popup (so `Exclusive` interactivity would apply
                    //    directly, which `focus_target_is_valid` does
                    //    accept). That hit a different, worse problem in
                    //    testing: the surface captured keyboard input
                    //    globally (blocking other apps until Escape) while
                    //    never actually rendering visibly.
                    //
                    // Root-caused as far as an applet can go without tracing
                    // (or patching) cosmic-comp itself — see the project's
                    // issue tracker for the write-up to file upstream.
                    get_popup(popup_settings)
                }
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::SurfaceOpened(id) => {
                if self.popup.as_ref() == Some(&id) {
                    return widget::text_input::focus(self.search_id.clone()).map(cosmic::Action::App);
                }
            }
            Message::Search(query) => {
                self.search_query = query;
            }
            Message::TogglePowerMenu => {
                self.power_menu_open = !self.power_menu_open;
            }
            Message::LaunchProgram(entry) => {
                if let Some(tx) = self.token_tx.as_ref() {
                    let exec = format!("{} {}", entry.program, entry.args.join(" "));
                    self.pending_launch = Some(entry);
                    let _ = tx.send(TokenRequest {
                        app_id: Self::APP_ID.to_string(),
                        exec,
                    });
                } else {
                    spawn_program(&entry, None);
                }

                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                }
            }
            Message::Token(update) => match update {
                TokenUpdate::Init(tx) => self.token_tx = Some(tx),
                TokenUpdate::Finished => self.token_tx = None,
                TokenUpdate::ActivationToken { token, .. } => {
                    if let Some(entry) = self.pending_launch.take() {
                        spawn_program(&entry, token);
                    }
                }
            },
            Message::Power(action) => {
                let service = Arc::clone(&self.service);
                return Task::perform(async move { service.perform_power_action(action).await }, |result| {
                    cosmic::Action::App(Message::PowerResult(result))
                });
            }
            Message::PowerResult(Err(why)) => {
                eprintln!("cosmic-applet-startmenu: power action failed: {why}");
            }
            Message::PowerResult(Ok(())) => {}
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cosmic::iced::advanced::widget::operation::focusable::Focusable;
    use cosmic::iced::runtime::{task::into_stream, Action as RuntimeAction};
    use cosmic::iced::Rectangle;
    use cosmic::Application;
    use futures::executor::block_on_stream;

    use crate::domain::{
        AppEntry, AppEntryRepository, EmbeddedIcons, HostDetector, IconLookup, OsKind,
        PowerControl, PowerError,
    };

    use super::*;

    struct NoPrograms;
    impl AppEntryRepository for NoPrograms {
        fn all(&self) -> Vec<AppEntry> {
            Vec::new()
        }
    }

    struct NoPower;
    #[async_trait::async_trait]
    impl PowerControl for NoPower {
        async fn shutdown(&self) -> Result<(), PowerError> {
            Ok(())
        }
        async fn restart(&self) -> Result<(), PowerError> {
            Ok(())
        }
        async fn suspend(&self) -> Result<(), PowerError> {
            Ok(())
        }
    }

    struct NoHost;
    impl HostDetector for NoHost {
        fn detect(&self) -> OsKind {
            OsKind::Other
        }
    }

    struct NoIcons;
    impl IconLookup for NoIcons {
        fn find_first(&self, _candidates: &[&str]) -> Option<PathBuf> {
            None
        }
    }
    impl EmbeddedIcons for NoIcons {
        fn bytes(&self, _kind: OsKind) -> &'static [u8] {
            &[]
        }
    }

    fn test_app() -> AppModel {
        let service = Arc::new(StartMenuService::new(
            Arc::new(NoPrograms),
            Arc::new(NoPower),
            Arc::new(NoHost),
            Arc::new(NoIcons),
            Arc::new(NoIcons),
        ));

        let mut core = cosmic::Core::default();
        core.set_main_window_id(Some(Id::unique()));

        AppModel {
            core,
            popup: None,
            service,
            start_icon: icon::from_svg_bytes(&[]),
            programs: Vec::new(),
            search_query: String::new(),
            search_id: widget::Id::unique(),
            token_tx: None,
            pending_launch: None,
            power_menu_open: false,
        }
    }

    /// A minimal stand-in for the search field's internal focus state,
    /// exercised the same way the real iced runtime exercises a `TextInput`
    /// when it applies a `text_input::focus` operation.
    struct MockFocusable {
        focused: bool,
    }
    impl Focusable for MockFocusable {
        fn is_focused(&self) -> bool {
            self.focused
        }
        fn focus(&mut self) {
            self.focused = true;
        }
        fn unfocus(&mut self) {
            self.focused = false;
        }
    }

    #[test]
    fn opening_the_popup_focuses_the_search_field() {
        let mut app = test_app();
        let search_id = app.search_id.clone();

        // The popup surface doesn't exist the instant `TogglePopup` fires —
        // only once the compositor confirms it, signalled here by
        // `SurfaceOpened`. Focusing before that point is a no-op because the
        // widget tree it targets doesn't exist yet.
        let _ = app.update(Message::TogglePopup);
        let popup_id = app.popup.expect("TogglePopup should have opened a popup");
        let task = app.update(Message::SurfaceOpened(popup_id));

        let mut found_focus_op = false;
        let mut mock = MockFocusable { focused: false };
        if let Some(stream) = into_stream(task) {
            for action in block_on_stream(stream) {
                if let RuntimeAction::Widget(mut operation) = action {
                    operation.focusable(Some(&search_id), Rectangle::default(), &mut mock);
                    if mock.is_focused() {
                        found_focus_op = true;
                    }
                }
            }
        }

        assert!(
            found_focus_op,
            "opening the popup must focus the search field so typing immediately \
             searches, without requiring a mouse click first"
        );
    }

    #[test]
    fn toggling_the_popup_never_touches_layer_surface_keyboard_interactivity() {
        use cosmic::iced::runtime::platform_specific::{self, wayland};

        // KNOWN LIMITATION (see the comment on `Message::TogglePopup`'s open
        // branch): typing into the search field still requires a click on
        // it first; the popup's `xdg_popup` grab doesn't transfer real
        // keyboard focus on cosmic-comp. Setting
        // `KeyboardInteractivity::Exclusive` on the panel button's layer
        // surface to try to work around that was tested and made things
        // worse — cosmic-comp's focus-fixup pass rejects the popup as a
        // focus target outright whenever the panel button's surface is
        // Exclusive, and force-unsets the popup's own grab as a result. So
        // opening or closing the popup must never touch the layer surface's
        // keyboard interactivity.
        let mut app = test_app();

        let open_task = app.update(Message::TogglePopup);
        assert_no_keyboard_interactivity_action(open_task);

        let close_task = app.update(Message::TogglePopup);
        assert_no_keyboard_interactivity_action(close_task);

        fn assert_no_keyboard_interactivity_action(task: Task<cosmic::Action<Message>>) {
            if let Some(stream) = into_stream(task) {
                for action in block_on_stream(stream) {
                    if let RuntimeAction::PlatformSpecific(platform_specific::Action::Wayland(
                        wayland::Action::LayerSurface(
                            wayland::layer_surface::Action::KeyboardInteractivity { .. },
                        ),
                    )) = action
                    {
                        panic!(
                            "toggling the popup must not touch the layer surface's \
                             keyboard interactivity"
                        );
                    }
                }
            }
        }
    }

    fn program_entry(name: &str) -> ProgramEntry {
        ProgramEntry {
            name: name.to_string(),
            program: name.to_string(),
            args: Vec::new(),
            icon: None,
        }
    }

    #[test]
    fn no_results_pane_still_fills_height_like_the_populated_list() {
        let populated = program_list_pane(&[program_entry("Files")]);
        let empty = program_list_pane(&[]);

        assert_eq!(
            populated.as_widget().size().height,
            Length::Fill,
            "the populated program list fills the pane's height"
        );
        assert_eq!(
            empty.as_widget().size().height,
            Length::Fill,
            "the no-results placeholder must also fill the pane's height, or the \
             search field (stacked right after it) jumps up to sit under the \
             shorter placeholder instead of staying pinned near the bottom"
        );
    }
}
