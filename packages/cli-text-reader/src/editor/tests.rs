#[cfg(test)]
mod tests {
  use super::super::core::{BufferState, Editor, EditorMode, ViewMode};
  
  // Helper to create a test editor
  fn create_test_editor() -> Editor {
    let lines = vec![
      "Line 1".to_string(),
      "Line 2".to_string(),
      "Line 3".to_string(),
      "Line 4".to_string(),
      "Line 5".to_string(),
    ];
    Editor::new(lines, 80)
  }

  #[test]
  fn test_buffer_push_and_pop() {
    let mut editor = create_test_editor();
    
    // Initial state should have one buffer
    assert_eq!(editor.buffers.len(), 1);
    assert_eq!(editor.active_buffer, 0);
    
    // Push a new buffer
    let new_lines = vec!["Command output".to_string()];
    let buffer = BufferState::new(new_lines);
    editor.push_buffer(buffer);
    
    // Should now have two buffers with the second active
    assert_eq!(editor.buffers.len(), 2);
    assert_eq!(editor.active_buffer, 1);
    
    // Should be in split view mode
    match &editor.view_mode {
      ViewMode::Overlay { active, top_buffer_idx, .. } => {
        assert_eq!(*active, 1);
        assert_eq!(*top_buffer_idx, 0); // Should show buffer 0 in top
      }
      _ => panic!("Expected split view mode"),
    }
    
    // Pop the buffer
    let popped = editor.pop_buffer();
    assert!(popped.is_some());
    
    // Should be back to one buffer in normal mode
    assert_eq!(editor.buffers.len(), 1);
    assert_eq!(editor.active_buffer, 0);
    match &editor.view_mode {
      ViewMode::Normal => {}
      _ => panic!("Expected normal view mode"),
    }
  }

  #[test]
  fn test_split_dimensions() {
    let mut editor = create_test_editor();
    editor.height = 24; // Typical terminal height
    
    let (top_height, bottom_height, separator_line) = editor.calculate_split_dimensions();
    
    // Check dimensions are reasonable
    assert_eq!(separator_line, 11); // (24-1)/2
    assert_eq!(top_height, 11);
    assert_eq!(bottom_height, 11); // 24-1-11-1
    
    // Total should be less than terminal height
    assert!(top_height + bottom_height + 1 < editor.height);
  }

  #[test]
  fn test_command_execution_mode() {
    let mut editor = create_test_editor();
    
    // Start in normal mode
    assert_eq!(editor.editor_state.mode, EditorMode::Normal);
    
    // Enter command mode
    editor.editor_state.mode = EditorMode::Command;
    
    // Type '!' to switch to CommandExecution mode
    // This would be done by the event handler
    editor.editor_state.mode = EditorMode::CommandExecution;
    editor.editor_state.command_buffer.push('!');
    
    assert_eq!(editor.editor_state.mode, EditorMode::CommandExecution);
    assert_eq!(editor.editor_state.command_buffer, "!");
  }

  #[test]
  fn test_buffer_state_preservation() {
    let mut editor = create_test_editor();
    
    // Set some state
    editor.offset = 2;
    editor.cursor_x = 3;
    editor.cursor_y = 1;
    editor.editor_state.search_query = "test".to_string();
    
    // Save state
    editor.save_current_buffer_state();
    
    // Change state
    editor.offset = 0;
    editor.cursor_x = 0;
    editor.cursor_y = 0;
    editor.editor_state.search_query.clear();
    
    // Load state back
    editor.load_buffer_state(0);
    
    // State should be restored
    assert_eq!(editor.offset, 2);
    assert_eq!(editor.cursor_x, 3);
    assert_eq!(editor.cursor_y, 1);
    assert_eq!(editor.editor_state.search_query, "test");
  }

  #[test]
  fn test_viewport_aware_navigation() {
    let mut editor = create_test_editor();
    editor.height = 10;
    
    // Test in normal mode
    let viewport_height = match &editor.view_mode {
      ViewMode::Normal => editor.height.saturating_sub(1),
      ViewMode::Overlay { bottom_height, .. } => *bottom_height,
    };
    assert_eq!(viewport_height, 9);
    
    // Create a split view
    let buffer = BufferState::new(vec!["Output".to_string()]);
    editor.push_buffer(buffer);
    
    // Test in split mode
    let viewport_height = match &editor.view_mode {
      ViewMode::Normal => editor.height.saturating_sub(1),
      ViewMode::Overlay { bottom_height, .. } => *bottom_height,
    };
    assert!(viewport_height < 9); // Should be smaller in split view
  }

  #[test]
  fn test_can_close_buffer() {
    let mut editor = create_test_editor();
    
    // Initially should not be able to close the only buffer
    assert!(!editor.can_close_buffer());
    
    // Add a buffer
    let buffer = BufferState::new(vec!["Test".to_string()]);
    editor.push_buffer(buffer);
    
    // Now should be able to close
    assert!(editor.can_close_buffer());
  }

  #[test]
  fn test_cursor_centering_in_split() {
    let mut editor = create_test_editor();
    editor.height = 20;
    
    // Add more lines to test scrolling
    for i in 6..20 {
      editor.lines.push(format!("Line {}", i));
    }
    editor.total_lines = editor.lines.len();
    
    // Create split view
    let buffer_lines: Vec<String> = (0..10).map(|i| format!("Output {}", i)).collect();
    let mut buffer = BufferState::new(buffer_lines);
    
    // Calculate expected viewport height
    let (_, bottom_height, _) = editor.calculate_split_dimensions();
    buffer.viewport_height = bottom_height;
    
    editor.push_buffer(buffer);
    
    // Test centering in the active buffer
    editor.center_cursor_in_buffer();
    
    // Cursor should be centered within the buffer's viewport
    let expected_center = bottom_height / 2;
    assert!(editor.cursor_y <= expected_center);
  }

  #[test]
  fn test_cursor_bounds_in_split() {
    let mut editor = create_test_editor();
    editor.height = 10; // Small terminal
    
    // Create split
    let buffer = BufferState::new(vec!["Line 1".to_string(), "Line 2".to_string()]);
    editor.push_buffer(buffer);
    
    // Get viewport height
    let viewport_height = match &editor.view_mode {
      ViewMode::Overlay { bottom_height, .. } => *bottom_height,
      _ => panic!("Expected split mode"),
    };
    
    // Cursor Y should never exceed viewport height
    editor.cursor_y = 0;
    editor.move_cursor_down();
    assert!(editor.cursor_y < viewport_height);
    
    // Even at document boundaries
    editor.offset = 0;
    editor.cursor_y = 0;
    editor.move_cursor_up(); // Should not move
    assert_eq!(editor.cursor_y, 0);
  }

  #[test]
  fn test_nested_overlays() {
    let mut editor = create_test_editor();
    editor.height = 24;
    
    // Initial state
    assert_eq!(editor.buffers.len(), 1);
    assert_eq!(editor.active_buffer, 0);
    assert!(matches!(editor.view_mode, ViewMode::Normal));
    
    // Create first overlay
    let buffer1 = BufferState::new(vec![
      "First overlay".to_string(),
      "Line 2".to_string(),
      "Line 3".to_string(),
    ]);
    editor.push_buffer(buffer1);
    
    // Check first overlay state
    assert_eq!(editor.buffers.len(), 2);
    assert_eq!(editor.active_buffer, 1);
    match &editor.view_mode {
      ViewMode::Overlay { active, top_buffer_idx, .. } => {
        assert_eq!(*active, 1);
        assert_eq!(*top_buffer_idx, 0); // Shows original buffer in top
      }
      _ => panic!("Expected split view after first overlay"),
    }
    
    // Create second overlay (nested)
    let buffer2 = BufferState::new(vec![
      "Second overlay (nested)".to_string(),
      "Nested line 2".to_string(),
    ]);
    editor.push_buffer(buffer2);
    
    // Check nested overlay state
    assert_eq!(editor.buffers.len(), 3);
    assert_eq!(editor.active_buffer, 2);
    match &editor.view_mode {
      ViewMode::Overlay { active, top_buffer_idx, .. } => {
        assert_eq!(*active, 2);
        assert_eq!(*top_buffer_idx, 1); // Should show first overlay in top
      }
      _ => panic!("Expected split view after second overlay"),
    }
    
    // Test popping nested overlay
    let popped = editor.pop_buffer();
    assert!(popped.is_some());
    assert_eq!(editor.buffers.len(), 2);
    assert_eq!(editor.active_buffer, 1);
    match &editor.view_mode {
      ViewMode::Overlay { active, top_buffer_idx, .. } => {
        assert_eq!(*active, 1);
        assert_eq!(*top_buffer_idx, 0); // Back to showing original in top
      }
      _ => panic!("Expected split view after popping nested"),
    }
    
    // Pop first overlay
    let popped = editor.pop_buffer();
    assert!(popped.is_some());
    assert_eq!(editor.buffers.len(), 1);
    assert_eq!(editor.active_buffer, 0);
    assert!(matches!(editor.view_mode, ViewMode::Normal));
  }
}