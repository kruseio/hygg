mod code_block;
mod prompt_signals;

pub(crate) use code_block::looks_like_code_block_line;
pub(crate) use prompt_signals::{
  looks_like_command_prompt_line, looks_like_git_log_graph_line,
  looks_like_toc_entry,
};
