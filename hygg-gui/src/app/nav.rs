//! Browser-style navigation history for the context menu's Back/Forward. Every
//! screen change goes through [`HyggGui::navigate`], which records a [`Nav`]
//! entry (unless it's itself a Back/Forward step); Back/Forward replay earlier
//! entries. Leaving the reader always saves the reading position first.

use iced::Task;

use super::tasks::{load_library, load_reader};
use super::{HyggGui, Message, Reader, Screen};

/// A place in the app the history can return to.
#[derive(Debug, Clone)]
pub enum Nav {
  Home,
  Reader(String),
  Settings,
  About,
  Credits,
}

impl HyggGui {
  /// Go to `nav`, performing its side effects (reader load / library refresh /
  /// account re-check) and saving the reading position when leaving the reader.
  /// `record` pushes a history entry (false for Back/Forward replays).
  pub(super) fn navigate(&mut self, nav: Nav, record: bool) -> Task<Message> {
    self.context_menu = None;
    if record {
      self.history.truncate(self.hist_pos + 1);
      self.history.push(nav.clone());
      self.hist_pos = self.history.len() - 1;
    }
    let save = if matches!(self.screen, Screen::Reader) {
      self.save_reader_progress()
    } else {
      Task::none()
    };
    self.blur_field();
    let go = match nav {
      Nav::Home => {
        self.screen = Screen::Home;
        self.reader = Reader::default();
        // Re-pull from the server on return so the library + positions reflect
        // anything a peer changed while reading.
        let resync = match self.settings.creds() {
          Some(creds) => self.resync(creds),
          None => Task::none(),
        };
        Task::batch([
          resync,
          Task::perform(load_library(), |(l, p)| Message::LibraryLoaded(l, p)),
        ])
      }
      Nav::Reader(id) => {
        self.reader = Reader { id: id.clone(), ..Reader::default() };
        self.screen = Screen::Reader;
        let creds = self.settings.creds();
        let col = self.settings.import_col;
        Task::perform(load_reader(id, creds, col), Message::BookLoaded)
      }
      Nav::Settings => {
        self.screen = Screen::Settings;
        // If connected, confirm the stored credentials still work and surface
        // the account's plan (mirrors the PWA's account panel).
        match self.settings.creds() {
          Some(creds) if self.account.label.is_empty() => Task::perform(
            async move { crate::sync::fetch_me(&creds).await },
            Message::AccountChecked,
          ),
          _ => Task::none(),
        }
      }
      Nav::About => {
        self.screen = Screen::About;
        Task::none()
      }
      Nav::Credits => {
        self.screen = Screen::Credits;
        self.load_credits()
      }
    };
    Task::batch([save, go])
  }

  /// Kick off the Credits page's background loads: the GitHub contributor list
  /// (once) and the author's avatar (if not already fetched). Everything is
  /// best-effort — a failure just leaves the page in its offline fallback.
  pub(super) fn load_credits(&mut self) -> Task<Message> {
    let mut tasks = Vec::new();
    if self.credits.contributors.is_none() && !self.credits.loading {
      self.credits.loading = true;
      tasks.push(Task::perform(
        crate::credits::fetch_contributors(),
        Message::ContributorsLoaded,
      ));
    }
    // Fetch the author avatar directly so the author card shows a picture even
    // when the contributor list can't be reached.
    let owner = crate::build_info::OWNER;
    if !self.credits.avatars.contains_key(owner) {
      let url = format!(
        "https://github.com/{owner}.png?size={}",
        crate::credits::AVATAR_PX
      );
      tasks.push(Task::perform(
        crate::credits::fetch_avatar(owner.to_string(), url),
        |(l, h)| Message::AvatarLoaded(l, h),
      ));
    }
    Task::batch(tasks)
  }

  /// Step back one history entry, if any.
  pub(super) fn nav_back(&mut self) -> Task<Message> {
    if self.hist_pos == 0 {
      return Task::none();
    }
    self.hist_pos -= 1;
    self.navigate(self.history[self.hist_pos].clone(), false)
  }

  /// Step forward one history entry, if any.
  pub(super) fn nav_forward(&mut self) -> Task<Message> {
    if self.hist_pos + 1 >= self.history.len() {
      return Task::none();
    }
    self.hist_pos += 1;
    self.navigate(self.history[self.hist_pos].clone(), false)
  }

  /// Whether Back / Forward have anywhere to go (for greying the menu items).
  pub fn can_back(&self) -> bool {
    self.hist_pos > 0
  }

  pub fn can_forward(&self) -> bool {
    self.hist_pos + 1 < self.history.len()
  }

  /// Escape: dismiss the top-most transient overlay (confirm dialog, context
  /// menu, then card sheet); else leave Settings for the previous screen; else
  /// drop any text-field focus.
  pub(super) fn on_escape(&mut self) -> Task<Message> {
    if self.confirm_delete.take().is_some()
      || self.context_menu.take().is_some()
      || self.card_menu.take().is_some()
    {
      return Task::none();
    }
    if matches!(self.screen, Screen::Settings | Screen::About | Screen::Credits)
    {
      return self.nav_back();
    }
    self.blur_field();
    Task::none()
  }
}

/// Whether a key event is the Escape key.
pub(super) fn is_escape(key: &iced::keyboard::Key) -> bool {
  use iced::keyboard::{Key, key::Named};
  matches!(key, Key::Named(Named::Escape))
}
