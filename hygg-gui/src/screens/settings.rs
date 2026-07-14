//! Settings: reading preferences (text size, theme, column width) plus the
//! optional sync server URL. Every control persists immediately. This is where
//! all configuration lives now that the reader has no command line. The three
//! credential inputs are the custom [`field`] (app-owned selection) so the
//! right-click menu can copy the selection and paste at the caret.

use iced::widget::{
  Space, button, checkbox, column, container, row, scrollable, slider, text,
};
use iced::{Alignment, Length};

use super::field::field;
use super::settings_chips::{
  autosync_scope_chips, connection_hint, image_mode_chips, scope_hint,
  theme_chips,
};
use super::{Element, link_button, sel, semibold, top_bar};
use crate::app::{FieldId, HyggGui, Message};
use crate::build_info as bi;
use crate::theme::{Palette, style};
use crate::widget::selectable::SelectionOwner;

pub fn view(state: &HyggGui) -> Element<'_> {
  let p = state.palette();
  let s = state.settings();
  let owner = state.sel_owner();

  let body = column![
    setting(
      p,
      "Text size",
      row![
        slider(0.7f32..=1.6f32, s.text_zoom, Message::SetZoom).step(0.05f32),
        value(p, format!("{:.0}%", s.text_zoom * 100.0), &owner),
      ]
      .spacing(14)
      .align_y(Alignment::Center)
      .into(),
      "The column auto-fills the window; this zooms it in or out.",
      &owner,
    ),
    setting(
      p,
      "Theme",
      theme_chips(p, s.theme),
      "Dark, light, or sepia — applied instantly across the app.",
      &owner,
    ),
    setting(
      p,
      "Figures & tables",
      image_mode_chips(p, s.image_mode),
      "How PDF images and tables render. A view-only choice — progress still \
       syncs with every other device whichever you pick.",
      &owner,
    ),
    setting(
      p,
      "Column width",
      row![
        slider(40..=100u16, s.import_col as u16, Message::SetColumn).step(2u16),
        value(p, format!("{} cols", s.import_col), &owner),
      ]
      .spacing(14)
      .align_y(Alignment::Center)
      .into(),
      "Applies to documents imported from now on.",
      &owner,
    ),
    setting(
      p,
      "Server",
      input(state, FieldId::ServerUrl, "https://…"),
      connection_hint(s),
      &owner,
    ),
    account_section(state),
    about_section(state),
  ]
  .spacing(4)
  .padding(20)
  .max_width(640.0);

  let page = column![
    top_bar(p, "Settings", Some(Message::GoHome), None, None),
    scrollable(container(body).width(Length::Fill).center_x(Length::Fill))
      .height(Length::Fill),
  ];

  container(page)
    .style(style::app(p))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A custom credential field bound to app-owned value + caret/selection.
fn input<'a>(
  state: &'a HyggGui,
  fid: FieldId,
  placeholder: &'a str,
) -> Element<'a> {
  field(
    fid,
    state.field_value(fid),
    state.editor(fid),
    state.field_focused(fid),
    placeholder,
    state.palette(),
  )
}

/// One labelled setting block with a control and a hint line.
fn setting<'a>(
  p: Palette,
  label: &'a str,
  control: Element<'a>,
  hint: &'a str,
  owner: &SelectionOwner,
) -> Element<'a> {
  container(
    column![
      sel(label, owner.clone(), p).size(15).font(semibold()).color(p.fg),
      control,
      sel(hint, owner.clone(), p).size(13).color(p.muted).width(Length::Fill),
    ]
    .spacing(12),
  )
  .padding([18, 0])
  .into()
}

/// Settings → Account: connect this device to the sync server. The user pastes
/// a **device token** (created in the server's Devices page) alongside their
/// **username**; it's validated against `/me`. Connected, it shows the plan and
/// an auto-sync toggle. Fully optional — the reader works offline.
fn account_section(state: &HyggGui) -> Element<'_> {
  let p = state.palette();
  let s = state.settings();
  let a = state.account();
  let owner = state.sel_owner();
  let mut inner = column![
    sel("Account", owner.clone(), p).size(15).font(semibold()).color(p.fg)
  ]
  .spacing(12);

  if s.is_connected() {
    inner = inner.push(
      row![
        sel("Connected for sync", owner.clone(), p).size(14).color(p.fg),
        Space::with_width(Length::Fill),
        button(text("Disconnect").size(14))
          .style(style::ghost(p))
          .padding([9, 16])
          .on_press(Message::Disconnect),
      ]
      .align_y(Alignment::Center),
    );
    if !a.label.is_empty() {
      inner = inner
        .push(sel(a.label.clone(), owner.clone(), p).size(13).color(p.muted));
    }
    inner = inner.push(
      checkbox("Sync with this server", s.sync_enabled)
        .on_toggle(Message::ToggleSyncEnabled)
        .size(18)
        .text_size(14),
    );
    if s.sync_enabled {
      inner = inner
        .push(
          sel("Auto-sync which documents", owner.clone(), p)
            .size(12)
            .color(p.muted),
        )
        .push(autosync_scope_chips(p, s.auto_sync_scope))
        .push(
          sel(scope_hint(s.auto_sync_scope), owner.clone(), p)
            .size(13)
            .color(p.muted)
            .width(Length::Fill),
        );
    }
  } else {
    // Connect requires a token, so gate the button on both fields being filled
    // and no request already in flight.
    let ready =
      !a.busy && !a.user.trim().is_empty() && !a.token.trim().is_empty();
    let mut connect = button(text("Connect").size(14))
      .style(style::primary(p))
      .padding([10, 18]);
    if ready {
      connect = connect.on_press(Message::Connect);
    }
    inner = inner
      .push(input(state, FieldId::User, "Username"))
      .push(input(state, FieldId::Token, "Device token"))
      .push(connect)
      .push(
        sel(
          "Create a device token in the server\u{2019}s Devices page (or via \
           the API), then enter your username and paste the token here.",
          owner.clone(),
          p,
        )
        .size(13)
        .color(p.muted)
        .width(Length::Fill),
      );
  }

  if !a.status.is_empty() {
    inner = inner
      .push(sel(a.status.clone(), owner.clone(), p).size(13).color(p.muted));
  }

  container(inner).padding([18, 0]).into()
}

fn value<'a>(p: Palette, v: String, owner: &SelectionOwner) -> Element<'a> {
  container(sel(v, owner.clone(), p).size(14).color(p.muted))
    .width(Length::Fixed(84.0))
    .align_x(Alignment::End)
    .into()
}

/// Settings → About: shortcuts to the About and Credits screens, plus the
/// current build's version + commit line so it's visible at a glance.
fn about_section(state: &HyggGui) -> Element<'_> {
  let p = state.palette();
  let owner = state.sel_owner();
  container(
    column![
      sel("About", owner.clone(), p).size(15).font(semibold()).color(p.fg),
      row![
        link_button(
          p,
          crate::icons::external(p.fg, 16.0),
          "About hygg",
          Message::OpenAbout,
        ),
        link_button(
          p,
          crate::icons::heart(p.fg, 16.0),
          "Credits",
          Message::OpenCredits,
        ),
      ]
      .spacing(10),
      sel(
        format!("Version {} \u{00b7} {}", bi::VERSION, bi::GIT_SHA),
        owner.clone(),
        p,
      )
      .size(13)
      .color(p.muted)
      .width(Length::Fill),
    ]
    .spacing(12),
  )
  .padding([18, 0])
  .into()
}
