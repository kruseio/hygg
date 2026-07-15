//! Settings chip rows + hint text, split out of [`super::settings`] for the
//! source LOC budget. Each `*_chips` builder renders a mutually-exclusive
//! button row; the `*_hint`/`connection_hint` helpers return the caption below
//! a control.

use iced::Length;
use iced::widget::{Space, button, row, text};

use super::Element;
use crate::app::Message;
use crate::settings::{ImageMode, Settings, Theme};
use crate::theme::{Palette, style};

pub(super) fn theme_chips<'a>(p: Palette, current: Theme) -> Element<'a> {
  let mut r = row![].spacing(10);
  for theme in Theme::ALL {
    let on = theme == current;
    // `chip_on` and `ghost` are distinct opaque closure types, so branch inside
    // a single style closure rather than in the `if` (which would need the two
    // arms to unify).
    let on_style = style::chip_on(p);
    let off_style = style::ghost(p);
    let chip = button(text(theme.label()).size(14))
      .padding([9, 16])
      .on_press(Message::SetTheme(theme))
      .style(move |t, s| if on { on_style(t, s) } else { off_style(t, s) });
    r = r.push(chip);
  }
  r = r.push(Space::with_width(Length::Fill));
  r.into()
}

/// Auto-sync scope selector (Everything / Books / Manual), matching the theme
/// and image-mode chip rows.
pub(super) fn autosync_scope_chips<'a>(
  p: Palette,
  current: hygg_shared::sync::AutoSyncPolicy,
) -> Element<'a> {
  use hygg_shared::sync::AutoSyncPolicy;
  let mut r = row![].spacing(10);
  for scope in
    [AutoSyncPolicy::All, AutoSyncPolicy::Books, AutoSyncPolicy::Manual]
  {
    let on = scope == current;
    let on_style = style::chip_on(p);
    let off_style = style::ghost(p);
    let chip = button(text(scope_label(scope)).size(14))
      .padding([9, 16])
      .on_press(Message::SetAutoSyncScope(scope))
      .style(move |t, s| if on { on_style(t, s) } else { off_style(t, s) });
    r = r.push(chip);
  }
  r = r.push(Space::with_width(Length::Fill));
  r.into()
}

fn scope_label(scope: hygg_shared::sync::AutoSyncPolicy) -> &'static str {
  use hygg_shared::sync::AutoSyncPolicy;
  match scope {
    AutoSyncPolicy::All => "Everything",
    AutoSyncPolicy::Books => "Books",
    AutoSyncPolicy::Manual => "Manual",
  }
}

pub(super) fn scope_hint(
  scope: hygg_shared::sync::AutoSyncPolicy,
) -> &'static str {
  use hygg_shared::sync::AutoSyncPolicy;
  match scope {
    AutoSyncPolicy::All => "Every document syncs across your devices.",
    AutoSyncPolicy::Books => {
      "Books sync automatically. Add other documents from their menu."
    }
    AutoSyncPolicy::Manual => "Only documents you add from their menu sync.",
  }
}

pub(super) fn image_mode_chips<'a>(
  p: Palette,
  current: ImageMode,
) -> Element<'a> {
  let mut r = row![].spacing(10);
  for mode in ImageMode::ALL {
    let on = mode == current;
    let on_style = style::chip_on(p);
    let off_style = style::ghost(p);
    let chip = button(text(mode.label()).size(14))
      .padding([9, 16])
      .on_press(Message::SetImageMode(mode))
      .style(move |t, s| if on { on_style(t, s) } else { off_style(t, s) });
    r = r.push(chip);
  }
  r = r.push(Space::with_width(Length::Fill));
  r.into()
}

pub(super) fn connection_hint(s: &Settings) -> &'static str {
  if s.is_connected() {
    "Connected. Reading position syncs across your devices."
  } else {
    "Used for optional sync. The reader works fully offline without it."
  }
}
