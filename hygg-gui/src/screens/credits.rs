//! The Credits screen: the author (with their GitHub avatar), every repository
//! contributor pulled live from GitHub (avatars fetched + circular-masked in
//! [`crate::credits`]), a mock "Buy me a coffee" support button, and a shortcut
//! back to Settings. Contributor loading is best-effort — offline it shows the
//! author card and a gentle note. Opened from Settings or the About screen.

use iced::widget::{Space, button, column, container, image, row, text};
use iced::{Alignment, Background, Border, Length};

use super::{Element, bold, link_button, sel, semibold, top_bar};
use crate::app::{CreditsState, HyggGui, Message};
use crate::build_info as bi;
use crate::credits::Contributor;
use crate::theme::{Palette, style};
use crate::widget::selectable::SelectionOwner;

/// Placeholder donation link behind the mock "Buy me a coffee" button. TODO:
/// swap for the real Buy Me a Coffee page once the account is set up.
const SUPPORT_URL: &str = "https://www.buymeacoffee.com/kruseio";

pub fn view(state: &HyggGui) -> Element<'_> {
  let p = state.palette();
  let owner = state.sel_owner();
  let credits = state.credits();
  let width = state.viewport().width;

  let footer = row![
    link_button(
      p,
      crate::icons::gear(p.fg, 18.0),
      "Open settings",
      Message::OpenSettings,
    ),
    link_button(
      p,
      crate::icons::external(p.fg, 16.0),
      "About hygg",
      Message::OpenAbout,
    ),
  ]
  .spacing(10);

  let body = column![
    author_card(p, &owner, credits),
    support_card(p, &owner),
    contributors_card(p, &owner, credits, width),
    footer,
  ]
  .spacing(22)
  .padding(24)
  .max_width(720.0);

  let page = column![
    top_bar(
      p,
      "Credits",
      Some(Message::Back),
      None,
      Some(Message::OpenSettings)
    ),
    iced::widget::scrollable(
      container(body).width(Length::Fill).center_x(Length::Fill)
    )
    .height(Length::Fill),
  ];

  container(page)
    .style(style::app(p))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The author card: a large round avatar, name, role, and a GitHub link.
fn author_card<'a>(
  p: Palette,
  owner: &SelectionOwner,
  credits: &CreditsState,
) -> Element<'a> {
  let profile = column![
    sel(bi::AUTHOR, owner.clone(), p).size(22).font(bold()).color(p.fg),
    sel("Author & maintainer", owner.clone(), p).size(13).color(p.muted),
    link_button(
      p,
      crate::icons::github(p.fg, 16.0),
      format!("github.com/{}", bi::OWNER),
      Message::OpenUrl(format!("https://github.com/{}", bi::OWNER)),
    ),
  ]
  .spacing(8);

  container(
    row![avatar(p, credits, bi::OWNER, bi::AUTHOR, 88.0), profile]
      .spacing(18)
      .align_y(Alignment::Center),
  )
  .style(style::card(p))
  .padding(20)
  .width(Length::Fill)
  .into()
}

/// The "support the project" card with the mock Buy-me-a-coffee button.
fn support_card<'a>(p: Palette, owner: &SelectionOwner) -> Element<'a> {
  let coffee = button(
    row![
      crate::icons::coffee(p.on_accent, 18.0),
      text("Buy me a coffee").size(14).font(semibold()).color(p.on_accent),
    ]
    .spacing(8)
    .align_y(Alignment::Center),
  )
  .style(style::primary(p))
  .padding([11, 18])
  .on_press(Message::OpenUrl(SUPPORT_URL.to_string()));

  container(
    column![
      sel("Support the project", owner.clone(), p)
        .size(16)
        .font(semibold())
        .color(p.fg),
      sel(
        "hygg is free and open source. If it makes your reading calmer, you \
         can chip in for a coffee.",
        owner.clone(),
        p,
      )
      .size(13)
      .color(p.muted)
      .width(Length::Fill),
      coffee,
    ]
    .spacing(12),
  )
  .style(style::card(p))
  .padding(20)
  .width(Length::Fill)
  .into()
}

/// The contributors card: a heading plus a responsive grid of avatars, or a
/// loading / offline note while the GitHub list is unavailable.
fn contributors_card<'a>(
  p: Palette,
  owner: &SelectionOwner,
  credits: &'a CreditsState,
  width: f32,
) -> Element<'a> {
  let mut col = column![
    sel("Contributors", owner.clone(), p).size(16).font(semibold()).color(p.fg)
  ]
  .spacing(16);

  let body: Element = match &credits.contributors {
    None => note(p, owner, "Loading contributors\u{2026}".to_string()),
    Some(Ok(list)) if list.is_empty() => {
      note(p, owner, "No contributors found yet.".to_string())
    }
    Some(Ok(list)) => grid(p, credits, list, width),
    Some(Err(e)) => note(
      p,
      owner,
      format!("Couldn't load contributors ({e}). They'll appear once online."),
    ),
  };
  col = col.push(body);
  container(col).style(style::card(p)).padding(20).width(Length::Fill).into()
}

/// A wrapped grid of contributor chips, packed to the window width.
fn grid<'a>(
  p: Palette,
  credits: &'a CreditsState,
  list: &'a [Contributor],
  width: f32,
) -> Element<'a> {
  let cols = ((width / 108.0).floor() as usize).clamp(2, 8);
  let mut grid = column![].spacing(16);
  for chunk in list.chunks(cols) {
    let mut r = row![].spacing(12);
    for c in chunk {
      r = r.push(chip(p, credits, c));
    }
    for _ in chunk.len()..cols {
      r = r.push(Space::with_width(Length::Fixed(88.0)));
    }
    grid = grid.push(r);
  }
  grid.into()
}

/// One contributor: a round avatar over their login, linking to their profile.
fn chip<'a>(
  p: Palette,
  credits: &'a CreditsState,
  c: &'a Contributor,
) -> Element<'a> {
  button(
    column![
      avatar(p, credits, &c.login, &c.login, 60.0),
      text(c.login.clone()).size(12).color(p.fg),
    ]
    .spacing(8)
    .align_x(Alignment::Center)
    .width(Length::Fixed(88.0)),
  )
  .style(style::plain(p))
  .padding(4)
  .on_press(Message::OpenUrl(c.html_url.clone()))
  .into()
}

/// A round avatar for `login`: the fetched circular image if it arrived, else a
/// filled initial disc as a graceful fallback.
fn avatar<'a>(
  p: Palette,
  credits: &CreditsState,
  login: &str,
  name: &str,
  size: f32,
) -> Element<'a> {
  match credits.avatars.get(login) {
    Some(handle) => image(handle.clone())
      .width(Length::Fixed(size))
      .height(Length::Fixed(size))
      .into(),
    None => initials(p, name, size),
  }
}

/// The fallback avatar: the first letter of `name` on an accent-filled disc.
fn initials<'a>(p: Palette, name: &str, size: f32) -> Element<'a> {
  let ch = name
    .chars()
    .next()
    .map(|c| c.to_uppercase().to_string())
    .unwrap_or_else(|| "?".to_string());
  container(text(ch).size(size * 0.4).color(p.on_accent))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_: &iced::Theme| container::Style {
      background: Some(Background::Color(p.accent)),
      border: Border { radius: (size / 2.0).into(), ..Border::default() },
      ..container::Style::default()
    })
    .into()
}

fn note<'a>(p: Palette, owner: &SelectionOwner, s: String) -> Element<'a> {
  sel(s, owner.clone(), p).size(13).color(p.muted).width(Length::Fill).into()
}
