//! The app-wide right-click context menu, overlaid on any screen at the cursor.
//! Its items adapt to what was right-clicked (see [`MenuCtx`]): reader/field
//! Copy + Select-all, field Paste, and Back/Forward navigation everywhere.
//! Clicking outside dismisses it (the backdrop `mouse_area`).

use iced::widget::{button, column, container, mouse_area, opaque, text};
use iced::{Length, Padding, Point};

use super::Element;
use crate::app::{HyggGui, MenuCtx, Message};
use crate::theme::{Palette, style};

/// Build the context menu for `ctx`, anchored at `at` (window coords, clamped
/// on-screen).
pub fn view<'a>(state: &'a HyggGui, at: Point, ctx: &MenuCtx) -> Element<'a> {
  let p = state.palette();
  let mut items: Vec<Element> = Vec::new();

  match ctx {
    MenuCtx::Reader => {
      let sel = state.reader_has_selection().then_some(Message::MenuCopy);
      items.push(item(p, "Copy", sel));
      items.push(item(p, "Select all", Some(Message::MenuSelectAll)));
    }
    MenuCtx::Field(fid) => {
      let (a, b) = state.editor(*fid).range();
      items.push(item(p, "Copy", (a != b).then_some(Message::MenuCopy)));
      items.push(item(p, "Paste", Some(Message::MenuPaste)));
      items.push(item(p, "Select all", Some(Message::MenuSelectAll)));
    }
    MenuCtx::Blank => {}
  }
  items.push(item(p, "Back", state.can_back().then_some(Message::Back)));
  items.push(item(
    p,
    "Forward",
    state.can_forward().then_some(Message::Forward),
  ));

  // Clamp so the menu stays on-screen (estimate its height from the item
  // count).
  let vp = state.viewport();
  let h = items.len() as f32 * 30.0 + 16.0;
  let x = at.x.min((vp.width - 200.0).max(0.0));
  let y = at.y.min((vp.height - h).max(0.0));

  let mut col = column![].spacing(2);
  for it in items {
    col = col.push(it);
  }
  let menu =
    container(col).style(style::card(p)).padding(6).width(Length::Fixed(190.0));

  let positioned = container(menu)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding { top: y, right: 0.0, bottom: 0.0, left: x });

  opaque(mouse_area(positioned).on_press(Message::CloseMenu))
}

/// One menu row. `msg = None` renders it disabled (greyed, not clickable).
fn item<'a>(p: Palette, label: &'a str, msg: Option<Message>) -> Element<'a> {
  let color = if msg.is_some() { p.fg } else { p.muted };
  let mut b = button(text(label).size(14).color(color))
    .style(style::icon(p))
    .padding([6, 12])
    .width(Length::Fill);
  if let Some(m) = msg {
    b = b.on_press(m);
  }
  b.into()
}
