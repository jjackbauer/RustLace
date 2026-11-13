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
//!
//! # One table, three readers (A11Y-001, A11Y-007)
//!
//! [`BINDINGS`] is the app's complete shortcut registration: every global command, every
//! pane action and every editor command, with the scope and one-sentence description a Help
//! surface shows. Three readers consume it and none of them keeps a second list:
//!
//! - the window's dispatcher ([`binding_for`] / [`action_for`]),
//! - the legacy per-kind mappers ([`command_for_shortcut`], [`file_command_for_shortcut`],
//!   [`editor_command_for_shortcut`]), which now resolve their answer through the table,
//! - Help > Keyboard Shortcuts, which renders [`reference_rows`] — a projection of the same
//!   table, so a binding can never exist in the app and be missing from the reference (or the
//!   reverse).
//!
//! `Ctrl` below means the platform command modifier (Command on macOS).
//!
//! # Adding a binding
//!
//! Add one [`Binding`] to [`BINDINGS`] and handle its [`Action`] in the window. If the action is
//! a new command kind, add the enum variant and a binding for it: the
//! `every_registered_binding_is_reachable_through_the_dispatcher` and
//! `the_reference_lists_exactly_the_registered_bindings` tests fail when the table, the
//! dispatcher and the reference disagree.

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

    /// Every file command, in the order the File bar offers them.
    #[cfg_attr(not(test), allow(dead_code))] // the completeness test iterates it
    pub const ALL: [FileCommand; 5] = [
        FileCommand::New,
        FileCommand::Open,
        FileCommand::Save,
        FileCommand::SaveAs,
        FileCommand::CloseTab,
    ];
}

impl WorkbenchCommand {
    /// Every workbench command.
    #[cfg_attr(not(test), allow(dead_code))] // the completeness test iterates it
    pub const ALL: [WorkbenchCommand; 2] = [WorkbenchCommand::Run, WorkbenchCommand::Cancel];
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
        // Resolved through the same table the Help reference renders, so the registration and
        // the dispatch cannot drift apart.
        return registered_workbench(Action::Workbench(WorkbenchCommand::Run));
    }
    if escape_pressed {
        return registered_workbench(Action::Workbench(WorkbenchCommand::Cancel));
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

    /// Every editor navigation command.
    #[cfg_attr(not(test), allow(dead_code))] // the completeness test iterates it
    pub const ALL: [EditorCommand; 6] = [
        EditorCommand::GoToLine,
        EditorCommand::GoToMatchingBracket,
        EditorCommand::FindSymbol,
        EditorCommand::Find,
        EditorCommand::NextTab,
        EditorCommand::PreviousTab,
    ];
}

// -------------------------------------------------------------------------------------------
// A11Y-001 / A11Y-007: the one binding table.
// -------------------------------------------------------------------------------------------

/// One key a binding can name, independent of the windowing library.
///
/// The window converts its own key events into these names, so the table (and every test of it)
/// needs no egui types and the headless workbench and the window read the same rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyName {
    /// Enter.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Space.
    Space,
    /// Comma.
    Comma,
    /// F1.
    F1,
    /// F6.
    F6,
    /// F8.
    F8,
    /// The `=`/`+` key.
    Plus,
    /// The `-` key.
    Minus,
    /// The digit key, 0-9.
    Digit(u8),
    /// A letter, matched case-insensitively.
    Char(char),
}

impl KeyName {
    /// The key's name as the reference shows it.
    pub fn label(self) -> String {
        match self {
            KeyName::Enter => "Enter".to_string(),
            KeyName::Escape => "Esc".to_string(),
            KeyName::Tab => "Tab".to_string(),
            KeyName::Space => "Space".to_string(),
            KeyName::Comma => ",".to_string(),
            KeyName::F1 => "F1".to_string(),
            KeyName::F6 => "F6".to_string(),
            KeyName::F8 => "F8".to_string(),
            KeyName::Plus => "=".to_string(),
            KeyName::Minus => "-".to_string(),
            KeyName::Digit(digit) => digit.to_string(),
            KeyName::Char(c) => c.to_ascii_uppercase().to_string(),
        }
    }
}

/// One keyboard chord: a key plus the modifiers that must be held.
///
/// The comparison is exact, so `Ctrl+Shift+S` can never be answered by `Ctrl+S`. The two
/// documented exceptions live in the legacy mappers: Esc cancels with or without Ctrl.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// The key.
    pub key: KeyName,
    /// The platform command modifier (Ctrl, or Command on macOS).
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt.
    pub alt: bool,
}

impl Shortcut {
    /// Builds a shortcut.
    pub const fn new(key: KeyName, ctrl: bool, shift: bool, alt: bool) -> Shortcut {
        Shortcut {
            key,
            ctrl,
            shift,
            alt,
        }
    }

    /// A shortcut with no modifiers.
    pub const fn plain(key: KeyName) -> Shortcut {
        Shortcut::new(key, false, false, false)
    }

    /// The chord as the reference and the hover text show it, e.g. `Ctrl+Shift+S`.
    pub fn chord(self) -> String {
        let mut text = String::new();
        if self.ctrl {
            text.push_str("Ctrl+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        if self.alt {
            text.push_str("Alt+");
        }
        text.push_str(&self.key.label());
        text
    }
}

/// One of the window's keyboard-focus groups (A11Y-001).
///
/// F6 and Ctrl+1..5 move keyboard focus between these groups; each group focuses its first
/// control, so a keyboard user reaches every pane without a pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The toolbar (Run is its first control).
    Toolbar,
    /// The editor surface.
    Editor,
    /// The plot pane (Fit is its first control).
    Plot,
    /// The workspace pane (the Result/Inspector tabs).
    Workspace,
    /// The logs pane (the quick-eval field).
    Logs,
}

impl Pane {
    /// Every focus group, in F6 order.
    pub const ALL: [Pane; 5] = [
        Pane::Toolbar,
        Pane::Editor,
        Pane::Plot,
        Pane::Workspace,
        Pane::Logs,
    ];

    /// The group's visible name.
    pub fn label(self) -> &'static str {
        match self {
            Pane::Toolbar => "toolbar",
            Pane::Editor => "editor",
            Pane::Plot => "plot",
            Pane::Workspace => "workspace",
            Pane::Logs => "logs",
        }
    }

    /// The next group, wrapping after logs.
    pub fn next(self) -> Pane {
        let index = Pane::ALL.iter().position(|pane| *pane == self).unwrap_or(0);
        Pane::ALL[(index + 1) % Pane::ALL.len()]
    }

    /// The previous group, wrapping before the toolbar.
    pub fn previous(self) -> Pane {
        let index = Pane::ALL.iter().position(|pane| *pane == self).unwrap_or(0);
        Pane::ALL[(index + Pane::ALL.len() - 1) % Pane::ALL.len()]
    }
}

/// One command a registered binding requests.
///
/// The variants the window's dedicated paths already execute (`Workbench`, `File`, `Editor`)
/// stay as they are; the rest are the pane and help actions A11Y-001 added. `Undo`/`Redo` are
/// dispatched by the editor through [`crate::history::command_for_shortcut`], which the table's
/// tests cross-check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Run or cancel the active document ([`WorkbenchCommand`]).
    Workbench(WorkbenchCommand),
    /// A file-lifecycle command ([`FileCommand`]).
    File(FileCommand),
    /// An editor navigation command ([`EditorCommand`]).
    Editor(EditorCommand),
    /// Move focus to the next focus group.
    NextPane,
    /// Move focus to the previous focus group.
    PreviousPane,
    /// Move focus to one focus group directly.
    FocusPane(Pane),
    /// Show the workspace's structured Result view.
    WorkspaceResultView,
    /// Show the workspace's Inspector view.
    WorkspaceInspectorView,
    /// Move focus to the workspace's filter field.
    WorkspaceFilterFocus,
    /// Clear the committed variables, keeping functions.
    ClearVariables,
    /// Fit the whole plot in the pane.
    PlotFit,
    /// Plot at natural size.
    PlotActualSize,
    /// Zoom the plot in one step.
    PlotZoomIn,
    /// Zoom the plot out one step.
    PlotZoomOut,
    /// Rasterise the plot to a PNG beside the engine's capture, without a dialog.
    PlotExportPng,
    /// Move focus to the quick-eval field.
    QuickEvalFocus,
    /// Open the editor settings window.
    EditorSettings,
    /// Open the Help > Keyboard Shortcuts dialog.
    ShortcutsHelp,
    /// Switch between ASCII and Unicode mathematical glyph rendering.
    ToggleUnicodeGlyphs,
    /// Switch between the dark and light theme at runtime.
    ToggleTheme,
    /// Quit, asking about every unsaved document.
    Quit,
    /// Jump the caret to the last failed run's diagnostic.
    JumpToDiagnostic,
    /// Undo the active document's last edit (dispatched by the editor).
    Undo,
    /// Redo the last undone edit (dispatched by the editor).
    Redo,
    /// Open the completion popup at the caret.
    Completion,
}

impl Action {
    /// The action's stable name, for logs and for the reference.
    pub fn label(self) -> &'static str {
        match self {
            Action::Workbench(command) => command.label(),
            Action::File(command) => command.label(),
            Action::Editor(command) => command.label(),
            Action::NextPane => "Next pane",
            Action::PreviousPane => "Previous pane",
            Action::FocusPane(pane) => match pane {
                Pane::Toolbar => "Focus toolbar",
                Pane::Editor => "Focus editor",
                Pane::Plot => "Focus plot",
                Pane::Workspace => "Focus workspace",
                Pane::Logs => "Focus logs",
            },
            Action::WorkspaceResultView => "Result view",
            Action::WorkspaceInspectorView => "Inspector view",
            Action::WorkspaceFilterFocus => "Focus filter",
            Action::ClearVariables => "Clear variables",
            Action::PlotFit => "Fit plot",
            Action::PlotActualSize => "Plot 100%",
            Action::PlotZoomIn => "Zoom in",
            Action::PlotZoomOut => "Zoom out",
            Action::PlotExportPng => "Export plot PNG",
            Action::QuickEvalFocus => "Focus quick eval",
            Action::EditorSettings => "Editor settings",
            Action::ShortcutsHelp => "Keyboard shortcuts",
            Action::ToggleUnicodeGlyphs => "Toggle Unicode glyphs",
            Action::ToggleTheme => "Toggle theme",
            Action::Quit => "Quit",
            Action::JumpToDiagnostic => "Jump to diagnostic",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::Completion => "Completion",
        }
    }
}

/// Where a binding is listed in the reference, and which part of the window executes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// A command that works wherever keyboard focus is: file, run, panes, plot, help.
    Global,
    /// An editor command: navigation, completion and undo/redo.
    Editor,
}

impl Scope {
    /// The section heading the reference shows.
    pub fn label(self) -> &'static str {
        match self {
            Scope::Global => "Global",
            Scope::Editor => "Editor",
        }
    }

    /// Every scope, in reference order.
    pub const ALL: [Scope; 2] = [Scope::Global, Scope::Editor];
}

/// One row of the binding table.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// The action the chord requests.
    pub action: Action,
    /// The chord.
    pub keys: Shortcut,
    /// The reference section it is listed under.
    pub scope: Scope,
    /// The one-sentence description the reference shows and the hover text repeats.
    pub description: &'static str,
}

const fn binding(
    action: Action,
    keys: Shortcut,
    scope: Scope,
    description: &'static str,
) -> Binding {
    Binding {
        action,
        keys,
        scope,
        description,
    }
}

use Action::{
    ClearVariables, Completion, EditorSettings, FocusPane, JumpToDiagnostic, NextPane, PlotActualSize,
    PlotExportPng, PlotFit, PlotZoomIn, PlotZoomOut, PreviousPane, QuickEvalFocus, Quit,
    ShortcutsHelp, ToggleTheme, ToggleUnicodeGlyphs, WorkspaceFilterFocus, WorkspaceInspectorView,
    WorkspaceResultView,
};
use KeyName::{Char, Comma, Digit, Enter, Escape, F1, F6, F8, Minus, Plus, Space, Tab};
use Scope::{Editor, Global};

/// The app's complete keyboard registration.
///
/// This is the only list. The dispatcher, the legacy mappers and the Help > Keyboard Shortcuts
/// dialog all read it; no other module holds a shortcut string.
pub const BINDINGS: &[Binding] = &[
    // ---- global: run, cancel and the file lifecycle ---------------------------------------
    binding(
        Action::Workbench(WorkbenchCommand::Run),
        Shortcut::new(Enter, true, false, false),
        Global,
        "Run the active document on a worker thread",
    ),
    binding(
        Action::Workbench(WorkbenchCommand::Cancel),
        Shortcut::plain(Escape),
        Global,
        "Cancel the running document, or close the open prompt",
    ),
    binding(
        Action::File(FileCommand::New),
        Shortcut::new(Char('n'), true, false, false),
        Global,
        "Open a new, empty document tab",
    ),
    binding(
        Action::File(FileCommand::Open),
        Shortcut::new(Char('o'), true, false, false),
        Global,
        "Open a Lovelace script from disk",
    ),
    binding(
        Action::File(FileCommand::Save),
        Shortcut::new(Char('s'), true, false, false),
        Global,
        "Save the active document",
    ),
    binding(
        Action::File(FileCommand::SaveAs),
        Shortcut::new(Char('s'), true, true, false),
        Global,
        "Save the active document under a new name",
    ),
    binding(
        Action::File(FileCommand::CloseTab),
        Shortcut::new(Char('w'), true, false, false),
        Global,
        "Close the active tab; asks when it has unsaved changes",
    ),
    binding(
        Quit,
        Shortcut::new(Char('q'), true, false, false),
        Global,
        "Quit; asks about every unsaved document",
    ),
    // ---- global: focus movement -----------------------------------------------------------
    binding(
        NextPane,
        Shortcut::plain(F6),
        Global,
        "Move keyboard focus to the next pane group",
    ),
    binding(
        PreviousPane,
        Shortcut::new(F6, false, true, false),
        Global,
        "Move keyboard focus to the previous pane group",
    ),
    binding(
        FocusPane(Pane::Toolbar),
        Shortcut::new(Digit(1), true, false, false),
        Global,
        "Move keyboard focus to the toolbar",
    ),
    binding(
        FocusPane(Pane::Editor),
        Shortcut::new(Digit(2), true, false, false),
        Global,
        "Move keyboard focus to the editor",
    ),
    binding(
        FocusPane(Pane::Plot),
        Shortcut::new(Digit(3), true, false, false),
        Global,
        "Move keyboard focus to the plot pane",
    ),
    binding(
        FocusPane(Pane::Workspace),
        Shortcut::new(Digit(4), true, false, false),
        Global,
        "Move keyboard focus to the workspace pane",
    ),
    binding(
        FocusPane(Pane::Logs),
        Shortcut::new(Digit(5), true, false, false),
        Global,
        "Move keyboard focus to the logs pane",
    ),
    // ---- global: workspace and plot pane actions ------------------------------------------
    binding(
        WorkspaceResultView,
        Shortcut::new(Digit(1), true, false, true),
        Global,
        "Show the workspace Result view",
    ),
    binding(
        WorkspaceInspectorView,
        Shortcut::new(Digit(2), true, false, true),
        Global,
        "Show the workspace Inspector view",
    ),
    binding(
        WorkspaceFilterFocus,
        Shortcut::new(Char('f'), true, false, true),
        Global,
        "Move keyboard focus to the workspace filter",
    ),
    binding(
        ClearVariables,
        Shortcut::new(Char('c'), true, false, true),
        Global,
        "Clear the committed variables; function definitions are kept",
    ),
    binding(
        PlotFit,
        Shortcut::new(Digit(0), true, false, false),
        Global,
        "Fit the whole plot in its pane",
    ),
    binding(
        PlotActualSize,
        Shortcut::new(Digit(0), true, true, false),
        Global,
        "Show the plot at natural size (100%)",
    ),
    binding(
        PlotZoomIn,
        Shortcut::new(Plus, true, false, false),
        Global,
        "Zoom the plot in one step",
    ),
    binding(
        PlotZoomOut,
        Shortcut::new(Minus, true, false, false),
        Global,
        "Zoom the plot out one step",
    ),
    binding(
        PlotExportPng,
        Shortcut::new(Char('p'), true, true, false),
        Global,
        "Export the plot as a PNG beside the engine's SVG capture, without a dialog",
    ),
    // ---- global: tools, help and appearance ------------------------------------------------
    binding(
        QuickEvalFocus,
        Shortcut::new(Char('e'), true, true, false),
        Global,
        "Move keyboard focus to the quick-eval field",
    ),
    binding(
        EditorSettings,
        Shortcut::new(Comma, true, false, false),
        Global,
        "Open the editor settings (indent, tabs, auto-close, glyphs)",
    ),
    binding(
        ShortcutsHelp,
        Shortcut::plain(F1),
        Global,
        "Open Help > Keyboard Shortcuts (this list)",
    ),
    binding(
        ToggleUnicodeGlyphs,
        Shortcut::new(Char('u'), true, true, false),
        Global,
        "Switch between ASCII and Unicode mathematical glyph rendering",
    ),
    binding(
        ToggleTheme,
        Shortcut::new(Char('t'), true, true, false),
        Global,
        "Switch between the dark and light theme at runtime",
    ),
    binding(
        JumpToDiagnostic,
        Shortcut::plain(F8),
        Global,
        "Jump the caret to the last failed run's diagnostic",
    ),
    // ---- editor: navigation ----------------------------------------------------------------
    binding(
        Action::Editor(EditorCommand::GoToLine),
        Shortcut::new(Char('g'), true, false, false),
        Editor,
        "Go to a 1-based line, or line:column",
    ),
    binding(
        Action::Editor(EditorCommand::GoToMatchingBracket),
        Shortcut::new(Char('m'), true, false, false),
        Editor,
        "Jump to the bracket matching the caret's",
    ),
    binding(
        Action::Editor(EditorCommand::FindSymbol),
        Shortcut::new(Char('o'), true, true, false),
        Editor,
        "Filter the document's identifiers and jump to one",
    ),
    binding(
        Action::Editor(EditorCommand::Find),
        Shortcut::new(Char('f'), true, false, false),
        Editor,
        "Find and replace in the active document",
    ),
    binding(
        Action::Editor(EditorCommand::NextTab),
        Shortcut::new(Tab, true, false, false),
        Editor,
        "Select the next document tab",
    ),
    binding(
        Action::Editor(EditorCommand::PreviousTab),
        Shortcut::new(Tab, true, true, false),
        Editor,
        "Select the previous document tab",
    ),
    // ---- editor: completion and history ----------------------------------------------------
    binding(
        Completion,
        Shortcut::new(Space, true, false, false),
        Editor,
        "Open the completion popup at the caret",
    ),
    binding(
        Action::Undo,
        Shortcut::new(Char('z'), true, false, false),
        Editor,
        "Undo the active document's last edit",
    ),
    binding(
        Action::Redo,
        Shortcut::new(Char('y'), true, false, false),
        Editor,
        "Redo the last undone edit",
    ),
    binding(
        Action::Redo,
        Shortcut::new(Char('z'), true, true, false),
        Editor,
        "Redo the last undone edit",
    ),
];

/// The binding registered for exactly this chord, if any.
///
/// The match is exact: an extra or missing modifier is a different chord.
pub fn binding_for(shortcut: Shortcut) -> Option<&'static Binding> {
    BINDINGS.iter().find(|binding| binding.keys == shortcut)
}

/// The first binding registered for an action.
pub fn binding_record(action: Action) -> Option<&'static Binding> {
    BINDINGS.iter().find(|binding| binding.action == action)
}

/// The chord registered for an action, as the reference shows it.
pub fn chord_of(action: Action) -> String {
    binding_record(action)
        .map(|binding| binding.keys.chord())
        .unwrap_or_else(|| "unbound".to_string())
}

/// The action a chord requests, if the table registers it.
pub fn action_for(shortcut: Shortcut) -> Option<Action> {
    binding_for(shortcut).map(|binding| binding.action)
}

/// One row of the Help > Keyboard Shortcuts reference.
///
/// [`reference_rows`] builds these by projecting [`BINDINGS`], so the reference cannot list a
/// shortcut the app does not register and cannot omit one it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceRow {
    /// The section the row belongs to.
    pub scope: Scope,
    /// The chord, e.g. `Ctrl+Shift+S`.
    pub chord: String,
    /// The one-sentence description.
    pub description: &'static str,
}

/// Every row the Help > Keyboard Shortcuts dialog shows.
pub fn reference_rows() -> Vec<ReferenceRow> {
    BINDINGS
        .iter()
        .map(|binding| ReferenceRow {
            scope: binding.scope,
            chord: binding.keys.chord(),
            description: binding.description,
        })
        .collect()
}

/// A [`WorkbenchCommand`] as the table registers it.
fn registered_workbench(action: Action) -> Option<WorkbenchCommand> {
    match binding_record(action)?.action {
        Action::Workbench(command) => Some(command),
        _ => None,
    }
}

/// A [`FileCommand`] as the table registers it, when the chord matches.
fn registered_file(shortcut: Shortcut) -> Option<FileCommand> {
    match binding_for(shortcut)?.action {
        Action::File(command) => Some(command),
        _ => None,
    }
}

/// An [`EditorCommand`] as the table registers it, when the chord matches.
fn registered_editor(shortcut: Shortcut) -> Option<EditorCommand> {
    match binding_for(shortcut)?.action {
        Action::Editor(command) => Some(command),
        _ => None,
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
    let key = match key {
        '\t' => KeyName::Tab,
        ' ' => KeyName::Space,
        other => KeyName::Char(other.to_ascii_lowercase()),
    };
    // Resolved through the single binding table, so the reference the user reads and the mapping
    // this function executes are the same registration.
    registered_editor(Shortcut::new(key, true, shift, false))
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
    // Resolved through the single binding table (DOC-002's shortcut table now lives there).
    registered_file(Shortcut::new(
        KeyName::Char(letter.to_ascii_lowercase()),
        true,
        shift,
        false,
    ))
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

    // ---- A11Y-001 / A11Y-007: the binding table and its reference ---------------------------

    #[test]
    fn the_reference_lists_exactly_the_registered_bindings() {
        let rows = reference_rows();
        assert_eq!(
            rows.len(),
            BINDINGS.len(),
            "the Help reference and the registration must have the same size"
        );
        for (row, binding) in rows.iter().zip(BINDINGS) {
            assert_eq!(row.scope, binding.scope, "row {row:?}");
            assert_eq!(
                row.chord,
                binding.keys.chord(),
                "the reference chord must be the registered chord"
            );
            assert_eq!(row.description, binding.description);
            assert!(!row.chord.is_empty());
            assert!(!row.description.is_empty());
        }
        // No chord is registered twice, so `binding_for` is unambiguous.
        let mut seen = std::collections::HashSet::new();
        for binding in BINDINGS {
            assert!(
                seen.insert(binding.keys),
                "duplicate chord {}",
                binding.keys.chord()
            );
        }
        // Both sections the requirement names are present: global commands and editor commands.
        for scope in Scope::ALL {
            assert!(
                rows.iter().any(|row| row.scope == scope),
                "the reference has no {} section",
                scope.label()
            );
        }
    }

    #[test]
    fn every_registered_binding_is_reachable_through_the_dispatcher() {
        for binding in BINDINGS {
            assert_eq!(
                binding_for(binding.keys).map(|found| found.action),
                Some(binding.action),
                "{} is registered but not dispatched",
                binding.keys.chord()
            );
            assert_eq!(action_for(binding.keys), Some(binding.action));
            assert!(!binding.action.label().is_empty());
        }
    }

    #[test]
    fn every_command_enum_value_is_registered() {
        for command in WorkbenchCommand::ALL {
            assert!(
                binding_record(Action::Workbench(command)).is_some(),
                "{} is not in the binding table",
                command.label()
            );
        }
        for command in FileCommand::ALL {
            assert!(
                binding_record(Action::File(command)).is_some(),
                "{} is not in the binding table",
                command.label()
            );
        }
        for command in EditorCommand::ALL {
            assert!(
                binding_record(Action::Editor(command)).is_some(),
                "{} is not in the binding table",
                command.label()
            );
        }
    }

    #[test]
    fn the_legacy_mappers_resolve_through_the_table() {
        for binding in BINDINGS {
            match binding.action {
                Action::File(command) => {
                    let KeyName::Char(letter) = binding.keys.key else {
                        panic!("a file binding must be a letter: {binding:?}");
                    };
                    assert_eq!(
                        file_command_for_shortcut(letter, true, binding.keys.shift),
                        Some(command)
                    );
                    assert_eq!(
                        file_command_for_shortcut(letter, false, binding.keys.shift),
                        None,
                        "a file command must require the modifier"
                    );
                }
                Action::Editor(command) => {
                    let key = match binding.keys.key {
                        KeyName::Tab => '\t',
                        KeyName::Space => ' ',
                        KeyName::Char(letter) => letter,
                        _ => continue,
                    };
                    assert_eq!(
                        editor_command_for_shortcut(key, true, binding.keys.shift),
                        Some(command),
                        "{} must map to {}",
                        binding.keys.chord(),
                        command.label()
                    );
                    assert_eq!(
                        editor_command_for_shortcut(key, false, binding.keys.shift),
                        None
                    );
                }
                Action::Workbench(command) => {
                    let mapped = match binding.keys {
                        Shortcut {
                            key: KeyName::Enter,
                            ctrl: true,
                            ..
                        } => command_for_shortcut(true, false, true),
                        Shortcut {
                            key: KeyName::Escape,
                            ctrl: false,
                            ..
                        } => command_for_shortcut(false, true, false),
                        _ => continue,
                    };
                    assert_eq!(mapped, Some(command));
                }
                Action::Undo | Action::Redo => {
                    let (key, shift) = match binding.keys.key {
                        KeyName::Char(letter) => (letter, binding.keys.shift),
                        _ => panic!("a history binding must be a letter: {binding:?}"),
                    };
                    let mapped = crate::history::command_for_shortcut(
                        key == 'z',
                        key == 'y',
                        shift,
                        binding.keys.ctrl,
                    );
                    let expected = match binding.action {
                        Action::Undo => crate::history::HistoryCommand::Undo,
                        Action::Redo => crate::history::HistoryCommand::Redo,
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        mapped,
                        Some(expected),
                        "{} must reach {:?} through the editor's own mapping",
                        binding.keys.chord(),
                        expected
                    );
                }
                Action::NextPane
                | Action::PreviousPane
                | Action::FocusPane(_)
                | Action::WorkspaceResultView
                | Action::WorkspaceInspectorView
                | Action::WorkspaceFilterFocus
                | Action::ClearVariables
                | Action::PlotFit
                | Action::PlotActualSize
                | Action::PlotZoomIn
                | Action::PlotZoomOut
                | Action::PlotExportPng
                | Action::QuickEvalFocus
                | Action::EditorSettings
                | Action::ShortcutsHelp
                | Action::ToggleUnicodeGlyphs
                | Action::ToggleTheme
                | Action::Quit
                | Action::JumpToDiagnostic
                | Action::Completion => {}
            }
        }
    }

    #[test]
    fn pane_cycle_wraps_in_f6_order() {
        assert_eq!(
            Pane::ALL,
            [
                Pane::Toolbar,
                Pane::Editor,
                Pane::Plot,
                Pane::Workspace,
                Pane::Logs
            ]
        );
        assert_eq!(Pane::Editor.next(), Pane::Plot);
        assert_eq!(Pane::Logs.next(), Pane::Toolbar, "F6 wraps");
        assert_eq!(Pane::Toolbar.previous(), Pane::Logs, "Shift+F6 wraps");
        assert_eq!(Pane::Plot.previous(), Pane::Editor);
        for pane in Pane::ALL {
            assert_eq!(pane.next().previous(), pane);
            assert!(!pane.label().is_empty());
        }
    }

    #[test]
    fn chords_render_as_the_reference_and_the_hover_text_show_them() {
        assert_eq!(
            binding_for(Shortcut::new(KeyName::Char('s'), true, true, false))
                .expect("Save As")
                .keys
                .chord(),
            "Ctrl+Shift+S"
        );
        assert_eq!(Shortcut::plain(KeyName::F6).chord(), "F6");
        assert_eq!(Shortcut::new(KeyName::F6, false, true, false).chord(), "Shift+F6");
        assert_eq!(Shortcut::new(KeyName::Digit(3), true, false, false).chord(), "Ctrl+3");
        assert_eq!(Shortcut::new(KeyName::Comma, true, false, false).chord(), "Ctrl+,");
        assert_eq!(Shortcut::new(KeyName::Plus, true, false, false).chord(), "Ctrl+=");
        assert_eq!(Shortcut::plain(KeyName::Escape).chord(), "Esc");
        assert_eq!(Shortcut::new(KeyName::Tab, true, false, false).chord(), "Ctrl+Tab");
        assert_eq!(
            Shortcut::new(KeyName::Space, true, false, false).chord(),
            "Ctrl+Space"
        );
        assert_eq!(
            Shortcut::new(KeyName::Digit(1), true, false, true).chord(),
            "Ctrl+Alt+1"
        );
    }

    #[test]
    fn the_new_pane_actions_are_registered_with_their_chords() {
        let cases = [
            (Action::NextPane, "F6"),
            (Action::PreviousPane, "Shift+F6"),
            (Action::FocusPane(Pane::Workspace), "Ctrl+4"),
            (Action::WorkspaceInspectorView, "Ctrl+Alt+2"),
            (Action::WorkspaceFilterFocus, "Ctrl+Alt+F"),
            (Action::ClearVariables, "Ctrl+Alt+C"),
            (Action::PlotFit, "Ctrl+0"),
            (Action::PlotActualSize, "Ctrl+Shift+0"),
            (Action::PlotZoomIn, "Ctrl+="),
            (Action::PlotZoomOut, "Ctrl+-"),
            (Action::PlotExportPng, "Ctrl+Shift+P"),
            (Action::ShortcutsHelp, "F1"),
            (Action::ToggleUnicodeGlyphs, "Ctrl+Shift+U"),
            (Action::ToggleTheme, "Ctrl+Shift+T"),
            (Action::Quit, "Ctrl+Q"),
        ];
        for (action, chord) in cases {
            assert_eq!(chord_of(action), chord, "{}", action.label());
            assert_eq!(
                action_for(binding_record(action).expect("registered").keys),
                Some(action)
            );
        }
    }
}
