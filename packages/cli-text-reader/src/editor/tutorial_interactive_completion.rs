use super::core::Editor;
use crate::interactive_tutorial_buffer::TutorialSuccessCondition;

impl Editor {
  // Check if current tutorial step is completed
  pub fn check_tutorial_completion(&mut self) -> bool {
    if let Some(ref check) = self.current_tutorial_condition {
      match check {
        TutorialSuccessCondition::NoCondition => false, /* Don't auto-advance on informational steps */
        TutorialSuccessCondition::KeyPress(_key) => {
          // This is handled in key event processing
          false
        }
        TutorialSuccessCondition::CursorPosition { line, col } => {
          self.cursor_y == *line && self.cursor_x == *col
        }
        TutorialSuccessCondition::VisualSelection { start_line, end_line } => {
          if let (Some(start), Some(end)) =
            (self.editor_state.selection_start, self.editor_state.selection_end)
          {
            let start_y = self.offset + start.0;
            let end_y = self.offset + end.0;
            start_y == *start_line && end_y == *end_line
          } else {
            false
          }
        }
        TutorialSuccessCondition::HighlightCreated => {
          self.tutorial_highlight_created
        }
        TutorialSuccessCondition::BookmarkSet(mark) => {
          self.marks.contains_key(mark)
        }
        TutorialSuccessCondition::BookmarkSetAndJumped(mark) => {
          let result =
            self.marks.contains_key(mark) && self.tutorial_bookmark_jumped;
          if result {
            self.debug_log(&format!(
              "Bookmark {mark} set and jumped, completing step"
            ));
          }
          result
        }
        TutorialSuccessCondition::SearchTerm(term) => {
          self.editor_state.search_query == *term
        }
        TutorialSuccessCondition::SearchTermAndNavigated(term) => {
          self.editor_state.search_query == *term
            && self.tutorial_search_navigated
        }
        TutorialSuccessCondition::YankOperation => self.tutorial_yank_performed,
        TutorialSuccessCondition::SearchBothDirections => {
          // Check if both forward and backward search have been used
          self.tutorial_forward_search_used
            && self.tutorial_backward_search_used
            && self.tutorial_search_navigated
        }
        TutorialSuccessCondition::CommandExecuted(cmd) => {
          self.debug_log(&format!(
            "Checking CommandExecuted: expected='{}', actual={:?}",
            cmd, self.last_executed_command
          ));
          if let Some(ref executed) = self.last_executed_command {
            // Check if the executed command starts with the expected command
            executed.starts_with(cmd)
          } else {
            false
          }
        }
        TutorialSuccessCondition::CommandExecutedAndYanked(cmd) => {
          self.debug_log(&format!("Checking CommandExecutedAndYanked: cmd='{}', executed={:?}, yanked={}", 
            cmd, self.last_executed_command, self.tutorial_yank_performed));
          if let Some(ref executed) = self.last_executed_command {
            // Check both conditions: command executed AND yank performed
            executed.starts_with(cmd) && self.tutorial_yank_performed
          } else {
            false
          }
        }
        TutorialSuccessCondition::CommandExecutedYankedAndPasted(cmd) => {
          self.debug_log(&format!("Checking CommandExecutedYankedAndPasted: cmd='{}', executed={:?}, yanked={}, pasted={}", 
            cmd, self.last_executed_command, self.tutorial_yank_performed, self.tutorial_paste_performed));
          if let Some(ref executed) = self.last_executed_command {
            // Check all three conditions: command executed AND yank performed
            // AND paste performed
            executed.starts_with(cmd)
              && self.tutorial_yank_performed
              && self.tutorial_paste_performed
          } else {
            false
          }
        }
      }
    } else {
      false
    }
  }
}
