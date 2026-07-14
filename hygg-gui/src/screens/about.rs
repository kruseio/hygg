//! The About screen: which build of hygg this is — version, the git commit it
//! was built from (short hash + date, with a link to that commit on GitHub),
//! the author, and the repository — plus shortcuts to Credits and Settings.
//! Opened from Settings. The same facts feed the macOS "About" panel (see
//! `platform/macos/bundle.sh`).

use iced::Length;
use iced::widget::{column, container, scrollable};

use super::{Element, bold, link_button, sel, semibold, top_bar};
use crate::app::{HyggGui, Message};
use crate::build_info as bi;
use crate::theme::{Palette, style};
use crate::widget::selectable::SelectionOwner;

pub fn view(state: &HyggGui) -> Element<'_> {
  let p = state.palette();
  let owner = state.sel_owner();

  let header = column![
    sel("hygg", owner.clone(), p).size(30).font(bold()).color(p.fg),
    sel("A calm, offline-first document reader.", owner.clone(), p)
      .size(14)
      .color(p.muted),
  ]
  .spacing(6);

  let mut rows =
    column![info_row(p, &owner, "Version", bi::VERSION.to_string())]
      .spacing(12);
  rows = rows.push(info_row(p, &owner, "Commit", commit_label()));
  let committed = bi::commit_timestamp();
  if !committed.is_empty() {
    rows = rows.push(info_row(p, &owner, "Committed", committed));
  }
  rows = rows.push(info_row(p, &owner, "Author", bi::AUTHOR.to_string()));
  rows = rows.push(info_row(p, &owner, "License", "AGPL-3.0-only".to_string()));
  let info_card =
    container(rows).style(style::card(p)).padding(20).width(Length::Fill);

  let links = column![
    link_button(
      p,
      crate::icons::github(p.fg, 18.0),
      "View on GitHub",
      Message::OpenUrl(bi::REPOSITORY.to_string()),
    ),
    link_button(
      p,
      crate::icons::external(p.fg, 16.0),
      "View this commit",
      Message::OpenUrl(bi::commit_url()),
    ),
    link_button(
      p,
      crate::icons::heart(p.fg, 17.0),
      "Credits",
      Message::OpenCredits,
    ),
  ]
  .spacing(10);

  let body =
    column![header, info_card, links].spacing(20).padding(24).max_width(560.0);

  let page = column![
    top_bar(p, "About", Some(Message::Back), None, Some(Message::OpenSettings)),
    scrollable(container(body).width(Length::Fill).center_x(Length::Fill))
      .height(Length::Fill),
  ];

  container(page)
    .style(style::app(p))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The short commit hash, tagged when built outside a git checkout.
fn commit_label() -> String {
  if bi::GIT_SHA == "unknown" {
    "unknown (built outside a git checkout)".to_string()
  } else {
    bi::GIT_SHA.to_string()
  }
}

/// One `Label   value` line: a fixed-width muted label and a selectable value.
fn info_row<'a>(
  p: Palette,
  owner: &SelectionOwner,
  label: &'a str,
  value: String,
) -> Element<'a> {
  iced::widget::row![
    sel(label, owner.clone(), p)
      .size(14)
      .font(semibold())
      .color(p.muted)
      .width(Length::Fixed(110.0)),
    sel(value, owner.clone(), p).size(14).color(p.fg).width(Length::Fill),
  ]
  .spacing(12)
  .into()
}
