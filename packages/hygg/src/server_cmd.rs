//! Headless server/automation command: `--set-progress`. It talks to the
//! configured sync server (`SERVER_URL` + `API_TOKEN`, taken from the
//! environment or `~/.config/hygg/.env`) without the interactive reader, so the
//! cross-device flow can be driven from scripts and tests.

use std::time::{SystemTime, UNIX_EPOCH};

use cli_text_reader::config::load_server_config;
use cli_text_reader::sync::{ProgressPayload, SyncClient};
use hygg_shared::sync::content_sha256;

use crate::args::Args;

/// What `main` should do after the server flags are considered.
pub enum Outcome {
  /// A command ran to completion; `main` should exit.
  Handled,
  /// No server flag was set; `main` continues normally.
  NotApplicable,
}

pub fn handle_server_command(
  args: &Args,
) -> Result<Outcome, Box<dyn std::error::Error>> {
  if let Some(offset) = args.set_progress {
    let file =
      args.file.as_deref().ok_or("--set-progress requires a file argument")?;
    set_progress(file, offset)?;
    Ok(Outcome::Handled)
  } else {
    Ok(Outcome::NotApplicable)
  }
}

fn client() -> Result<SyncClient, Box<dyn std::error::Error>> {
  SyncClient::from_config(&load_server_config()).ok_or_else(|| {
    "no server configured: set SERVER_URL and API_TOKEN (or use :connect / \
     :auth in the reader)"
      .into()
  })
}

fn set_progress(
  file: &str,
  offset: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  let bytes = std::fs::read(file)?;
  let book_id = content_sha256(&bytes);
  let total_lines =
    String::from_utf8_lossy(&bytes).lines().count().max(offset + 1);
  let payload = ProgressPayload {
    book_id: book_id.clone(),
    offset,
    total_lines,
    percentage: (offset as f64 / total_lines as f64) * 100.0,
    viewport_offset: Some(offset),
    cursor_y: Some(0),
    page: None,
    line_in_page: None,
    word_offset: None,
    op_id: uuid::Uuid::new_v4().to_string(),
    updated_at: now_millis(),
  };
  client()?.push_progress(&[payload])?;
  println!("progress set to line {offset}/{total_lines} for {book_id}");
  Ok(())
}

fn now_millis() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}
