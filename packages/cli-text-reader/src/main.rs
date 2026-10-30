// Issue #99: slicing a str at a byte that is not a char boundary panics, and
// took the reader down on accented text. Every slice goes through
// `editor::char_boundary` (`slice`, `slice_from`, `slice_to`) instead; with
// CI's `-D warnings` a raw `&s[a..b]` is a build error. Tests may slice.
#![cfg_attr(not(test), warn(clippy::string_slice))]

use std::env;
use std::io::{self, BufRead};

pub fn print_help_menu(args: Vec<String>, opts: getopts::Options) {
  let brief = format!("Usage: {} FILE [options]", args[0]);
  print!("{}", opts.usage(&brief));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Keep bound for all of `main`: restores the terminal and writes a crash
  // report if a panic escapes the reader (see `cli_text_reader::crash`).
  let _crash_guard = cli_text_reader::install_panic_hook();
  let args: Vec<String> = env::args().collect();
  let mut opts = getopts::Options::new();

  opts.optopt("c", "col", "set the column, defaults to 110", "NUMBER");
  opts.optflag("h", "help", "print this help menu");

  let matches = opts.parse(&args[1..])?;

  if matches.opt_present("h") {
    print_help_menu(args, opts);
    return Ok(());
  }

  let col: usize = match matches.opt_str("c") {
    Some(x) => x.parse().unwrap_or(110),
    None => 110,
  };

  let lines_vec_arc = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
  let lines_vec_arc_clone = std::sync::Arc::clone(&lines_vec_arc);

  let handle = std::thread::spawn(move || {
    let stdin = io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
      match lines_vec_arc_clone.lock() {
        Ok(mut vec) => vec.push(line),
        Err(e) => {
          eprintln!("Warning: Mutex lock failed: {e}");
          return; // Exit thread on lock failure
        }
      }
    }
  });

  // Wait for the thread to finish reading all input
  if let Err(e) = handle.join() {
    return Err(format!("Input reading thread failed: {e:?}").into());
  }

  let lines_vec = lines_vec_arc
    .lock()
    .map_err(|e| format!("Failed to acquire mutex lock: {e}"))?;

  let lines = cli_justify::justify(&lines_vec.join("\n"), col);

  cli_text_reader::run_cli_text_reader(lines, col)?;

  Ok(())
}
