//! Thin binary shim; all the work is in the library.
//!
//! The OS launches this binary with the document path in `argv` when hygg-gui
//! is the registered handler (double-click a PDF); [`hygg_gui::launch`] picks
//! it up.

fn main() {
  hygg_gui::launch();
}
