mod args;
mod binary_lookup;
mod demo_mode;
mod input_pipeline;

use args::Args;
use clap::Parser;
use demo_mode::handle_demo_modes;
use input_pipeline::{
  cleanup_temp_file, prepare_input, read_stdin_content, resolve_pdf_path,
};
use std::io::IsTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  let stdin_content = read_stdin_content();
  let ocr_from_cli = args.ocr.is_some();
  let ocr_enabled = if let Some(mode) = args.ocr {
    let enabled = mode.enabled();
    cli_text_reader::save_ocr_enabled_config(enabled)?;
    enabled
  } else {
    cli_text_reader::load_ocr_enabled_config()
  };

  if handle_demo_modes(&args)? {
    return Ok(());
  }

  // Streaming path: terminal output, a PDF file, no stdin.
  if std::io::stdout().is_terminal()
    && stdin_content.is_none()
    && let Some(pdf_path) = resolve_pdf_path(&args)
  {
    let pdf_path = match hygg_shared::normalize_file_path(&pdf_path) {
      Ok(path) => path.to_string_lossy().to_string(),
      Err(e) => {
        eprintln!("Error:\nUnable to read PDF file '{pdf_path}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    };
    if let Err(e) = redirect_stderr::redirect_stderr() {
      eprintln!("Warning: Failed to redirect stderr: {e}");
    }
    if ocr_enabled {
      cli_text_reader::run_cli_text_reader_pdf_path_with_bundled_ocr(
        pdf_path, args.col,
      )?;
    } else {
      cli_text_reader::run_cli_text_reader_pdf_path(pdf_path, args.col)?;
    }
    return Ok(());
  }

  // Redirected PDFs keep the extracted text and include inline truecolor image
  // art. With OCR enabled, OCR text is overlaid onto those rendered regions.
  if !std::io::stdout().is_terminal()
    && stdin_content.is_none()
    && let Some(pdf_path) = resolve_pdf_path(&args)
  {
    let rendered = if ocr_enabled {
      let result =
        cli_pdf_to_text::pdf_to_ansi_text_with_bundled_ocr(&pdf_path, args.col);
      if result.is_err() && !ocr_from_cli {
        cli_pdf_to_text::pdf_to_ansi_text(&pdf_path, args.col)
      } else {
        result
      }
    } else {
      cli_pdf_to_text::pdf_to_ansi_text(&pdf_path, args.col)
    };
    match rendered {
      Ok(content) => println!("{content}"),
      Err(e) => {
        eprintln!("Error:\nUnable to read PDF file '{pdf_path}'\n");
        eprintln!("Details:\n{e}\n");
        std::process::exit(1);
      }
    }
    return Ok(());
  }

  // Server flags are currently placeholders and intentionally no-op.
  let prepared = prepare_input(&args, stdin_content, ocr_enabled)?;

  if !std::io::stdout().is_terminal() {
    println!("{}", prepared.lines.join("\n"));
    cleanup_temp_file(prepared.temp_file.as_deref())?;
    return Ok(());
  }

  if let Err(e) = redirect_stderr::redirect_stderr() {
    eprintln!("Warning: Failed to redirect stderr: {e}");
  }

  if let Some(content) = prepared.raw_content {
    cli_text_reader::run_cli_text_reader_with_content(
      prepared.lines,
      args.col,
      Some(content),
      false,
    )?;
  } else {
    cli_text_reader::run_cli_text_reader(prepared.lines, args.col)?;
  }

  cleanup_temp_file(prepared.temp_file.as_deref())?;
  Ok(())
}
