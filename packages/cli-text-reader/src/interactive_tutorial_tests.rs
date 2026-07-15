#[cfg(test)]
mod tests {
  use crate::interactive_tutorial::*;

  #[test]
  fn test_tutorial_steps_count() {
    let steps = get_interactive_tutorial_steps();
    assert_eq!(steps.len(), 9, "Should have exactly 9 tutorial steps");
  }

  // TODO: The following tests need to be updated to work with the new
  // InteractiveTutorialStep structure The old TutorialStep type and
  // associated functions have been refactored

  #[test]
  fn test_tutorial_steps_have_titles() {
    let steps = get_interactive_tutorial_steps();
    for (i, step) in steps.iter().enumerate() {
      assert!(
        !step.title.is_empty(),
        "Step {} should have a non-empty title",
        i + 1
      );
      // assert!(!step.content.is_empty(), "Step {} should have content", i +
      // 1); // 'content' field no longer exists
    }
  }

  /* Commented out - needs update for new InteractiveTutorialStep structure
  #[test]
  fn test_check_tutorial_key_basic() {
    let step = TutorialStep {
      title: "Test".to_string(),
      content: vec!["Test content".to_string()],
      expected_key: Some("j".to_string()),
    };

    assert!(check_tutorial_key(&step, "j"));
    assert!(check_tutorial_key(&step, "Down")); // j should also accept Down
    assert!(!check_tutorial_key(&step, "k"));
  }

  #[test]
  fn test_check_tutorial_key_no_expected() {
    let step = TutorialStep {
      title: "Test".to_string(),
      content: vec!["Test content".to_string()],
      expected_key: None,
    };

    // Should accept common navigation keys
    assert!(check_tutorial_key(&step, "j"));
    assert!(check_tutorial_key(&step, "k"));
    assert!(check_tutorial_key(&step, "Down"));
    assert!(check_tutorial_key(&step, "Up"));
    assert!(!check_tutorial_key(&step, "x"));
  }

  #[test]
  fn test_format_tutorial_step_basic() {
    let step = TutorialStep {
      title: "Test Step".to_string(),
      content: vec![
        "Line 1".to_string(),
        "→ Action item".to_string(),
        "".to_string(),
        "  • Bullet point".to_string(),
      ],
      expected_key: Some("j".to_string()),
    };

    let formatted = format_tutorial_step(&step, 0, 10, 80);

    // Should have a frame structure
    assert!(formatted.len() > 5, "Formatted output should have multiple lines");
    assert!(formatted[0].contains("╭"), "Should have top border");
    assert!(formatted.last().unwrap().contains("╯"), "Should have bottom border");

    // Should contain the title
    let title_line = &formatted[1];
    assert!(title_line.contains("Test Step"), "Should contain the title");
    assert!(title_line.contains("["), "Should contain progress indicator");
  }
  */

  /* Commented out - expected_key field no longer exists
  #[test]
  fn test_tutorial_progression() {
    let steps = get_interactive_tutorial_steps();

    // First step should expect 'j'
    assert_eq!(steps[0].expected_key, Some("j".to_string()));

    // Second step should expect 'k'
    assert_eq!(steps[1].expected_key, Some("k".to_string()));

    // Some steps should have no specific expected key
    let has_none = steps.iter().any(|step| step.expected_key.is_none());
    assert!(has_none, "Some steps should allow any navigation key");
  }
  */

  /* Commented out - highlight_key function no longer exists
  #[test]
  fn test_highlight_key_function() {
    // Test single key highlighting
    let result = highlight_key("'j'");
    assert!(result.len() > 3, "Highlighted key should have color codes");

    // Test text with no keys
    let result = highlight_key("regular text");
    assert_eq!(result, "regular text");
  }
  */
}
