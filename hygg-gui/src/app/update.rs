//! The `update` message handler, split from `app/mod.rs` for the LOC budget.

use iced::Task;
use iced::widget::scrollable;

use super::nav::{Nav, is_escape};
use super::tasks::{
  delete_book, import_bytes, load_library, open_path, pick_file,
};
use super::{HyggGui, MenuCtx, Message, Screen};
use crate::screens;
use crate::storage;

impl HyggGui {
  pub(super) fn update(&mut self, message: Message) -> Task<Message> {
    match message {
      // Navigation records history for the menu's Back/Forward (see `nav`).
      Message::OpenReader(id) => self.navigate(Nav::Reader(id), true),
      Message::GoHome => self.navigate(Nav::Home, true),
      Message::OpenSettings => self.navigate(Nav::Settings, true),
      Message::OpenAbout => self.navigate(Nav::About, true),
      Message::OpenCredits => self.navigate(Nav::Credits, true),
      Message::OpenUrl(url) => {
        crate::util::open_url(&url);
        Task::none()
      }
      Message::Back => self.nav_back(),
      Message::Forward => self.nav_forward(),
      // The contributor list resolved: store it and, on success, fetch each
      // avatar we don't already have (the author's is fetched on nav).
      Message::ContributorsLoaded(res) => {
        self.credits.loading = false;
        let mut tasks = Vec::new();
        if let Ok(list) = &res {
          for c in list {
            if !c.avatar_url.is_empty()
              && !self.credits.avatars.contains_key(&c.login)
            {
              let (login, url) = (c.login.clone(), c.avatar_url.clone());
              tasks.push(Task::perform(
                crate::credits::fetch_avatar(login, url),
                |(l, h)| Message::AvatarLoaded(l, h),
              ));
            }
          }
        }
        self.credits.contributors = Some(res);
        Task::batch(tasks)
      }
      Message::AvatarLoaded(login, handle) => {
        if let Some(h) = handle {
          self.credits.avatars.insert(login, h);
        }
        Task::none()
      }
      Message::LibraryLoaded(lib, prog) => {
        self.library = lib;
        self.progress = prog;
        Task::none()
      }
      Message::ImportClicked => {
        self.status = "Choose a document to import…".to_string();
        Task::perform(pick_file(), Message::FilePicked)
      }
      Message::FilePicked(Some((name, bytes))) => {
        self.status = format!("Importing {name}…");
        let col = self.settings.import_col;
        Task::perform(import_bytes(name, bytes, col), Message::Imported)
      }
      Message::FilePicked(None) => {
        self.status.clear();
        Task::none()
      }
      Message::Imported(Ok(_)) => {
        self.status.clear();
        Task::perform(load_library(), |(l, p)| Message::LibraryLoaded(l, p))
      }
      Message::Imported(Err(e)) => {
        self.status = e;
        Task::none()
      }
      Message::OpenedExternal(Ok(id)) => {
        // A double-clicked / dropped document: import (done) then open it.
        self.update(Message::OpenReader(id))
      }
      Message::OpenedExternal(Err(e)) => {
        self.status = e;
        self.screen = Screen::Home;
        Task::none()
      }
      Message::SetConfirmDelete(v) => {
        self.confirm_delete = v;
        Task::none()
      }
      Message::DeleteBook(id) => {
        self.card_menu = None;
        self.confirm_delete = None;
        Task::perform(delete_book(id), |_| Message::Reloaded)
      }
      Message::SetSyncMode(id, mode) => {
        Task::perform(storage::set_local_sync_mode(id, mode), |_| {
          Message::Reloaded
        })
      }
      Message::SetDocOptin(id, opt_in) => {
        Task::perform(storage::set_auto_sync_optin(id, opt_in), |_| {
          Message::Reloaded
        })
      }
      Message::OpenCardMenu(id) => {
        self.card_menu = Some(id);
        Task::none()
      }
      Message::CloseCardMenu => {
        self.card_menu = None;
        Task::none()
      }
      // Explicit "sync now": pull + push under the current scope. Blocked only
      // when the master switch is off (serverless) or not connected.
      Message::SyncNow => match self.settings.creds_manual() {
        Some(creds) => {
          self.status = "Syncing…".to_string();
          self.resync(creds)
        }
        None => {
          self.status = "Connect an account in Settings to sync.".to_string();
          Task::none()
        }
      },
      // Background refresh, only when Home is showing and sync is enabled.
      Message::Tick => match (&self.screen, self.settings.creds()) {
        (Screen::Home, Some(creds)) => self.resync(creds),
        _ => Task::none(),
      },
      Message::Reloaded | Message::ServerSynced => {
        if self.status == "Syncing…" {
          self.status.clear();
        }
        Task::perform(load_library(), |(l, p)| Message::LibraryLoaded(l, p))
      }
      Message::BookLoaded(Ok((book, progress))) => {
        let lh = self.line_height(&book);
        let (_, vh) = self.reader_viewport();
        let target = (progress.line as f32 * lh - vh / 2.0).max(0.0);
        self.reader.scroll_y = target;
        self.reader.book = Some(book);
        self.reader.error = None;
        self.reader.restored = true;
        // `navigate` reset the reader, so `assets` is already empty; extraction
        // (Images mode, PDFs) streams in later via `AssetsLoaded`.
        let scroll = scrollable::scroll_to(
          screens::reader::scroll_id(),
          scrollable::AbsoluteOffset { x: 0.0, y: target },
        );
        Task::batch([scroll, self.maybe_load_assets()])
      }
      Message::BookLoaded(Err(e)) => {
        self.reader.error = Some(e);
        Task::none()
      }
      // The visual source is ready: size the per-page tracker, keep it, and
      // decode the pages already on screen. Ignored if the user has moved on.
      Message::AssetSourceReady(id, src) if self.reader.id == id => {
        self.reader.source_pending = false;
        match src {
          Some(s) => {
            self.reader.pages_done = vec![false; s.total_pages + 1];
            self.reader.source = Some(s);
            self.request_visible_pages()
          }
          None => Task::none(),
        }
      }
      Message::AssetSourceReady(..) => Task::none(),
      // Merge a page batch into the sorted asset list, unless the user has
      // moved on since.
      Message::AssetsLoaded(id, mut a) if self.reader.id == id => {
        self.reader.assets.append(&mut a);
        self.reader.assets.sort_by_key(|x| x.line_start);
        Task::none()
      }
      Message::AssetsLoaded(..) => Task::none(),
      Message::Scrolled(viewport) => {
        self.on_reader_scroll(viewport.absolute_offset().y);
        Task::batch([
          self.save_reader_progress_throttled(),
          self.request_visible_pages(),
        ])
      }
      Message::SelectStart => {
        self.reader_press();
        Task::none()
      }
      Message::SelectMove(p) => {
        self.reader_move(p);
        Task::none()
      }
      Message::SelectEnd => {
        self.reader.selecting = false;
        Task::none()
      }
      Message::SetShift(held) => {
        self.reader.shift_held = held;
        Task::none()
      }
      Message::KeyPressed(ref key, ..) if is_escape(key) => self.on_escape(),
      // Else: edit the focused text field, or the reader's Cmd/Ctrl+C.
      Message::KeyPressed(key, text, mods) => {
        if let Some(fid) = self.fields.focused {
          self.field_key_input(fid, key, text, mods)
        } else if mods.command()
          && matches!(key.as_ref(), iced::keyboard::Key::Character("c"))
        {
          match self.selection_text() {
            Some(t) => iced::clipboard::write(t),
            None => Task::none(),
          }
        } else {
          Task::none()
        }
      }
      Message::Field(m) => self.field_update(m),
      Message::CursorMoved(p) => {
        self.cursor = p;
        Task::none()
      }
      Message::OpenMenu(ctx) => {
        self.context_menu = Some((self.cursor, ctx));
        Task::none()
      }
      Message::CloseMenu => {
        self.context_menu = None;
        Task::none()
      }
      Message::MenuCopy => match self.context_menu.take() {
        Some((_, MenuCtx::Reader)) => match self.selection_text() {
          Some(t) => iced::clipboard::write(t),
          None => Task::none(),
        },
        Some((_, MenuCtx::Field(fid))) => self.field_copy(fid),
        _ => Task::none(),
      },
      Message::MenuPaste => match self.context_menu.take() {
        Some((_, MenuCtx::Field(fid))) => self.field_paste(fid),
        _ => Task::none(),
      },
      Message::MenuSelectAll => {
        match self.context_menu.take() {
          Some((_, MenuCtx::Reader)) => self.reader_select_all(),
          Some((_, MenuCtx::Field(fid))) => self.field_select_all(fid),
          _ => {}
        }
        Task::none()
      }
      Message::AnimTick => self.anim_step(),
      // Settings + account/sync arms live in the sibling `update_account`
      // module (LOC budget); delegate them unchanged.
      m @ (Message::SetTheme(_)
      | Message::SetImageMode(_)
      | Message::SetZoom(_)
      | Message::SetColumn(_)
      | Message::Connect
      | Message::Connected(_)
      | Message::AccountChecked(_)
      | Message::Disconnect
      | Message::ToggleSyncEnabled(_)
      | Message::SetAutoSyncScope(_)) => self.update_account(m),
      Message::WindowResized(size) => {
        self.viewport = size;
        Task::none()
      }
      Message::FileOpened(path) => {
        let col = self.settings.import_col;
        Task::perform(open_path(path, col), Message::OpenedExternal)
      }
      Message::Noop => Task::none(),
    }
  }
}
