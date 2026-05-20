use std::env;
use std::path::PathBuf;

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

  if cfg!(windows)
    && let Ok(current_dir) = env::current_dir()
  {
    for &ext in &extensions {
      let current_dir_path = current_dir.join(format!("{binary}{ext}"));
      if current_dir_path.is_file()
        && let Ok(canonical) = current_dir_path.canonicalize()
      {
        return Some(canonical);
      }
    }
  }

  None
}
