//! Style helpers producing the theme-aware closures iced 0.13 widgets expect.
//! Split out of the `theme` module for the source LOC budget; reached as
//! `crate::theme::style::*`.

use iced::widget::overlay::menu;
use iced::widget::{button, container, pick_list, progress_bar};
use iced::{Background, Border, Color, Theme};

use super::Palette;

/// The app background container.
pub fn app(p: Palette) -> impl Fn(&Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(p.bg)),
    text_color: Some(p.fg),
    ..container::Style::default()
  }
}

/// A rounded card surface (library items, stat tiles, top bar).
pub fn card(p: Palette) -> impl Fn(&Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(p.card)),
    text_color: Some(p.fg),
    border: Border { color: p.border, width: 1.0, radius: 16.0.into() },
    ..container::Style::default()
  }
}

/// The translucent, bottom-bordered top bar.
pub fn topbar(p: Palette) -> impl Fn(&Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(p.card)),
    text_color: Some(p.fg),
    border: Border { color: p.border, width: 1.0, radius: 0.0.into() },
    ..container::Style::default()
  }
}

/// The corner reading-percentage pill.
pub fn pill(p: Palette) -> impl Fn(&Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(p.card)),
    text_color: Some(p.fg),
    border: Border { color: p.border, width: 1.0, radius: 999.0.into() },
    ..container::Style::default()
  }
}

/// Primary (accent-filled) button — Import, install, connect.
pub fn primary(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match status {
      button::Status::Hovered | button::Status::Pressed => {
        Palette::mix(p.accent, p.fg, 0.08)
      }
      _ => p.accent,
    };
    button::Style {
      background: Some(Background::Color(bg)),
      text_color: p.on_accent,
      border: Border { radius: 14.0.into(), ..Border::default() },
      ..button::Style::default()
    }
  }
}

/// Neutral bordered button / chip (unselected).
pub fn ghost(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match status {
      button::Status::Hovered | button::Status::Pressed => p.card,
      _ => Color::TRANSPARENT,
    };
    button::Style {
      background: Some(Background::Color(bg)),
      text_color: p.fg,
      border: Border { color: p.border, width: 1.0, radius: 999.0.into() },
      ..button::Style::default()
    }
  }
}

/// A selected chip / segmented control button (accent fill).
pub fn chip_on(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, _| button::Style {
    background: Some(Background::Color(p.accent)),
    text_color: p.on_accent,
    border: Border { radius: 999.0.into(), ..Border::default() },
    ..button::Style::default()
  }
}

/// A borderless icon button (top bar + card controls): transparent at rest, a
/// soft card-tint on hover/press, rounded corners.
pub fn icon(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match status {
      button::Status::Hovered | button::Status::Pressed => {
        Palette::mix(p.card, p.fg, 0.10)
      }
      _ => Color::TRANSPARENT,
    };
    button::Style {
      background: Some(Background::Color(bg)),
      text_color: p.fg,
      border: Border { radius: 12.0.into(), ..Border::default() },
      ..button::Style::default()
    }
  }
}

/// A Material-style outlined select (the card sheet's sync picker): a flat,
/// rounded field on the app surface, its outline warming to the accent when
/// hovered or open.
pub fn select(
  p: Palette,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
  move |_, status| {
    let border = match status {
      pick_list::Status::Hovered | pick_list::Status::Opened => p.accent,
      pick_list::Status::Active => p.border,
    };
    pick_list::Style {
      text_color: p.fg,
      placeholder_color: p.muted,
      handle_color: p.muted,
      background: Background::Color(p.bg),
      border: Border { color: border, width: 1.0, radius: 12.0.into() },
    }
  }
}

/// The select's drop-down list: a card surface, rounded, with a soft accent
/// tint on the hovered row.
pub fn select_menu(p: Palette) -> impl Fn(&Theme) -> menu::Style {
  move |_| menu::Style {
    background: Background::Color(p.card),
    border: Border { color: p.border, width: 1.0, radius: 12.0.into() },
    text_color: p.fg,
    selected_text_color: p.fg,
    selected_background: Background::Color(p.accent_tint(0.20)),
  }
}

/// A destructive icon control (remove): the danger color at rest, a red-tinted
/// fill + red outline on hover so the action reads clearly.
pub fn danger(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let (bg, border) = match status {
      button::Status::Hovered | button::Status::Pressed => {
        (Color { a: 0.14, ..p.danger }, p.danger)
      }
      _ => (Color::TRANSPARENT, p.border),
    };
    button::Style {
      background: Some(Background::Color(bg)),
      text_color: p.danger,
      border: Border { color: border, width: 1.0, radius: 999.0.into() },
      ..button::Style::default()
    }
  }
}

/// A filled destructive button (the confirmation dialog's "Remove"): a solid
/// danger fill that darkens on hover.
pub fn danger_fill(
  p: Palette,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match status {
      button::Status::Hovered | button::Status::Pressed => {
        Palette::mix(p.danger, Color::BLACK, 0.12)
      }
      _ => p.danger,
    };
    button::Style {
      background: Some(Background::Color(bg)),
      text_color: Color::WHITE,
      border: Border { radius: 12.0.into(), ..Border::default() },
      ..button::Style::default()
    }
  }
}

/// A borderless, transparent button (a whole-card open target that inherits the
/// surrounding card surface).
pub fn plain(p: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, _| button::Style {
    background: None,
    text_color: p.fg,
    border: Border::default(),
    ..button::Style::default()
  }
}

/// The reader text-selection highlight: a translucent accent block drawn behind
/// the selected glyphs.
pub fn selection(p: Palette) -> impl Fn(&Theme) -> container::Style {
  move |_| container::Style {
    background: Some(Background::Color(p.accent_tint(0.35))),
    ..container::Style::default()
  }
}

/// The reading-progress bar (muted track, accent fill).
pub fn progress(p: Palette) -> impl Fn(&Theme) -> progress_bar::Style {
  move |_| progress_bar::Style {
    background: Background::Color(p.accent_tint(0.22)),
    bar: Background::Color(p.accent),
    border: Border { radius: 999.0.into(), ..Border::default() },
  }
}
