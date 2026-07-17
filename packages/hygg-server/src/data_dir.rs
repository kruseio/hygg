//! Ownership of the directory the server keeps its state in.
//!
//! Everything the server persists — the SQLite database, rotating logs, cached
//! OCR models — lives under one directory, and in a container that directory is
//! whatever the operator's `-v` flag points at. A mount is a single string in a
//! shell command: it is easy to aim at a directory that already holds something
//! else, and the server would then quietly scatter its tree through it.
//!
//! So the directory is claimed. The first time the server uses one it writes a
//! [`MARKER_FILE`]; every later start checks for it. A directory that is empty,
//! or already carries the marker, or holds something of hygg's already, is ours
//! to use. Anything else is a refusal — [`claim`] returns an error and the
//! server does not boot, because a directory whose contents we cannot account
//! for is more likely an operator's mistake than an invitation.
//!
//! "Ours" is the project's, not this process's. The directory is hygg's tree
//! and hygg-server is its principal occupant, but not the only one: sibling
//! tools write their own logs beside the server's under the shared
//! [`LOG_SUBDIR`] (`packages/hygg-pwa/serve_dist.py` does). Finding one of
//! their directories and no database means hygg put it there and the server
//! simply has not run yet — an ordinary first start, not a stranger's
//! directory.
//!
//! This is a guard against accidents, not an access control: it answers "is
//! this hygg's?", and it can only do that before anything else writes to the
//! directory. Call it first — see [`crate::runtime::run`].

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{DB_FILE_NAME, LOG_SUBDIR};

/// Written into a data directory the first time the server claims it, and the
/// proof of ownership every later start looks for.
pub const MARKER_FILE: &str = ".hygg-server";

/// Contents of [`MARKER_FILE`]. `version` describes the directory layout, not
/// the server: a later layout change can recognise an older tree by it and
/// convert deliberately, instead of inferring from whatever files it finds.
const MARKER_CONTENTS: &str = r#"{"service":"hygg-server","version":1}"#;

/// Ignored when deciding whether a directory is empty. macOS Finder drops one
/// into any directory a user so much as looks at, and a directory containing
/// nothing else is not the unrelated data this guard exists to protect.
const IGNORED_WHEN_EMPTY: &[&str] = &[".DS_Store"];

/// How many of a rejected directory's entries the error lists. Enough to
/// recognise the directory, not so many that the message scrolls away.
const SAMPLE_ENTRIES: usize = 8;

/// How [`claim`] came by the data directory. Returned rather than logged
/// because the claim has to run before logging is initialised — the log files
/// live inside the directory being claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
  /// The directory did not exist and was created.
  Created,
  /// The directory already carried our marker: the ordinary restart.
  Owned,
  /// The directory existed but was empty, and is now marked.
  ClaimedEmpty,
  /// The directory held a hygg-server database but no marker — a deployment
  /// that predates it — and is now marked.
  ClaimedExisting,
}

/// Why the server would not adopt a data directory.
pub enum DataDirError {
  /// The path exists but is a file, symlink, or something else not usable.
  NotADirectory(PathBuf),
  /// The directory holds something we did not put there.
  Occupied { path: PathBuf, entries: Vec<String> },
  /// The directory could not be read, created, or marked.
  Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for DataDirError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotADirectory(path) => write!(
        f,
        "data directory {} exists but is not a directory. Point \
         HYGG_DATA_DIR at a directory, or remove that path.",
        path.display()
      ),
      Self::Occupied { path, entries } => {
        write!(
          f,
          "data directory {} is not empty and holds nothing of hygg's (no \
           {MARKER_FILE} marker, no {DB_FILE_NAME}, no {LOG_SUBDIR}/). \
           Refusing to start rather than write into a directory that may not \
           be ours.\n\
           \n\
           It holds: {}\n\
           \n\
           If this is the right directory, empty it and start again. If it is \
           not, point HYGG_DATA_DIR — or the host side of the container's \
           -v mount — at a directory kept for hygg-server alone.",
          path.display(),
          describe(entries)
        )
      }
      Self::Io { path, source } => {
        write!(f, "cannot use data directory {}: {source}", path.display())
      }
    }
  }
}

/// Rendered as [`Display`]. This error reaches the operator through `main`'s
/// `Result<(), Box<dyn Error>>`, which prints its `Debug` — and a derived one
/// would show a struct dump in place of the instructions.
impl fmt::Debug for DataDirError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

impl std::error::Error for DataDirError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Io { source, .. } => Some(source),
      _ => None,
    }
  }
}

/// Claim `data_dir` for this server, creating it if it is not there.
///
/// Succeeds when the directory is ours — created here, already marked, empty,
/// or holding a database from a deployment older than the marker — and marks it
/// so later starts recognise it. Fails when it holds anything else.
///
/// Call this before opening the database or initialising logging: both write
/// inside the directory, and a check that runs after its own side effects
/// cannot distinguish a stranger's directory from one it has begun filling.
pub fn claim(data_dir: &Path) -> Result<Claim, DataDirError> {
  let metadata = match fs::metadata(data_dir) {
    Ok(metadata) => metadata,
    Err(e) if e.kind() == io::ErrorKind::NotFound => {
      create_dir(data_dir)?;
      write_marker(data_dir)?;
      return Ok(Claim::Created);
    }
    Err(source) => return Err(io_error(data_dir, source)),
  };
  if !metadata.is_dir() {
    return Err(DataDirError::NotADirectory(data_dir.to_path_buf()));
  }
  if data_dir.join(MARKER_FILE).is_file() {
    return Ok(Claim::Owned);
  }
  let entries = entries(data_dir)?;
  if entries.is_empty() {
    write_marker(data_dir)?;
    return Ok(Claim::ClaimedEmpty);
  }
  if holds_hygg_data(data_dir) {
    write_marker(data_dir)?;
    return Ok(Claim::ClaimedExisting);
  }
  Err(DataDirError::Occupied { path: data_dir.to_path_buf(), entries })
}

/// Whether `dir` already holds something hygg put there, and is therefore ours
/// to claim even without a marker.
///
/// The database means a deployment older than the marker bind-mounts this
/// directory; refusing to start on an upgrade would be a worse failure than the
/// one being guarded against.
///
/// The log directory means hygg has used this tree even though the server has
/// not run yet — a sibling tool logging under it (see the module docs) reaches
/// the directory first on a fresh checkout, and that is a first start, not an
/// intrusion. Both names belong to hygg alone: nothing else creates a
/// `hygg-server.db` or a `hygg-logs`, so neither can be mistaken for a
/// stranger's file.
fn holds_hygg_data(dir: &Path) -> bool {
  dir.join(DB_FILE_NAME).is_file() || dir.join(LOG_SUBDIR).is_dir()
}

fn create_dir(data_dir: &Path) -> Result<(), DataDirError> {
  fs::create_dir_all(data_dir).map_err(|e| io_error(data_dir, e))
}

fn write_marker(data_dir: &Path) -> Result<(), DataDirError> {
  let path = data_dir.join(MARKER_FILE);
  fs::write(&path, format!("{MARKER_CONTENTS}\n"))
    .map_err(|e| io_error(&path, e))
}

fn io_error(path: &Path, source: io::Error) -> DataDirError {
  DataDirError::Io { path: path.to_path_buf(), source }
}

/// Names in `dir`, minus the ones that never count as contents. Sorted, so a
/// rejection message reads the same way twice.
fn entries(dir: &Path) -> Result<Vec<String>, DataDirError> {
  let mut names = Vec::new();
  for entry in fs::read_dir(dir).map_err(|e| io_error(dir, e))? {
    let entry = entry.map_err(|e| io_error(dir, e))?;
    let name = entry.file_name().to_string_lossy().into_owned();
    if IGNORED_WHEN_EMPTY.contains(&name.as_str()) {
      continue;
    }
    names.push(name);
  }
  names.sort();
  Ok(names)
}

/// Render a directory's entries for the rejection message, capped so a large
/// directory does not bury the instructions under its own listing.
fn describe(entries: &[String]) -> String {
  let shown =
    entries.iter().take(SAMPLE_ENTRIES).cloned().collect::<Vec<_>>().join(", ");
  match entries.len().checked_sub(SAMPLE_ENTRIES) {
    Some(rest) if rest > 0 => format!("{shown} (and {rest} more)"),
    _ => shown,
  }
}
