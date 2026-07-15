use crossterm::style::{
  Attribute, Color, ResetColor, SetAttribute, SetForegroundColor,
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum TutorialSuccessCondition {
  KeyPress(String),
  VisualSelection { start_line: usize, end_line: usize },
  HighlightCreated,
  BookmarkSet(char),
  BookmarkSetAndJumped(char), // Requires both setting and jumping to bookmark
  CommandExecuted(String),
  CursorPosition { line: usize, col: usize },
  SearchTerm(String),
  SearchTermAndNavigated(String), /* Requires both searching and navigating
                                   * with n/N */
  SearchBothDirections, /* Requires using both forward (/) and backward (?)
                         * search */
  YankOperation,
  CommandExecutedAndYanked(String), /* Requires both command execution and
                                     * yank */
  CommandExecutedYankedAndPasted(String), /* Requires command execution,
                                           * yank, and paste */
  NoCondition, // For informational steps
}

// Tutorial step with practice content
pub struct InteractiveTutorialStep {
  pub title: String,
  pub instructions: Vec<String>,
  pub practice_text: Vec<String>,
  pub success_check: TutorialSuccessCondition,
}

// Create a complete tutorial buffer with both UI and content
pub fn create_tutorial_buffer(
  step: &InteractiveTutorialStep,
  step_num: usize,
  total_steps: usize,
  _terminal_width: usize,
  step_completed: bool,
) -> Vec<String> {
  let mut lines = Vec::new();

  // Adjust step numbering: don't count step 0 (Welcome) and last step (Credits)
  let is_welcome = step_num == 0;
  let is_credits = step_num == total_steps - 1;
  let visible_total_steps = total_steps - 2; // Exclude Welcome and Credits

  // Header with step info
  if is_welcome || is_credits {
    // Don't show step counter for Welcome and Credits screens
    lines.push(format!(
      "{}━━━ {} ━━━{}",
      SetForegroundColor(Color::Blue),
      step.title,
      ResetColor
    ));
  } else {
    // Show adjusted step number (1-7 instead of 1-9)
    let adjusted_step_num = step_num; // Since we skip 0, step 1 = 1, step 2 = 2, etc.
    let progress = adjusted_step_num as f32 / visible_total_steps as f32;
    let bar_width = 20;
    let filled = (progress * bar_width as f32) as usize;
    let progress_bar =
      format!("{}{}", "█".repeat(filled), "░".repeat(bar_width - filled));

    lines.push(format!(
      "{}━━━ {} [{} {}/{}] ━━━{}",
      SetForegroundColor(Color::Blue),
      step.title,
      progress_bar,
      adjusted_step_num,
      visible_total_steps,
      ResetColor
    ));
  }
  lines.push("".to_string());

  // Instructions
  for line in &step.instructions {
    if let Some(stripped) = line.strip_prefix("→") {
      lines.push(
        format!(
          "{}{}{} {}",
          SetForegroundColor(Color::Yellow),
          SetAttribute(Attribute::Bold),
          "→",
          ResetColor
        ) + stripped,
      );
    } else if let Some(stripped) = line.strip_prefix("•") {
      lines.push(
        format!(
          "{}{} {}",
          SetForegroundColor(Color::DarkGrey),
          "•",
          ResetColor
        ) + stripped,
      );
    } else {
      lines.push(line.clone());
    }
  }

  // Separator - simple empty line instead of border
  lines.push("".to_string());

  // Practice content without borders
  lines.extend(step.practice_text.clone());

  // Footer - just empty line for spacing
  lines.push("".to_string());

  // Navigation help - single line to save space
  if is_welcome {
    // Welcome screen - always show :next to continue
    lines.push(format!(
      "{} Type :next to begin the tutorial {}",
      SetForegroundColor(Color::Green),
      ResetColor
    ));
  } else if is_credits {
    // Credits screen - show return message
    lines.push(format!(
      "{}{} Type :next to return to your document {}",
      SetForegroundColor(Color::Magenta),
      SetAttribute(Attribute::Bold),
      ResetColor
    ));
  } else if step_num == total_steps - 2 {
    // Congratulations step (step before credits) - always show :next
    lines.push(format!(
      "{}{} ✓ Type :next to continue {}",
      SetForegroundColor(Color::Green),
      SetAttribute(Attribute::Bold),
      ResetColor
    ));
  } else {
    // Regular tutorial steps
    if step_completed {
      lines.push(format!(
        "{}{} ✓ Great job! Type :next to continue {}",
        SetForegroundColor(Color::Green),
        SetAttribute(Attribute::Bold),
        ResetColor
      ));
    } else {
      lines.push(format!(
        "{} Complete the task above | :back to go back | :q to exit {}",
        SetForegroundColor(Color::DarkGrey),
        ResetColor
      ));
    }
  }

  lines
}
