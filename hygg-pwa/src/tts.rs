//! Text-to-speech via the browser Web Speech API. Speaks the document
//! line-by-line (skipping blank + ASCII-art rows), highlighting the current
//! line and auto-scrolling so the reader follows along — the same
//! read-and-follow behavior as the terminal narration, without a native engine.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::SpeechSynthesisUtterance;

use crate::model::{Book, LineKind};

/// Everything `speak_from` needs, bundled so the chained `onend` closure can
/// re-enter it. All fields are `Copy` (signals + node ref + scalars).
#[derive(Clone, Copy)]
pub struct SpeakCtx {
  pub book: RwSignal<Option<Book>>,
  pub speaking_line: RwSignal<Option<usize>>,
  pub playing: RwSignal<bool>,
  pub scroll_ref: NodeRef<leptos::html::Div>,
  pub rate: f32,
  pub line_h: f64,
}

/// Begin narration from `from_line`.
pub fn start(ctx: SpeakCtx, from_line: usize) {
  if let Some(s) = synth() {
    s.cancel();
  }
  ctx.playing.set(true);
  speak_from(ctx, from_line);
}

/// Stop narration and clear the highlight.
pub fn stop(playing: RwSignal<bool>, speaking_line: RwSignal<Option<usize>>) {
  playing.set(false);
  speaking_line.set(None);
  if let Some(s) = synth() {
    s.cancel();
  }
}

fn speak_from(ctx: SpeakCtx, line: usize) {
  if !ctx.playing.get_untracked() {
    ctx.speaking_line.set(None);
    return;
  }
  let Some((lines, kinds)) = ctx
    .book
    .with_untracked(|b| b.as_ref().map(|x| (x.lines.clone(), x.kinds.clone())))
  else {
    ctx.playing.set(false);
    return;
  };

  // Advance to the next speakable line (non-blank prose).
  let mut i = line;
  while i < lines.len()
    && (lines[i].trim().is_empty()
      || matches!(kinds.get(i), Some(LineKind::Ansi)))
  {
    i += 1;
  }
  if i >= lines.len() {
    ctx.playing.set(false);
    ctx.speaking_line.set(None);
    return;
  }

  ctx.speaking_line.set(Some(i));
  scroll_to(ctx, i);

  let Ok(utter) = SpeechSynthesisUtterance::new_with_text(&lines[i]) else {
    return;
  };
  utter.set_rate(ctx.rate);
  // One small closure per line; `forget` is safe (we can't drop the closure
  // we're currently executing inside). Bounded by lines spoken per session.
  let onend = Closure::<dyn FnMut()>::new(move || speak_from(ctx, i + 1));
  utter.set_onend(Some(onend.as_ref().unchecked_ref()));
  onend.forget();
  if let Some(s) = synth() {
    s.speak(&utter);
  }
}

/// Center the spoken line in the viewport (native smooth scroll).
fn scroll_to(ctx: SpeakCtx, line: usize) {
  if let Some(el) = ctx.scroll_ref.get_untracked() {
    let target =
      (line as f64 * ctx.line_h - el.client_height() as f64 / 2.0).max(0.0);
    el.set_scroll_top(target as i32);
  }
}

fn synth() -> Option<web_sys::SpeechSynthesis> {
  web_sys::window().and_then(|w| w.speech_synthesis().ok())
}

pub fn play_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <polygon points="6 4 20 12 6 20 6 4"/>
    </svg>
  }
}

pub fn pause_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <line x1="9" y1="5" x2="9" y2="19"/>
      <line x1="15" y1="5" x2="15" y2="19"/>
    </svg>
  }
}
