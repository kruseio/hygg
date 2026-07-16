use crate::editor::command_translation::translate_command_for_windows;
use std::collections::HashSet;

// Command output structure
pub struct CommandOutput {
  pub stdout: String,
  pub stderr: String,
  pub status: std::process::ExitStatus,
}

// Secure command structure for validated commands
#[derive(Debug)]
pub struct SecureCommand {
  pub program: String,
  pub args: Vec<String>,
}

// Parse and validate command using whitelist approach
pub fn parse_secure_command(cmd: &str) -> Result<SecureCommand, String> {
  let cmd = cmd.trim();
  if cmd.is_empty() {
    return Err("Empty command".to_string());
  }

  // On Windows, translate Unix commands to PowerShell
  #[cfg(target_os = "windows")]
  let cmd_string = translate_command_for_windows(cmd);
  #[cfg(target_os = "windows")]
  let cmd_to_parse = cmd_string.as_str();
  #[cfg(target_os = "windows")]
  let is_powershell_command = {
    // Check if the translation resulted in a PowerShell command
    cmd_string.contains("Get-")
      || cmd_string.contains("Select-")
      || cmd_string.contains("Format-")
      || cmd_string.contains(" | ")
  };

  // For non-Windows, keep the original reference. `is_powershell_command` is
  // only consulted by the Windows-only wrap block below (validation no longer
  // branches on it — it is always strict, and always on the user's own args),
  // so it is not defined here.
  #[cfg(not(target_os = "windows"))]
  let cmd_to_parse = cmd;

  // Whitelist of allowed commands - focus on read-only, generally safe commands
  // Security Note: Even read-only commands can have security implications:
  // - Some may read sensitive files if given appropriate paths
  // - Network commands (curl, wget) can make outbound connections
  // - Archive commands may extract to arbitrary locations with specially
  //   crafted files
  // However, these are standard system utilities and the risk is acceptable for
  // a text reader
  let allowed_commands: HashSet<&str> = [
    // File/directory listing and navigation
    "ls",
    "pwd",
    "find",
    "locate",
    "which",
    "whereis",
    // File viewing and reading (core functionality for text reader)
    "cat",
    "less",
    "more",
    "head",
    "tail",
    "file",
    "stat",
    "wc",
    "nl",
    // Text processing (read-only operations)
    "grep",
    "awk",
    "sed",
    "sort",
    "uniq",
    "cut",
    "tr",
    "fmt",
    "fold",
    // System information (generally safe, read-only)
    "date",
    "uptime",
    "whoami",
    "id",
    "uname",
    "hostname",
    "df",
    "free",
    "ps",
    "top",
    // `env` is deliberately absent: `env PROG ...` runs PROG, so allowlisting
    // it whitelists everything (`:!env sh`, `:!env rm file`). `printenv`
    // prints the environment and launches nothing, so it stays.
    "printenv",
    "history",
    // Archive viewing (read-only access, but see security note above)
    "tar",
    "zip",
    "unzip",
    "gzip",
    "gunzip",
    "zcat",
    // Network utilities (outbound connections only, read-only data)
    "ping",
    "dig",
    "nslookup",
    "curl",
    "wget",
    // Text utilities (path manipulation, generally safe)
    "echo",
    "printf",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    // PowerShell commands (Windows)
    "Get-ChildItem",
    "Get-Content",
    "Get-Location",
    "Select-String",
    "Get-Date",
    "Get-Process",
    "Get-Host",
    "Format-Table",
    "Select-Object",
    "Measure-Object",
    "Where-Object",
    "Sort-Object",
    // `powershell` / `powershell.exe` are deliberately absent: a bare
    // `:!powershell.exe -Command ...` (or `-EncodedCommand <base64>`) runs
    // arbitrary code, which is the whole policy this list exists to hold. The
    // Windows translation path below still spawns powershell.exe itself, but
    // only after an allowlisted *cmdlet* (Get-Content, …) passed this check —
    // it hardcodes the program, it does not read it from the user.
  ]
  .iter()
  .cloned()
  .collect();

  // Split command into parts
  let parts: Vec<&str> = cmd_to_parse.split_whitespace().collect();
  if parts.is_empty() {
    return Err("Invalid command".to_string());
  }

  let program = parts[0];

  // Check if command is whitelisted
  if !allowed_commands.contains(program) {
    return Err(format!("Command '{program}' is not allowed"));
  }

  // A handful of allowlisted read-only tools have an argument form that
  // launches another program — `find -exec`, `tar --to-command` — which is
  // the same whole-whitelist bypass that keeping `env` off the list closes.
  // Deny those forms per utility, on the original command's tokens; every
  // read-only use of each tool keeps working. Matched against the user's own
  // program name (these are not translated on Windows, so it is the real
  // utility either way).
  let orig_parts: Vec<&str> = cmd.split_whitespace().collect();
  let denied_args: &[&str] = match orig_parts.first().copied().unwrap_or("") {
    "find" => &[
      "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprintf", "-fprint",
      "-fls",
    ],
    "tar" => {
      &["--to-command", "--use-compress-program", "-I", "--checkpoint-action"]
    }
    "zip" | "unzip" => &["-T", "-TT", "--unzip-command"],
    _ => &[],
  };
  for arg in orig_parts.iter().skip(1) {
    // `--flag=value` and `--flag value` both, so the `=` form cannot slip past.
    let flag = arg.split('=').next().unwrap_or(arg);
    if denied_args.contains(&flag) {
      return Err(format!(
        "Argument '{arg}' is not allowed for '{}'",
        orig_parts[0]
      ));
    }
  }

  // Reject dangerous characters to prevent shell/PowerShell injection. Validate
  // the ORIGINAL user input, never the translated string: on Windows the
  // translated command is handed to `powershell.exe -Command`, which re-parses
  // it as source, so `$()`, backticks, and `|` a user smuggled into an argument
  // would execute there. The only pipes a translated command contains are the
  // ones the translator itself emitted (Get-ChildItem | Select-Object …), and
  // those are trusted by construction — so `|` can stay in the strict set that
  // applies to the user's own tokens. (On non-Windows the translation is a
  // no-op and this is exactly the original strict check.)
  let dangerous_chars: &[char] =
    &['|', '&', ';', '`', '$', '(', ')', '<', '>', '\\', '*', '?'];
  for arg in orig_parts.iter().skip(1) {
    if arg.chars().any(|c| dangerous_chars.contains(&c)) {
      return Err(format!("Argument contains dangerous characters: {arg}"));
    }

    // Additional safety: reject very long arguments that could cause buffer
    // overflows
    if arg.len() > 1000 {
      return Err("Argument too long (max 1000 characters)".to_string());
    }
  }

  // Additional safety: limit total number of arguments
  if parts.len() > 50 {
    return Err("Too many arguments (max 50)".to_string());
  }

  // On Windows, if we have a PowerShell command, wrap it properly
  #[cfg(target_os = "windows")]
  {
    // Check if this is a PowerShell cmdlet or contains pipes
    if is_powershell_command
      || program.contains('-')
      || program.starts_with("Get-")
      || program.starts_with("Select-")
    {
      // For PowerShell commands, pass the entire translated command as a single
      // argument
      return Ok(SecureCommand {
        program: "powershell.exe".to_string(),
        args: vec![
          "-NoProfile".to_string(),
          "-Command".to_string(),
          cmd_to_parse.to_string(),
        ],
      });
    }
  }

  Ok(SecureCommand {
    program: program.to_string(),
    args: parts[1..].iter().map(|s| s.to_string()).collect(),
  })
}
