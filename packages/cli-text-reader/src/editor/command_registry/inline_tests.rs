use super::*;

#[test]
fn classifies_voice_with_id() {
  assert_eq!(
    classify_command("voice af_bella"),
    RegisteredCommand::Voice("af_bella".to_string())
  );
  // Blend syntax is a single token and must pass through untouched.
  assert_eq!(
    classify_command("voice af_heart.6+am_michael.4"),
    RegisteredCommand::Voice("af_heart.6+am_michael.4".to_string())
  );
}

#[test]
fn classifies_speed_and_rejects_non_numeric() {
  assert_eq!(classify_command("speed 1.25"), RegisteredCommand::Speed(1.25));
  assert_eq!(classify_command("speed 2"), RegisteredCommand::Speed(2.0));
  // Non-numeric speed is not a valid command.
  assert_eq!(classify_command("speed fast"), RegisteredCommand::Unknown);
}

#[test]
fn voice_and_speed_without_args_are_not_setters() {
  // Bare `:voice` / `:speed` carry no value, so they are not setters.
  assert_eq!(classify_command("voice"), RegisteredCommand::Unknown);
  assert_eq!(classify_command("speed"), RegisteredCommand::Unknown);
}

#[test]
fn voice_completes_known_ids() {
  let completion = complete_command("voice af_he");
  assert_eq!(completion.replacement.as_deref(), Some("voice af_heart"));
}
