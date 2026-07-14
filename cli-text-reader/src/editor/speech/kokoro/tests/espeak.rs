// Ground-truth guard that runs the *real* espeak phonemizer end to end, so a
// future espeak/dep bump that again drops clause punctuation fails CI. Skips
// (stays green) when espeak-ng-data can't be located.

use std::path::PathBuf;

use crate::editor::speech::vocab::tokenize;

use super::super::align::{phonemize, tokenize_with_alignment};

#[test]
fn clause_marks_become_pause_tokens_with_real_espeak() {
  if !ensure_espeak_data() || phonemize("test").is_empty() {
    return; // espeak-ng-data unavailable
  }
  let (tokens, wmap) =
    tokenize_with_alignment("In Chapter 2, the kitchen — we were here.");
  assert!(tokens.contains(&3), "comma pause token missing: {tokens:?}");
  assert!(tokens.contains(&4), "period pause token missing: {tokens:?}");
  assert!(
    tokens.contains(&tokenize("—")[0]),
    "em-dash pause missing: {tokens:?}"
  );
  assert!(tokens.len() > 6, "expected real word tokens: {tokens:?}");
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(
    pos("2") < pos(","),
    "comma must follow '2', not merge into 'kitchen'"
  );
  assert!(pos("kitchen") < pos("—") && pos("—") < pos("we"), "{wmap:?}");
}

// Point espeak at the espeak-ng-data the build vendored under an ancestor's
// target/, so the real-espeak test can run in CI without manual env setup.
fn ensure_espeak_data() -> bool {
  if std::env::var_os("PIPER_ESPEAKNG_DATA_DIRECTORY").is_some() {
    return true;
  }
  let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  loop {
    if let Some(share) = find_espeak_share(&dir.join("target")) {
      // SAFETY: test-only. This is the sole writer of the variable and the only
      // espeak caller in the suite (the other espeak/model tests are
      // #[ignore]), so it runs before any espeak read with no concurrent
      // access.
      unsafe {
        std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &share);
      }
      return true;
    }
    if !dir.pop() {
      return false;
    }
  }
}

fn find_espeak_share(target: &std::path::Path) -> Option<PathBuf> {
  for profile in ["debug", "release"] {
    let build = target.join(profile).join("build");
    let Ok(entries) = std::fs::read_dir(&build) else {
      continue;
    };
    for e in entries.flatten() {
      let share = e.path().join("out").join("share");
      if share.join("espeak-ng-data").join("phontab").is_file() {
        return Some(share);
      }
    }
  }
  None
}
