use super::command_registry::{
  RegisteredCommand, TutorialCommand, classify_command, command_help_lines,
  complete_command,
};

#[test]
fn empty_completion_lists_top_level_commands() {
  let completion = complete_command("");

  assert!(completion.replacement.is_none());
  assert!(completion.suggestions.contains(&"ocr"));
  assert!(completion.suggestions.contains(&"tutorial"));
  assert!(completion.suggestions.contains(&"!"));
}

#[test]
fn home_and_rex_both_return_to_the_library() {
  assert_eq!(classify_command("home"), RegisteredCommand::Home);
  assert_eq!(classify_command("Rex"), RegisteredCommand::Home);
  // `:Rex` is case-sensitive (mirrors Vim's netrw command).
  assert_eq!(classify_command("rex"), RegisteredCommand::Unknown);
}

#[test]
fn unique_top_level_prefix_completes_in_place() {
  let completion = complete_command("ocr");

  assert_eq!(completion.replacement.as_deref(), Some("ocr"));
  assert!(completion.suggestions.is_empty());
}

#[test]
fn ambiguous_top_level_prefix_lists_matches() {
  let completion = complete_command("no");

  assert!(completion.replacement.is_none());
  assert_eq!(
    completion.suggestions,
    vec!["nohl", "nohlsearch", "note", "notutorial"]
  );
}

#[test]
fn ocr_argument_completion_lists_toggle_values() {
  let completion = complete_command("ocr ");

  assert!(completion.replacement.is_none());
  assert_eq!(completion.suggestions, vec!["on", "off"]);
}

#[test]
fn tutorial_argument_completion_includes_step_placeholder() {
  let completion = complete_command("tutorial ");

  assert!(completion.replacement.is_none());
  assert_eq!(completion.suggestions, vec!["on", "off", "{n}"]);
}

#[test]
fn shell_execution_commands_do_not_complete() {
  let completion = complete_command("!ec");

  assert!(completion.replacement.is_none());
  assert!(completion.suggestions.is_empty());
}

#[test]
fn shell_command_prefix_does_not_complete_bang_command() {
  let completion = complete_command("!");

  assert!(completion.replacement.is_none());
  assert!(completion.suggestions.is_empty());
}

#[test]
fn registry_classifies_dispatch_commands() {
  assert_eq!(classify_command("ocr on"), RegisteredCommand::Ocr(true));
  assert_eq!(classify_command("ocr off"), RegisteredCommand::Ocr(false));
  assert_eq!(classify_command("ocron"), RegisteredCommand::Unknown);
  assert_eq!(
    classify_command("tutorial 3"),
    RegisteredCommand::Tutorial(TutorialCommand::Step(3))
  );
  assert_eq!(
    classify_command("local-progress"),
    RegisteredCommand::LocalProgress
  );
  assert_eq!(
    classify_command("!echo ok"),
    RegisteredCommand::Shell("echo ok".to_string())
  );
}

#[test]
fn help_metadata_comes_from_registry() {
  let help = command_help_lines().join("\n");

  assert!(help.contains(":ocr on, :ocr off"));
  assert!(help.contains(":!{cmd}"));
  assert!(help.contains(":tutorial {n}"));
  assert!(help.contains(":local-progress"));
}
