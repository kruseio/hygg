//! Home: a reading dashboard — summary stats over a library grid where each
//! document shows its progress, percentage, and when it was last read — plus
//! the importer. A native, offline-first mirror of the PWA home. Each card's
//! sync + remove controls live behind a "more options" sheet (see `home_menu`).

use std::collections::HashMap;

use iced::widget::{
  Space, button, column, container, progress_bar, row, scrollable, text,
};
use iced::{Alignment, Length};

use super::{Element, bold, sel, semibold, top_bar};
use crate::app::Message;
use crate::model::{BookSummary, Progress};
use crate::theme::{Palette, style};
use crate::util::{fmt_duration, fmt_relative};
use crate::widget::selectable::SelectionOwner;

type ProgressMap = HashMap<String, Progress>;

// A render function drawing from several slices of app state — grouping them
// into a struct would only add indirection.
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
  p: &Palette,
  library: &'a [BookSummary],
  progress: &'a ProgressMap,
  status: &'a str,
  width: f32,
  menu: Option<&'a str>,
  confirm: Option<&'a str>,
  owner: SelectionOwner,
) -> Element<'a> {
  let p = *p;

  let import_row = row![
    button(
      text("Import document").size(15).font(semibold()).color(p.on_accent)
    )
    .style(style::primary(p))
    .padding([12, 18])
    .on_press(Message::ImportClicked),
    // "Sync now" moved off the top bar to sit beside the importer.
    button(crate::icons::refresh(p.fg, 19.0))
      .style(style::icon(p))
      .padding(8)
      .on_press(Message::SyncNow),
    text(status).size(14).color(p.muted),
  ]
  .spacing(14)
  .align_y(Alignment::Center);

  let grid: Element = if library.is_empty() {
    text(
      "Your library is empty. Import a PDF, EPUB, or text file to start \
       reading.",
    )
    .size(15)
    .color(p.muted)
    .into()
  } else {
    library_grid(p, library, progress, width, &owner)
  };

  let body = column![stats(p, library, progress, &owner), import_row, grid]
    .spacing(22)
    .padding(20)
    .max_width(880.0);

  let page = column![
    top_bar(p, "", None, None, Some(Message::OpenSettings)),
    scrollable(container(body).width(Length::Fill).center_x(Length::Fill))
      .height(Length::Fill),
  ];

  let content: Element = container(page)
    .style(style::app(p))
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  // The open card's "more options" sheet, overlaid on the whole home.
  let content = match menu.and_then(|id| library.iter().find(|b| b.id == id)) {
    Some(b) => super::home_menu::modal(p, content, b, &owner),
    None => content,
  };
  // A remove-confirmation dialog sits on top of everything.
  match confirm.and_then(|id| library.iter().find(|b| b.id == id)) {
    Some(b) => super::home_menu::confirm_modal(p, content, b, &owner),
    None => content,
  }
}

/// The summary stat row: total reading time, document count, started, finished.
fn stats<'a>(
  p: Palette,
  library: &[BookSummary],
  progress: &ProgressMap,
  owner: &SelectionOwner,
) -> Element<'a> {
  let seconds: f64 = progress.values().map(|p| p.seconds).sum();
  let started = progress.values().filter(|p| p.started()).count();
  let finished = progress.values().filter(|p| p.finished()).count();
  let tiles = [
    (fmt_duration(seconds), "reading time"),
    (library.len().to_string(), "documents"),
    (started.to_string(), "started"),
    (finished.to_string(), "finished"),
  ];
  let mut r = row![].spacing(12);
  for (value, label) in tiles {
    r = r.push(
      container(
        column![
          sel(value, owner.clone(), p).size(20).font(bold()).color(p.fg),
          sel(label, owner.clone(), p).size(12).color(p.muted),
        ]
        .spacing(2),
      )
      .style(style::card(p))
      .padding([14, 16])
      .width(Length::Fill),
    );
  }
  r.into()
}

/// A responsive grid of library cards, packed into rows sized to the window.
fn library_grid<'a>(
  p: Palette,
  library: &'a [BookSummary],
  progress: &'a ProgressMap,
  width: f32,
  owner: &SelectionOwner,
) -> Element<'a> {
  let cols = ((width / 260.0).floor() as usize).clamp(1, 4);
  let mut grid = column![].spacing(14);
  for chunk in library.chunks(cols) {
    let mut r = row![].spacing(14);
    for b in chunk {
      let prog = progress.get(&b.id).copied().unwrap_or_default();
      r = r.push(card(p, b, prog, owner));
    }
    // Pad the final row so cards keep a uniform width.
    for _ in chunk.len()..cols {
      r = r.push(Space::with_width(Length::Fill));
    }
    grid = grid.push(r);
  }
  grid.into()
}

/// One library card: title, progress bar + percentage, format, "last read" +
/// total reading time, and a "more options" control opening the sync/remove
/// sheet.
fn card<'a>(
  p: Palette,
  b: &'a BookSummary,
  prog: Progress,
  owner: &SelectionOwner,
) -> Element<'a> {
  let pct = prog.percent.round() as i64;
  let last = fmt_relative(prog.updated_at);
  // The total time spent reading this document (blank until it's been opened).
  let read = if prog.seconds >= 1.0 {
    format!(" · {} read", fmt_duration(prog.seconds))
  } else {
    String::new()
  };
  let sub = match (prog.started(), last.is_empty()) {
    (true, false) => format!("Last read {last}{read}"),
    (true, true) => format!("In progress{read}"),
    _ => "Not started".to_string(),
  };
  // Size lives on the server / filesystem; the card just shows progress + kind.
  let meta = format!("{pct}% · {}", b.format.to_uppercase());

  let open = button(
    column![
      sel(b.title.as_str(), owner.clone(), p)
        .size(16)
        .font(semibold())
        .color(p.fg)
        .width(Length::Fill),
      progress_bar(0.0..=100.0, prog.percent as f32)
        .height(Length::Fixed(6.0))
        .style(style::progress(p)),
      sel(meta, owner.clone(), p).size(12).color(p.muted),
      sel(sub, owner.clone(), p).size(12).color(p.muted),
    ]
    .spacing(8),
  )
  .style(style::plain(p))
  .padding(0)
  .width(Length::Fill)
  .on_press(Message::OpenReader(b.id.clone()));

  let more = button(crate::icons::more(p.muted, 20.0))
    .style(style::icon(p))
    .padding([2, 8])
    .on_press(Message::OpenCardMenu(b.id.clone()));
  let actions = row![Space::with_width(Length::Fill), more];

  container(column![open, actions].spacing(10))
    .style(style::card(p))
    .padding(18)
    .width(Length::Fill)
    .into()
}
