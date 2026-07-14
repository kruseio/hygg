//! The application message type, split out of `mod.rs` for the source LOC
//! budget. Handled in [`super::update`]; produced by the view, subscriptions,
//! and async tasks.

use std::collections::HashMap;
use std::path::PathBuf;

use hygg_shared::sync::SyncMode;
use iced::Size;
use iced::widget::scrollable;

use super::{FieldMsg, MenuCtx};
use crate::assets::ImageAsset;
use crate::model::{Book, BookSummary, Progress};
use crate::settings::{ImageMode, Theme};

#[derive(Debug, Clone)]
pub enum Message {
  // Navigation.
  OpenReader(String),
  GoHome,
  OpenSettings,
  OpenAbout,
  OpenCredits,
  /// Open an external URL in the system browser (About / Credits links).
  OpenUrl(String),
  /// Context-menu navigation through the visited-screen history.
  Back,
  Forward,
  // About & Credits.
  /// The GitHub contributor list finished loading (or failed).
  ContributorsLoaded(Result<Vec<crate::credits::Contributor>, String>),
  /// A contributor's (or the author's) avatar finished downloading + masking;
  /// `None` on any network / decode failure (the card falls back to initials).
  AvatarLoaded(String, Option<iced::widget::image::Handle>),
  // Library.
  LibraryLoaded(Vec<BookSummary>, HashMap<String, Progress>),
  ImportClicked,
  FilePicked(Option<(String, Vec<u8>)>),
  Imported(Result<String, String>),
  OpenedExternal(Result<String, String>),
  /// Open (`Some(id)`) or dismiss (`None`) the remove-confirmation dialog.
  SetConfirmDelete(Option<String>),
  DeleteBook(String),
  SetSyncMode(String, Option<SyncMode>),
  /// Toggle a document's "auto-sync this document" opt-in from its card menu.
  SetDocOptin(String, bool),
  /// Open a library card's "more options" sheet (its sync + remove controls).
  OpenCardMenu(String),
  /// Dismiss the open card sheet.
  CloseCardMenu,
  /// Explicit "sync now" from the home top bar. Pulls from the server even when
  /// background auto-sync is off.
  SyncNow,
  // Reader.
  BookLoaded(Result<(Book, Progress), String>),
  /// The open PDF's live visual source finished parsing — carries the book id
  /// it was opened for (so a stale result is ignored) and `None` on non-PDF or
  /// parse failure. Viewport pages are decoded from it on demand.
  AssetSourceReady(String, Option<crate::assets::AssetSource>),
  /// A batch of page visuals finished decoding — carries the book id it was
  /// extracted for, so a stale result (the user moved on) is ignored. Merged
  /// into the reader's sorted asset list.
  AssetsLoaded(String, Vec<ImageAsset>),
  Scrolled(scrollable::Viewport),
  // Reader text selection (mouse drag) + clipboard hotkeys.
  /// Left button pressed over the reader — begins a drag (anchor set on the
  /// first move).
  SelectStart,
  /// Pointer moved during a drag; carries the position relative to the scroll
  /// viewport's top-left.
  SelectMove(iced::Point),
  /// Left button released — ends the drag.
  SelectEnd,
  /// Shift's held state changed (extends the selection on click).
  SetShift(bool),
  /// A key was pressed — routed to the focused text field, else reader hotkeys.
  KeyPressed(iced::keyboard::Key, Option<String>, iced::keyboard::Modifiers),
  /// A custom text-field interaction (mouse selection / resolved paste).
  Field(FieldMsg),
  /// Cursor moved (window coords) — tracked so a right-click anchors its menu.
  CursorMoved(iced::Point),
  /// Right-click — open the context menu for whatever is under the cursor.
  OpenMenu(MenuCtx),
  /// Dismiss the context menu.
  CloseMenu,
  /// Menu action: copy the reader / field selection, then close.
  MenuCopy,
  /// Menu action: paste into the focused field, then close.
  MenuPaste,
  /// Menu action: select all (the reader document, or the field).
  MenuSelectAll,
  /// One frame of the top bar's slide animation.
  AnimTick,
  // Settings.
  SetTheme(Theme),
  SetImageMode(ImageMode),
  SetZoom(f32),
  SetColumn(u16),
  // Settings → Account (connect this device by username + device token).
  Connect,
  /// A connect attempt resolved: `(username, token, principal)` on success.
  Connected(
    Result<(String, String, hygg_shared::sync::proto::MeResponse), String>,
  ),
  /// A background re-check of stored credentials on opening Settings resolved.
  AccountChecked(Result<hygg_shared::sync::proto::MeResponse, String>),
  Disconnect,
  /// Master sync switch: `false` = fully serverless.
  ToggleSyncEnabled(bool),
  /// Set which documents auto-sync (all / books / manual).
  SetAutoSyncScope(hygg_shared::sync::AutoSyncPolicy),
  // Platform.
  WindowResized(Size),
  /// The OS dropped a document on the window (file handler / drag-drop).
  FileOpened(PathBuf),
  Reloaded,
  ServerSynced,
  /// Periodic background tick; refreshes the home from the server when it's the
  /// open screen and the account is connected with sync enabled.
  Tick,
  Noop,
}
