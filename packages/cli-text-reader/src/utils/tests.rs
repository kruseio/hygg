#[cfg(test)]
use crate::utils::{
  ensure_config_file_with_defaults, get_hygg_config_dir, get_hygg_config_file,
  get_hygg_subdir_file, parse_bool_env_var, safe_mutex_lock,
};
#[cfg(test)]
use std::env;
#[cfg(test)]
use tempfile::tempdir;

#[test]
fn test_parse_bool_env_var() {
  // Test true values
  unsafe {
    env::set_var("TEST_TRUE_1", "true");
    env::set_var("TEST_TRUE_2", "TRUE");
    env::set_var("TEST_TRUE_3", "True");
  }

  assert_eq!(parse_bool_env_var("TEST_TRUE_1"), Some(true));
  assert_eq!(parse_bool_env_var("TEST_TRUE_2"), Some(true));
  assert_eq!(parse_bool_env_var("TEST_TRUE_3"), Some(true));

  // Test false values
  unsafe {
    env::set_var("TEST_FALSE_1", "false");
    env::set_var("TEST_FALSE_2", "FALSE");
    env::set_var("TEST_FALSE_3", "anything_else");
  }

  assert_eq!(parse_bool_env_var("TEST_FALSE_1"), Some(false));
  assert_eq!(parse_bool_env_var("TEST_FALSE_2"), Some(false));
  assert_eq!(parse_bool_env_var("TEST_FALSE_3"), Some(false));

  // Test non-existent variable
  assert_eq!(parse_bool_env_var("NON_EXISTENT_VAR"), None);

  // Cleanup
  unsafe {
    env::remove_var("TEST_TRUE_1");
    env::remove_var("TEST_TRUE_2");
    env::remove_var("TEST_TRUE_3");
    env::remove_var("TEST_FALSE_1");
    env::remove_var("TEST_FALSE_2");
    env::remove_var("TEST_FALSE_3");
  }
}

#[test]
fn test_safe_mutex_lock() {
  use std::sync::Mutex;

  let mutex = Mutex::new(42);

  // Test successful lock
  {
    let guard = safe_mutex_lock(&mutex).expect("Should acquire lock");
    assert_eq!(*guard, 42);
  }

  // Test that we can acquire the lock again after it's released
  {
    let guard = safe_mutex_lock(&mutex).expect("Should acquire lock again");
    assert_eq!(*guard, 42);
  }
}

#[test]
fn test_ensure_config_file_with_defaults() {
  let temp_dir = tempdir().unwrap();
  let config_file = temp_dir.path().join("test_config.env");

  // Test file creation with defaults
  let default_content = "TEST=true\nDEBUG=false\n";
  ensure_config_file_with_defaults(&config_file, default_content).unwrap();

  assert!(config_file.exists());
  let content = std::fs::read_to_string(&config_file).unwrap();
  assert_eq!(content, default_content);

  // Test that existing file is not overwritten
  let new_content = "MODIFIED=true\n";
  std::fs::write(&config_file, new_content).unwrap();

  ensure_config_file_with_defaults(&config_file, default_content).unwrap();
  let content = std::fs::read_to_string(&config_file).unwrap();
  assert_eq!(content, new_content); // Should not be overwritten
}

#[test]
fn test_hygg_config_functions_integration() {
  // These are integration tests that may create actual config directories
  // They should be safe since they use the standard config directory structure

  // Test basic config directory creation
  let config_dir = get_hygg_config_dir();
  assert!(config_dir.is_ok(), "Should be able to get config directory");

  if let Ok(dir) = config_dir {
    assert!(dir.exists(), "Config directory should exist after creation");
    assert!(dir.ends_with("hygg"), "Directory should end with 'hygg'");
  }

  // Test config file path generation
  let config_file = get_hygg_config_file("test.conf");
  assert!(config_file.is_ok(), "Should be able to generate config file path");

  if let Ok(file_path) = config_file {
    assert!(
      file_path.ends_with("test.conf"),
      "File path should end with filename"
    );
  }

  // Test subdirectory file path generation
  let subdir_file = get_hygg_subdir_file("testdir", "test.json");
  assert!(subdir_file.is_ok(), "Should be able to generate subdir file path");

  if let Ok(file_path) = subdir_file {
    assert!(
      file_path.ends_with("test.json"),
      "File path should end with filename"
    );
    assert!(
      file_path.to_string_lossy().contains("testdir"),
      "Path should contain subdirectory"
    );
  }
}
