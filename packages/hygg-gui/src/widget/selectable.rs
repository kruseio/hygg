//! A read-only, mouse-selectable static-text widget — a drop-in replacement for
//! `iced::widget::text` that adds browser-style selection (drag to select,
//! double-click a word, triple-click everything, Cmd/Ctrl+A to select all when
//! hovered, Cmd/Ctrl+C to copy). Selection lives entirely in the widget's tree
//! state and copy writes straight to the clipboard, so the widget emits no
//! application messages and needs no application state.
//!
//! The widget is fixed to the app's concrete `iced::Renderer` / `iced::Theme`
//! (it is generic only over the never-used `Message`), which keeps the trait
//! bounds free of generic soup.

use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use iced::advanced::clipboard::Kind;
use iced::advanced::layout::{self, Layout, Limits};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::text::{Paragraph, Renderer as _};
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, Renderer as _, Shell};
use iced::event::Status;
use iced::window::RedrawRequest;
use iced::{
  Background, Border, Color, Element, Event, Font, Length, Rectangle, Shadow,
  Size, keyboard, mouse,
};

#[path = "selectable_ops.rs"]
mod ops;

/// The app's concrete renderer — the widget is not generic over it.
type Renderer = iced::Renderer;
/// The renderer's rich-text paragraph (an `iced_graphics` `Paragraph`).
type Para = <Renderer as iced::advanced::text::Renderer>::Paragraph;

/// Shared "who owns the current selection" token, so only one selectable is
/// highlighted at a time. Create one with `SelectionOwner::default()` and clone
/// it into every selectable.
pub type SelectionOwner = Rc<Cell<u64>>;

/// A selectable static-text widget. Build one with [`selectable`].
pub struct Selectable<'a> {
  content: String,
  owner: SelectionOwner,
  size: Option<f32>,
  font: Option<Font>,
  color: Option<Color>,
  width: Length,
  selection_color: Color,
  marker: PhantomData<&'a ()>,
}

/// Creates a [`Selectable`] showing `content`, sharing the `owner` token with
/// every other selectable so at most one holds the highlight at a time.
pub fn selectable<'a>(
  content: impl Into<String>,
  owner: SelectionOwner,
) -> Selectable<'a> {
  Selectable {
    content: content.into(),
    owner,
    size: None,
    font: None,
    color: None,
    width: Length::Shrink,
    selection_color: Color { a: 0.30, ..Color::from_rgb(0.5, 0.5, 0.5) },
    marker: PhantomData,
  }
}

impl<'a> Selectable<'a> {
  /// Sets the text size in logical pixels (default `14.0`). Accepts a number or
  /// `Pixels`, like `iced::widget::text`.
  pub fn size(mut self, size: impl Into<iced::Pixels>) -> Self {
    self.size = Some(size.into().0);
    self
  }

  /// Sets the font (defaults to the renderer's default font).
  pub fn font(mut self, font: Font) -> Self {
    self.font = Some(font);
    self
  }

  /// Sets the text color (defaults to the theme's text color).
  pub fn color(mut self, color: Color) -> Self {
    self.color = Some(color);
    self
  }

  /// Sets the width strategy (default [`Length::Shrink`]).
  pub fn width(mut self, width: impl Into<Length>) -> Self {
    self.width = width.into();
    self
  }

  /// Sets the highlight fill drawn behind the selected glyphs.
  pub fn selection_color(mut self, color: Color) -> Self {
    self.selection_color = color;
    self
  }
}

/// Per-widget selection state, stored in the widget tree.
#[derive(Default)]
struct State {
  para: Para,
  bounds: Size,
  anchor: usize,
  cursor: usize,
  click_at: usize,
  pressed: bool,
  drag: bool,
  last_ms: f64,
  clicks: u8,
  owner: u64,
}

impl<Message> Widget<Message, iced::Theme, Renderer> for Selectable<'_> {
  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<State>()
  }

  fn state(&self) -> tree::State {
    tree::State::new(State::default())
  }

  fn size(&self) -> Size<Length> {
    Size { width: self.width, height: Length::Shrink }
  }

  fn layout(
    &self,
    tree: &mut Tree,
    renderer: &Renderer,
    limits: &Limits,
  ) -> layout::Node {
    let state = tree.state.downcast_mut::<State>();
    let size = self.size.unwrap_or_else(|| renderer.default_size().0);
    let font = self.font.unwrap_or_else(|| renderer.default_font());
    layout::sized(limits, self.width, Length::Shrink, |limits| {
      let bounds = limits.max();
      state.bounds = bounds;
      let sel = ops::ordered(state.anchor, state.cursor);
      state.para =
        ops::build(&self.content, sel, bounds, size, font, self.color);
      state.para.min_bounds()
    })
  }

  fn draw(
    &self,
    tree: &Tree,
    renderer: &mut Renderer,
    _theme: &iced::Theme,
    style: &Style,
    layout: Layout<'_>,
    _cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    let state = tree.state.downcast_ref::<State>();
    let bounds = layout.bounds();
    if state.anchor != state.cursor {
      for r in state.para.span_bounds(1) {
        let quad = Quad {
          bounds: Rectangle {
            x: bounds.x + r.x,
            y: bounds.y + r.y,
            width: r.width,
            height: r.height,
          },
          border: Border::default(),
          shadow: Shadow::default(),
        };
        renderer.fill_quad(quad, Background::Color(self.selection_color));
      }
    }
    let color = self.color.unwrap_or(style.text_color);
    renderer.fill_paragraph(&state.para, bounds.position(), color, *viewport);
  }

  fn on_event(
    &mut self,
    tree: &mut Tree,
    event: Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    _viewport: &Rectangle,
  ) -> Status {
    let state = tree.state.downcast_mut::<State>();
    let bounds = layout.bounds();
    let chars = self.content.chars().count();
    let mut changed = false;

    // Drop a stale highlight when another selectable has claimed ownership.
    if state.owner != self.owner.get() && state.anchor != state.cursor {
      state.anchor = 0;
      state.cursor = 0;
      changed = true;
    }

    let status = match event {
      Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
        match ops::locate(state, bounds, cursor, &self.content) {
          Some(idx) => {
            let cap = ops::press(state, &self.owner, &self.content, idx, chars);
            changed = true;
            if cap { Status::Captured } else { Status::Ignored }
          }
          None => Status::Ignored,
        }
      }
      Event::Mouse(mouse::Event::CursorMoved { .. }) if state.pressed => {
        // Only a move that crosses to a different character counts as a drag —
        // a stationary click can emit a spurious `CursorMoved`, and treating
        // that as a drag would swallow the click from a card button underneath.
        let idx =
          ops::locate(state, bounds, cursor, &self.content).unwrap_or(chars);
        if idx != state.anchor {
          state.cursor = idx;
          state.drag = true;
          changed = true;
        }
        if state.drag { Status::Captured } else { Status::Ignored }
      }
      Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
        let was_drag = state.drag;
        state.pressed = false;
        state.drag = false;
        if was_drag { Status::Captured } else { Status::Ignored }
      }
      Event::Keyboard(keyboard::Event::KeyPressed {
        key, modifiers, ..
      }) => {
        let owned = state.owner == self.owner.get();
        let has_sel = state.anchor != state.cursor;
        match key.as_ref() {
          keyboard::Key::Character("c")
            if modifiers.command() && owned && has_sel =>
          {
            let text = ops::selected(&self.content, state.anchor, state.cursor);
            clipboard.write(Kind::Standard, text);
            Status::Captured
          }
          keyboard::Key::Character("a")
            if modifiers.command() && cursor.is_over(bounds) =>
          {
            state.anchor = 0;
            state.cursor = chars;
            ops::claim(&self.owner, state);
            changed = true;
            Status::Captured
          }
          _ => Status::Ignored,
        }
      }
      _ => Status::Ignored,
    };

    if changed {
      let size = self.size.unwrap_or_else(|| renderer.default_size().0);
      let font = self.font.unwrap_or_else(|| renderer.default_font());
      let sel = ops::ordered(state.anchor, state.cursor);
      state.para =
        ops::build(&self.content, sel, state.bounds, size, font, self.color);
      shell.request_redraw(RedrawRequest::NextFrame);
    }
    status
  }

  fn mouse_interaction(
    &self,
    _tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    _viewport: &Rectangle,
    _renderer: &Renderer,
  ) -> mouse::Interaction {
    if cursor.is_over(layout.bounds()) {
      mouse::Interaction::Text
    } else {
      mouse::Interaction::Idle
    }
  }
}

impl<'a, Message> From<Selectable<'a>> for Element<'a, Message>
where
  Message: 'a,
{
  fn from(widget: Selectable<'a>) -> Self {
    Element::new(widget)
  }
}
