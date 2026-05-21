use serde_json::Value;
use std::sync::{LazyLock, Mutex};

static GITHUB_STARS_CACHE: LazyLock<Mutex<Option<String>>> =
  LazyLock::new(|| Mutex::new(None));

// Fetch GitHub stars count with caching to avoid lag
pub fn fetch_github_stars() -> String {
  // Check cache first
  if let Ok(mut cache) = GITHUB_STARS_CACHE.lock() {
    if let Some(ref stars) = *cache {
      return stars.clone();
    }

    // Fetch stars if not cached
    let agent = ureq::Agent::config_builder()
      .timeout_global(Some(std::time::Duration::from_secs(2)))
      .build()
      .new_agent();
    let stars_text =
      match agent.get("https://api.github.com/repos/kruseio/hygg").call() {
        Ok(mut response) => {
          if let Ok(json) = response.body_mut().read_json::<Value>() {
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
