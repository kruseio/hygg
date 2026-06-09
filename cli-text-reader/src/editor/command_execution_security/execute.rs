use super::parse::{CommandOutput, SecureCommand};
use std::process::{Command, Stdio};
use std::time::Duration;

// Execute a validated command with timeout
pub fn execute_secure_command_with_timeout(
  secure_cmd: SecureCommand,
  timeout: Duration,
) -> Result<CommandOutput, String> {
  let mut child = Command::new(&secure_cmd.program)
    .args(&secure_cmd.args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| {
      format!("Failed to execute command '{}': {}", secure_cmd.program, e)
    })?;

  // Wait for the command with timeout
  match child.wait_timeout(timeout) {
    Ok(Some(status)) => {
      // Command completed within timeout
      let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to read output: {e}"))?;

      Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        status,
      })
    }
    Ok(None) => {
      // Timeout occurred, kill the process
      let _ = child.kill();
      Err("Command timed out after 30 seconds".to_string())
    }
    Err(e) => Err(format!("Failed to wait for command: {e}")),
  }
}

// Extension trait for waiting with timeout
trait WaitTimeout {
  fn wait_timeout(
    &mut self,
    dur: Duration,
  ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl WaitTimeout for std::process::Child {
  fn wait_timeout(
    &mut self,
    dur: Duration,
  ) -> std::io::Result<Option<std::process::ExitStatus>> {
    let start = std::time::Instant::now();

    loop {
      match self.try_wait()? {
        Some(status) => return Ok(Some(status)),
        None => {
          if start.elapsed() >= dur {
            return Ok(None);
          }
          std::thread::sleep(Duration::from_millis(100));
        }
      }
    }
  }
}
