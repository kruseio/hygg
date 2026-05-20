use pdf_extract::{ConvertToFmt, OutputDev, OutputError, Transform};

pub struct LayoutTextOutput<W: ConvertToFmt> {
  writer: W::Writer,
  flip_ctm: Transform,
  media_left_x: f64,
  media_right_x: f64,
  doc_left_x: f64,
  have_doc_left_x: bool,
  page_left_x: f64,
  have_page_left_x: bool,
  last_end: f64,
  last_x: f64,
  last_y: f64,
  have_char: bool,
  at_word_start: bool,
  skip_current_word: bool,
  pending_newline: bool,
}

impl<W: ConvertToFmt> LayoutTextOutput<W> {
  pub fn new(writer: W) -> Self {
    Self {
      writer: writer.convert(),
      flip_ctm: Transform::identity(),
      media_left_x: 0.0,
      media_right_x: 0.0,
      doc_left_x: 0.0,
      have_doc_left_x: false,
      page_left_x: 0.0,
      have_page_left_x: false,
      last_end: 0.0,
      last_x: 0.0,
      last_y: 0.0,
      have_char: false,
      at_word_start: false,
      skip_current_word: false,
      pending_newline: false,
    }
  }

  fn transformed_font_size(trm: &Transform, font_size: f64) -> f64 {
    let x_vec = (trm.m11 + trm.m21) * font_size;
    let y_vec = (trm.m12 + trm.m22) * font_size;
    let v = (x_vec * y_vec).abs().sqrt();
    if v.is_finite() && v > 0.000_001 { v } else { font_size.max(0.000_001) }
  }

  fn space_width(font_size: f64) -> f64 {
    font_size * 0.35
  }

  fn write_n_spaces(
    writer: &mut impl std::fmt::Write,
    mut count: usize,
  ) -> Result<(), OutputError> {
    use std::fmt::Write;
    const MAX_SPACES: usize = 200;
    if count > MAX_SPACES {
      count = MAX_SPACES;
    }
    if count == 0 {
      return Ok(());
    }
    let s = " ".repeat(count);
    write!(writer, "{s}")?;
    Ok(())
  }

  fn is_mostly_vertical(trm: &Transform) -> bool {
    let horizontal = trm.m11.abs();
    let vertical = trm.m12.abs();
    vertical > (horizontal * 1.4) && vertical > 0.000_001
  }

  fn near_right_margin_x(&self, x: f64) -> bool {
    let width = (self.media_right_x - self.media_left_x).abs();
    if width <= 0.0 {
      return false;
    }
    let margin = (width * 0.12).clamp(24.0, 96.0);
    x >= self.media_right_x - margin
  }

  fn should_skip_margin_word(
    &self,
    trm: &Transform,
    x: f64,
    y: f64,
    fs: f64,
    ch: &str,
  ) -> bool {
    if !self.near_right_margin_x(x) {
      return false;
    }
    let is_alpha = ch.chars().all(char::is_alphabetic);
    if !is_alpha {
      return false;
    }
    if Self::is_mostly_vertical(trm) {
      return true;
    }
    if !self.have_char {
      return true;
    }
    let dx = (x - self.last_x).abs();
    let dy = (y - self.last_y).abs();
    dx <= fs * 0.35 && dy >= fs * 0.45
  }
}

impl<W: ConvertToFmt> OutputDev for LayoutTextOutput<W> {
  fn begin_page(
    &mut self,
    _page_num: u32,
    media_box: &pdf_extract::MediaBox,
    _art_box: Option<(f64, f64, f64, f64)>,
  ) -> Result<(), OutputError> {
    self.flip_ctm = Transform::row_major(
      1.0,
      0.0,
      0.0,
      -1.0,
      0.0,
      media_box.ury - media_box.lly,
    );
    self.media_left_x = media_box.llx;
    self.media_right_x = media_box.urx;
    if self.have_doc_left_x {
      self.page_left_x = self.doc_left_x;
      self.have_page_left_x = true;
    } else {
      self.have_page_left_x = false;
      self.page_left_x = 0.0;
    }
    self.last_end = 0.0;
    self.last_x = 0.0;
    self.last_y = 0.0;
    self.have_char = false;
    self.at_word_start = false;
    self.skip_current_word = false;
    self.pending_newline = false;
    Ok(())
  }

  fn end_page(&mut self) -> Result<(), OutputError> {
    use std::fmt::Write;
    if self.have_char {
      write!(self.writer, "\n")?;
      write!(self.writer, "\n")?;
    }
    Ok(())
  }

  fn output_character(
    &mut self,
    trm: &Transform,
    width: f64,
    _spacing: f64,
    font_size: f64,
    ch: &str,
  ) -> Result<(), OutputError> {
    use std::fmt::Write;

    let position = trm.post_transform(&self.flip_ctm);
    let (x, y) = (position.m31, position.m32);
    let fs = Self::transformed_font_size(trm, font_size);
    let sw = Self::space_width(fs);

    if self.at_word_start {
      self.skip_current_word = self.should_skip_margin_word(trm, x, y, fs, ch);
      if self.skip_current_word {
        self.at_word_start = false;
        return Ok(());
      }

      if !self.have_page_left_x {
        self.page_left_x = x;
        self.have_page_left_x = true;
      } else if x < self.page_left_x {
        self.page_left_x = x;
      }
      if !self.have_doc_left_x {
        self.doc_left_x = x;
        self.have_doc_left_x = true;
      } else if x < self.doc_left_x {
        self.doc_left_x = x;
      }

      if self.have_char {
        let dy = (y - self.last_y).abs();
        let moved_left = x < self.last_x - (fs * 0.25);
        if dy > fs * 0.90 || (moved_left && dy > fs * 0.25) {
          self.pending_newline = true;
        }
      }

      if self.pending_newline {
        write!(self.writer, "\n")?;
        self.last_end = 0.0;
        self.pending_newline = false;

        if self.have_page_left_x {
          let indent = (x - self.page_left_x).max(0.0);
          let spaces = (indent / sw).round() as usize;
          Self::write_n_spaces(&mut self.writer, spaces)?;
        }
      } else if self.have_char {
        let gap = x - self.last_end;
        if gap > sw * 0.75 {
          let spaces = (gap / sw).round().max(1.0) as usize;
          Self::write_n_spaces(&mut self.writer, spaces)?;
        }
      } else if self.have_page_left_x {
        let indent = (x - self.page_left_x).max(0.0);
        if indent > sw * 0.5 {
          let spaces = (indent / sw).round() as usize;
          Self::write_n_spaces(&mut self.writer, spaces)?;
        }
      }
    }

    if self.skip_current_word {
      return Ok(());
    }

    write!(self.writer, "{ch}")?;

    self.have_char = true;
    self.at_word_start = false;
    self.last_x = x;
    self.last_y = y;
    self.last_end = x + (width * fs);
    Ok(())
  }

  fn begin_word(&mut self) -> Result<(), OutputError> {
    self.at_word_start = true;
    self.skip_current_word = false;
    Ok(())
  }

  fn end_word(&mut self) -> Result<(), OutputError> {
    self.skip_current_word = false;
    Ok(())
  }

  fn end_line(&mut self) -> Result<(), OutputError> {
    Ok(())
  }
}
