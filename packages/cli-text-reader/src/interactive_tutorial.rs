// Interactive tutorial v3 module - re-exports

// Re-export everything for backward compatibility
pub use crate::interactive_tutorial_buffer::{
  InteractiveTutorialStep, create_tutorial_buffer,
};
pub use crate::interactive_tutorial_steps::get_interactive_tutorial_steps;
pub use crate::interactive_tutorial_utils::fetch_github_stars;
