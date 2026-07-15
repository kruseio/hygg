use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::QueueableCommand;
use crossterm::style::{Color, SetBackgroundColor};

use crate::editor::core::Editor;
use crate::editor::speech::{
  SpeechMsg, SpeechState, TtsStatus, build_word_spans,
};

// Visual, timing-based end-to-end demo of the real worker thread driving the
// highlight + auto-scroll. Not run in CI; run explicitly:
//   cargo test -p cli-text-reader --lib demo_live_narration -- --ignored
// --nocapture
#[test]
#[ignore = "timing-based visual demo"]
fn demo_live_narration() {
  let lines: Vec<String> =
    (0..14).map(|i| format!("Line {i} reads aloud")).collect();
  let mut editor = Editor::new(lines, 40);
  editor.height = 7; // 6 content rows
  editor.width = 40;
  editor.total_lines = editor.lines.len();
  editor.offset = 0; // start narration from the top so the scroll is visible
  editor.cursor_y = 0;
  let spans = build_word_spans(&editor.lines, &editor.line_kinds);
  editor.start_fake_narration(spans);

  for frame in 0..16 {
    std::thread::sleep(Duration::from_millis(220));
    editor.drain_speech();
    editor.center_cursor(); // the main loop does this every redraw
    let current = editor.speech.as_ref().and_then(|s| s.current);
    println!(
      "\n--- frame {frame}: offset={} cursor_y={} ---",
      editor.offset, editor.cursor_y
    );
    for row in 0..editor.height.saturating_sub(1) {
      let line_idx = editor.offset + row;
      let Some(line) = editor.lines.get(line_idx) else {
        println!("   ~");
        continue;
      };
      let mut shown = line.clone();
      if let (Some((ws, we)), Some((ls, _))) = (
        current,
        Editor::persistent_highlight_line_range(
          line_idx,
          &editor.lines,
          &editor.line_kinds,
        ),
      ) && ws < ls + line.len()
        && we > ls
      {
        let s = ws.saturating_sub(ls).min(line.len());
        let e = we.saturating_sub(ls).min(line.len());
        if s < e {
          shown = format!("{}[{}]{}", &line[..s], &line[s..e], &line[e..]);
        }
      }
      let cursor =
        if line_idx == editor.offset + editor.cursor_y { ">" } else { " " };
      println!("{cursor}  {shown}");
    }
    if !editor.is_narrating() {
      break;
    }
  }
  editor.stop_narration();
}

#[test]
fn renders_highlight_escape_around_spoken_word() {
  let mut editor = Editor::new(vec!["alpha beta gamma".to_string()], 80);
  editor.height = 3;
  editor.width = 80;
  editor.show_highlighter = true;
  editor.speech = Some(SpeechState {
    rx: mpsc::channel().1,
    cancel: Arc::new(AtomicBool::new(false)),
    worker: None,
    current: Some((6, 10)), // "beta"
    playing: true,
    status: Arc::new(Mutex::new(TtsStatus::Speaking)),
  });

  let mut buffer = Vec::new();
  editor.draw_content_buffered(&mut buffer, 80, "").unwrap();
  let out = String::from_utf8_lossy(&buffer);

  // The exact escape crossterm emits for our highlight background.
  let mut want = Vec::new();
  want.queue(SetBackgroundColor(Color::Cyan)).unwrap();
  let want = String::from_utf8(want).unwrap();

  assert!(out.contains(&want), "expected the cyan background escape");
  assert!(out.contains("beta"), "spoken word should be rendered");
  assert!(out.contains("alpha "), "leading text should be rendered");
}

#[test]
fn drain_speech_advances_highlight_and_scrolls() {
  let mut editor = Editor::new(
    vec![
      "line zero".to_string(),
      "line one".to_string(),
      "target two".to_string(),
    ],
    80,
  );
  editor.height = 24;
  editor.total_lines = 3;

  let (tx, rx) = mpsc::channel();
  editor.speech = Some(SpeechState {
    rx,
    cancel: Arc::new(AtomicBool::new(false)),
    worker: None,
    current: None,
    playing: true,
    status: Arc::new(Mutex::new(TtsStatus::Speaking)),
  });

  // "target" on line 2: abs offset = len("line zero")+1+len("line one")+1.
  let abs = "line zero".len() + 1 + "line one".len() + 1;
  tx.send(SpeechMsg::Word { abs_start: abs, abs_end: abs + 6, line: 2 })
    .unwrap();
  editor.drain_speech();

  let state = editor.speech.as_ref().unwrap();
  assert_eq!(state.current, Some((abs, abs + 6)));
  assert!(state.playing);
  // reading line followed the word
  assert_eq!(editor.offset + editor.cursor_y, 2);
  // the spoken word is on screen row (2 - offset) and highlights
  assert!(editor.spoken_word_on_line(2 - editor.offset));

  // Finished clears the highlight and marks not playing.
  tx.send(SpeechMsg::Finished).unwrap();
  editor.drain_speech();
  let state = editor.speech.as_ref().unwrap();
  assert_eq!(state.current, None);
  assert!(!state.playing);
}
