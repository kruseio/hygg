use serde_json::Value;
use std::sync::Mutex;

lazy_static::lazy_static! {
  static ref GITHUB_STARS_CACHE: Mutex<Option<String>> = Mutex::new(None);
}

// Fetch GitHub stars count with caching to avoid lag
pub fn fetch_github_stars() -> String {
  // Check cache first
  if let Ok(mut cache) = GITHUB_STARS_CACHE.lock() {
    if let Some(ref stars) = *cache {
      return stars.clone();
    }

    // Fetch stars if not cached
    let stars_text =
      match ureq::get("https://api.github.com/repos/kruseio/hygg")
        .timeout(std::time::Duration::from_secs(2))
        .call()
      {
        Ok(response) => {
          if let Ok(json) = response.into_json::<Value>() {
            if let Some(stars) = json["stargazers_count"].as_u64() {
              format!("⭐ {stars} stars")
            } else {
              "⭐ stars".to_string()
            }
          } else {
            "⭐ stars".to_string()
          }
        }
        Err(_) => "⭐ stars".to_string(),
      };

    // Cache the result
    *cache = Some(stars_text.clone());
    stars_text
  } else {
    "⭐ stars".to_string()
  }
}
