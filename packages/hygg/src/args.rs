use clap::{Parser, ValueEnum};
use std::env;

/// Generic `on|off` switch shared by the `--ocr` and `--tts` flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Toggle {
  On,
  Off,
}

impl Toggle {
  pub(crate) fn enabled(self) -> bool {
    matches!(self, Self::On)
  }
}

/// Simplifying the way you read
#[derive(Parser)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    long_about = None,
    help_template = concat!(
        "{before-help}{name} {version}\n",
        "{author-with-newline}{about-with-newline}",
        "Repository: ", env!("CARGO_PKG_REPOSITORY"), "\n",
        "License: ", env!("CARGO_PKG_LICENSE"), "\n\n",
        "{usage-heading} {usage}\n\n",
        "{all-args}{after-help}\n"
    )
)]
pub(crate) struct Args {
  /// Input file to process
  pub(crate) file: Option<String>,

  /// Set the column width
  //
  // Rejected at parse time rather than clamped inside the formatter: a width of
  // zero drives cli_justify::justify into an unbounded loop, and `--col 0` is
  // easy to type. The formatter guards itself too, but a usable error beats a
  // silently-substituted width. The parser keeps the field a `usize`, so no
  // call site changes.
  #[arg(
    short,
    long,
    default_value = "80",
    value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
  )]
  pub(crate) col: usize,

  /// Print the hygg config file path and exit
  #[arg(long)]
  pub(crate) config: bool,

  /// Use English OCR for scanned PDF documents (overrides HYGG_OCR/config).
  /// Only works in a build made with --features ocr; the ONNX models are
  /// downloaded from the ocr-models release and cached on first use.
  #[arg(
    long,
    value_enum,
    value_name = "on|off",
    num_args = 0..=1,
    require_equals = true,
    default_missing_value = "on"
  )]
  pub(crate) ocr: Option<Toggle>,

  /// Enable or disable text-to-speech narration
  #[arg(
    long,
    value_enum,
    value_name = "on|off",
    num_args = 0..=1,
    require_equals = true,
    default_missing_value = "on"
  )]
  pub(crate) tts: Option<Toggle>,

  /// Headless automation: push reading progress (a line offset) for the given
  /// file to the server, without opening the reader. Requires a file argument.
  #[arg(long, value_name = "OFFSET", hide = true)]
  pub(crate) set_progress: Option<usize>,

  /// Run the interactive tutorial in showcase demo mode (7 seconds total)
  #[arg(long, default_value = "false", hide = true)]
  pub(crate) tutorial_demo: bool,

  /// Run demo by ID (e.g., --demo 0)
  #[arg(long, conflicts_with = "tutorial_demo", hide = true)]
  pub(crate) demo: Option<usize>,

  /// List all available demos
  #[arg(long, hide = true)]
  pub(crate) list_demos: bool,

  /// List all demo components
  #[arg(long, hide = true)]
  pub(crate) list_components: bool,

  /// Run custom demo from component list
  #[arg(long, hide = true)]
  pub(crate) demo_compose: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_explicit_ocr_modes() {
    let on = Args::try_parse_from(["hygg", "--ocr=on", "doc.pdf"])
      .expect("--ocr=on should parse");
    assert_eq!(on.ocr, Some(Toggle::On));

    let off = Args::try_parse_from(["hygg", "--ocr=off", "doc.pdf"])
      .expect("--ocr=off should parse");
    assert_eq!(off.ocr, Some(Toggle::Off));
  }

  #[test]
  fn bare_ocr_flag_defaults_to_on_without_eating_file_argument() {
    let args = Args::try_parse_from(["hygg", "--ocr", "doc.pdf"])
      .expect("bare --ocr should parse as on");

    assert_eq!(args.ocr, Some(Toggle::On));
    assert_eq!(args.file.as_deref(), Some("doc.pdf"));
  }

  #[test]
  fn parses_explicit_tts_modes() {
    let on = Args::try_parse_from(["hygg", "--tts=on", "doc.pdf"])
      .expect("--tts=on should parse");
    assert_eq!(on.tts, Some(Toggle::On));

    let off = Args::try_parse_from(["hygg", "--tts=off", "doc.pdf"])
      .expect("--tts=off should parse");
    assert_eq!(off.tts, Some(Toggle::Off));
  }

  #[test]
  fn bare_tts_flag_defaults_to_on_without_eating_file_argument() {
    let args = Args::try_parse_from(["hygg", "--tts", "doc.pdf"])
      .expect("bare --tts should parse as on");

    assert_eq!(args.tts, Some(Toggle::On));
    assert_eq!(args.file.as_deref(), Some("doc.pdf"));
  }

  #[test]
  fn tts_mode_is_optional() {
    let args = Args::try_parse_from(["hygg", "doc.pdf"])
      .expect("omitted --tts should parse");

    assert_eq!(args.tts, None);
  }

  #[test]
  fn ocr_mode_is_optional() {
    let args = Args::try_parse_from(["hygg", "doc.pdf"])
      .expect("omitted --ocr should parse");

    assert_eq!(args.ocr, None);
  }

  #[test]
  fn parses_set_progress_flag() {
    let set =
      Args::try_parse_from(["hygg", "--set-progress", "120", "book.txt"])
        .expect("--set-progress should parse with a file");
    assert_eq!(set.set_progress, Some(120));
    assert_eq!(set.file.as_deref(), Some("book.txt"));
  }

  #[test]
  fn parses_config_path_flag() {
    let args = Args::try_parse_from(["hygg", "--config"])
      .expect("--config should parse");
    assert!(args.config);
    assert_eq!(args.file, None);
  }
}
