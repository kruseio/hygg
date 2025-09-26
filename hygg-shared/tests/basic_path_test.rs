use hygg_shared::normalize_file_path;
use std::fs::File;
use std::io::Write;

#[test]
fn test_basic_path_formats() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Create test files
    File::create("test.csv").unwrap().write_all(b"data").unwrap();
    File::create("Nærri\\ lýsing\\ av\\ gjalding_flyting\\ \\(1\\).csv").unwrap().write_all(b"data").unwrap();

    // Test 1: Forward slash relative path
    let result = normalize_file_path("./test.csv");
    assert!(result.is_ok(), "Forward slash path should work: {:?}", result.err());

    // Test 2: Backslash relative path behavior
    let result = normalize_file_path(".\\test.csv");
    if cfg!(windows) {
        assert!(result.is_ok(), "Backslash should work on Windows: {:?}", result.err());
    } else {
        // On Unix, backslash is literal character, should fail because no file with that name exists
        assert!(result.is_err(), "Backslash should fail on Unix (literal character)");
    }

    // Test 3: Path with spaces and special chars (unescaped - shell escaping is handled before our function)
    let result = normalize_file_path("Nærri\\ lýsing\\ av\\ gjalding_flyting\\ \\(1\\).csv");
    assert!(result.is_ok(), "Path with spaces should work: {:?}", result.err());

    std::env::set_current_dir(original_dir).unwrap();
}