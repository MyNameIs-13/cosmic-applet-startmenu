// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use std::time::{Duration, Instant};

use cosmic::{
    applet::token::subscription::{activation_token_subscription, TokenRequest, TokenUpdate},
    cctk::sctk::reexports::calloop,
    iced::{
        event::{
            wayland::{Event as WaylandEvent, LayerEvent, PopupEvent},
            PlatformSpecific,
        },
        keyboard, mouse,
        platform_specific::{
            runtime::wayland::layer_surface::SctkLayerSurfaceSettings,
            shell::wayland::commands::{
                layer_surface::{set_keyboard_interactivity, KeyboardInteractivity, Layer},
                popup::{destroy_popup, get_popup},
            },
        },
        window::Id,
        Alignment, Event, Length, Limits, Subscription,
    },
    prelude::*,
    theme,
    widget::{self, icon},
};

use crate::application::{PowerAction, ProgramEntry, StartIcon, StartMenuService};
use crate::fl;

use super::search_keys::{self, SearchKey};
use super::widgets::{program_row, sidebar_item, start_button};

/// How long after the menu was dismissed by losing focus a click on the
/// Start button is treated as part of that same dismissal. Clicking Start
/// while the menu is open moves focus away from it — closing it — just
/// before the click itself arrives; without this, that click would
/// immediately reopen the menu instead of leaving it closed.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// How long to wait, after one of the menu's surfaces loses keyboard focus,
/// before deciding focus really left the menu. Focus hops between the
/// popup and the helper surface (and cosmic-comp briefly focuses the popup
/// right after it opens), so an unfocus alone isn't proof the user moved on.
const UNFOCUS_SETTLE: Duration = Duration::from_millis(50);

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
    /// The invisible helper surface holding keyboard focus while the menu
    /// is open — see `capture_keyboard`.
    keyboard_surface: Option<Id>,
    /// Which of the menu's surfaces (popup or helper) has keyboard focus,
    /// if either does.
    focused_surface: Option<Id>,
    /// Whether the pointer is over the popup; the menu never closes on lost
    /// focus while the user is pointing at it.
    pointer_inside_popup: bool,
    /// When the menu last closed because focus left it — see `REOPEN_GUARD`.
    dismissed_at: Option<Instant>,
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
    SurfaceFocused(Id),
    SurfaceUnfocused(Id),
    /// Fired `UNFOCUS_SETTLE` after a menu surface lost focus: closes the
    /// menu if focus hasn't come back to it by then.
    CloseIfUnfocused(Id),
    PointerInside(Id, bool),
    /// A key press the search field itself didn't receive.
    SearchKey(SearchKey),
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

/// Maps the window-level events the menu's focus handling cares about.
fn surface_event(event: Event, id: Id) -> Option<Message> {
    match event {
        Event::Window(cosmic::iced::window::Event::Opened { .. }) => Some(Message::SurfaceOpened(id)),
        Event::PlatformSpecific(PlatformSpecific::Wayland(
            WaylandEvent::Layer(LayerEvent::Focused, _, id) | WaylandEvent::Popup(PopupEvent::Focused, _, id),
        )) => Some(Message::SurfaceFocused(id)),
        Event::PlatformSpecific(PlatformSpecific::Wayland(
            WaylandEvent::Layer(LayerEvent::Unfocused, _, id) | WaylandEvent::Popup(PopupEvent::Unfocused, _, id),
        )) => Some(Message::SurfaceUnfocused(id)),
        Event::Mouse(mouse::Event::CursorEntered) => Some(Message::PointerInside(id, true)),
        Event::Mouse(mouse::Event::CursorLeft) => Some(Message::PointerInside(id, false)),
        _ => None,
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
            keyboard_surface: None,
            focused_surface: None,
            pointer_inside_popup: false,
            dismissed_at: None,
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
    ///
    /// Every other surface (the invisible keyboard helper) renders nothing.
    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        if self.popup != Some(id) {
            return widget::text::body("").into();
        }

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
            .align_y(Alignment::Center)
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
            cosmic::iced::event::listen_with(|event, _status, id| surface_event(event, id)),
            if self.popup.is_some() {
                cosmic::iced::event::listen_with(|event, status, _id| match event {
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key, modifiers, text, ..
                    }) => search_keys::interpret(
                        &key,
                        modifiers,
                        text.as_deref(),
                        status == cosmic::iced::event::Status::Captured,
                    )
                    .map(Message::SearchKey),
                    _ => None,
                })
            } else {
                Subscription::none()
            },
        ])
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                if self.popup.is_some() {
                    return self.close_popup();
                }
                if self.dismissed_at.take().is_some_and(|at| at.elapsed() < REOPEN_GUARD) {
                    return Task::none();
                }
                return self.open_popup();
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    return self.release_keyboard();
                }
            }
            Message::SurfaceOpened(id) => {
                if self.popup == Some(id) {
                    return Task::batch([
                        self.capture_keyboard(),
                        widget::text_input::focus(self.search_id.clone()).map(cosmic::Action::App),
                    ]);
                }
            }
            Message::SurfaceFocused(id) => {
                if self.popup == Some(id) {
                    self.focused_surface = Some(id);
                } else if self.keyboard_surface == Some(id) {
                    self.focused_surface = Some(id);
                    // Focus is ours now; drop back to `OnDemand` so a click
                    // into the popup (or anywhere else) can still take it.
                    return set_keyboard_interactivity(id, KeyboardInteractivity::OnDemand);
                }
            }
            Message::SurfaceUnfocused(id) => {
                if self.popup != Some(id) && self.keyboard_surface != Some(id) {
                    return Task::none();
                }
                if self.focused_surface == Some(id) {
                    self.focused_surface = None;
                }
                if let Some(popup) = self.popup {
                    return Task::perform(async { tokio::time::sleep(UNFOCUS_SETTLE).await }, move |()| {
                        cosmic::Action::App(Message::CloseIfUnfocused(popup))
                    });
                }
            }
            Message::CloseIfUnfocused(popup) => {
                if self.popup == Some(popup) && self.focused_surface.is_none() && !self.pointer_inside_popup {
                    self.dismissed_at = Some(Instant::now());
                    return self.close_popup();
                }
            }
            Message::PointerInside(id, inside) => {
                if self.popup == Some(id) {
                    self.pointer_inside_popup = inside;
                }
            }
            Message::SearchKey(key) => match key {
                SearchKey::Append(text) => self.search_query.push_str(&text),
                SearchKey::Backspace => {
                    self.search_query.pop();
                }
                SearchKey::Dismiss => return self.close_popup(),
            },
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

                return self.close_popup();
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

impl AppModel {
    fn open_popup(&mut self) -> Task<cosmic::Action<Message>> {
        // The set of installed programs can change while the applet is
        // running; refresh it each time the menu opens.
        self.programs = self.service.load_programs();
        self.search_query.clear();
        self.power_menu_open = false;
        self.focused_surface = None;
        self.pointer_inside_popup = false;

        let new_id = Id::unique();
        self.popup = Some(new_id);
        let mut popup_settings =
            self.core
                .applet
                .get_popup_settings(self.core.main_window_id().unwrap(), new_id, None, None, None);
        popup_settings.positioner.size_limits = Limits::NONE
            .max_width(510.0)
            .min_width(510.0)
            .min_height(300.0)
            .max_height(720.0);
        // No `xdg_popup` grab: cosmic-comp hands a grabbed popup keyboard
        // focus, then takes it back ~200ms later, so typing went nowhere
        // until the search field was clicked. Keyboard focus comes from
        // `capture_keyboard` instead — and a grab would get the popup
        // dismissed the moment that helper surface takes focus. Without the
        // grab, closing on an outside click is handled by
        // `Message::SurfaceUnfocused`.
        popup_settings.grab = false;
        get_popup(popup_settings)
    }

    fn close_popup(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(popup) = self.popup.take() else {
            return Task::none();
        };
        self.focused_surface = None;
        self.pointer_inside_popup = false;
        Task::batch([destroy_popup(popup), self.release_keyboard()])
    }

    /// Takes keyboard focus for the open menu.
    ///
    /// cosmic-comp won't let an applet's popup keep keyboard focus without
    /// a click on it, but it does give focus to a layer surface asking for
    /// `Exclusive` keyboard interactivity. So the menu opens a 1×1,
    /// input-transparent, invisible overlay surface that asks for exactly
    /// that; key presses land there and reach the search query through the
    /// keyboard subscription (see `search_keys`). The same approach as
    /// cosmic-ext-applet-clip-keep. Once focused, the helper drops to
    /// `OnDemand` (see `Message::SurfaceFocused`) so it doesn't trap the
    /// keyboard.
    fn capture_keyboard(&mut self) -> Task<cosmic::Action<Message>> {
        if self.keyboard_surface.is_some() {
            return Task::none();
        }
        let id = Id::unique();
        self.keyboard_surface = Some(id);
        cosmic::surface::surface_task(cosmic::surface::action::app_layer_shell::<Self>(
            |_| cosmic::surface::action::LiveSettings::default(),
            move |_| keyboard_helper_settings(id),
            None,
        ))
    }

    fn release_keyboard(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(id) = self.keyboard_surface.take() else {
            return Task::none();
        };
        if self.focused_surface == Some(id) {
            self.focused_surface = None;
        }
        cosmic::surface::surface_task(cosmic::surface::action::destroy_layer_shell(id))
    }
}

/// The invisible surface `AppModel::capture_keyboard` opens to receive
/// keyboard focus.
fn keyboard_helper_settings(id: Id) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        // Empty input region: clicks pass straight through it.
        input_zone: Some(Vec::new()),
        namespace: format!("{}.keyboard-focus", <AppModel as cosmic::Application>::APP_ID),
        size: Some((Some(1), Some(1))),
        exclusive_zone: -1,
        ..SctkLayerSurfaceSettings::default()
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
            keyboard_surface: None,
            focused_surface: None,
            pointer_inside_popup: false,
            dismissed_at: None,
        }
    }

    /// Runs a task to completion and returns everything it emitted.
    fn actions(task: Task<cosmic::Action<Message>>) -> Vec<RuntimeAction<cosmic::Action<Message>>> {
        into_stream(task)
            .map(|stream| block_on_stream(stream).collect())
            .unwrap_or_default()
    }

    /// Opens the menu the way the runtime does: the popup is requested, then
    /// the compositor confirms the surface exists. Returns what opening it
    /// emitted once the surface existed.
    fn open_menu(app: &mut AppModel) -> Vec<RuntimeAction<cosmic::Action<Message>>> {
        let _ = app.update(Message::TogglePopup);
        let popup = app.popup.expect("TogglePopup should have opened a popup");
        actions(app.update(Message::SurfaceOpened(popup)))
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
    fn opening_the_menu_takes_keyboard_focus_through_an_exclusive_helper_surface() {
        // cosmic-comp gives a freshly opened applet popup keyboard focus and
        // takes it back ~200ms later, so focusing the search field alone
        // left typing going nowhere. Only a layer surface asking for
        // `Exclusive` interactivity keeps focus; it must also be invisible
        // to clicks so it doesn't block the screen.
        type HelperSettings = Box<dyn Fn(&mut AppModel) -> SctkLayerSurfaceSettings + Send + Sync>;

        let mut app = test_app();
        let emitted = open_menu(&mut app);

        let helper = emitted
            .into_iter()
            .find_map(|action| match action {
                RuntimeAction::Output(cosmic::Action::Cosmic(cosmic::app::Action::Surface(
                    cosmic::surface::Action::AppLayerShell(settings, _, _),
                ))) => Some(settings),
                _ => None,
            })
            .expect("opening the menu must open a keyboard helper surface");
        let settings = helper.downcast_ref::<HelperSettings>().expect("layer-shell settings")(&mut app);

        assert_eq!(Some(settings.id), app.keyboard_surface);
        assert_eq!(settings.keyboard_interactivity, KeyboardInteractivity::Exclusive);
        assert_eq!(
            settings.input_zone,
            Some(Vec::new()),
            "the helper must let clicks through"
        );
    }

    #[test]
    fn the_popup_is_opened_without_a_grab() {
        use cosmic::iced::runtime::platform_specific::{self, wayland};

        // A grabbed popup is dismissed by the compositor as soon as the
        // keyboard helper surface takes focus.
        let mut app = test_app();
        let grab = actions(app.update(Message::TogglePopup))
            .into_iter()
            .find_map(|action| match action {
                RuntimeAction::PlatformSpecific(platform_specific::Action::Wayland(wayland::Action::Popup(
                    wayland::popup::Action::Popup { popup },
                ))) => Some(popup.grab),
                _ => None,
            });

        assert_eq!(grab, Some(false));
    }

    #[test]
    fn keys_the_search_field_never_saw_still_edit_the_query() {
        let mut app = test_app();
        let _ = open_menu(&mut app);

        let _ = app.update(Message::SearchKey(SearchKey::Append("fi".into())));
        let _ = app.update(Message::SearchKey(SearchKey::Append("x".into())));
        let _ = app.update(Message::SearchKey(SearchKey::Backspace));

        assert_eq!(app.search_query, "fi");
    }

    #[test]
    fn escape_closes_the_menu_and_its_helper_surface() {
        let mut app = test_app();
        let _ = open_menu(&mut app);

        let _ = app.update(Message::SearchKey(SearchKey::Dismiss));

        assert_eq!(app.popup, None);
        assert_eq!(app.keyboard_surface, None);
    }

    #[test]
    fn focus_moving_to_another_app_closes_the_menu() {
        let mut app = test_app();
        let _ = open_menu(&mut app);
        let popup = app.popup.unwrap();
        let helper = app.keyboard_surface.unwrap();

        let _ = app.update(Message::SurfaceFocused(helper));
        let _ = app.update(Message::SurfaceUnfocused(helper));
        let _ = app.update(Message::CloseIfUnfocused(popup));

        assert_eq!(app.popup, None);
        assert_eq!(app.keyboard_surface, None);
    }

    #[test]
    fn focus_moving_from_the_helper_into_the_popup_keeps_the_menu_open() {
        let mut app = test_app();
        let _ = open_menu(&mut app);
        let popup = app.popup.unwrap();
        let helper = app.keyboard_surface.unwrap();

        let _ = app.update(Message::SurfaceFocused(helper));
        let _ = app.update(Message::SurfaceUnfocused(helper));
        let _ = app.update(Message::SurfaceFocused(popup));
        let _ = app.update(Message::CloseIfUnfocused(popup));

        assert_eq!(app.popup, Some(popup));
    }

    #[test]
    fn clicking_start_to_close_the_menu_does_not_reopen_it() {
        // Clicking Start while the menu is open first moves focus away
        // (closing the menu), then delivers the click as a toggle.
        let mut app = test_app();
        let _ = open_menu(&mut app);
        let popup = app.popup.unwrap();
        let helper = app.keyboard_surface.unwrap();

        let _ = app.update(Message::SurfaceFocused(helper));
        let _ = app.update(Message::SurfaceUnfocused(helper));
        let _ = app.update(Message::CloseIfUnfocused(popup));
        let _ = app.update(Message::TogglePopup);

        assert_eq!(app.popup, None);
    }

    #[test]
    fn toggling_the_popup_never_touches_layer_surface_keyboard_interactivity() {
        use cosmic::iced::runtime::platform_specific::{self, wayland};

        // Setting `KeyboardInteractivity::Exclusive` on the panel button's
        // own layer surface was tried once to get the menu keyboard focus.
        // It made things worse: cosmic-comp then rejects the popup as a
        // focus target outright. Only the separate helper surface (see
        // `AppModel::capture_keyboard`) may ever have its interactivity
        // changed, and only once it's focused — never on open or close.
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
