// Slicing a str at a byte that is not a char boundary panics (hygg issue #99).
// With CI's `-D warnings` a raw `&s[a..b]` is a build error: use `str::get`
// or a boundary-clamping helper. Tests may slice.
#![cfg_attr(not(test), warn(clippy::string_slice))]

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let file_path = std::env::args().nth(1).ok_or_else(|| {
    eprintln!(
      "Usage: {} <epub_file>",
      std::env::args().next().unwrap_or_else(|| "cli-epub-to-text".to_string())
    );
    std::process::exit(1);
  })?;

  println!("{}", cli_epub_to_text::epub_to_text(&file_path)?);

  Ok(())
}
