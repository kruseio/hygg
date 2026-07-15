use super::core::Editor;
use crossterm::{
  QueueableCommand,
  cursor::MoveTo,
  execute,
  style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
  },
};
use std::io::Write;

impl Editor {
  // Calculate the height needed for demo hint display
  #[allow(dead_code)]
  pub fn calculate_demo_hint_height(&self) -> usize {
    match &self.demo_hint_text {
      Some(text) if !text.is_empty() => {
        let hint_lines: Vec<&str> = text.split('\n').collect();
        hint_lines.len() + 2 // +2 for top/bottom borders
      }
      _ => 0,
    }
  }
  // Render demo hint if active - minimalistic box with text
  pub fn render_demo_hint(
    &self,
    stdout: &mut std::io::Stdout,
    width: usize,
    height: usize,
  ) -> std::io::Result<()> {
    // Only render if we have hint text
    let hint_text = match &self.demo_hint_text {
      Some(text) if !text.is_empty() => text,
      _ => return Ok(()),
    };

    // Split hint text into lines
    let hint_lines: Vec<&str> = hint_text.split('\n').collect();
    let max_line_len =
      hint_lines.iter().map(|s| s.chars().count()).max().unwrap_or(0);

    // Box dimensions with padding
    let padding = 2;
    let box_width = max_line_len + (padding * 2) + 2; // +2 for borders
    let box_height = hint_lines.len() + 2; // +2 for top/bottom borders

    // Calculate position - centered horizontally, near bottom
    let box_x = (width.saturating_sub(box_width)) / 2;
    let box_y = height.saturating_sub(box_height + 2); // Position near bottom with some margin

    // No clearing - render with background directly to create true overlay

    // Dark background color for the box
    let bg_color = Color::Rgb { r: 20, g: 20, b: 20 };
    let border_color = Color::Rgb { r: 255, g: 191, b: 0 }; // Amber
    let text_color = Color::Rgb { r: 255, g: 191, b: 0 }; // Amber

    // Draw top border
    execute!(
      stdout,
      MoveTo(box_x as u16, box_y as u16),
      SetBackgroundColor(bg_color),
      SetForegroundColor(border_color),
      Print(format!("╭{}╮", "─".repeat(box_width - 2))),
      ResetColor,
    )?;

    // Draw content lines
    for (i, line) in hint_lines.iter().enumerate() {
      let y = box_y + i + 1;
      let line_chars = line.chars().count();
      let left_padding = (max_line_len - line_chars) / 2 + padding;
      let right_padding = box_width - line_chars - left_padding - 2;

      execute!(
        stdout,
        MoveTo(box_x as u16, y as u16),
        SetBackgroundColor(bg_color),
        SetForegroundColor(border_color),
        Print("│"),
        SetForegroundColor(text_color),
        Print(format!(
          "{}{}{}",
          " ".repeat(left_padding),
          line,
          " ".repeat(right_padding)
        )),
        SetForegroundColor(border_color),
        Print("│"),
        ResetColor,
      )?;
    }

    // Draw bottom border
    execute!(
      stdout,
      MoveTo(box_x as u16, (box_y + box_height - 1) as u16),
      SetBackgroundColor(bg_color),
      SetForegroundColor(border_color),
      Print(format!("╰{}╯", "─".repeat(box_width - 2))),
      ResetColor,
    )?;

    Ok(())
  }

  // Buffered version of render_demo_hint
  pub fn render_demo_hint_buffered(
    &self,
    buffer: &mut Vec<u8>,
    width: usize,
    height: usize,
  ) -> std::io::Result<()> {
    // Only render if we have hint text
    let hint_text = match &self.demo_hint_text {
      Some(text) if !text.is_empty() => text,
      _ => return Ok(()),
    };

    // Split hint text into lines
    let hint_lines: Vec<&str> = hint_text.split('\n').collect();
    let max_line_len =
      hint_lines.iter().map(|s| s.chars().count()).max().unwrap_or(0);

    // Box dimensions with padding
    let padding = 2;
    let box_width = max_line_len + (padding * 2) + 2; // +2 for borders
    let box_height = hint_lines.len() + 2; // +2 for top/bottom borders

    // Calculate position - centered horizontally, near bottom
    let box_x = (width.saturating_sub(box_width)) / 2;
    let box_y = height.saturating_sub(box_height + 2); // Position near bottom with some margin

    // No clearing - render with background directly to create true overlay

    // Dark background color for the box
    let bg_color = Color::Rgb { r: 20, g: 20, b: 20 };
    let border_color = Color::Rgb { r: 255, g: 191, b: 0 }; // Amber
    let text_color = Color::Rgb { r: 255, g: 191, b: 0 }; // Amber

    // Draw top border
    buffer.queue(MoveTo(box_x as u16, box_y as u16))?;
    buffer.queue(SetBackgroundColor(bg_color))?;
    buffer.queue(SetForegroundColor(border_color))?;
    write!(buffer, "╭{}╮", "─".repeat(box_width - 2))?;
    buffer.queue(ResetColor)?;

    // Draw content lines
    for (i, line) in hint_lines.iter().enumerate() {
      let y = box_y + i + 1;
      let line_chars = line.chars().count();
      let left_padding = (max_line_len - line_chars) / 2 + padding;
      let right_padding = box_width - line_chars - left_padding - 2;

      buffer.queue(MoveTo(box_x as u16, y as u16))?;
      buffer.queue(SetBackgroundColor(bg_color))?;
      buffer.queue(SetForegroundColor(border_color))?;
      write!(buffer, "│")?;
      buffer.queue(SetForegroundColor(text_color))?;
      write!(
        buffer,
        "{}{}{}",
        " ".repeat(left_padding),
        line,
        " ".repeat(right_padding)
      )?;
      buffer.queue(SetForegroundColor(border_color))?;
      write!(buffer, "│")?;
      buffer.queue(ResetColor)?;
    }

    // Draw bottom border
    buffer.queue(MoveTo(box_x as u16, (box_y + box_height - 1) as u16))?;
    buffer.queue(SetBackgroundColor(bg_color))?;
    buffer.queue(SetForegroundColor(border_color))?;
    write!(buffer, "╰{}╯", "─".repeat(box_width - 2))?;
    buffer.queue(ResetColor)?;

    Ok(())
  }
}
