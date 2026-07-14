//! Small cross-cutting helpers: time, id generation, host resource metrics.

pub mod host;
pub mod ids;
pub mod time;

pub use ids::new_id;
pub use time::now_millis;
