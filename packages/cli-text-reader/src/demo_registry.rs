use crate::demo_content::get_marketing_demo_content;
use crate::demo_script::DemoScript;

// List all available demos
pub fn list_all_demos() -> Vec<(usize, &'static str, &'static str)> {
  vec![
    (0, "Marketing Demo", "Comprehensive feature showcase"),
    (1, "Speed Demo", "Quick demo for social media"),
    (2, "Power User Demo", "Advanced features for developers"),
    (3, "Minimal Demo", "Clean and simple reading experience"),
    (4, "Workflow Demo", "Document processing workflow"),
  ]
}

// Get demo script by ID
pub fn get_demo_by_id(id: usize) -> Option<DemoScript> {
  match id {
    0 => Some(DemoScript::marketing_demo()),
    1 => Some(DemoScript::speed_demo()),
    2 => Some(DemoScript::power_user_demo()),
    3 => Some(DemoScript::minimal_demo()),
    4 => Some(DemoScript::workflow_demo()),
    _ => None,
  }
}

// Get demo content by ID
pub fn get_demo_content_by_id(_id: usize) -> String {
  // All demos use the same marketing content
  get_marketing_demo_content()
}
