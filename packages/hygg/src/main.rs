mod args;
mod binary_lookup;
mod demo_mode;
mod input_pipeline;
mod server_cmd;

use args::Args;
use clap::Parser;
use demo_mode::handle_demo_modes;
use input_pipeline::{
  cleanup_temp_file, prepare_input, read_stdin_content, resolve_pdf_path,
};
use std::io::IsTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut args = Args::parse();
  if args.config {
    println!("{}", cli_text_reader::config_env_path()?.display());
    return Ok(());
  }

  let mut stdin_content = read_stdin_content();
  let ocr_from_cli = args.ocr.is_some();
  // OCR enablement, highest priority first: the `--ocr` flag (which also
  // persists the choice), then the `HYGG_OCR` runtime override, then the saved
  // config, then the compiled-in default — `cfg!(feature = "ocr")`, so a build
  // made with `--features ocr` OCRs scanned PDFs by default, while a build
  // without it returns an actionable "rebuild with --features ocr" error if OCR
  // is ever asked for.
  let ocr_enabled = if let Some(mode) = args.ocr {
    let enabled = mode.enabled();
    cli_text_reader::save_ocr_enabled_config(enabled)?;
    enabled
  } else if let Some(over) = hygg_shared::parse_bool_env("HYGG_OCR") {
    over
  } else {
    cli_text_reader::ocr_enabled_config_opt().unwrap_or(cfg!(feature = "ocr"))
  };

  // Persist the TTS on/off preference when passed; the reader picks it up from
  // config when it starts, so `--tts off` sticks across sessions.
  if let Some(mode) = args.tts {
    cli_text_reader::save_tts_enabled_config(mode.enabled())?;
  }

  if handle_demo_modes(&args)? {
    return Ok(());
  }

  // Headless automation flag (--set-progress): pushes reading progress to the
  // server and exits without opening the reader.
  match server_cmd::handle_server_command(&args)? {
    server_cmd::Outcome::Handled => return Ok(()),
    server_cmd::Outcome::NotApplicable => {}
  }

  // A reader session ends either by quitting hygg or by asking to return to the
  // home library (`:home` / `:Rex`). This loop re-shows the picker on a
  // return-home request and opens whatever is selected next, so `:home` lands
  // on exactly the same full-screen view as launching `hygg` with no
  // arguments.
  let mut return_to_home = false;
  loop {
    // Home view: launched interactively with no document, or a reader asked to
    // come back here. Show the library of recently-read books; the user picks
    // one to resume (or quits). The chosen path is injected as the input file
    // so the normal open + progress-restore path takes over and resumes
    // where they left off.
    if std::io::stdout().is_terminal()
      && (return_to_home
        || (stdin_content.is_none()
          && args.file.is_none()
          && cli_text_reader::should_show_home()))
    {
      match cli_text_reader::run_home(args.col)? {
        Some(path) => args.file = Some(path),
        None => return Ok(()),
      }
      // Coming from the picker we always have a concrete file to open; any
      // piped stdin from the original launch no longer applies.
      stdin_content = None;
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
      let outcome = if ocr_enabled {
        cli_text_reader::run_cli_text_reader_pdf_path_with_bundled_ocr(
          pdf_path, args.col,
        )?
      } else {
        cli_text_reader::run_cli_text_reader_pdf_path(pdf_path, args.col)?
      };
      if outcome == cli_text_reader::RunOutcome::Home {
        args.file = None;
        return_to_home = true;
        continue;
      }
      return Ok(());
    }

    // Redirected PDFs keep the extracted text and include inline truecolor
    // image art. With OCR enabled, OCR text is overlaid onto those rendered
    // regions.
    if !std::io::stdout().is_terminal()
      && stdin_content.is_none()
      && let Some(pdf_path) = resolve_pdf_path(&args)
    {
      let rendered = if ocr_enabled {
        let result = cli_pdf_to_text::pdf_to_ansi_text_with_bundled_ocr(
          &pdf_path, args.col,
        );
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

    let prepared = prepare_input(&args, stdin_content.take(), ocr_enabled)?;

    if !std::io::stdout().is_terminal() {
      println!("{}", prepared.lines.join("\n"));
      cleanup_temp_file(prepared.temp_file.as_deref())?;
      return Ok(());
    }

    if let Err(e) = redirect_stderr::redirect_stderr() {
      eprintln!("Warning: Failed to redirect stderr: {e}");
    }

    let outcome = if let Some(content) = prepared.raw_content {
      cli_text_reader::run_cli_text_reader_with_source(
        prepared.lines,
        args.col,
        Some(content),
        prepared.source_path.clone(),
      )?
    } else {
      cli_text_reader::run_cli_text_reader(prepared.lines, args.col)?
    };

    cleanup_temp_file(prepared.temp_file.as_deref())?;

    if outcome == cli_text_reader::RunOutcome::Home {
      args.file = None;
      return_to_home = true;
      continue;
    }
    return Ok(());
  }
}
