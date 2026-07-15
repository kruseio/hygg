use std::fs;
use std::path::Path;
#[cfg(feature = "pdf-ocr-bundled")]
use std::path::PathBuf;
use std::process::Command;

pub fn strip_ansi_escapes(input: &str) -> String {
  let mut output = String::with_capacity(input.len());
  let mut chars = input.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\x1b' && chars.peek() == Some(&'[') {
      chars.next();
      for c in chars.by_ref() {
        if c.is_ascii_alphabetic() {
          break;
        }
      }
      continue;
    }
    output.push(ch);
  }
  output
}

pub fn hygg_command(test_name: &str) -> Command {
  let config_home = std::env::temp_dir()
    .join(format!("hygg-config-{}-{test_name}", std::process::id()));
  let mut command = Command::new(env!("CARGO_BIN_EXE_hygg"));
  command.env("XDG_CONFIG_HOME", config_home);
  command
}

#[cfg(feature = "pdf-ocr-bundled")]
pub fn write_native_text_pdf(path: &Path, text: &str) {
  let stream = format!("BT\n/F1 18 Tf\n40 90 Td\n({text}) Tj\nET\n");
  let objects = [
    "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()),
  ];

  let mut pdf = String::from("%PDF-1.4\n");
  let mut offsets = vec![0usize];
  for (index, object) in objects.iter().enumerate() {
    offsets.push(pdf.len());
    pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
  }

  let xref_offset = pdf.len();
  pdf.push_str("xref\n");
  pdf.push_str(&format!("0 {}\n", objects.len() + 1));
  pdf.push_str("0000000000 65535 f \n");
  for offset in offsets.iter().skip(1) {
    pdf.push_str(&format!("{offset:010} 00000 n \n"));
  }
  pdf.push_str(&format!(
    "trailer\n<< /Root 1 0 R /Size {} >>\nstartxref\n{xref_offset}\n%%EOF\n",
    objects.len() + 1
  ));

  fs::write(path, pdf).expect("failed to write test PDF");
}

#[cfg(feature = "pdf-ocr-bundled")]
pub fn prepend_fake_ocrmypdf_to_path(dir: &Path) -> PathBuf {
  let fake_bin = dir.join("bin");
  fs::create_dir(&fake_bin).expect("failed to create fake bin directory");
  let fake_ocrmypdf = fake_bin.join("ocrmypdf");
  let marker = dir.join("ocrmypdf-was-invoked");
  fs::write(
    &fake_ocrmypdf,
    format!("#!/bin/sh\ntouch '{}'\nexit 42\n", marker.display()),
  )
  .expect("failed to write fake ocrmypdf");

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&fake_ocrmypdf, fs::Permissions::from_mode(0o755))
      .expect("failed to make fake ocrmypdf executable");
  }

  fake_bin
}
