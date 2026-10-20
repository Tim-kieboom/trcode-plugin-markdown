//! A markdown preview for TerminalCode, in a pane beside the editor.
//!
//! The editor sends the saved text of the markdown file in front
//! ([`trcode_plugin_api::ToPlugin::FileText`]) and how wide the pane is; [`render`] turns
//! it into lines of styled text, wrapped to that width, and [`Viewer`] keeps the
//! place the reader is at, finds the links and opens the ones that lead to files. Both
//! know nothing of WebAssembly, so they are tested on the machine they are written on;
//! the last line makes the viewer the module.

pub use render::{Doc, Line, Span, render};
pub use viewer::Viewer;

mod render;
#[cfg(test)]
mod tests;
mod viewer;

trcode_plugin_api::export_plugin!(Viewer);
