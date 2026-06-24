use crate::interactive_tutorial_buffer::{
  InteractiveTutorialStep, TutorialSuccessCondition,
};
use crossterm::style::{
  Attribute, Color, ResetColor, SetAttribute, SetForegroundColor,
};

pub(crate) fn advanced_steps() -> Vec<InteractiveTutorialStep> {
  vec![
    InteractiveTutorialStep {
      title: "Command Execution".to_string(),
      instructions: vec![
        "Run shell commands and use their output:".to_string(),
        "".to_string(),
        "• :!command = execute shell command (opens a split)".to_string(),
        "• Command output appears in bottom split".to_string(),
        "• Move cursor with j/k to navigate output".to_string(),
        "• yy = yank (copy) output line".to_string(),
        "• Ctrl+V or Ctrl+R 0 = paste yanked text in command mode".to_string(),
        "• :q = close split and return to tutorial".to_string(),
        "".to_string(),
        "→ Try :!echo hello (or any echo command)".to_string(),
        "   Then practice the full workflow:".to_string(),
        "   yank output → close → paste in new command".to_string(),
      ],
      practice_text: vec![
        "Execute commands from within hygg!".to_string(),
        "".to_string(),
        "Full workflow practice:".to_string(),
        "".to_string(),
        "Step 1: Type :!echo hello".to_string(),
        "        A split will open with the output".to_string(),
        "".to_string(),
        "Step 2: In the split, yank the output:".to_string(),
        "        - Navigate to 'hello' with j/k".to_string(),
        "        - Press yy to yank the line".to_string(),
        "        - Type :q to close the split".to_string(),
        "".to_string(),
        "Step 3: Create a new command using the output:".to_string(),
        "        - Type :!echo<space> (but don't press Enter yet) ".to_string(),
        "        - Press Ctrl+V or Ctrl+R 0 to paste".to_string(),
        "        - Press Enter to execute".to_string(),
        "".to_string(),
        "This powerful workflow lets you chain commands!".to_string(),
      ],
      success_check: TutorialSuccessCondition::CommandExecutedYankedAndPasted(
        "!echo".to_string(),
      ),
    },
    InteractiveTutorialStep {
      title: "Congratulations! 🎉".to_string(),
      instructions: vec![
        "You've mastered the essentials of hygg!".to_string(),
        "".to_string(),
        "You've learned:".to_string(),
        "• ✓ Navigation and movement".to_string(),
        "• ✓ Visual selection and text objects".to_string(),
        "• ✓ Search and bookmarks".to_string(),
        "• ✓ Command execution".to_string(),
        "".to_string(),
        "Resources:".to_string(),
        format!(
          "• Type {}:help{} to see all available commands",
          SetForegroundColor(Color::Yellow),
          ResetColor
        ),
        format!(
          "• Documentation: {}github.com/kruseio/hygg{}",
          SetForegroundColor(Color::Blue),
          ResetColor
        ),
        "".to_string(),
      ],
      practice_text: vec![
        "Quick Reference:".to_string(),
        "".to_string(),
        "Essential commands:".to_string(),
        "  :help     - Show all commands".to_string(),
        "  :tutorial - Restart this tutorial".to_string(),
        "  :q        - Quit".to_string(),
        "".to_string(),
        "Navigation:".to_string(),
        "  j/k       - Down/up".to_string(),
        "  Ctrl+d/Ctrl+u - Page down/up".to_string(),
        "  gg/G      - Start/end of document".to_string(),
        "".to_string(),
        "Search:".to_string(),
        "  /pattern  - Search forward".to_string(),
        "  ?pattern  - Search backward".to_string(),
        "  n/N       - Next/previous match".to_string(),
      ],
      success_check: TutorialSuccessCondition::NoCondition,
    },
    InteractiveTutorialStep {
      title: "Credits".to_string(),
      instructions: vec![
        format!(
          "{}About the Author{}",
          SetAttribute(Attribute::Bold),
          ResetColor
        ),
        "".to_string(),
        "hygg is created and maintained by Ragnar Kruse (kruseio),".to_string(),
        "a passionate open-source developer dedicated to building".to_string(),
        "high-quality tools that enhance productivity and make".to_string(),
        "computing more enjoyable.".to_string(),
        "".to_string(),
        format!(
          "{}Connect & Follow:{}",
          SetAttribute(Attribute::Bold),
          ResetColor
        ),
        format!(
          "• GitHub: {}github.com/kruseio{}",
          SetForegroundColor(Color::Blue),
          ResetColor
        ),
        "".to_string(),
        format!(
          "{}Support the Project:{}",
          SetAttribute(Attribute::Bold),
          ResetColor
        ),
        "If hygg has improved your reading experience, consider".to_string(),
        "supporting its continued development. Your contribution".to_string(),
        "helps maintain and improve hygg for everyone!".to_string(),
        "".to_string(),
        "→ Type :next to finish and return to your document...".to_string(),
      ],
      practice_text: vec![
        "┌──────────────────────────────────────────┐".to_string(),
        "│             Support Open Source          │".to_string(),
        "├──────────────────────────────────────────┤".to_string(),
        "│                                          │".to_string(),
        "│  Your support directly enables:          │".to_string(),
        "│  • New features & improvements           │".to_string(),
        "│  • Better platform compatibility         │".to_string(),
        "│  • Faster bug fixes & updates            │".to_string(),
        "│  • More comprehensive documentation      │".to_string(),
        "│  • A sustainable open-source project     │".to_string(),
        "│                                          │".to_string(),
        "└──────────────────────────────────────────┘".to_string(),
      ],
      success_check: TutorialSuccessCondition::NoCondition,
    },
  ]
}
