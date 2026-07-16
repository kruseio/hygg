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

#[test]
fn env_and_powershell_are_no_longer_a_whitelist_bypass() {
  // `env PROG` and a bare powershell invocation each run an arbitrary program,
  // so they must not be accepted as first-token commands.
  for cmd in [
    "env rm /tmp/x",
    "env sh",
    "powershell -Command Start-Process calc",
    "powershell.exe -EncodedCommand ZWNobyBo",
  ] {
    assert!(parse_secure_command(cmd).is_err(), "{cmd} should be rejected");
  }
}

#[test]
fn exec_delegating_flags_are_rejected() {
  // Allowlisted read-only tools that can be steered into launching another
  // program via a flag are blocked at that flag, while their ordinary forms
  // still parse.
  for cmd in [
    "find . -exec rm {} +",
    "find /tmp -delete",
    "tar --to-command=id -xf a.tar",
    "tar --use-compress-program=sh -xf a.tar",
  ] {
    assert!(parse_secure_command(cmd).is_err(), "{cmd} should be rejected");
  }
  for cmd in ["find . -name x", "tar -tf a.tar"] {
    assert!(parse_secure_command(cmd).is_ok(), "{cmd} should be allowed");
  }
}

#[test]
fn printenv_still_works() {
  assert!(parse_secure_command("printenv PATH").is_ok());
}
