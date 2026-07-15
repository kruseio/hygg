//! Custom iced widgets the toolkit does not provide out of the box.
//!
//! iced 0.13 ships no *selectable static-text* widget, so
//! [`selectable`](selectable::selectable) implements one directly on top of the
//! `iced::advanced::Widget` trait: it renders like `iced::widget::text` but
//! supports drag-to-select, double/triple click, select-all and copy — all in
//! the widget's own tree state, emitting no application messages.

pub mod selectable;
