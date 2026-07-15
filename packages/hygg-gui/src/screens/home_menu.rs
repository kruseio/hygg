//! The library card's "more options" sheet: a modal, overlaid on the home, that
//! holds the per-device sync selector and the remove control moved off the card
//! face. Split out of `home.rs` for the source LOC budget.

use hygg_shared::sync::SyncMode;
use iced::widget::{
  Space, button, checkbox, column, container, mouse_area, opaque, pick_list,
  row, stack, text,
};
use iced::{Alignment, Background, Color, Length};

use super::{Element, sel, semibold};
use crate::app::Message;
use crate::model::BookSummary;
use crate::theme::{Palette, style};
use crate::widget::selectable::SelectionOwner;

/// Overlay the card menu for `b` above `base`: a sync selector and an X remove
/// button, dismissed by tapping the dimmed backdrop.
pub(super) fn modal<'a>(
  p: Palette,
  base: Element<'a>,
  b: &'a BookSummary,
  owner: &SelectionOwner,
) -> Element<'a> {
  let id = b.id.clone();
  let choices = vec![
    SyncChoice::Inherit,
    SyncChoice::Full,
    SyncChoice::Metadata,
    SyncChoice::Off,
  ];
  let selected = SyncChoice::from(b.local_sync_mode);
  let sync = pick_list(choices, Some(selected), move |c| {
    Message::SetSyncMode(id.clone(), c.to_mode())
  })
  .text_size(14)
  .padding([10, 12])
  .width(Length::Fill)
  .style(style::select(p))
  .menu_style(style::select_menu(p));

  // Explicit per-document auto-sync opt-in, so a report or note syncs even when
  // the scope is books-only or manual.
  let optin_id = b.id.clone();
  let optin = checkbox("Auto-sync this document", b.auto_sync_optin)
    .on_toggle(move |on| Message::SetDocOptin(optin_id.clone(), on))
    .size(18)
    .text_size(13);

  let head = row![
    sel(b.title.as_str(), owner.clone(), p)
      .size(16)
      .font(semibold())
      .color(p.fg)
      .width(Length::Fill),
    button(crate::icons::close(p.muted, 18.0))
      .style(style::icon(p))
      .padding(6)
      .on_press(Message::CloseCardMenu),
  ]
  .align_y(Alignment::Center)
  .spacing(10);

  let remove_row = row![
    sel("Remove from library", owner.clone(), p).size(13).color(p.muted),
    Space::with_width(Length::Fill),
    // Opens a confirmation dialog rather than deleting immediately.
    button(crate::icons::close(p.danger, 18.0))
      .style(style::danger(p))
      .padding(8)
      .on_press(Message::SetConfirmDelete(Some(b.id.clone()))),
  ]
  .align_y(Alignment::Center)
  .spacing(10);

  let sheet = container(
    column![
      head,
      sel("Sync on this device", owner.clone(), p).size(12).color(p.muted),
      sync,
      optin,
      remove_row,
    ]
    .spacing(12),
  )
  .style(style::card(p))
  .padding(20)
  .max_width(360.0)
  .width(Length::Fill);

  stack![base, opaque(backdrop(sheet.into(), Message::CloseCardMenu))].into()
}

/// A destructive-action confirmation dialog for removing `b`, overlaid above
/// `base`: a "Remove" (danger fill) and a "Cancel", dismissed by the backdrop.
pub(super) fn confirm_modal<'a>(
  p: Palette,
  base: Element<'a>,
  b: &'a BookSummary,
  owner: &SelectionOwner,
) -> Element<'a> {
  let buttons = row![
    Space::with_width(Length::Fill),
    button(text("Cancel").size(14).color(p.fg))
      .style(style::ghost(p))
      .padding([8, 16])
      .on_press(Message::SetConfirmDelete(None)),
    button(text("Remove").size(14).font(semibold()))
      .style(style::danger_fill(p))
      .padding([8, 16])
      .on_press(Message::DeleteBook(b.id.clone())),
  ]
  .align_y(Alignment::Center)
  .spacing(10);

  let notice = format!(
    "“{}” will be removed from this library. This can't be undone.",
    b.title
  );
  let sheet = container(
    column![
      sel("Remove document?", owner.clone(), p)
        .size(16)
        .font(semibold())
        .color(p.fg),
      sel(notice, owner.clone(), p).size(13).color(p.muted).width(Length::Fill),
      buttons,
    ]
    .spacing(14),
  )
  .style(style::card(p))
  .padding(20)
  .max_width(360.0)
  .width(Length::Fill);

  let dismiss = Message::SetConfirmDelete(None);
  stack![base, opaque(backdrop(sheet.into(), dismiss))].into()
}

/// A dimmed, click-to-dismiss backdrop hosting a modal `sheet`, floated above
/// the vertical center (2:3 spacers) so the sync selector's drop-down has room
/// to open *downward* — iced flips the menu to whichever side has more space.
fn backdrop<'a>(sheet: Element<'a>, dismiss: Message) -> Element<'a> {
  let placed = column![
    Space::with_height(Length::FillPortion(2)),
    opaque(sheet),
    Space::with_height(Length::FillPortion(3)),
  ]
  .align_x(Alignment::Center);
  mouse_area(
    container(placed)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(24)
      .style(|_: &iced::Theme| container::Style {
        background: Some(Background::Color(Color { a: 0.55, ..Color::BLACK })),
        ..container::Style::default()
      }),
  )
  .on_press(dismiss)
  .into()
}

/// The per-document sync selector's choices (a UI enum over
/// `Option<SyncMode>`).
#[derive(Clone, PartialEq, Eq)]
enum SyncChoice {
  Inherit,
  Full,
  Metadata,
  Off,
}

impl SyncChoice {
  fn to_mode(&self) -> Option<SyncMode> {
    match self {
      SyncChoice::Inherit => None,
      SyncChoice::Full => Some(SyncMode::Full),
      SyncChoice::Metadata => Some(SyncMode::Metadata),
      SyncChoice::Off => Some(SyncMode::Off),
    }
  }
}

impl From<Option<SyncMode>> for SyncChoice {
  fn from(m: Option<SyncMode>) -> Self {
    match m {
      None => SyncChoice::Inherit,
      Some(SyncMode::Full) => SyncChoice::Full,
      Some(SyncMode::Metadata) => SyncChoice::Metadata,
      Some(SyncMode::Off) => SyncChoice::Off,
    }
  }
}

impl std::fmt::Display for SyncChoice {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let s = match self {
      SyncChoice::Inherit => "Sync: inherit",
      SyncChoice::Full => "Sync: full",
      SyncChoice::Metadata => "Sync: metadata",
      SyncChoice::Off => "Sync: off",
    };
    f.write_str(s)
  }
}
