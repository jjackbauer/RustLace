//! The only place the product opens a native file dialog.
//!
//! ARC-003 keeps `src/ui/**` free of platform window handles. The window declares what it wants
//! (a title, filters, an extension and an initial directory) through the `FileDialogs` trait and
//! the plot-export helper; this adapter supplies the platform's own chooser. Nothing here knows
//! about documents, panes or engine state.
//!
//! - Windows: the comdlg32 common dialogs, owned by the product window (`src/win32/ffi.rs`).
//! - Linux: the desktop's chooser, `zenity` (GNOME) or `kdialog` (KDE), run as a child process.
//!   Both print the chosen absolute path on stdout and exit non-zero when the user cancels, so
//!   the adapter returns `None` exactly as the Win32 wrapper does for IDCANCEL. A machine with
//!   neither chooser installed observes the same `None` as a cancelled dialog; the workbench
//!   never blocks and never invents a path. `--help`/docs note the package that supplies it.
//!
//! The call blocks the UI thread while the modal chooser is open, which is what the Win32 path
//! does too (`GetOpenFileNameW` pumps a modal loop on the calling thread).

use std::path::{Path, PathBuf};

/// Shows the Open dialog owned by the product window.
pub fn open_file_dialog(
    title: &str,
    filters: &[(&str, &str)],
    initial_dir: Option<&Path>,
) -> Option<PathBuf> {
    imp::open(title, filters, initial_dir)
}

/// Shows a save dialog owned by the product window.
pub fn save_file_dialog(
    suggested: &str,
    title: &str,
    filters: &[(&str, &str)],
    extension: &str,
    initial_dir: Option<&Path>,
) -> Option<PathBuf> {
    imp::save(suggested, title, filters, extension, initial_dir)
}

/// The Windows implementation: the shipped comdlg32 wrappers, owned by the process's window.
#[cfg(windows)]
mod imp {
    use std::path::{Path, PathBuf};

    pub(super) fn open(
        title: &str,
        filters: &[(&str, &str)],
        initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        let owner = crate::win32::ffi::foreground_window_of_this_process();
        crate::win32::ffi::open_file_dialog_with(owner, title, filters, initial_dir)
    }

    pub(super) fn save(
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
}

/// The Linux implementation: the desktop session's chooser as a blocking child process.
///
/// `zenity` is preferred because it is the GTK chooser shipped by GNOME and available on most
/// distributions; `kdialog` is the KDE counterpart. The chosen path is read from stdout; a
/// non-zero exit (the user pressed Cancel) or a missing chooser yields `None`.
#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// One run of a chooser: the program and the arguments for the dialog.
    struct Chooser {
        program: &'static str,
        args: Vec<String>,
    }

    /// The chooser installed on this machine, in preference order.
    fn chooser() -> Option<&'static str> {
        // Honour an explicit override so a test or a session can pin the tool.
        if let Some(value) = std::env::var_os("LOVELACE_FILE_DIALOG") {
            let value = value.to_string_lossy().into_owned();
            if value == "zenity" || value == "kdialog" {
                return Some(if value == "zenity" {
                    "zenity"
                } else {
                    "kdialog"
                });
            }
        }
        for candidate in ["zenity", "kdialog"] {
            if which(candidate) {
                return Some(candidate);
            }
        }
        None
    }

    /// True when `program` is an executable on PATH.
    fn which(program: &str) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        std::env::split_paths(&path).any(|dir| {
            let candidate = dir.join(program);
            candidate.is_file()
        })
    }

    /// The GTK filter syntax: `Name | *.lv *.ls`, from the Win32 `("Name", "*.lv;*.ls")` pair.
    fn zenity_filter(name: &str, pattern: &str) -> String {
        let patterns = pattern
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        format!("{name} | {patterns}")
    }

    /// The KDE filter syntax: `*.lv *.ls|Name`.
    fn kdialog_filter(name: &str, pattern: &str) -> String {
        let patterns = pattern
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        format!("{patterns}|{name}")
    }

    fn build(
        save: bool,
        suggested: &str,
        title: &str,
        filters: &[(&str, &str)],
        initial_dir: Option<&Path>,
    ) -> Option<Chooser> {
        let program = chooser()?;
        let mut args: Vec<String> = Vec::new();
        if program == "zenity" {
            args.push("--file-selection".to_string());
            if save {
                args.push("--save".to_string());
                args.push("--confirm-overwrite".to_string());
            }
            args.push(format!("--title={title}"));
            let start = initial_dir
                .map(|dir| dir.join(suggested))
                .unwrap_or_else(|| PathBuf::from(suggested));
            args.push(format!("--filename={}", start.display()));
            for (name, pattern) in filters {
                args.push(format!("--file-filter={}", zenity_filter(name, pattern)));
            }
        } else {
            args.push(if save {
                "--getsavefilename".to_string()
            } else {
                "--getopenfilename".to_string()
            });
            let start = initial_dir
                .map(|dir| dir.display().to_string())
                .unwrap_or_default();
            args.push(start);
            for (name, pattern) in filters {
                args.push(kdialog_filter(name, pattern));
            }
            if !title.is_empty() {
                args.push("--title".to_string());
                args.push(title.to_string());
            }
        }
        Some(Chooser { program, args })
    }

    fn run(chooser: Chooser) -> Option<PathBuf> {
        let output = Command::new(chooser.program)
            .args(&chooser.args)
            .output()
            .ok()?;
        if !output.status.success() {
            // Exit 1 is Cancel. Anything else is a broken session (no display, no portal); both
            // are "no path chosen", never a fabricated one.
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let path = text
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())?;
        Some(PathBuf::from(path))
    }

    pub(super) fn open(
        title: &str,
        filters: &[(&str, &str)],
        initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        run(build(false, "", title, filters, initial_dir)?)
    }

    pub(super) fn save(
        suggested: &str,
        title: &str,
        filters: &[(&str, &str)],
        _extension: &str,
        initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        run(build(true, suggested, title, filters, initial_dir)?)
    }
}

/// The fallback for a desktop platform with no chooser adapter: no path, exactly as a cancel.
#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
mod imp {
    use std::path::{Path, PathBuf};

    pub(super) fn open(
        _title: &str,
        _filters: &[(&str, &str)],
        _initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        None
    }

    pub(super) fn save(
        _suggested: &str,
        _title: &str,
        _filters: &[(&str, &str)],
        _extension: &str,
        _initial_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        None
    }
}
