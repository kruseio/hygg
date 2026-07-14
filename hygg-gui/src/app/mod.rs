//! The iced application: state, subscriptions, view routing, and the target
//! entry points. The message handler lives in [`update`] and the async
//! side-effects in [`tasks`]; screen rendering lives in [`crate::screens`].

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{mouse_area, stack};
use iced::{Size, Subscription, Task};

use crate::model::{BookSummary, Progress};
use crate::screens;
use crate::settings::{Account, Settings};
use crate::theme::Palette;

mod field_ops;
mod fields;
mod messages;
mod nav;
mod reader_assets;
mod reader_ops;
mod tasks;
mod update;
mod update_account;

use fields::Fields;
pub use fields::{Editor, FieldId, FieldMsg};
pub use messages::Message;
use nav::Nav;
pub use reader_ops::Reader;
use tasks::{load_library, on_platform_event, open_path};

/// Bundled monospace font (Fira Mono, SIL OFL), registered at startup so the
/// reader's justified column and ASCII-art rows render regardless of the system
/// font stack. See [`crate::layout::MONO`].
const MONO_FONT: &[u8] =
  include_bytes!("../../assets/fonts/FiraMono-Medium.ttf");

/// Height of the top bar (matches `--bar-h` in the PWA).
pub const TOPBAR_H: f32 = 52.0;
/// Extra lines rendered above/below the viewport so fast scrolls never flash
/// blank — the reader's virtualization overscan.
pub const OVERSCAN: usize = 8;

/// Which screen is showing. The open document's id lives in [`Reader::id`].
#[derive(Clone, Debug)]
pub enum Screen {
  Home,
  Reader,
  Settings,
  About,
  Credits,
}

/// Credits page state: the GitHub contributor-list load result plus the fetched
/// circular avatar handles (keyed by login). `contributors == None` means it
/// hasn't been fetched yet; `loading` guards a second in-flight request.
#[derive(Default)]
pub struct CreditsState {
  pub contributors: Option<Result<Vec<crate::credits::Contributor>, String>>,
  pub avatars: HashMap<String, iced::widget::image::Handle>,
  pub loading: bool,
}

/// What the right-click context menu was invoked over — drives which items it
/// offers. Copy/Select-all act on the reader selection or the field selection;
/// Paste only applies to a field; Back/Forward are always available.
#[derive(Clone, Debug)]
pub enum MenuCtx {
  Reader,
  Field(FieldId),
  Blank,
}

/// The whole-app state.
pub struct HyggGui {
  settings: Settings,
  palette: Palette,
  screen: Screen,
  library: Vec<BookSummary>,
  progress: HashMap<String, Progress>,
  reader: Reader,
  account: Account,
  status: String,
  /// Last known window size, used to fit the reader column and size the
  /// virtualized window. Defaults until the first resize event lands.
  viewport: Size,
  /// The library card whose "more options" sheet is open (`None` = closed).
  card_menu: Option<String>,
  /// The document awaiting remove-confirmation (`None` = no dialog open).
  confirm_delete: Option<String>,
  /// Custom text-field editors (caret + selection) for the Settings inputs.
  fields: Fields,
  /// Browser-style navigation history and the current position within it.
  history: Vec<Nav>,
  hist_pos: usize,
  /// Last known window-relative cursor position, for placing the menu.
  cursor: iced::Point,
  /// The open right-click menu: its anchor + what it was invoked over.
  context_menu: Option<(iced::Point, MenuCtx)>,
  /// Shared "who owns the text selection" token for the selectable-text
  /// widgets, so only one static label stays highlighted at a time.
  sel_owner: crate::widget::selectable::SelectionOwner,
  /// Credits page: GitHub contributor list + fetched avatars, loaded lazily
  /// the first time the Credits screen is opened.
  credits: CreditsState,
}

impl HyggGui {
  fn new(initial: Option<PathBuf>) -> (Self, Task<Message>) {
    let settings = Settings::load();
    let palette = Palette::of(settings.theme);
    let col = settings.import_col;

    let mut tasks = vec![Task::perform(load_library(), |(lib, prog)| {
      Message::LibraryLoaded(lib, prog)
    })];
    // When connected, pull the account's library + reading positions from the
    // server in the background, then refresh the view.
    if let Some(creds) = settings.creds() {
      tasks.push(Task::perform(tasks::sync_from_server(creds, col), |_| {
        Message::ServerSynced
      }));
    }
    if let Some(path) = initial {
      tasks.push(Task::perform(open_path(path, col), Message::OpenedExternal));
    }

    let state = HyggGui {
      settings,
      palette,
      screen: Screen::Home,
      library: Vec::new(),
      progress: HashMap::new(),
      reader: Reader::default(),
      account: Account::default(),
      status: String::new(),
      viewport: Size::new(1024.0, 768.0),
      card_menu: None,
      confirm_delete: None,
      fields: Fields::default(),
      history: vec![Nav::Home],
      hist_pos: 0,
      cursor: iced::Point::ORIGIN,
      context_menu: None,
      sel_owner: crate::widget::selectable::SelectionOwner::default(),
      credits: CreditsState::default(),
    };
    (state, Task::batch(tasks))
  }

  fn title(&self) -> String {
    match &self.screen {
      Screen::Reader => self
        .reader
        .book
        .as_ref()
        .map(|b| format!("{} — hygg", b.title))
        .unwrap_or_else(|| "hygg".to_string()),
      _ => "hygg".to_string(),
    }
  }

  fn theme(&self) -> iced::Theme {
    self.palette.iced_theme()
  }

  fn subscription(&self) -> Subscription<Message> {
    let resize = iced::window::resize_events()
      .map(|(_id, size)| Message::WindowResized(size));
    // Raw window events: OS drops, Shift state, and key presses (routed to the
    // focused text field, or the reader's Ctrl/Cmd+C — see `update`).
    let events = iced::event::listen_with(on_platform_event);
    // Refresh the home from the server periodically while the app is open.
    let tick = iced::time::every(std::time::Duration::from_secs(60))
      .map(|_| Message::Tick);
    let mut subs = vec![resize, events, tick];
    // Drive the top bar's slide only while it's actually in motion.
    if matches!(self.screen, Screen::Reader)
      && self.reader.nav_offset != self.reader.nav_target()
    {
      subs.push(iced::window::frames().map(|_| Message::AnimTick));
    }
    Subscription::batch(subs)
  }

  fn view(&self) -> iced::Element<'_, Message> {
    let base: iced::Element<'_, Message> = match &self.screen {
      Screen::Home => screens::home::view(
        &self.palette,
        &self.library,
        &self.progress,
        &self.status,
        self.viewport.width,
        self.card_menu.as_deref(),
        self.confirm_delete.as_deref(),
        self.sel_owner(),
      ),
      Screen::Reader => {
        let (w, vh) = self.reader_viewport();
        screens::reader::view(self, &self.reader, self.palette, w, vh)
      }
      Screen::Settings => screens::settings::view(self),
      Screen::About => screens::about::view(self),
      Screen::Credits => screens::credits::view(self),
    };
    // A right-click on any blank area opens a Back/Forward menu; the reader and
    // the text fields capture their own right-clicks for a richer menu.
    let base =
      mouse_area(base).on_right_press(Message::OpenMenu(MenuCtx::Blank));
    // Always wrap in a stack (even with the menu closed) so the root widget
    // type never changes — otherwise toggling the menu would swap the root
    // between `mouse_area` and `stack` and reset the reader scrollable's
    // position (scrolling it to the top).
    stack![base]
      .push_maybe(
        self
          .context_menu
          .as_ref()
          .map(|(at, ctx)| screens::menu::view(self, *at, ctx)),
      )
      .into()
  }

  /// Read-only settings access for the screens module.
  pub fn settings(&self) -> &Settings {
    &self.settings
  }

  /// Read-only palette access for the screens module.
  pub fn palette(&self) -> Palette {
    self.palette
  }

  /// The window size, for clamping the context menu on-screen.
  pub fn viewport(&self) -> Size {
    self.viewport
  }

  /// Read-only account (connect form) access for the screens module.
  pub fn account(&self) -> &Account {
    &self.account
  }

  /// A clone of the shared selection-owner token for the selectable widgets.
  pub fn sel_owner(&self) -> crate::widget::selectable::SelectionOwner {
    self.sel_owner.clone()
  }

  /// Read-only Credits state (contributor list + avatars) for the screens.
  pub fn credits(&self) -> &CreditsState {
    &self.credits
  }
}

/// The connected account's label, exactly as the server sent it (mirrors the
/// PWA account panel). A server that sends none shows nothing.
pub(super) fn account_label(
  me: &hygg_shared::sync::proto::MeResponse,
) -> String {
  me.label.clone().filter(|l| !l.is_empty()).unwrap_or_default()
}

/// Percentage read: line index over the line count (the metric the CLI and
/// server share, so a synced position matches).
pub fn line_percent(line: usize, total: usize) -> f64 {
  if total == 0 {
    0.0
  } else {
    (line as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
  }
}

// ------------------------------------------------------------ entry points ---

/// Build and run the iced application. `initial` is a document to open on
/// launch (from the OS file handler); `None` starts on the library.
fn run(initial: Option<PathBuf>) -> iced::Result {
  iced::application(HyggGui::title, HyggGui::update, HyggGui::view)
    .theme(HyggGui::theme)
    .subscription(HyggGui::subscription)
    .window_size(iced::Size::new(1280.0, 800.0))
    // Register the bundled monospace font for the reader's justified column.
    .font(MONO_FONT)
    .antialiasing(true)
    .run_with(move || HyggGui::new(initial))
}

/// Entry point: pick up a document path from `argv` (the OS passes it when
/// hygg-gui is the registered handler) and launch.
pub fn launch() {
  let initial =
    std::env::args_os().skip(1).map(PathBuf::from).find(|p| p.is_file());
  if let Err(e) = run(initial) {
    eprintln!("hygg-gui: {e}");
  }
}
