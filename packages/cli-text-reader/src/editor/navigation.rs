// Navigation module - re-exports navigation functionality from specialized
// modules This file serves as a compatibility layer for existing code that
// imports navigation functions

// Re-export all navigation functions from their specialized modules
pub use super::char_navigation::*;
pub use super::line_navigation::*;
pub use super::word_navigation::*;
