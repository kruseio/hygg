use std::{
  env,
  io::{self, BufRead},
};

use cli_justify::justify;
use getopts::Options;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args: Vec<String> = env::args().collect();
  let mut opts = Options::new();

  opts.optopt("c", "col", "set the column, defaults to 110", "NUMBER");
  opts.optflag("h", "help", "print this help menu");

  let matches = opts.parse(&args[1..])?;

  if matches.opt_present("h") {
    let brief = format!("Usage: {} FILE [options]", args[0]);
    print!("{}", opts.usage(&brief));
    return Ok(());
  }

  let col: usize = match matches.opt_str("c") {
    Some(x) => x.parse().unwrap_or(110),
    None => 110,
  };

  let lines_vec: Vec<String> =
    io::stdin().lock().lines().map_while(Result::ok).collect();
  let lines = justify(&lines_vec.join("\n"), col).join("\n");

  println!("{lines}");

  Ok(())
}
