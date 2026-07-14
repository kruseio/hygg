use std::env;
use std::path::PathBuf;

/// Resolve `binary` to an absolute path by searching `PATH`.
///
/// The current directory is deliberately not searched. Windows' own
/// `CreateProcess` looks there before `PATH`, so spawning a bare name would run
/// a `pandoc.exe` sitting next to the document being opened; callers spawn the
/// absolute path returned here instead, which keeps that from happening.
pub(crate) fn which(binary: &str) -> Option<PathBuf> {
  if binary.is_empty() || binary.contains('\0') {
    return None;
  }

  let extensions = if cfg!(windows) {
    vec!["", ".exe", ".com", ".bat", ".cmd"]
  } else {
    vec![""]
  };

  let paths = env::var("PATH").ok()?;
  for path in env::split_paths(&paths) {
    if !path.exists() || !path.is_dir() {
      continue;
    }

    for &ext in &extensions {
      let full_path = path.join(format!("{binary}{ext}"));
      if full_path.is_file()
        && let Ok(canonical) = full_path.canonicalize()
      {
        return Some(canonical);
      }
    }
  }

  None
}
