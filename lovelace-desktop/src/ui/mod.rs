//! The native egui workbench window (eframe/egui 0.37) and its panes.
//!
//! The measured prototype in `rust/lovelace-desktop-spike` is the source of the editor,
//! syntax, plot and layout code; this module adapts those pieces to the product's
//! [`crate::workbench::Workbench`] instead of the spike's sample documents, sample variables
//! and sample SVG.
//!
//! A11Y-003/A11Y-014: [`a11y`] owns the window's single keyboard-focus ring and the ASCII/Unicode
//! localization of the frame's `DocumentView`, so both are defined once instead of per pane.
//! A11Y-005: [`typography`] resolves the configurable editor font, size, line height and ligature
//! choice into the one set of metrics every editor decoration derives from.

pub(crate) mod a11y;
mod app;
pub(crate) mod editor;
pub(crate) mod file_dialogs;
pub(crate) mod measure;
pub(crate) mod plot;
mod syntax;
pub(crate) mod theme;
pub(crate) mod typography;

pub use app::run;
