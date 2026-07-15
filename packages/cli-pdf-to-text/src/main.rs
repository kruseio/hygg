fn main() -> Result<(), Box<dyn std::error::Error>> {
  let file = std::env::args().nth(1).ok_or_else(|| {
    eprintln!(
      "Usage: {} <pdf_file>",
      std::env::args().next().unwrap_or_else(|| "cli-pdf-to-text".to_string())
    );
    std::process::exit(1);
  })?;

  println!("{}", cli_pdf_to_text::pdf_to_text(&file)?);

  Ok(())
}
