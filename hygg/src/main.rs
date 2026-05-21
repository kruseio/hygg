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

  if handle_demo_modes(&args)? {
    return Ok(());
  }

  // Streaming path: terminal output, a PDF file, no OCR, no stdin.
  if std::io::stdout().is_terminal()
    && stdin_content.is_none()
    && !args.ocr
    && let Some(pdf_path) = resolve_pdf_path(&args)
  {
    if let Err(e) = redirect_stderr::redirect_stderr() {
      eprintln!("Warning: Failed to redirect stderr: {e}");
    }
    cli_text_reader::run_cli_text_reader_pdf_path(pdf_path, args.col)?;
    return Ok(());
  }

  // Server flags are currently placeholders and intentionally no-op.
  let prepared = prepare_input(&args, stdin_content)?;

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
