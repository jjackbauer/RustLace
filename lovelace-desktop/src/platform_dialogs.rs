//! The only place the product calls the Win32 common file dialogs.
//!
//! ARC-003 keeps `src/ui/**` free of platform window handles. The window declares what it wants
//! (a title, filters, an extension and an initial directory) through the `FileDialogs` trait and
//! the plot-export helper; this adapter supplies the native owner window and forwards to the
//! existing comdlg32 wrappers. Nothing here knows about documents, panes or engine state.

use std::path::{Path, PathBuf};

/// Shows the Open dialog owned by the product window.
pub fn open_file_dialog(
    title: &str,
    filters: &[(&str, &str)],
    initial_dir: Option<&Path>,
) -> Option<PathBuf> {
    let owner = crate::win32::ffi::foreground_window_of_this_process();
    crate::win32::ffi::open_file_dialog_with(owner, title, filters, initial_dir)
}

/// Shows a save dialog owned by the product window.
pub fn save_file_dialog(
    suggested: &str,
    title: &str,
    filters: &[(&str, &str)],
    extension: &str,
    initial_dir: Option<&Path>,
) -> Option<PathBuf> {
    let owner = crate::win32::ffi::foreground_window_of_this_process();
    crate::win32::ffi::save_file_dialog_with(
        owner,
        suggested,
        title,
        filters,
        extension,
        initial_dir,
    )
}
