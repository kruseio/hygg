use hygg_shared::normalize_file_path;
use std::{
  env,
  io::{BufWriter, Cursor},
};

pub fn pdf_to_text(
  pdf_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
  // Use shared path normalization function
  let canonical_path = normalize_file_path(pdf_path)?;
  #[cfg(target_os = "windows")]
  redirect_stderr::redirect_stdout()?;

  #[allow(unused_assignments)]
  let mut original_fd = -1;

  #[allow(unused_assignments)]
  let mut duplicate_fd = -1;

  #[cfg(not(target_os = "windows"))]
  {
    extern crate libc;

    use std::fs::File;
    use std::io::{self, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::io::FromRawFd;

    let stdout = io::stdout();
    original_fd = stdout.as_raw_fd();

    duplicate_fd = unsafe { libc::dup(original_fd) };

    let dev_null = File::open("/dev/null")
      .map_err(|e| format!("Failed to open /dev/null: {e}"))?;
    unsafe {
      libc::dup2(dev_null.as_raw_fd(), original_fd);
    }
  }

  let mut output_buf = Vec::new();
  {
    let mut output_file = BufWriter::new(Cursor::new(&mut output_buf));

    let doc = pdf_extract::Document::load(&canonical_path)?;

    pdf_extract::print_metadata(&doc);

    let mut output = Box::new(pdf_extract::PlainTextOutput::new(
      &mut output_file as &mut dyn std::io::Write,
    ));

    pdf_extract::output_doc(&doc, output.as_mut())?;
  }

  #[cfg(target_os = "windows")]
  redirect_stderr::restore_stdout()?;

  #[cfg(not(target_os = "windows"))]
  {
    extern crate libc;

    use std::fs::File;
    use std::io::{self, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::io::FromRawFd;

    unsafe {
      libc::dup2(duplicate_fd, original_fd);
    }
  }

  // println!("{:?}", output_buf);
  // panic!();

  let res = std::str::from_utf8(&output_buf)
    .map_err(|e| format!("Failed to convert PDF output to UTF-8: {e}"))?
    .to_owned();

  Ok(res)
}
