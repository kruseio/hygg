//! The `:help` command listing. Split out from `types` to keep each file
//! within the repository's per-file line budget.

pub(crate) fn command_help_lines() -> Vec<String> {
  vec![
    "    :q, :q!, :quit, :exit  Quit".to_string(),
    "    :help, :commands       Show this help".to_string(),
    "    :tutorial              Start interactive tutorial".to_string(),
    "    :tutorial {n}          Jump to tutorial step n".to_string(),
    "    :tutorial on           Enable tutorial for next launch".to_string(),
    "    :tutorial off          Disable tutorial (same as :notutorial)"
      .to_string(),
    "    :notutorial            Permanently disable tutorial".to_string(),
    "    :next, :continue       Next tutorial step (when completed)"
      .to_string(),
    "    :back, :prev, :previous Previous tutorial step".to_string(),
    "    :home, :Rex            Return to your library (recently read)"
      .to_string(),
    "    :note                  Take notes for this document".to_string(),
    "    :connect <url>         Connect to a hygg sync server".to_string(),
    "    :auth <user> <token>   Authenticate with your username + device token"
      .to_string(),
    "    :autosync              Show sync scope + this document's status"
      .to_string(),
    "    :autosync on/off       Master switch (off = fully serverless)"
      .to_string(),
    "    :autosync all|books|manual  Which documents auto-sync".to_string(),
    "    :autosync add/remove   Auto-sync this document (or stop)".to_string(),
    "    :sync                  Sync now (push + pull)".to_string(),
    "    :syncmode              Show this document's sync mode".to_string(),
    "    :syncmode full|metadata|off  Set this device's sync for the document"
      .to_string(),
    "    :syncmode inherit      Follow the account-wide sync ceiling".to_string(),
    "    :syncmode server <m>   Set the account-wide ceiling (all devices)"
      .to_string(),
    "    :server-progress       Jump to the latest server position".to_string(),
    "    :local-progress        Keep local position and overwrite server"
      .to_string(),
    "    :disconnect            Disconnect from the sync server".to_string(),
    "    :encryption            End-to-end encryption status + setup wizard"
      .to_string(),
    "    :encryption setup      Turn on encryption (generates your key)"
      .to_string(),
    "    :encryption use <key>  Set up this device with the account key"
      .to_string(),
    "    :encryption convert    Encrypt documents uploaded before you enabled it"
      .to_string(),
    "    :z                     Toggle line highlighter".to_string(),
    "    :p                     Toggle progress display".to_string(),
    "    :ocr on, :ocr off      Toggle PDF OCR for this PDF and future launches"
      .to_string(),
    "    :speak, :speak stop    Narrate from the cursor (any key stops)"
      .to_string(),
    "    :voice <id>            Narration voice (e.g. af_heart, am_michael)"
      .to_string(),
    "    :speed <n>             Narration speed, 0.5-2.0 (e.g. 1.25)"
      .to_string(),
    "    :cursor, :c            Toggle cursor visibility".to_string(),
    "    :h                     Highlight selected text (in visual mode)"
      .to_string(),
    "    :nohl, :nohlsearch     Clear search highlighting".to_string(),
    "    :credits, :author      Show credits".to_string(),
    "    :about                 Show about information".to_string(),
    "    :!{cmd}                Execute shell command (opens in split view)"
      .to_string(),
  ]
}
