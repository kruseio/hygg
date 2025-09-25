use clap::Parser;
use std::env;
use std::io::{self, Read};

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
struct Args {
  /// Input file to process
  file: Option<String>,

  /// Set the column width
  #[arg(short, long, default_value = "80")]
  col: usize,

  /// Use OCR to extract text from scanned PDF documents
  /// Depends on ocrmypdf and tesseract-ocr lang e.g.
  /// sudo apt install ocrmypdf tesseract-ocr-eng
  #[arg(short, long, default_value = "false")]
  ocr: bool,

  /// Use the hygg server upload
  #[arg(short, long)]
  upload: Option<String>,

  /// Use the hygg server list
  #[arg(short, long, default_value = "false")]
  list: bool,

  /// Use the hygg server read
  #[arg(short, long)]
  read: Option<String>,

  /// Run interactive tutorial in demo mode for marketing (7 seconds total)
  #[arg(long, default_value = "false")]
  tutorial_demo: bool,

  /// Run demo by ID (e.g., --demo 0)
  #[arg(long, conflicts_with = "tutorial_demo")]
  demo: Option<usize>,

  /// List all available demos
  #[arg(long)]
  list_demos: bool,

  /// List all demo components
  #[arg(long)]
  list_components: bool,

  /// Run custom demo from component list
  #[arg(long)]
  demo_compose: Option<String>,
}

pub fn which(binary: &str) -> Option<std::path::PathBuf> {
  if binary.is_empty() || binary.contains('\0') {
    return None;
  }

  let extensions = if cfg!(windows) {
    vec!["", ".exe", ".com", ".bat", ".cmd"]
  } else {
    vec![""]
  };

  let paths = env::var("PATH").ok()?;

  for path in env::split_paths(&paths) {
    if !path.exists() || !path.is_dir() {
      continue;
    }

    for &ext in &extensions {
      let binary_with_ext = format!("{}{}", binary, ext);
      let full_path = path.join(&binary_with_ext);

      if full_path.is_file() {
        if let Ok(canonical) = full_path.canonicalize() {
          return Some(canonical);
        }
      }
    }
  }

  if cfg!(windows) {
    if let Ok(current_dir) = env::current_dir() {
      for &ext in &extensions {
        let binary_with_ext = format!("{}{}", binary, ext);
        let current_dir_path = current_dir.join(&binary_with_ext);

        if current_dir_path.is_file() {
          if let Ok(canonical) = current_dir_path.canonicalize() {
            return Some(canonical);
          }
        }
      }
    }
  }

  None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();

  // Check if stdin has content
  let stdin_content = if atty::is(atty::Stream::Stdin) {
    None
  } else {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
      Ok(_) => {
        if buffer.is_empty() {
          None
        } else {
          Some(buffer)
        }
      }
      Err(_) => None,
    }
  };

  // Only redirect stderr after we've validated the file
  // This ensures error messages are visible to users

  // Handle server operations (commented out as in original)
  // if let Some(server_upload) = &args.upload {
  //   // TODO function to upload a local file to the server here
  //   return Ok(());
  // }

  // if args.list {
  //   // TODO function to list all file hashes, progress and original file
  // names here   return Ok(());
  // }

  // if let Some(server_read) = &args.read {
  //   // TODO function to download file with the hash of `server_read`
  //   // and store locally with the hash as name here
  //   return Ok(());
  // }

  // Handle list demos
  if args.list_demos {
    use cli_text_reader::demo_registry::list_all_demos;
    println!("Available demos:");
    for (id, name, description) in list_all_demos() {
      println!("  {id} - {name} : {description}");
    }
    return Ok(());
  }

  // Handle list components
  if args.list_components {
    use cli_text_reader::demo_components::list_all_components;
    println!("Available demo components:");
    for component in list_all_components() {
      println!(
        "  {} - {} : {}",
        component.id, component.name, component.description
      );
    }
    return Ok(());
  }

  // Handle demo compose
  if let Some(component_list) = args.demo_compose {
    // For custom composed demos, we'll use the existing demo infrastructure
    // Since we can't dynamically register demos, we'll print a message
    println!(
      "Demo composition from command line is not yet fully implemented."
    );
    println!("Components requested: {component_list}");
    println!("Please use predefined demos with --demo <ID>");
    return Ok(());
  }

  // Handle specific demo ID
  if let Some(demo_id) = args.demo {
    cli_text_reader::run_cli_text_reader_with_demo_id(
      vec![],
      args.col,
      demo_id,
    )?;
    return Ok(());
  }

  // For tutorial demo mode (backward compatibility)
  if args.tutorial_demo {
    // Run demo with empty content - the demo will load its own content
    cli_text_reader::run_cli_text_reader_with_demo(vec![], args.col, true)?;
    return Ok(());
  }

  // Get the file to process - either from args or from command line
  let file = if let Some(file) = args.file {
    Some(file)
  } else if stdin_content.is_none() {
    // Only check for extra arguments if there's no stdin content
    // If no file provided via clap, check if there's an extra argument
    // (for backward compatibility with direct file paths)
    let args_vec: Vec<String> = std::env::args().collect();
    // If we only have the program name (1 arg), no file was provided
    if args_vec.len() <= 1 {
      None
    } else {
      // Get the last argument that isn't the program name
      args_vec.last().cloned()
    }
  } else {
    None
  };

  // If stdin has content, use it directly
  let (lines, temp_file, raw_content) = if let Some(content) = stdin_content {
    let lines = cli_justify::justify(&content, args.col);
    (lines, None, Some(content))
  } else if let Some(file) = file {
    let temp_file = format!("{file}-{}", uuid::Uuid::new_v4());

    let content = if (args.ocr && which("ocrmypdf").is_some()) {
      // Validate file path to prevent command injection
      if let Err(e) = validate_file_path(&file) {
        eprintln!("Error: Invalid file path: {e}");
        std::process::exit(1);
      }

      // Additional validation for temp file path
      if temp_file.contains("..")
        || temp_file.contains(";")
        || temp_file.contains("|")
        || temp_file.contains("&")
      {
        eprintln!("Error: Invalid temporary file path");
        std::process::exit(1);
      }

      // Use Command with explicit arguments to prevent shell injection
      let mut cmd = std::process::Command::new("ocrmypdf");
      cmd
        .arg("--force-ocr")
        .arg("--") // End of options marker
        .arg(&file)
        .arg(&temp_file)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

      let output = cmd.output().map_err(|e| e.to_string())?;

      if !output.status.success() {
        eprintln!("OCR processing failed");
        std::process::exit(1);
      }

      cli_pdf_to_text::pdf_to_text(&temp_file)?
    } else {
      // Check file extension first for better format routing
      let extension = std::path::Path::new(&file)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase());

      match extension.as_deref() {
        Some("epub") => match cli_epub_to_text::epub_to_text(&file) {
          Ok(content) => content,
          Err(e) => {
            eprintln!("Error:\nUnable to read EPUB file '{file}'\n");
            eprintln!("Details:\n{e}\n");
            std::process::exit(1);
          }
        },
        Some("pdf") => match cli_pdf_to_text::pdf_to_text(&file) {
          Ok(content) => content,
          Err(e) => {
            eprintln!("Error:\nUnable to read PDF file '{file}'\n");
            eprintln!("Details:\n{e}\n");
            std::process::exit(1);
          }
        },
        _ => {
          // For other formats, try pandoc first, then fall back to other
          // converters
          match pandoc_to_text(&file)
            .or_else(|_| cli_epub_to_text::epub_to_text(&file))
            .or_else(|_| cli_pdf_to_text::pdf_to_text(&file))
          {
            Ok(content) => content,
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
      }
    };

    let lines = cli_justify::justify(&content, args.col);

    // Check if we have any content to display
    if lines.is_empty() || (lines.len() == 1 && lines[0].trim().is_empty()) {
      eprintln!("Error: No readable content found in file '{file}'");
      eprintln!(
        "The file may be empty, corrupted, or in an unsupported format."
      );
      std::process::exit(1);
    }

    (lines, Some(temp_file), Some(content))
  } else {
    // No file provided - start with empty content
    // Users can access tutorial with :tutorial command
    (vec![], None, None)
  };

  // Now redirect stderr after file validation is complete
  if let Err(e) = redirect_stderr::redirect_stderr() {
    eprintln!("Warning: Failed to redirect stderr: {e}");
    // Continue execution - this is not critical for main functionality
  }

  // Pass raw content for consistent hashing across different column widths
  if let Some(content) = raw_content {
    cli_text_reader::run_cli_text_reader_with_content(
      lines,
      args.col,
      Some(content),
      false,
    )?;
  } else {
    cli_text_reader::run_cli_text_reader(lines, args.col)?;
  }

  if let Some(temp_file) = temp_file
    && std::path::Path::new(&temp_file).exists()
  {
    std::fs::remove_file(&temp_file)?;
  }

  Ok(())
}

// Validate file path to prevent command injection
fn validate_file_path(file_path: &str) -> Result<(), String> {
  // Check for dangerous characters that could be used for command injection
  let dangerous_chars =
    ['|', '&', ';', '`', '$', '(', ')', '<', '>', '\\', '\n', '\r'];

  if file_path.chars().any(|c| dangerous_chars.contains(&c)) {
    return Err("File path contains dangerous characters".to_string());
  }

  // Check for path traversal attempts
  if file_path.contains("..") {
    return Err("Path traversal not allowed".to_string());
  }

  // Check for null bytes
  if file_path.contains('\0') {
    return Err("Null bytes not allowed in file path".to_string());
  }

  // Ensure the file exists and is a regular file
  let path = std::path::Path::new(file_path);
  if !path.exists() {
    return Err("File does not exist".to_string());
  }

  if !path.is_file() {
    return Err("Path is not a regular file".to_string());
  }

  Ok(())
}

// Convert document to text using pandoc
fn pandoc_to_text(
  file_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  // Check if pandoc is available
  if which("pandoc").is_none() {
    return Err(
      "pandoc not found. Install with:\nsudo apt install pandoc\n# scoop install pandoc\n# brew install pandoc".into(),
    );
  }

  // Validate file path
  validate_file_path(file_path)?;

  // Run pandoc with plain text output
  let mut cmd = std::process::Command::new("pandoc");
  cmd
    .arg("--to=plain")
    .arg("--wrap=none")
    .arg("--")
    .arg(file_path)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());

  let output = cmd.output()?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!("pandoc failed: {stderr}").into());
  }

  Ok(String::from_utf8(output.stdout)?)
}
