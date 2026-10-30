// Slicing a str at a byte that is not a char boundary panics (hygg issue #99).
// With CI's `-D warnings` a raw `&s[a..b]` is a build error: use `str::get`
// or a boundary-clamping helper. Tests may slice.
#![cfg_attr(not(test), warn(clippy::string_slice))]

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
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // First thing, before anything can take over the terminal: once the reader
  // owns the screen a panic is otherwise invisible (#99). The guard has to
  // stay bound for all of `main` — its drop during an escaping panic is what
  // restores the terminal and writes the crash report. Exit codes are
  // unchanged: the panic still unwinds out of `main` (101).
  let _crash_guard = cli_text_reader::install_panic_hook();
  let mut args = Args::parse();
  if args.config {
    println!("{}", cli_text_reader::config_env_path()?.display());
    return Ok(());
  }

  // HYGG_DEBUG only. Opened here rather than when the reader starts so the
  // log's header records the terminal and environment as launched, and the
  // input line below lands in it. A log that can't be opened must not stop
  // hygg from reading the book.
  let _ = cli_text_reader::init_debug_logging();
  let mut stdin_content = read_stdin_content();
  debug_log_input(&args, stdin_content.is_some());
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
      let picked = {
        let _stderr = capture_stderr_for_tui();
        cli_text_reader::run_home(args.col)?
      };
      match picked {
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
      let outcome = {
        let _stderr = capture_stderr_for_tui();
        if ocr_enabled {
          cli_text_reader::run_cli_text_reader_pdf_path_with_bundled_ocr(
            pdf_path, args.col,
          )?
        } else {
          cli_text_reader::run_cli_text_reader_pdf_path(pdf_path, args.col)?
        }
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

    let outcome = {
      let _stderr = capture_stderr_for_tui();
      if let Some(content) = prepared.raw_content {
        cli_text_reader::run_cli_text_reader_with_source(
          prepared.lines,
          args.col,
          Some(content),
          prepared.source_path.clone(),
        )?
      } else {
        cli_text_reader::run_cli_text_reader(prepared.lines, args.col)?
      }
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

/// Sends stderr to `<log dir>/stderr.log` for as long as the returned guard
/// lives; bind it around a call that puts the TUI on screen.
///
/// hygg's own code no longer writes to stderr while the TUI is up, and in the
/// default build none of the PDF/EPUB/OCR libraries do either (they only use
/// `log`/`tracing`, and no logger is installed). The `tts` build is the
/// exception: rodio prints "Dropping DeviceSink..." and audio-stream errors,
/// cpal can complain about real-time thread priority, and the vendored
/// espeak-ng `fprintf`s data errors — all from audio threads, mid-session,
/// straight over the reader's screen. So stderr is still diverted while the
/// TUI is up, but into a file rather than /dev/null (nothing is lost), and only
/// for that window: errors before the TUI starts and after it exits reach the
/// terminal. (Panics are reported by the crash guard either way.)
fn capture_stderr_for_tui() -> Option<redirect_stderr::StderrRedirect> {
  // One stderr.log per run: truncate on the first capture, then append, so
  // `:home` -> reader -> home doesn't wipe what an earlier screen captured.
  static FIRST_CAPTURE: AtomicBool = AtomicBool::new(true);
  let path = cli_text_reader::stderr_log_path();
  let truncate = FIRST_CAPTURE.swap(false, Ordering::Relaxed);
  let file = path
    .parent()
    .map_or(Ok(()), std::fs::create_dir_all)
    .and_then(|()| {
      std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(truncate)
        .append(!truncate)
        .open(&path)
    })
    // No writable log dir: fall back to discarding, as hygg always used to —
    // a clean screen matters more than keeping library chatter.
    .or_else(|_| {
      std::fs::File::create(if cfg!(windows) { "NUL" } else { "/dev/null" })
    })
    .ok()?;
  redirect_stderr::redirect_stderr_to_file(&file).ok()
}

/// Debug mode only: what hygg was asked to open, so a bug report says which
/// input path it took. Plain-text sources (stdin, `.txt`) get their detected
/// encoding logged by the input pipeline, which is where they're decoded.
fn debug_log_input(args: &Args, has_stdin: bool) {
  if !cli_text_reader::debug_enabled() || has_stdin {
    return;
  }
  let input = match args.file.as_deref() {
    Some(file) => {
      let kind = std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .map_or("no extension".to_string(), str::to_lowercase);
      format!("input: {file} (type {kind})")
    }
    None => "input: none (home view)".to_string(),
  };
  cli_text_reader::debug_log("startup", &input);
}
