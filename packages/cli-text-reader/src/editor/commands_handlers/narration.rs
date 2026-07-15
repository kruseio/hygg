use super::super::core::{Editor, EditorMode};
use super::super::speech::SpeakAction;

impl Editor {
  // Handle :speak / :speak stop - start or stop TTS narration
  pub fn handle_speak_command(
    &mut self,
    action: SpeakAction,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    match action {
      SpeakAction::Start => self.start_narration(),
      SpeakAction::Stop => self.stop_narration(),
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
    Ok(false)
  }

  // Handle :voice <id> - set the narration voice live. If narration is already
  // playing, restart it from the current position so the change is immediate;
  // otherwise it applies to the next :speak. Unknown ids surface as an error
  // when synthesis runs (see the narration worker).
  pub fn handle_voice_command(
    &mut self,
    voice: String,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.tts_voice = voice;
    if self.is_narrating() {
      self.start_narration();
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
    Ok(false)
  }

  // Handle :speed <n> - set the narration speed live, clamped to a sane range.
  // Restarts narration from the current position if currently playing.
  pub fn handle_speed_command(
    &mut self,
    speed: f32,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    self.tts_speed = speed.clamp(0.5, 2.0);
    if self.is_narrating() {
      self.start_narration();
    }
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
    Ok(false)
  }
}
