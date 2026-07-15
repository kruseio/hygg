use crate::interactive_tutorial_buffer::{
  InteractiveTutorialStep, TutorialSuccessCondition,
};
use crate::interactive_tutorial_utils::fetch_github_stars;
use crossterm::style::{Color, ResetColor, SetForegroundColor};

pub(crate) fn basics_steps() -> Vec<InteractiveTutorialStep> {
  let stars_text = fetch_github_stars();

  vec![
    InteractiveTutorialStep {
      title: "hygg - simplifying the way you read".to_string(),
      instructions: vec![
        "".to_string(),
        "Why choose hygg?".to_string(),
        "• 📖 Universal document support - PDF, EPUB, and OCR".to_string(),
        "• ⚡ Lightning-fast navigation with vim keybindings".to_string(),
        "• 🔍 Powerful search, highlights, and bookmarks".to_string(),
        "• 💾 Never lose your place - automatic progress saving".to_string(),
        "• 🛠️ Extensible workflows - execute commands from text".to_string(),
        "• 🔒 Privacy-first - your documents stay on your machine".to_string(),
        "".to_string(),
        format!(
          "{}github.com/kruseio/hygg{} | {}",
          SetForegroundColor(Color::Blue),
          ResetColor,
          stars_text
        ),
        "".to_string(),
        "This interactive tutorial will teach you everything in ~5 minutes."
          .to_string(),
        "".to_string(),
        format!(
          "{}Don't want the tutorial? Type :notutorial to disable it permanently.{}",
          SetForegroundColor(Color::Yellow),
          ResetColor
        ),
        "".to_string(),
        "→ Type :next to begin your journey...".to_string(),
      ],
      practice_text: vec![],
      success_check: TutorialSuccessCondition::NoCondition,
    },
    InteractiveTutorialStep {
      title: "Basic Navigation".to_string(),
      instructions: vec![
        "Let's start with basic cursor movement.".to_string(),
        "".to_string(),
        "The area below is interactive - you can practice here!".to_string(),
        "".to_string(),
        "Movement keys:".to_string(),
        "  • h/← = left    • j/↓ = down    • k/↑ = up    • l/→ = right"
          .to_string(),
        "".to_string(),
        "Advanced movement:".to_string(),
        "  • w/b = forward/backward by word".to_string(),
        "  • {/} = previous/next paragraph".to_string(),
        "  • gg/G = go to start/end of document".to_string(),
        "".to_string(),
        "→ Try it now: Press 'j' or ↓ to move down...".to_string(),
      ],
      practice_text: vec![
        "Welcome to the practice area!".to_string(),
        "You can move around freely here.".to_string(),
        "".to_string(),
        "Try pressing 'j' to move down ↓".to_string(),
        "".to_string(),
        "Your cursor should move to this line!".to_string(),
        "".to_string(),
        "Great job! Keep practicing movement.".to_string(),
      ],
      success_check: TutorialSuccessCondition::KeyPress("j".to_string()),
    },
    InteractiveTutorialStep {
      title: "Visual Selection".to_string(),
      instructions: vec![
        "Let's practice visual selection:".to_string(),
        "".to_string(),
        "1. Press 'v' to enter visual mode".to_string(),
        "2. Move to select text".to_string(),
        "3. Press 'y' to yank (copy) the selection".to_string(),
        "".to_string(),
        "→ Select any word below and yank it...".to_string(),
      ],
      practice_text: vec![
        "Practice selecting text in this paragraph.".to_string(),
        "You can select individual words or entire lines.".to_string(),
        "".to_string(),
        "Try selecting this word: example".to_string(),
        "Or select this entire line with 'V'.".to_string(),
        "".to_string(),
        "Remember: 'v' for character, 'V' for line selection.".to_string(),
      ],
      success_check: TutorialSuccessCondition::YankOperation,
    },
    InteractiveTutorialStep {
      title: "Text Objects - Paragraph Selection".to_string(),
      instructions: vec![
        "Text objects make selection powerful:".to_string(),
        "".to_string(),
        "• v = enter visual mode".to_string(),
        "• ip = inner paragraph (while in visual mode)".to_string(),
        "• :h = highlight selection".to_string(),
        "".to_string(),
        "→ Position cursor in a paragraph below".to_string(),
        "   Press 'v', then 'ip', then ':h' to highlight it...".to_string(),
      ],
      practice_text: vec![
        "This is the first paragraph. It contains".to_string(),
        "multiple lines that form a complete thought.".to_string(),
        "When you use 'vip', it selects all of this.".to_string(),
        "".to_string(),
        "This is the second paragraph. Notice how".to_string(),
        "paragraphs are separated by blank lines.".to_string(),
        "Try 'vip' here to select just this paragraph!".to_string(),
        "".to_string(),
        "And here's a third paragraph for practice.".to_string(),
      ],
      success_check: TutorialSuccessCondition::HighlightCreated,
    },
    InteractiveTutorialStep {
      title: "Search and Navigation".to_string(),
      instructions: vec![
        "Master both search directions:".to_string(),
        "".to_string(),
        "• / = search forward (from cursor down)".to_string(),
        "• ? = search backward (from cursor up)".to_string(),
        "• n/N = next/previous match".to_string(),
        "• * = search word under cursor".to_string(),
        "• :nohl = clear search highlighting".to_string(),
        "".to_string(),
        "→ Try both forward (/) and backward (?) search".to_string(),
        "   Search for 'special' and navigate with n/N".to_string(),
        "   You must use both / and ? to complete this step!".to_string(),
        "   Use :nohl to clear highlighting when done.".to_string(),
      ],
      practice_text: vec![
        "This text contains some special words.".to_string(),
        "When you find the word special, try both searches.".to_string(),
        "".to_string(),
        "The word special appears multiple times.".to_string(),
        "You can navigate between special occurrences.".to_string(),
        "Use n for next special, N for previous special.".to_string(),
        "".to_string(),
        "Try forward search (/) from the top,".to_string(),
        "and backward search (?) from the bottom.".to_string(),
        "This makes finding special text very easy!".to_string(),
      ],
      success_check: TutorialSuccessCondition::SearchBothDirections,
    },
    InteractiveTutorialStep {
      title: "Bookmarks".to_string(),
      instructions: vec![
        "Save positions with bookmarks:".to_string(),
        "".to_string(),
        "• ma = set bookmark 'a'".to_string(),
        "• 'a = jump to bookmark 'a'".to_string(),
        "• '' = return to previous position".to_string(),
        "".to_string(),
        "→ Set bookmark 'a' with 'ma', move away, then jump back with 'a..."
          .to_string(),
      ],
      practice_text: vec![
        "Set a bookmark on this line with 'ma'.".to_string(),
        "Then move somewhere else.".to_string(),
        "".to_string(),
        "You can return with 'a anytime.".to_string(),
        "Bookmarks persist between sessions!".to_string(),
        "".to_string(),
        "Try setting multiple bookmarks: mb, mc, etc.".to_string(),
      ],
      success_check: TutorialSuccessCondition::BookmarkSetAndJumped('a'),
    },
  ]
}
