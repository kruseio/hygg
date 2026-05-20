use crate::{args::Args, binary_lookup::which};
use hygg_shared::normalize_file_path;
use std::io::{self, Read};
use std::process::{Command, Stdio};

pub(crate) struct PreparedInput {
  pub(crate) lines: Vec<String>,
  pub(crate) temp_file: Option<String>,
  pub(crate) raw_content: Option<String>,
}

pub(crate) fn read_stdin_content() -> Option<String> {
  if atty::is(atty::Stream::Stdin) {
    return None;
  }

  let mut buffer = String::new();
  match io::stdin().read_to_string(&mut buffer) {
    Ok(_) if buffer.is_empty() => None,
    Ok(_) => Some(buffer),
    Err(_) => None,
  }
}

pub(crate) fn prepare_input(
  args: &Args,
  stdin_content: Option<String>,
) -> Result<PreparedInput, Box<dyn std::error::Error>> {
  if let Some(content) = stdin_content {
    return Ok(PreparedInput {
      lines: cli_justify::justify(&content, args.col),
      temp_file: None,
      raw_content: Some(content),
    });
  }

  let Some(file) = resolve_input_file(args.file.clone()) else {
    return Ok(PreparedInput {
      lines: vec![],
      temp_file: None,
      raw_content: None,
    });
  };

  process_file_input(args, &file)
}

pub(crate) fn cleanup_temp_file(
  temp_file: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
  if let Some(path) = temp_file
    && std::path::Path::new(path).exists()
  {
    std::fs::remove_file(path)?;
  }
  Ok(())
}

fn resolve_input_file(args_file: Option<String>) -> Option<String> {
  if let Some(file) = args_file {
    return Some(file);
  }

  let args_vec: Vec<String> = std::env::args().collect();
  if args_vec.len() <= 1 { None } else { args_vec.last().cloned() }
}

fn process_file_input(
  args: &Args,
  file: &str,
) -> Result<PreparedInput, Box<dyn std::error::Error>> {
  let temp_file = format!("{file}-{}", uuid::Uuid::new_v4());

  let extension = std::path::Path::new(file)
    .extension()
    .and_then(|ext| ext.to_str())
    .map(|ext| ext.to_lowercase());
  let is_pdf = extension.as_deref() == Some("pdf") || args.ocr;

  let content = if args.ocr && which("ocrmypdf").is_some() {
    extract_pdf_text_with_ocr(file, &temp_file)?
  } else {
    read_content_without_ocr(file, extension.as_deref())?
  };

  let lines = if is_pdf {
    cli_justify::justify_pdf_hybrid(&content, args.col)
  } else {
    cli_justify::justify(&content, args.col)
  };

  if lines.is_empty() || (lines.len() == 1 && lines[0].trim().is_empty()) {
    eprintln!("Error: No readable content found in file '{file}'");
    eprintln!("The file may be empty, corrupted, or in an unsupported format.");
    std::process::exit(1);
  }

  Ok(PreparedInput {
    lines,
    temp_file: Some(temp_file),
    raw_content: Some(content),
  })
}

fn extract_pdf_text_with_ocr(
  file: &str,
  temp_file: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  let canonical_file = match normalize_file_path(file) {
    Ok(path) => path.to_string_lossy().to_string(),
    Err(e) => {
      eprintln!("Error: Invalid file path: {e}");
      std::process::exit(1);
    }
  };

  if temp_file.contains("..")
    || temp_file.contains(";")
    || temp_file.contains("|")
    || temp_file.contains("&")
  {
    eprintln!("Error: Invalid temporary file path");
    std::process::exit(1);
  }

  let output = Command::new("ocrmypdf")
    .arg("--force-ocr")
    .arg("--")
    .arg(&canonical_file)
    .arg(temp_file)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .map_err(|e| e.to_string())?;

  if !output.status.success() {
    eprintln!("OCR processing failed");
    std::process::exit(1);
  }

  cli_pdf_to_text::pdf_to_text(temp_file)
}

fn read_content_without_ocr(
  file: &str,
  extension: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
  match extension {
    Some("epub") => match cli_epub_to_text::epub_to_text(file) {
      Ok(content) => Ok(content),
      Err(e) => {
        eprintln!("Error:\nUnable to read EPUB file '{file}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    },
    Some("pdf") => match cli_pdf_to_text::pdf_to_text(file) {
      Ok(content) => Ok(content),
      Err(e) => {
        eprintln!("Error:\nUnable to read PDF file '{file}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    },
    _ => read_via_best_effort(file),
  }
}

fn read_via_best_effort(
  file: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  match pandoc_to_text(file)
    .or_else(|_| cli_epub_to_text::epub_to_text(file))
    .or_else(|_| cli_pdf_to_text::pdf_to_text(file))
  {
    Ok(content) => Ok(content),
    Err(e) => {
      eprintln!("Error:\nUnable to read file '{file}'\n");
      eprintln!("Details:\n{e}\n");

      if which("pandoc").is_none() {
        eprintln!(
          "pandoc not installed!\n\nFor additional formats, install pandoc:\nsudo apt install pandoc\n# scoop install pandoc\n# brew install pandoc"
        );
      }
      std::process::exit(1);
    }
  }
}

fn pandoc_to_text(
  file_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  if which("pandoc").is_none() {
    return Err(
      "pandoc not found. Install with:\nsudo apt install pandoc\n# scoop install pandoc\n# brew install pandoc".into(),
    );
  }

  let canonical_path = normalize_file_path(file_path)?;
  let output = Command::new("pandoc")
    .arg("--to=plain")
    .arg("--wrap=none")
    .arg("--")
    .arg(canonical_path)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!("pandoc failed: {stderr}").into());
  }

  Ok(String::from_utf8(output.stdout)?)
}
