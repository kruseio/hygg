//! The hygg palette, mapped from the PWA's CSS custom properties onto iced.
//!
//! iced's built-in `Palette` only carries background/text/primary, so the extra
//! hygg surfaces (card, border, muted) live in this [`Palette`] and are applied
//! through the small `style::*` closures the widgets use. Colors are the exact
//! values from `packages/hygg-pwa/styles/main.css` so the two readers look
//! identical.

use iced::{Color, Theme};

use crate::settings::Theme as HyggTheme;

pub mod style;

/// A full hygg color set (one per [`HyggTheme`] variant).
#[derive(Clone, Copy)]
pub struct Palette {
  pub fg: Color,
  pub bg: Color,
  pub muted: Color,
  pub accent: Color,
  pub card: Color,
  pub border: Color,
  /// Foreground used *on* an accent fill (buttons/chips) — near-black brown.
  pub on_accent: Color,
  /// Destructive-action color (remove / delete), per theme.
  pub danger: Color,
}

/// Parse a `#rrggbb` hex string into an iced [`Color`] (opaque).
const fn hex(rgb: u32) -> Color {
  Color::from_rgb(
    ((rgb >> 16) & 0xff) as f32 / 255.0,
    ((rgb >> 8) & 0xff) as f32 / 255.0,
    (rgb & 0xff) as f32 / 255.0,
  )
}

impl Palette {
  pub fn of(theme: HyggTheme) -> Palette {
    match theme {
      HyggTheme::Dark => Palette {
        fg: hex(0xe8e6e0),
        bg: hex(0x0b0b0b),
        muted: hex(0x8a8a8a),
        accent: hex(0xc8a26a),
        card: hex(0x161616),
        border: hex(0x262626),
        on_accent: hex(0x1a1206),
        danger: hex(0xe06c6c),
      },
      HyggTheme::Light => Palette {
        fg: hex(0x1b1b1b),
        bg: hex(0xfbfbf9),
        muted: hex(0x6b6b6b),
        accent: hex(0xc8a26a),
        card: hex(0xffffff),
        border: hex(0xe2e2dc),
        on_accent: hex(0x1a1206),
        danger: hex(0xcf3b3b),
      },
      HyggTheme::Sepia => Palette {
        fg: hex(0x4a3f33),
        bg: hex(0xf4ecd8),
        muted: hex(0x8a795f),
        accent: hex(0xc8a26a),
        card: hex(0xefe6cf),
        border: hex(0xddcfae),
        on_accent: hex(0x1a1206),
        danger: hex(0xb5452f),
      },
    }
  }

  /// Build the iced base `Theme` so stock widgets (sliders, scrollbars, text
  /// inputs) already pick up the reader's background/text/accent.
  pub fn iced_theme(&self) -> Theme {
    Theme::custom(
      "hygg".to_string(),
      iced::theme::Palette {
        background: self.bg,
        text: self.fg,
        primary: self.accent,
        success: self.accent,
        danger: hex(0xd08770),
      },
    )
  }

  fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
      a.r + (b.r - a.r) * t,
      a.g + (b.g - a.g) * t,
      a.b + (b.b - a.b) * t,
    )
  }

  /// A translucent accent tint (the "speaking" line highlight / soft fills).
  pub fn accent_tint(&self, alpha: f32) -> Color {
    Color { a: alpha, ..self.accent }
  }
}
