use clap::Parser;
use std::env;

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

  /// Use OCR to extract text from scanned PDF documents
  /// Depends on ocrmypdf and tesseract-ocr lang e.g.
  /// sudo apt install ocrmypdf tesseract-ocr-eng
  #[arg(short, long, default_value = "false")]
  pub(crate) ocr: bool,

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
