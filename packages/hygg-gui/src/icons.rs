//! Small line icons for the top bar and card controls, rendered from embedded
//! SVG and tinted to the palette. Enabled by iced's `svg` feature: the color
//! filter repaints the whole glyph, so the black source strokes below are just
//! placeholders. The paths mirror `hygg-pwa`'s inline SVGs so the native and
//! browser readers speak one icon language.

use iced::widget::svg;
use iced::{Color, Length};

use crate::app::Message;

type Element<'a> = iced::Element<'a, Message>;

/// Back affordance (‹) — replaces the top bar's old "‹ Back" text.
const CHEVRON_LEFT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>"##;

/// Settings gear — replaces the top bar's old "Settings" text.
const GEAR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"##;

/// Sync now — a clockwise "refresh" pair of arrows.
const REFRESH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>"##;

/// The card's "more options" affordance — three dots opening its menu.
const MORE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#000"><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/><circle cx="5" cy="12" r="2"/></svg>"##;

/// Dismiss / remove — a plain X (the modal's close and remove controls).
const CLOSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"##;

/// GitHub mark — the "View on GitHub" / repository link on About & Credits.
const GITHUB: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#000"><path d="M12 .5C5.37.5 0 5.87 0 12.5c0 5.3 3.44 9.8 8.21 11.39.6.11.82-.26.82-.58 0-.29-.01-1.05-.02-2.06-3.34.73-4.04-1.61-4.04-1.61-.55-1.39-1.34-1.76-1.34-1.76-1.09-.75.08-.73.08-.73 1.21.09 1.84 1.24 1.84 1.24 1.07 1.84 2.81 1.31 3.5 1 .11-.78.42-1.31.76-1.61-2.67-.3-5.47-1.34-5.47-5.95 0-1.31.47-2.39 1.24-3.23-.13-.3-.54-1.53.12-3.18 0 0 1.01-.32 3.3 1.23a11.5 11.5 0 0 1 6.01 0c2.29-1.55 3.3-1.23 3.3-1.23.66 1.65.25 2.88.12 3.18.77.84 1.24 1.92 1.24 3.23 0 4.62-2.81 5.64-5.49 5.94.43.37.81 1.1.81 2.22 0 1.6-.01 2.9-.01 3.29 0 .32.22.7.83.58C20.56 22.29 24 17.8 24 12.5 24 5.87 18.63.5 12 .5z"/></svg>"##;

/// Coffee cup — the "Buy me a coffee" support button.
const COFFEE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8h1a4 4 0 0 1 0 8h-1"/><path d="M2 8h16v9a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V8z"/><line x1="6" y1="1" x2="6" y2="4"/><line x1="10" y1="1" x2="10" y2="4"/><line x1="14" y1="1" x2="14" y2="4"/></svg>"##;

/// External-link glyph — appended to links that leave the app.
const EXTERNAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>"##;

/// Heart — the Credits / "support the project" affordance.
const HEART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>"##;

/// Build a square, palette-tinted icon from embedded SVG source.
fn icon<'a>(src: &'static str, size: f32, color: Color) -> Element<'a> {
  svg(svg::Handle::from_memory(src.as_bytes()))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_theme, _status| svg::Style { color: Some(color) })
    .into()
}

pub fn chevron_left<'a>(color: Color, size: f32) -> Element<'a> {
  icon(CHEVRON_LEFT, size, color)
}

pub fn gear<'a>(color: Color, size: f32) -> Element<'a> {
  icon(GEAR, size, color)
}

pub fn refresh<'a>(color: Color, size: f32) -> Element<'a> {
  icon(REFRESH, size, color)
}

pub fn more<'a>(color: Color, size: f32) -> Element<'a> {
  icon(MORE, size, color)
}

pub fn close<'a>(color: Color, size: f32) -> Element<'a> {
  icon(CLOSE, size, color)
}

pub fn github<'a>(color: Color, size: f32) -> Element<'a> {
  icon(GITHUB, size, color)
}

pub fn coffee<'a>(color: Color, size: f32) -> Element<'a> {
  icon(COFFEE, size, color)
}

pub fn external<'a>(color: Color, size: f32) -> Element<'a> {
  icon(EXTERNAL, size, color)
}

pub fn heart<'a>(color: Color, size: f32) -> Element<'a> {
  icon(HEART, size, color)
}
