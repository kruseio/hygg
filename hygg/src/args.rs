use clap::{Parser, ValueEnum};
use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OcrMode {
  On,
  Off,
}

impl OcrMode {
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
  #[arg(short, long, default_value = "80")]
  pub(crate) col: usize,

  /// Use bundled English OCR for scanned PDF documents
  /// Requires a build with --features pdf-ocr-bundled
  #[arg(
    long,
    value_enum,
    value_name = "on|off",
    num_args = 0..=1,
    require_equals = true,
    default_missing_value = "on"
  )]
  pub(crate) ocr: Option<OcrMode>,

  /// Use the hygg server upload
  #[arg(short, long)]
  pub(crate) upload: Option<String>,

  /// Use the hygg server list
  #[arg(short, long, default_value = "false")]
  pub(crate) list: bool,

  /// Use the hygg server read
  #[arg(short, long)]
  pub(crate) read: Option<String>,

  /// Run interactive tutorial in demo mode for marketing (7 seconds total)
  #[arg(long, default_value = "false")]
  pub(crate) tutorial_demo: bool,

  /// Run demo by ID (e.g., --demo 0)
  #[arg(long, conflicts_with = "tutorial_demo")]
  pub(crate) demo: Option<usize>,

  /// List all available demos
  #[arg(long)]
  pub(crate) list_demos: bool,

  /// List all demo components
  #[arg(long)]
  pub(crate) list_components: bool,

  /// Run custom demo from component list
  #[arg(long)]
  pub(crate) demo_compose: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_explicit_ocr_modes() {
    let on = Args::try_parse_from(["hygg", "--ocr=on", "doc.pdf"])
      .expect("--ocr=on should parse");
    assert_eq!(on.ocr, Some(OcrMode::On));

    let off = Args::try_parse_from(["hygg", "--ocr=off", "doc.pdf"])
      .expect("--ocr=off should parse");
    assert_eq!(off.ocr, Some(OcrMode::Off));
  }

  #[test]
  fn bare_ocr_flag_defaults_to_on_without_eating_file_argument() {
    let args = Args::try_parse_from(["hygg", "--ocr", "doc.pdf"])
      .expect("bare --ocr should parse as on");

    assert_eq!(args.ocr, Some(OcrMode::On));
    assert_eq!(args.file.as_deref(), Some("doc.pdf"));
  }

  #[test]
  fn ocr_mode_is_optional() {
    let args = Args::try_parse_from(["hygg", "doc.pdf"])
      .expect("omitted --ocr should parse");

    assert_eq!(args.ocr, None);
  }
}
