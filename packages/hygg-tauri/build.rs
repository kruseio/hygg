//! Tauri's build step: generates the context (parses `tauri.conf.json`, embeds
//! the frontend `dist/`, wires capabilities). See the Tauri v2 docs.

fn main() {
  tauri_build::build();
}
