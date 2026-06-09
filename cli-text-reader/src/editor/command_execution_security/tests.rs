use super::parse_secure_command;

#[test]
fn test_allowed_commands() {
  let allowed = vec!["cat", "less", "head", "tail", "grep", "ls", "pwd"];
  for cmd in allowed {
    assert!(parse_secure_command(cmd).is_ok(), "{cmd} should be allowed");
  }
}

#[test]
fn test_rejected_commands() {
  let rejected = vec!["rm", "sudo", "kill", "reboot"];
  for cmd in rejected {
    assert!(parse_secure_command(cmd).is_err(), "{cmd} should be rejected");
  }
}

#[test]
fn test_dangerous_chars() {
  let dangerous =
    vec!["cat file; rm file", "echo `cmd`", "ls > file", "cmd | other"];
  for input in dangerous {
    assert!(parse_secure_command(input).is_err(), "{input} should be rejected");
  }
}
