//! The body of a refused request.

use serde::{Deserialize, Serialize};

/// The JSON body of a 403 the server refused a request with.
///
/// A deployment can extend the server with its own rules about who may do
/// what. When one of those rules turns a request away it supplies the wording,
/// and optionally somewhere the reader can go about it — so the server states
/// no policy of its own, and a client can present any refusal without knowing
/// what it was about.
///
/// A refusal that carries no `action_url` is simply shown as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenialBody {
  /// The wording to show the reader.
  pub error: String,
  /// Where the reader can act on it, if anywhere.
  #[serde(default)]
  pub action_url: Option<String>,
  /// The link's text. Ignored unless `action_url` is set.
  #[serde(default)]
  pub action_label: Option<String>,
}

impl DenialBody {
  /// The link to offer, when the refusal came with both a target and a label.
  pub fn action(&self) -> Option<(&str, &str)> {
    match (self.action_url.as_deref(), self.action_label.as_deref()) {
      (Some(url), Some(label)) => Some((url, label)),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn a_denial_with_an_action_round_trips() {
    let body = DenialBody {
      error: "not included".into(),
      action_url: Some("https://example.test/#more".into()),
      action_label: Some("See options".into()),
    };
    let value = serde_json::to_value(&body).unwrap();
    assert_eq!(value["error"], "not included");
    let back: DenialBody = serde_json::from_value(value).unwrap();
    assert_eq!(back, body);
    assert_eq!(
      back.action(),
      Some(("https://example.test/#more", "See options"))
    );
  }

  #[test]
  fn a_plain_error_body_deserialises_with_no_action() {
    // Every other error the server returns is a bare `{"error": ...}`.
    let body: DenialBody = serde_json::from_value(json!({
      "error": "forbidden"
    }))
    .unwrap();
    assert_eq!(body.action_url, None);
    assert_eq!(body.action(), None);
  }

  #[test]
  fn an_action_needs_both_a_url_and_a_label() {
    let body: DenialBody = serde_json::from_value(json!({
      "error": "nope", "action_url": "/somewhere"
    }))
    .unwrap();
    assert_eq!(body.action(), None);
  }
}
