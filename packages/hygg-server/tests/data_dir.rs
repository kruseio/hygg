//! The data directory guard: which directories the server will adopt, and
//! which it refuses rather than write into.

use std::fs;
use std::path::{Path, PathBuf};

use hygg_server::config::DB_FILE_NAME;
use hygg_server::data_dir::{Claim, DataDirError, MARKER_FILE, claim};

/// A directory unique to each test, removed first so a previous run cannot
/// decide this one. Under `target/`, which is already ignored and cleaned.
fn scratch(name: &str) -> PathBuf {
  let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
  let _ = fs::remove_dir_all(&path);
  path
}

fn is_marked(dir: &Path) -> bool {
  fs::read_to_string(dir.join(MARKER_FILE))
    .map(|s| s.contains("hygg-server"))
    .unwrap_or(false)
}

#[test]
fn a_missing_directory_is_created_and_marked() {
  let dir = scratch("missing");
  assert_eq!(claim(&dir).unwrap(), Claim::Created);
  assert!(is_marked(&dir));
}

/// The parent may not exist either — `HYGG_DATA_DIR=/srv/hygg/data` on a fresh
/// host is the ordinary case, not a misconfiguration.
#[test]
fn a_missing_parent_is_created_too() {
  let dir = scratch("missing-parent").join("nested").join("data");
  assert_eq!(claim(&dir).unwrap(), Claim::Created);
  assert!(is_marked(&dir));
}

#[test]
fn an_empty_directory_is_claimed() {
  let dir = scratch("empty");
  fs::create_dir_all(&dir).unwrap();
  assert_eq!(claim(&dir).unwrap(), Claim::ClaimedEmpty);
  assert!(is_marked(&dir));
}

/// The ordinary restart: claimed once, recognised every time after.
#[test]
fn a_marked_directory_is_ours_on_every_later_start() {
  let dir = scratch("restart");
  assert_eq!(claim(&dir).unwrap(), Claim::Created);
  assert_eq!(claim(&dir).unwrap(), Claim::Owned);
  assert_eq!(claim(&dir).unwrap(), Claim::Owned);
}

/// A restart must not care what the server has since written into its own
/// directory — the marker settles it, not the contents.
#[test]
fn a_marked_directory_stays_ours_once_full() {
  let dir = scratch("populated");
  claim(&dir).unwrap();
  fs::write(dir.join(DB_FILE_NAME), b"db").unwrap();
  fs::create_dir_all(dir.join("hygg-logs").join("hygg-server")).unwrap();
  assert_eq!(claim(&dir).unwrap(), Claim::Owned);
}

/// An upgrade from an image that predates the marker: the operator's bind mount
/// is full of our own data, and refusing to boot would be the worse failure.
#[test]
fn a_directory_holding_our_database_is_adopted_and_marked() {
  let dir = scratch("pre-marker");
  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join(DB_FILE_NAME), b"db").unwrap();
  fs::create_dir_all(dir.join("logs")).unwrap();

  assert_eq!(claim(&dir).unwrap(), Claim::ClaimedExisting);
  assert!(is_marked(&dir));
  // Adopting is not a reset: what was already there is still there.
  assert_eq!(fs::read(dir.join(DB_FILE_NAME)).unwrap(), b"db");
  // And now it is an ordinary restart.
  assert_eq!(claim(&dir).unwrap(), Claim::Owned);
}

/// A sibling tool (hygg-pwa's dev server) logs into the shared tree before the
/// server has ever run: a fresh checkout where the PWA was served first. Those
/// are hygg's own logs, so this is a first start and must not be a refusal.
#[test]
fn a_sibling_tools_logs_are_not_a_stranger() {
  let dir = scratch("sibling-logs");
  fs::create_dir_all(dir.join("hygg-logs").join("hygg-pwa")).unwrap();
  fs::write(
    dir.join("hygg-logs").join("hygg-pwa").join("access.log"),
    b"GET /",
  )
  .unwrap();

  assert_eq!(claim(&dir).unwrap(), Claim::ClaimedExisting);
  assert!(is_marked(&dir));
  // The sibling's logs are left exactly where it put them.
  assert_eq!(
    fs::read(dir.join("hygg-logs").join("hygg-pwa").join("access.log"))
      .unwrap(),
    b"GET /"
  );
}

/// The log directory alone is enough — the server need never have run here.
#[test]
fn an_empty_log_directory_is_enough_to_recognise_our_own_tree() {
  let dir = scratch("bare-logs");
  fs::create_dir_all(dir.join("hygg-logs")).unwrap();

  assert_eq!(claim(&dir).unwrap(), Claim::ClaimedExisting);
  assert!(is_marked(&dir));
}

/// Recognising hygg's log directory must not become a way in for everything
/// else: a `hygg-logs` name is not a licence over the files around it.
#[test]
fn a_stranger_is_still_refused_next_to_a_log_directory() {
  let dir = scratch("logs-plus-stranger");
  fs::create_dir_all(dir.join("logs")).unwrap(); // generic, not hygg's
  fs::write(dir.join("thesis.tex"), b"...").unwrap();

  assert!(matches!(claim(&dir).unwrap_err(), DataDirError::Occupied { .. }));
  assert!(!dir.join(MARKER_FILE).exists());
}

/// The whole point: a mount aimed at someone's directory is refused, and
/// nothing is written into it.
#[test]
fn an_unrelated_directory_is_refused_untouched() {
  let dir = scratch("someone-elses");
  fs::create_dir_all(dir.join("src")).unwrap();
  fs::write(dir.join("thesis.tex"), b"...").unwrap();

  let err = claim(&dir).unwrap_err();
  assert!(matches!(err, DataDirError::Occupied { .. }));
  assert!(!dir.join(MARKER_FILE).exists());
  assert_eq!(
    fs::read_dir(&dir).unwrap().count(),
    2,
    "a refused directory must be left exactly as it was found"
  );
}

/// The refusal has to tell an operator which directory, what is in it, and what
/// to do — it is the only thing they will see, since the server exits here.
#[test]
fn the_refusal_names_the_directory_its_contents_and_the_way_out() {
  let dir = scratch("refusal-message");
  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("thesis.tex"), b"...").unwrap();

  let message = claim(&dir).unwrap_err().to_string();
  assert!(message.contains(&dir.display().to_string()));
  assert!(message.contains("thesis.tex"));
  assert!(message.contains("HYGG_DATA_DIR"));
}

/// A long listing must not push the instructions out of view.
#[test]
fn the_refusal_caps_a_long_listing() {
  let dir = scratch("many-entries");
  fs::create_dir_all(&dir).unwrap();
  for i in 0..30 {
    fs::write(dir.join(format!("file-{i:02}")), b"x").unwrap();
  }

  let message = claim(&dir).unwrap_err().to_string();
  assert!(message.contains("and 22 more"));
  assert!(message.contains("HYGG_DATA_DIR"));
}

/// Finder leaves one of these in any directory a macOS user opens, including a
/// brand new empty one. It must not be read as somebody's data.
#[test]
fn a_ds_store_does_not_make_a_directory_occupied() {
  let dir = scratch("ds-store");
  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join(".DS_Store"), b"\0\0").unwrap();

  assert_eq!(claim(&dir).unwrap(), Claim::ClaimedEmpty);
  assert!(is_marked(&dir));
}

#[test]
fn a_file_in_the_directorys_place_is_refused() {
  let path = scratch("not-a-dir");
  fs::create_dir_all(path.parent().unwrap()).ok();
  fs::write(&path, b"i am a file").unwrap();

  let err = claim(&path).unwrap_err();
  assert!(matches!(err, DataDirError::NotADirectory(_)));
  assert!(err.to_string().contains("not a directory"));
}
