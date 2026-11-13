//! The workbench's keyboard commands, shared by the window and the headless acceptance mode.
//!
//! The window reads the shortcuts from egui's input state and maps them through
//! [`command_for_shortcut`] and [`file_command_for_shortcut`]; `--gui-selftest` drives the same
//! functions and the same [`crate::workbench::Workbench`] entry points, so the tested mapping is
//! the shipped mapping instead of a second copy.
//!
//! Ctrl+Enter (Command+Enter on macOS) runs the active document. Esc cancels the active run.
//! Both are global: they work whether the editor, the toolbar or another pane has focus.
//!
//! DOC-002 adds the file commands: Ctrl+N (new), Ctrl+O (open), Ctrl+S (save), Ctrl+Shift+S
//! (save as) and Ctrl+W (close tab). They are window-level commands because each of them may
//! open a native dialog, so they live in their own enum rather than in
//! [`WorkbenchCommand`], which the headless workbench applies without a window.

/// One command a keyboard shortcut can request from the workbench.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkbenchCommand {
    /// Evaluate the active document.
    Run,
    /// Cancel the active document's in-flight run.
    Cancel,
}

impl WorkbenchCommand {
    /// The command's stable name, for logs and for the acceptance output.
    pub fn label(self) -> &'static str {
        match self {
            WorkbenchCommand::Run => "Run",
            WorkbenchCommand::Cancel => "Cancel",
        }
    }
}

/// One file-lifecycle command a keyboard shortcut can request from the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCommand {
    /// Add a new, empty document tab (Ctrl+N).
    New,
    /// Open a file chosen in the native Open dialog (Ctrl+O).
    Open,
    /// Write the active document to its own path, or ask for one (Ctrl+S).
    Save,
    /// Ask for a path and write the active document there (Ctrl+Shift+S).
    SaveAs,
    /// Close the active tab, prompting when it has unsaved changes (Ctrl+W).
    CloseTab,
}

impl FileCommand {
    /// The command's stable name, for logs and for the acceptance output.
    pub fn label(self) -> &'static str {
        match self {
            FileCommand::New => "New",
            FileCommand::Open => "Open",
            FileCommand::Save => "Save",
            FileCommand::SaveAs => "Save As",
            FileCommand::CloseTab => "Close",
        }
    }
}

/// Maps the two shortcut keys to a workbench command.
///
/// `command` is the platform shortcut modifier: Ctrl on Windows and Linux, Command on macOS.
/// Ctrl+Enter wins over Esc when both key presses arrive in the same frame. A bare Enter is not
/// a command: it belongs to the editor and inserts a newline.
pub fn command_for_shortcut(
    enter_pressed: bool,
    escape_pressed: bool,
    command_modifier: bool,
) -> Option<WorkbenchCommand> {
    if enter_pressed && command_modifier {
        return Some(WorkbenchCommand::Run);
    }
    if escape_pressed {
        return Some(WorkbenchCommand::Cancel);
    }
    None
}

/// EDT-010: one editor navigation command a keyboard shortcut can request.
///
/// These are window-level commands because tab navigation and the prompts belong to the window,
/// while bracket matching and go-to-line act on the active editor. The window and `--gui-selftest`
/// map the same shortcut table through [`editor_command_for_shortcut`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorCommand {
    /// Ctrl+G: open the go-to-line prompt.
    GoToLine,
    /// Ctrl+M: jump to the bracket matching the one at the caret.
    GoToMatchingBracket,
    /// Ctrl+Shift+O: filter the document's identifiers and jump to one.
    FindSymbol,
    /// Ctrl+F: open the find bar and focus its find field (EDT-009).
    Find,
    /// Ctrl+Tab: select the next document tab.
    NextTab,
    /// Ctrl+Shift+Tab: select the previous document tab.
    PreviousTab,
}

impl EditorCommand {
    /// The command's stable name, for logs and for the acceptance output.
    pub fn label(self) -> &'static str {
        match self {
            EditorCommand::GoToLine => "Go to line",
            EditorCommand::GoToMatchingBracket => "Go to matching bracket",
            EditorCommand::FindSymbol => "Find symbol",
            EditorCommand::Find => "Find",
            EditorCommand::NextTab => "Next tab",
            EditorCommand::PreviousTab => "Previous tab",
        }
    }
}

/// Maps the editor-navigation shortcuts to a command.
///
/// `key` is the character of the pressed key (`"\t"` for Tab), matched case-insensitively;
/// `modifier` is Ctrl on Windows and Linux (Command on macOS) and must be held for every editor
/// command. Ctrl+Shift+Tab is Previous Tab; Ctrl+Tab is Next Tab. Ctrl+Shift+O is Find Symbol;
/// Ctrl+O without Shift remains the file-open command and is not claimed here. Ctrl+F is Find
/// (EDT-009); Ctrl+Shift+F is not claimed.
pub fn editor_command_for_shortcut(
    key: char,
    modifier: bool,
    shift: bool,
) -> Option<EditorCommand> {
    if !modifier {
        return None;
    }
    match key.to_ascii_lowercase() {
        '\t' if shift => Some(EditorCommand::PreviousTab),
        '\t' => Some(EditorCommand::NextTab),
        'g' if !shift => Some(EditorCommand::GoToLine),
        'm' if !shift => Some(EditorCommand::GoToMatchingBracket),
        'o' if shift => Some(EditorCommand::FindSymbol),
        'f' if !shift => Some(EditorCommand::Find),
        _ => None,
    }
}

/// Maps a letter typed with the command modifier to a file command.
///
/// `letter` is the character of the pressed key, compared case-insensitively; `modifier` is Ctrl
/// on Windows and Linux (Command on macOS) and must be held for every file command. `shift`
/// distinguishes Ctrl+Shift+S (Save As) from Ctrl+S (Save). Alt combinations are never file
/// commands; the caller passes `false` for them by not calling this function.
///
/// The returned command is what the window executes, so this function is the single definition of
/// DOC-002's shortcut table.
pub fn file_command_for_shortcut(letter: char, modifier: bool, shift: bool) -> Option<FileCommand> {
    if !modifier {
        return None;
    }
    match letter.to_ascii_lowercase() {
        'n' if !shift => Some(FileCommand::New),
        'o' if !shift => Some(FileCommand::Open),
        's' if shift => Some(FileCommand::SaveAs),
        's' => Some(FileCommand::Save),
        'w' if !shift => Some(FileCommand::CloseTab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_enter_runs_and_escape_cancels() {
        assert_eq!(
            command_for_shortcut(true, false, true),
            Some(WorkbenchCommand::Run)
        );
        assert_eq!(
            command_for_shortcut(false, true, false),
            Some(WorkbenchCommand::Cancel)
        );
        assert_eq!(
            command_for_shortcut(false, true, true),
            Some(WorkbenchCommand::Cancel),
            "Escape does not require a modifier"
        );
    }

    #[test]
    fn a_bare_enter_and_unrelated_keys_issue_no_command() {
        assert_eq!(command_for_shortcut(true, false, false), None);
        assert_eq!(command_for_shortcut(false, false, true), None);
        assert_eq!(command_for_shortcut(false, false, false), None);
    }

    #[test]
    fn ctrl_enter_wins_when_both_shortcuts_arrive_together() {
        assert_eq!(
            command_for_shortcut(true, true, true),
            Some(WorkbenchCommand::Run)
        );
    }

    #[test]
    fn ctrl_n_o_s_w_and_ctrl_shift_s_are_the_file_shortcuts() {
        assert_eq!(
            file_command_for_shortcut('n', true, false),
            Some(FileCommand::New)
        );
        assert_eq!(
            file_command_for_shortcut('O', true, false),
            Some(FileCommand::Open),
            "the letter is matched case-insensitively"
        );
        assert_eq!(
            file_command_for_shortcut('s', true, false),
            Some(FileCommand::Save)
        );
        assert_eq!(
            file_command_for_shortcut('s', true, true),
            Some(FileCommand::SaveAs),
            "Ctrl+Shift+S is Save As, not Save"
        );
        assert_eq!(
            file_command_for_shortcut('w', true, false),
            Some(FileCommand::CloseTab)
        );
    }

    #[test]
    fn file_shortcuts_require_the_modifier_and_reject_shift_where_it_is_not_bound() {
        for letter in ['n', 'o', 's', 'w'] {
            assert_eq!(
                file_command_for_shortcut(letter, false, false),
                None,
                "{letter} without the modifier must not be a command"
            );
        }
        assert_eq!(file_command_for_shortcut('n', true, true), None);
        assert_eq!(file_command_for_shortcut('o', true, true), None);
        assert_eq!(file_command_for_shortcut('w', true, true), None);
        assert_eq!(file_command_for_shortcut('x', true, false), None);
        assert_eq!(file_command_for_shortcut('q', true, false), None);
    }

    #[test]
    fn every_file_command_has_a_label() {
        assert_eq!(FileCommand::New.label(), "New");
        assert_eq!(FileCommand::Open.label(), "Open");
        assert_eq!(FileCommand::Save.label(), "Save");
        assert_eq!(FileCommand::SaveAs.label(), "Save As");
        assert_eq!(FileCommand::CloseTab.label(), "Close");
    }

    #[test]
    fn editor_navigation_shortcuts_map_without_the_mouse() {
        assert_eq!(
            editor_command_for_shortcut('g', true, false),
            Some(EditorCommand::GoToLine)
        );
        assert_eq!(
            editor_command_for_shortcut('G', true, false),
            Some(EditorCommand::GoToLine),
            "the letter is matched case-insensitively"
        );
        assert_eq!(
            editor_command_for_shortcut('m', true, false),
            Some(EditorCommand::GoToMatchingBracket)
        );
        assert_eq!(
            editor_command_for_shortcut('o', true, true),
            Some(EditorCommand::FindSymbol)
        );
        assert_eq!(
            editor_command_for_shortcut('f', true, false),
            Some(EditorCommand::Find)
        );
        assert_eq!(
            editor_command_for_shortcut('F', true, false),
            Some(EditorCommand::Find),
            "the letter is matched case-insensitively"
        );
        assert_eq!(
            editor_command_for_shortcut('\t', true, false),
            Some(EditorCommand::NextTab)
        );
        assert_eq!(
            editor_command_for_shortcut('\t', true, true),
            Some(EditorCommand::PreviousTab)
        );

        for key in ['g', 'm', '\t'] {
            assert_eq!(
                editor_command_for_shortcut(key, false, false),
                None,
                "{key:?} without the modifier must not be a command"
            );
        }
        assert_eq!(editor_command_for_shortcut('o', true, false), None);
        assert_eq!(editor_command_for_shortcut('g', true, true), None);
        assert_eq!(
            editor_command_for_shortcut('f', true, true),
            None,
            "Ctrl+Shift+F is not Find"
        );
        assert_eq!(editor_command_for_shortcut('x', true, false), None);
    }

    #[test]
    fn every_editor_command_has_a_label() {
        assert_eq!(EditorCommand::GoToLine.label(), "Go to line");
        assert_eq!(
            EditorCommand::GoToMatchingBracket.label(),
            "Go to matching bracket"
        );
        assert_eq!(EditorCommand::FindSymbol.label(), "Find symbol");
        assert_eq!(EditorCommand::Find.label(), "Find");
        assert_eq!(EditorCommand::NextTab.label(), "Next tab");
        assert_eq!(EditorCommand::PreviousTab.label(), "Previous tab");
    }
}
