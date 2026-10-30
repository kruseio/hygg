// Property test for issue #99 and its whole class: random documents in many
// scripts, random key sequences, a redraw after every key. The reader has to
// survive all of it, the selection columns have to stay on character
// boundaries of the line they point into, and the cursor on the start of a
// grapheme cluster (one visible character), at the display column j/k aimed
// for. Every hand-written test
// before #99 used ASCII lines, which is the one input where byte offsets and
// character indices agree, so the bug never had a chance to show.
use super::core::{Editor, EditorMode};
use crate::editor::char_boundary::{ByteCol, CursorCol, DisplayCol, Pos};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use proptest::prelude::*;

// Graphemes from a broad spread of scripts, chosen for the ways they stress a
// byte-offset editor: 2-, 3- and 4-byte UTF-8, wide (two-cell) CJK, combining
// marks that are separate chars but one grapheme, scripts without spaces, RTL
// text and bidi controls, ZWJ emoji, zero-width characters, and case mappings
// that change byte length when lowercased (search lowercases).
fn grapheme() -> impl Strategy<Value = &'static str> {
  prop::sample::select(vec![
    // ASCII, including the punctuation the motions and text objects key on
    "a",
    "Z",
    "0",
    "_",
    ".",
    ",",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "\"",
    "'",
    // Latin with diacritics: Spanish, French, German, Polish, Czech, Turkish,
    // Icelandic, Vietnamese (stacked)
    "á",
    "é",
    "í",
    "ó",
    "ú",
    "ñ",
    "¿",
    "¡",
    "ç",
    "œ",
    "ß",
    "ü",
    "ł",
    "ż",
    "ř",
    "ě",
    "İ",
    "ı",
    "ğ",
    "þ",
    "ð",
    "ệ",
    "ở",
    // Lowercasing changes byte length
    "Ⱥ",
    "ẞ",
    // Combining sequences: one grapheme, two or three chars
    "e\u{301}",
    "n\u{303}",
    "a\u{308}\u{304}",
    // Greek, Cyrillic, Armenian, Georgian
    "λ",
    "Ω",
    "ж",
    "Ї",
    "ա",
    "ქ",
    // Hebrew, Arabic, Persian (RTL)
    "ש",
    "ל",
    "ع",
    "ب",
    "ی",
    // Indic and Southeast Asian with combining vowel signs
    "क",
    "ि",
    "ক্ষ",
    "த",
    "ไ",
    "ที่",
    // Ethiopic
    "ሰ",
    // CJK, kana, Hangul, fullwidth
    "中",
    "文",
    "語",
    "ア",
    "ひ",
    "한",
    "Ａ",
    // 4-byte: emoji, ZWJ sequence, flag, supplementary CJK
    "😀",
    "👩\u{200d}💻",
    "🇪🇸",
    "𠜎",
    // Invisible: zero-width space, ZWJ, bidi override and isolates, NBSP
    "\u{200b}",
    "\u{200d}",
    "\u{202e}",
    "\u{2066}",
    "\u{2069}",
    "\u{a0}",
    // Combining sequences and bare combining marks or modifiers. A bare mark
    // attaches to whatever precedes it, so a random neighbour decides which
    // grapheme it belongs to
    "c\u{327}",
    "\u{301}",
    "\u{e48}",
    "\u{fe0f}",
    "\u{1f3fd}",
    // Indic and Thai clusters: vowel signs, viramas, tone marks
    "कि",
    "नमस्ते",
    "दुनिया",
    "นี่",
    "สวัสดี",
    // More two-cell CJK and fullwidth
    "漢",
    "字",
    "Ｂ",
    "ｃ",
    // Skin-tone emoji, and lone regional indicators that pair up with a random
    // neighbour into a flag
    "👍\u{1f3fd}",
    "🇪",
    "🇸",
  ])
}

fn line() -> impl Strategy<Value = String> {
  prop::collection::vec(prop_oneof![4 => grapheme(), 1 => Just(" ")], 0..24)
    .prop_map(|parts| parts.concat())
}

fn document() -> impl Strategy<Value = Vec<String>> {
  prop::collection::vec(line(), 1..6)
}

fn key(code: KeyCode) -> KeyEvent {
  KeyEvent::new(code, KeyModifiers::NONE)
}

// Keys that move, select, search or redraw. Excluded: keys whose handlers
// block on `event::read()` for a second key (f F t T g m i a ' `), keys that
// reach outside the process (y copies to the clipboard and writes OSC 52, :
// runs commands, q quits).
fn key_event() -> impl Strategy<Value = KeyEvent> {
  prop_oneof![
    prop::sample::select(vec![
      'h', 'j', 'k', 'l', 'w', 'W', 'b', 'B', 'e', 'E', '0', '$', '^', '%',
      'v', 'V', 'n', 'N', '*', '#', 'G', 'H', 'M', 'L', '/', '?',
    ])
    .prop_map(|c| key(KeyCode::Char(c))),
    prop::sample::select(vec![
      KeyCode::Left,
      KeyCode::Right,
      KeyCode::Up,
      KeyCode::Down,
      KeyCode::Esc,
      KeyCode::Enter,
      KeyCode::Backspace,
    ])
    .prop_map(key),
    // Typed into a search query when search mode is active
    grapheme().prop_filter_map("single char", |g| {
      let mut chars = g.chars();
      match (chars.next(), chars.next()) {
        (Some(c), None) => Some(key(KeyCode::Char(c))),
        _ => None,
      }
    }),
    prop::sample::select(vec!['d', 'u'])
      .prop_map(|c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
  ]
}

// The column must be a usable byte offset into its line: on a boundary and not
// past the end.
fn assert_on_boundary(
  editor: &Editor,
  line_idx: usize,
  col: usize,
  what: &str,
) -> Result<(), TestCaseError> {
  if let Some(line) = editor.lines.get(line_idx) {
    prop_assert!(
      col <= line.len() && line.is_char_boundary(col),
      "{what} column {col} is not a char boundary of line {line_idx} {line:?}"
    );
  }
  Ok(())
}

fn check_invariants(editor: &Editor) -> Result<(), TestCaseError> {
  let (line_idx, col) = editor.get_cursor_position().to_raw();
  assert_on_boundary(editor, line_idx, col, "cursor")?;
  if matches!(
    editor.get_active_mode(),
    EditorMode::Normal | EditorMode::VisualChar | EditorMode::VisualLine
  ) && let Some(line) = editor.lines.get(line_idx)
    && let Some(cursor) = ByteCol::checked(line, col)
  {
    // On the start of a visible character (grapheme cluster), never inside
    // one, and no further than the last one.
    prop_assert!(
      cursor.floor_grapheme(line) == cursor,
      "cursor column {col} is inside a grapheme of {line:?}"
    );
    prop_assert!(
      cursor <= ByteCol::last_grapheme(line),
      "cursor column {col} is past the last grapheme of {line:?}"
    );
    // If j/k left the cursor here aiming for a display column, the cursor is
    // on the grapheme covering that column, or on the last one when the line
    // is too short.
    if let Some(desired) = editor.editor_state.desired_column
      && desired.line == line_idx
      && desired.cursor_x.col() == cursor
      && desired.column != DisplayCol::END
    {
      let at = cursor.display_col(line);
      let next = cursor.next_grapheme(line);
      prop_assert!(
        at <= desired.column
          && (next >= ByteCol::end(line)
            || next.display_col(line) > desired.column),
        "cursor at display column {at:?} of {line:?} does not cover the \
         remembered column {:?}",
        desired.column
      );
    }
  }
  for (what, pos) in [
    ("selection start", editor.editor_state.selection_start.map(Pos::to_raw)),
    ("selection end", editor.editor_state.selection_end.map(Pos::to_raw)),
  ] {
    if let Some((l, c)) = pos {
      assert_on_boundary(editor, l, c, what)?;
    }
  }
  Ok(())
}

fn render(editor: &Editor) -> Result<(), TestCaseError> {
  let mut buffer = Vec::new();
  editor
    .draw_content_buffered(&mut buffer, 80, "")
    .map_err(|e| TestCaseError::fail(format!("draw failed: {e}")))?;
  prop_assert!(String::from_utf8(buffer).is_ok(), "frame is not UTF-8");
  Ok(())
}

proptest! {
  #![proptest_config(ProptestConfig::with_cases(256))]

  #[test]
  fn random_keys_over_unicode_text_never_panic(
    lines in document(),
    keys in prop::collection::vec(key_event(), 1..40),
    highlight in (0usize..200, 0usize..40),
  ) {
    let mut editor = Editor::new(lines, 80);
    editor.height = 8;
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.cursor_x = CursorCol::ZERO;
    // A persistent highlight at arbitrary absolute byte offsets (possibly
    // mid-character, possibly past the end) routes the selection and search
    // through the combined renderer as well as the plain ones.
    let (start, len) = highlight;
    editor.highlights.add_highlight(start, start + len);

    let mut stdout = std::io::stdout();
    render(&editor)?;
    for key in keys {
      editor
        .handle_event(key, &mut stdout)
        .map_err(|e| TestCaseError::fail(format!("{key:?} failed: {e}")))?;
      check_invariants(&editor)?;
      render(&editor)?;
    }
  }
}
