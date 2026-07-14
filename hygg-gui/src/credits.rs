//! GitHub credits: fetch the repository's contributor list and their avatars
//! for the Credits screen. Best-effort and fully optional — the reader works
//! offline, so a failed request just leaves the Credits page showing the author
//! card with an "offline" note. Avatars are decoded and circular-masked here so
//! the screen can render them as plain round images.

use serde::Deserialize;

use crate::build_info::{OWNER, REPO};

/// One GitHub contributor, as returned by the repo contributors API. Extra
/// fields (bots, anonymous rows) are tolerated by `serde` defaults.
#[derive(Clone, Debug, Deserialize)]
pub struct Contributor {
  pub login: String,
  #[serde(default)]
  pub avatar_url: String,
  #[serde(default)]
  pub html_url: String,
  #[serde(default)]
  pub contributions: u32,
}

/// GitHub requires a User-Agent on API requests; identify ourselves.
const UA: &str = concat!("hygg-gui/", env!("CARGO_PKG_VERSION"));

/// Rendered avatar diameter (source pixels); the screen scales it down.
pub const AVATAR_PX: u32 = 160;

/// Fetch the repo's contributors (most-contributions first, as GitHub orders
/// them). Anonymous / bot-only rows are dropped so the grid shows real people.
pub async fn fetch_contributors() -> Result<Vec<Contributor>, String> {
  let url = format!(
    "https://api.github.com/repos/{OWNER}/{REPO}/contributors?per_page=100"
  );
  let resp = reqwest::Client::new()
    .get(&url)
    .header("User-Agent", UA)
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(format!("GitHub returned {}", resp.status()));
  }
  let list: Vec<Contributor> = resp.json().await.map_err(|e| e.to_string())?;
  let mut people: Vec<Contributor> = list
    .into_iter()
    .filter(|c| !c.login.is_empty() && !c.login.ends_with("[bot]"))
    .collect();
  // GitHub already returns most-contributions-first, but sort defensively so
  // the author and top contributors always lead the grid.
  people.sort_by_key(|c| std::cmp::Reverse(c.contributions));
  Ok(people)
}

/// Download an avatar and turn it into a circular iced image handle, keyed by
/// the login it belongs to. Returns `(login, None)` on any network / decode
/// failure so the caller can fall back to initials.
pub async fn fetch_avatar(
  login: String,
  url: String,
) -> (String, Option<iced::widget::image::Handle>) {
  let handle = download_bytes(&url).await.and_then(|b| circular(&b, AVATAR_PX));
  (login, handle)
}

async fn download_bytes(url: &str) -> Option<Vec<u8>> {
  let resp = reqwest::Client::new()
    .get(url)
    .header("User-Agent", UA)
    .send()
    .await
    .ok()?;
  if !resp.status().is_success() {
    return None;
  }
  resp.bytes().await.ok().map(|b| b.to_vec())
}

/// Decode `bytes`, crop-fill to a square, and knock out the corners with a
/// soft-edged circular alpha mask, yielding an RGBA handle iced renders round.
fn circular(bytes: &[u8], size: u32) -> Option<iced::widget::image::Handle> {
  let img = image::load_from_memory(bytes).ok()?;
  let mut img = img
    .resize_to_fill(size, size, image::imageops::FilterType::Triangle)
    .to_rgba8();
  let r = size as f32 / 2.0;
  for (x, y, px) in img.enumerate_pixels_mut() {
    let dx = x as f32 + 0.5 - r;
    let dy = y as f32 + 0.5 - r;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > r {
      px[3] = 0;
    } else if dist > r - 1.5 {
      // Antialias the one-and-a-half-pixel rim so the circle isn't jagged.
      let a = ((r - dist) / 1.5).clamp(0.0, 1.0);
      px[3] = (px[3] as f32 * a) as u8;
    }
  }
  Some(iced::widget::image::Handle::from_rgba(size, size, img.into_raw()))
}
