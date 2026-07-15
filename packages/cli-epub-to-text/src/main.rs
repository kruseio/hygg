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
