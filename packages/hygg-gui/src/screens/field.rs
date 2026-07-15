//! A minimal, app-controlled single-line text field. Unlike iced's `text_input`
//! (whose selection/cursor are private), this renders in the reader's monospace
//! font so pixel↔character mapping is exact arithmetic, and the caret, anchor
//! and value all live in app state ([`crate::app::Fields`]). That lets the
//! right-click menu copy the real selection and paste at the caret — the thing
//! iced's own field can't expose. Mouse selection mirrors the reader;
//! keystrokes are routed here from the app (see `app::fields`).

use iced::widget::text::LineHeight;
use iced::widget::{Space, container, mouse_area, row, stack, text};
use iced::{Background, Border, Length, Pixels};

use super::Element;
use crate::app::{Editor, FieldId, FieldMsg, MenuCtx, Message};
use crate::layout;
use crate::theme::{Palette, style};

/// The field's monospace font size.
pub const FONT: f32 = 15.0;

fn advance() -> f32 {
  layout::char_advance(FONT as f64) as f32
}

/// Map a pointer x (relative to the field's text start) to a character index,
/// clamped to the value length — the field's monospace pixel↔char mapping.
pub fn char_index(x: f32, len: usize) -> usize {
  ((x / advance()).round().max(0.0) as usize).min(len)
}

/// A bordered single-line field: monospace value with a selection highlight and
/// (when focused) a caret, over a full-width click/drag area.
pub fn field<'a>(
  fid: FieldId,
  value: &'a str,
  ed: Editor,
  focused: bool,
  placeholder: &'a str,
  p: Palette,
) -> Element<'a> {
  let adv = advance();
  let lh = layout::line_height(FONT as f64) as f32;
  let (s, e) = (ed.cursor.min(ed.anchor), ed.cursor.max(ed.anchor));

  // Bottom layer: a full-width transparent strip so clicks past the text still
  // land (and the field fills its row).
  let base = Space::new(Length::Fill, Length::Fixed(lh));

  // Selection highlight (behind the glyphs), placed by column arithmetic.
  let sel: Element = if s != e {
    row![
      Space::with_width(Length::Fixed(s as f32 * adv)),
      container(Space::new(
        Length::Fixed((e - s) as f32 * adv),
        Length::Fixed(lh),
      ))
      .style(style::selection(p)),
    ]
    .into()
  } else {
    Space::new(0.0, 0.0).into()
  };

  let glyphs: Element = if value.is_empty() {
    text(placeholder).font(layout::MONO).size(FONT).color(p.muted).into()
  } else {
    text(value)
      .font(layout::MONO)
      .size(FONT)
      .color(p.fg)
      .line_height(LineHeight::Absolute(Pixels(lh)))
      .into()
  };

  // Caret (on top), a 2px bar at the cursor column.
  let caret: Element = if focused {
    row![
      Space::with_width(Length::Fixed(ed.cursor as f32 * adv)),
      container(Space::new(Length::Fixed(2.0), Length::Fixed(lh))).style(
        move |_: &iced::Theme| container::Style {
          background: Some(Background::Color(p.accent)),
          ..container::Style::default()
        },
      ),
    ]
    .into()
  } else {
    Space::new(0.0, 0.0).into()
  };

  let area = mouse_area(stack![base, sel, glyphs, caret])
    .on_press(Message::Field(FieldMsg::Press(fid)))
    .on_release(Message::Field(FieldMsg::Release))
    .on_right_press(Message::OpenMenu(MenuCtx::Field(fid)))
    .on_move(move |pt| Message::Field(FieldMsg::Move(fid, pt.x)))
    .interaction(iced::mouse::Interaction::Text);

  container(area)
    .padding([12, 14])
    .width(Length::Fill)
    .style(move |_: &iced::Theme| container::Style {
      background: Some(Background::Color(p.card)),
      text_color: Some(p.fg),
      border: Border {
        color: if focused { p.accent } else { p.border },
        width: 1.0,
        radius: 12.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}
