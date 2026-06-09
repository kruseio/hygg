mod advanced;
mod basics;

use crate::interactive_tutorial_buffer::InteractiveTutorialStep;

pub fn get_interactive_tutorial_steps() -> Vec<InteractiveTutorialStep> {
  let mut steps = basics::basics_steps();
  steps.extend(advanced::advanced_steps());
  steps
}
