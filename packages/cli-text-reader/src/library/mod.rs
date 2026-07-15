//! Local library index: a record of every document opened in the reader, so
//! `:home` can list recently-read documents with progress and re-open them.
//! Stored as append-only JSONL at `~/.config/hygg/.library.jsonl`; works fully
//! offline. Later phases reconcile this with the server's synced library.

mod entry;
mod index;

pub use entry::{LibraryEntry, kind_from_path, title_from_path};
pub use index::{
  latest_entry, load_index, record_open, remove_document, update_entry,
};
