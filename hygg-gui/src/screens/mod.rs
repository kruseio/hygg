//! Screen rendering. Each submodule builds one route's view; this module holds
//! the shared chrome (the top bar) and small style/text helpers so the three
//! screens stay visually consistent with the PWA.

pub mod about;
pub mod credits;
pub mod field;
pub mod home;
mod home_menu;
pub mod menu;
pub mod reader;
pub mod settings;
mod settings_chips;

use iced::font::{Font, Weight};
use iced::widget::{Space, button, container, row, text};
use iced::{Alignment, Length};

use crate::app::{Message, TOPBAR_H};
use crate::theme::{Palette, style};
use crate::widget::selectable::{Selectable, SelectionOwner, selectable};

type Element<'a> = iced::Element<'a, Message>;

/// A palette-tinted [`selectable`] label — a drop-in for `text(..)` that the
/// user can drag-select and copy (Cmd/Ctrl+A / +C). Callers still set
/// size/font/color/width as they would on a `text`.
pub fn sel<'a>(
  content: impl Into<String>,
  owner: SelectionOwner,
  p: Palette,
) -> Selectable<'a> {
  selectable(content, owner).selection_color(p.accent_tint(0.35))
}

// Emphasis fonts: cosmic-text resolves the requested weight from the system
// font stack.
pub fn bold() -> Font {
  Font { weight: Weight::Bold, ..Font::DEFAULT }
}

pub fn semibold() -> Font {
  Font { weight: Weight::Semibold, ..Font::DEFAULT }
}

/// The touch-first top bar: a back chevron, a centered title, and the
/// right-side controls (an optional "sync now" refresh, then a settings gear).
/// Each control fires the given message; `None` hides it — Home has no back and
/// blanks its title, Settings has neither sync nor gear.
pub fn top_bar<'a>(
  p: Palette,
  title: impl text::IntoFragment<'a>,
  back: Option<Message>,
  sync: Option<Message>,
  settings: Option<Message>,
) -> Element<'a> {
  let left: Element = match back {
    Some(msg) => icon_button(p, crate::icons::chevron_left(p.fg, 22.0), msg),
    None => Space::with_width(Length::Fixed(96.0)).into(),
  };

  let mut right = row![].spacing(4).align_y(Alignment::Center);
  if let Some(msg) = sync {
    right = right.push(icon_button(p, crate::icons::refresh(p.fg, 19.0), msg));
  }
  if let Some(msg) = settings {
    right = right.push(icon_button(p, crate::icons::gear(p.fg, 22.0), msg));
  }

  container(
    row![
      container(left).width(Length::FillPortion(1)),
      container(text(title).size(16).font(semibold()).color(p.fg))
        .width(Length::FillPortion(2))
        .align_x(Alignment::Center),
      container(right).width(Length::FillPortion(1)).align_x(Alignment::End),
    ]
    // Fill the bar's height so `align_y(Center)` truly centers the controls
    // vertically (a shrink-height row would sit at the top).
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .padding([0, 12]),
  )
  .style(style::topbar(p))
  .width(Length::Fill)
  .height(Length::Fixed(TOPBAR_H))
  .into()
}

/// A borderless top bar control wrapping a single palette-tinted icon.
fn icon_button<'a>(
  p: Palette,
  glyph: Element<'a>,
  msg: Message,
) -> Element<'a> {
  button(glyph).style(style::icon(p)).padding(8).on_press(msg).into()
}

/// A ghost-styled action button pairing a palette-tinted icon with a label —
/// the shared building block for the About / Credits link rows.
pub fn link_button<'a>(
  p: Palette,
  glyph: Element<'a>,
  label: impl text::IntoFragment<'a>,
  msg: Message,
) -> Element<'a> {
  button(
    row![glyph, text(label).size(14).color(p.fg)]
      .spacing(8)
      .align_y(Alignment::Center),
  )
  .style(style::ghost(p))
  .padding([10, 16])
  .on_press(msg)
  .into()
}
