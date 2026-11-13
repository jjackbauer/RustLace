//! The native workbench window: toolbar, tab strip, four panes and the status strip.
//!
//! The window is a thin view over [`crate::workbench::Workbench`]. It never evaluates a script:
//! Run only submits to the `ExecutionHost`, every frame polls the typed event stream, and every
//! value in the panes comes from the snapshot or the engine's plot capture. Panes with nothing
//! to show render their empty state instead of sample data.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, CursorIcon, FontId, Id, Key, Layout, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::engine::FunctionOrigin;
use crate::execution::{Diagnostic, DocumentId};
use crate::glyphs::GlyphMode;
use crate::inspector::{Inspection, InspectorExpansion, Section, TREE_ROOT_PATH, tree_path};
use crate::keyboard::{self, Action, FileCommand, KeyName, Pane, Shortcut};
use crate::log_kind::LogKind;
use crate::quick_eval::QuickEval;
use crate::recovery::{self, RecoveryJournal, RecoveryRecord, RecoveryScan};
use crate::results_tree::{
    ArrayNode, ExpansionState, ResultProjection, StructuredNode, element_path, field_path,
};
use crate::settings::{self, MAX_PRECISION, MIN_PRECISION, Settings};
use crate::workbench::{DocumentView, SaveOutcome, Workbench};
use crate::workspace_table::{
    FunctionColumn, FunctionTable, VariableColumn, VariableTable, cell_text, ordered_functions,
    ordered_variables,
};

use super::a11y;
use super::editor::{TextEditor, surface_id};
use super::file_dialogs::{self, FileDialogs, NativeFileDialogs};
use super::plot::{self, Plot, PngBackground};
use super::theme;
use super::typography;

/// Width of a splitter, in pixels.
pub const SPLIT: f32 = 6.0;
const MIN_EDITOR: f32 = 320.0;
const MIN_PLOT: f32 = 240.0;
const MIN_WORKSPACE: f32 = 180.0;
const MIN_LOGS: f32 = 100.0;
const MIN_TOP: f32 = 240.0;

/// Rectangles of every pane and splitter for one pass.
#[derive(Clone, Copy, Debug)]
pub struct PaneLayout {
    /// Editor pane.
    pub editor: Rect,
    /// Plot pane.
    pub plot: Rect,
    /// Workspace pane.
    pub workspace: Rect,
    /// Logs pane.
    pub logs: Rect,
    /// Editor / plot splitter.
    pub split_a: Rect,
    /// Plot / workspace splitter.
    pub split_b: Rect,
    /// Top / logs splitter.
    pub split_c: Rect,
}

/// Computes the four-pane layout from the saved fractions, honouring minimum sizes whenever the
/// window is large enough and compressing the minimums proportionally when it is not.
pub fn compute_layout(
    full: Rect,
    editor_frac: f32,
    workspace_frac: f32,
    logs_frac: f32,
) -> PaneLayout {
    let usable_w = (full.width() - 2.0 * SPLIT).max(0.0);
    let min_sum = MIN_EDITOR + MIN_PLOT + MIN_WORKSPACE;
    let shrink_w = if usable_w >= min_sum {
        1.0
    } else {
        usable_w / min_sum
    };
    let min_editor = MIN_EDITOR * shrink_w;
    let min_plot = MIN_PLOT * shrink_w;
    let min_workspace = MIN_WORKSPACE * shrink_w;

    let max_editor = (usable_w - min_plot - min_workspace).max(min_editor);
    let editor_w = (usable_w * editor_frac).clamp(min_editor, max_editor);
    let remaining = (usable_w - editor_w).max(0.0);
    let max_workspace = (remaining - min_plot).max(min_workspace);
    let workspace_w = (usable_w * workspace_frac).clamp(min_workspace, max_workspace);
    let plot_w = (remaining - workspace_w).max(0.0);

    let usable_h = (full.height() - SPLIT).max(0.0);
    let height_min_sum = MIN_TOP + MIN_LOGS;
    let shrink_h = if usable_h >= height_min_sum {
        1.0
    } else {
        usable_h / height_min_sum
    };
    let min_top = MIN_TOP * shrink_h;
    let min_logs = MIN_LOGS * shrink_h;
    let max_logs = (usable_h - min_top).max(min_logs);
    let logs_h = (full.height() * logs_frac).clamp(min_logs, max_logs);
    let top_h = (usable_h - logs_h).max(0.0);

    let top_rect = Rect::from_min_size(full.min, vec2(full.width(), top_h));
    let editor = Rect::from_min_size(top_rect.min, vec2(editor_w, top_h));
    let split_a = Rect::from_min_size(pos2(editor.right(), top_rect.top()), vec2(SPLIT, top_h));
    let plot = Rect::from_min_size(pos2(split_a.right(), top_rect.top()), vec2(plot_w, top_h));
    let split_b = Rect::from_min_size(pos2(plot.right(), top_rect.top()), vec2(SPLIT, top_h));
    let workspace = Rect::from_min_size(
        pos2(split_b.right(), top_rect.top()),
        vec2(workspace_w, top_h),
    );
    let split_c = Rect::from_min_size(
        pos2(full.left(), top_rect.bottom()),
        vec2(full.width(), SPLIT),
    );
    let logs = Rect::from_min_size(
        pos2(full.left(), split_c.bottom()),
        vec2(full.width(), logs_h),
    );

    PaneLayout {
        editor,
        plot,
        workspace,
        logs,
        split_a,
        split_b,
        split_c,
    }
}

/// The in-window PNG export options (PLT-006): a chosen scale and background.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PngExport {
    /// Pixels per SVG point.
    scale: f32,
    /// Opaque white or transparent.
    background: PngBackground,
}

impl Default for PngExport {
    fn default() -> PngExport {
        PngExport {
            scale: 2.0,
            background: PngBackground::Opaque,
        }
    }
}

/// EDT-010: the in-window go-to-line prompt state.
#[derive(Clone, Debug, Default)]
struct GoToLinePrompt {
    /// What the user has typed (`line` or `line:column`, 1-based).
    text: String,
    /// The last rejection, shown under the field until the next submission.
    error: Option<String>,
    /// True until the field has been given keyboard focus.
    focus: bool,
}

/// EDT-010: the in-window find-symbol prompt state.
#[derive(Clone, Debug, Default)]
struct SymbolPrompt {
    /// The case-insensitive filter over the document's identifiers.
    query: String,
    /// Index into the current filtered list of the highlighted entry.
    selected: usize,
    /// True until the field has been given keyboard focus.
    focus: bool,
}

/// What the user is answering when the unsaved-changes prompt is open (DOC-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnsavedAnswer {
    /// Write the document, then continue closing.
    Save,
    /// Close without writing; the edits are lost.
    Discard,
    /// Leave everything exactly as it was: the tab stays open, the buffer and the caret unchanged.
    Cancel,
}

impl UnsavedAnswer {
    /// The button label, which is also the prompt's accessible name in a UIA capture.
    fn label(self) -> &'static str {
        match self {
            UnsavedAnswer::Save => "Save",
            UnsavedAnswer::Discard => "Discard",
            UnsavedAnswer::Cancel => "Cancel",
        }
    }
}

/// What closing is being decided for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseTarget {
    /// One tab, identified by its stable document identity.
    Tab(DocumentId),
    /// The whole window (the title-bar close button, Quit and the OS close request).
    Window,
}

/// The pending Save/Discard/Cancel question (DOC-003).
///
/// Documents are identified by [`DocumentId`], not by tab index, so a tab change during the flow
/// cannot make the prompt name the wrong document.
#[derive(Clone, Debug)]
struct ClosePrompt {
    /// What is being closed once every question is answered.
    target: CloseTarget,
    /// Documents with unsaved changes still to ask about, in tab order.
    pending: Vec<DocumentId>,
    /// How many documents the flow started with, so the prompt can say "1 of 2".
    total: usize,
    /// The widget that had focus when the prompt opened; restored on Cancel.
    focus_before: Option<Id>,
    /// Why the last Save in the prompt failed, shown in the prompt itself.
    error: Option<String>,
}

impl ClosePrompt {
    /// The document this prompt is asking about.
    fn current(&self) -> DocumentId {
        self.pending[0]
    }
}

/// Where the window persists its state (DOC-004) and journals unsaved buffers (DOC-005).
///
/// The shipped window resolves both paths from the application directory
/// (`%APPDATA%\Lovelace` on Windows); the window tests supply scratch paths through
/// [`WorkbenchApp::with_storage`], so no test touches the user profile and no product code writes
/// beside the executable or in the process working directory (PLF-006).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AppStorage {
    /// The settings file, or `None` when this window must not persist anything.
    settings_path: Option<PathBuf>,
    /// The recovery directory, or `None` when this window must not journal anything.
    recovery_root: Option<PathBuf>,
}

impl AppStorage {
    /// The paths the shipped window uses.
    fn from_application_dir() -> AppStorage {
        AppStorage {
            settings_path: settings::default_config_path(),
            recovery_root: settings::default_recovery_dir(),
        }
    }
}

/// What pressing a button in the DOC-005 recovery prompt means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryAnswer {
    /// Put the record's text back into a tab; the file on disk is not touched.
    Recover,
    /// Delete the record without restoring it.
    Discard,
}

impl RecoveryAnswer {
    /// The button label, which is also the button's accessible name in a UIA capture.
    fn label(self) -> &'static str {
        match self {
            RecoveryAnswer::Recover => "Recover",
            RecoveryAnswer::Discard => "Discard",
        }
    }
}

/// The recovery prompt shown at launch when journaled buffers survived (DOC-005).
///
/// It is answered record by record. While it is open no keystroke reaches an editor and no pane
/// accepts a pointer, so the journaled text is offered before any save can overwrite a file.
#[derive(Clone, Debug)]
struct RecoveryPrompt {
    /// The records still to answer, oldest document identity first.
    records: Vec<RecoveryRecord>,
    /// Why the last answer failed, shown in the prompt itself.
    error: Option<String>,
    /// The widget that had focus when the prompt opened; restored when it is dismissed.
    focus_before: Option<Id>,
}

/// The window application.
pub struct WorkbenchApp {
    workbench: Workbench,
    /// DOC-004: the settings value this run loaded, updated as the session changes.
    settings: Settings,
    /// VIS-002: the theme this window paints. Persisted in `settings.theme`; dark by default.
    theme: theme::ThemeId,
    /// VIS-002: the theme whose egui styles were last written into the context.
    applied_theme: Option<theme::ThemeId>,
    /// VIS-002 test seam: the toolbar theme toggle's last painted rectangle. UIA reaches the same
    /// button by its accessible name.
    theme_button: Option<Rect>,
    /// VIS-002: the widget that owned keyboard focus when this pass began, before any toolbar
    /// interaction could move it. The theme toggle restores it, so switching theme never costs
    /// the user their place in the editor.
    focus_before_interaction: Option<Id>,
    /// VIS-002: the most recent pass that began with a focused widget. A pointer press can clear
    /// egui's focus before the click frame runs, so the toggle restores this instead of `None`.
    last_stable_focus: Option<Id>,
    /// DOC-004: where the settings file and the recovery directory live.
    storage: AppStorage,
    /// DOC-005: the debounced writer that keeps one recovery record per dirty document.
    recovery: RecoveryJournal,
    /// DOC-005: the launch recovery prompt, while one is open.
    recovery_prompt: Option<RecoveryPrompt>,
    /// DOC-005 test seam: the recovery prompt buttons painted in the last pass.
    recovery_buttons: Vec<(usize, RecoveryAnswer, Rect)>,
    /// DOC-004: the session signature last written, so an unchanged session is not rewritten.
    persisted_session: Option<String>,
    /// The last settings write failure, so it is logged once instead of every frame.
    settings_error: Option<String>,
    /// The one assembled state this frame renders (RUN-003).
    ///
    /// Rebuilt in [`WorkbenchApp::sync`] immediately after `poll()`, so every pane draws the
    /// same committed snapshot, log, revision and diagnostic even though it is a separate egui
    /// closure. No pane calls back into the workbench for pane data.
    view: DocumentView,
    editors: Vec<TextEditor>,
    /// The identity of the document each editor holds, parallel to `editors`.
    ///
    /// DOC-002: New, Open and Close change the tab list, so the editors are reconciled by document
    /// identity instead of by position. A surviving tab keeps its editor (caret, selection,
    /// history); a new tab gets a fresh one.
    editor_ids: Vec<DocumentId>,
    loaded_revisions: Vec<u64>,
    /// DOC-002: the native dialogs. A headless test replaces this with a scripted answer.
    dialogs: Box<dyn FileDialogs>,
    /// DOC-003: the open Save/Discard/Cancel question, if any.
    close: Option<ClosePrompt>,
    /// True once the close flow approved closing the window, so the OS request that follows is
    /// allowed instead of starting the flow again.
    window_close_approved: bool,
    /// The title last sent to the native window, so it is only sent when it changes.
    window_title: String,
    /// A widget to focus on the next pass (DOC-003's focus restoration).
    ///
    /// A focus request made in the pass that dismisses the modal cannot stick: the modal's layer
    /// is still the top layer while the panes paint, so the editor is not interactable in that
    /// pass. Queueing the id for the next pass is what makes Cancel leave the focus where it was.
    pending_focus: Option<Id>,
    /// DOC-003 test seam: the prompt buttons painted in the last pass and their rectangles.
    prompt_buttons: Vec<(UnsavedAnswer, Rect)>,
    /// DOC-002 test seam: the File bar buttons painted in the last pass and their rectangles.
    file_buttons: Vec<(&'static str, Rect)>,
    /// EDT-009 test seam: the find bar buttons painted in the last pass and their rectangles.
    find_buttons: Vec<(&'static str, Rect)>,
    plot: Plot,
    loaded_plot_svg: Option<String>,
    /// PLT-006: the open PNG export options window, if any.
    plot_export: Option<PngExport>,
    /// PLT-006: the last export's short result and whether it is an error, shown in the plot
    /// header so the action has feedback even when the file dialog closes without a trace.
    plot_export_status: Option<(String, bool)>,
    /// EDT-010: the open go-to-line prompt, if any.
    goto_line: Option<GoToLinePrompt>,
    /// EDT-010: the open find-symbol prompt, if any.
    find_symbol: Option<SymbolPrompt>,
    /// EDT-008: true while the editor-settings window is open.
    editor_settings_open: bool,
    /// EDT-009: true until the find bar's find field has taken keyboard focus.
    find_focus: bool,
    editor_frac: f32,
    workspace_frac: f32,
    logs_frac: f32,
    filter: String,
    /// The two Workspace tables, rebuilt from the frame's committed snapshot in [`Self::sync`].
    variables_table: VariableTable,
    functions_table: FunctionTable,
    /// RES-004: the RESULT tree's per-path collapse state. It is bound to the committed result
    /// in `paint_result_section`, so repaints of the same value keep every decision and a new
    /// result clears the map.
    expansion: ExpansionState,
    /// SYM-001: true while the Workspace pane's Inspector view is selected. The view is a tab
    /// beside the Result view; it renders only the committed snapshot's inspection payload.
    inspector_open: bool,
    /// SYM-001: the expression tree's per-path collapse state, scoped to the inspection payload.
    inspector_expansion: InspectorExpansion,
    /// SYM-001: expression-tree headers painted in the last pass (path and rectangle), the same
    /// test seam as `result_toggle_rects`.
    inspector_toggle_rects: Vec<(String, Rect)>,
    /// SYM-001: the Result and Inspector tab rectangles painted in the last pass. The pointer
    /// tests click these centres; UIA reaches the same buttons by their names.
    workspace_view_tabs: [Option<Rect>; 2],
    /// RESULT tree headers painted in the last pass: their node path and rectangle. The pointer
    /// tests click these centres, the same seam as `variable_header_rects`.
    result_toggle_rects: Vec<(String, Rect)>,
    /// Header rectangles from the last painted pass; the pointer tests click these centres and
    /// the pane uses them to decide which column a click belongs to.
    variable_header_rects: [Option<Rect>; 3],
    function_header_rects: [Option<Rect>; 3],
    /// The one-line quick-eval input's text and submitted-expression history.
    quick_eval: QuickEval,
    /// The quick-eval field's last painted rectangle, used by the pointer-focus test.
    quick_eval_rect: Option<Rect>,
    /// RUN-019: the toolbar's precision text field, its last validation error and the document
    /// precision it was last synchronised from.
    ///
    /// The text is kept instead of a numeric spin box so an invalid value can be rejected with
    /// guidance. `precision_seen` is what makes RUN-020 work: when a script's `setprecision(n)`
    /// changes the document precision, the next frame notices it differs from `precision_seen`
    /// and rewrites the field, so the control always shows the active precision.
    precision_input: String,
    precision_error: Option<String>,
    precision_seen: usize,
    layout: Option<PaneLayout>,
    /// A11Y-001: the focus group the keyboard is in, for F6 / Ctrl+1..5 movement.
    focused_pane: Pane,
    /// A11Y-001: a focus request waiting for its pane to paint its first control.
    pane_focus: Option<Pane>,
    /// A11Y-001: the Run button's widget id from the last painted pass, so the toolbar group can
    /// be focused without a pointer.
    run_button_id: Option<Id>,
    /// A11Y-001: the plot pane's first control (Fit), focused by the plot group.
    plot_fit_id: Option<Id>,
    /// A11Y-001: the workspace Result/Inspector tab ids and the filter field id, so the workspace
    /// group and Ctrl+Alt+F can focus them.
    workspace_tab_ids: [Option<Id>; 2],
    workspace_filter_id: Option<Id>,
    /// A11Y-007: true while Help > Keyboard Shortcuts is open.
    shortcuts_open: bool,
    /// A11Y-007: true on the pass that opens the dialog, so its Close button takes focus.
    shortcuts_focus: bool,
    /// A11Y-001: Ctrl+Alt+F requests the workspace filter on the pass it is painted.
    filter_focus: bool,
    /// A11Y-014: the ASCII/Unicode notation in force this frame, mirrored from the settings.
    glyph_mode: GlyphMode,
    /// A11Y-014: font coverage per character, probed once from the shipped font stack.
    glyph_coverage: std::collections::HashMap<char, bool>,
    /// True once the window has moved keyboard focus into the editor on its first frame.
    focus_requested: bool,
    /// True between handling Ctrl+Enter and the end of that frame (RUN-001 trace only).
    trace_pending: bool,
    /// True until the frame that follows the painted Running frame starts (trace only).
    trace_after_paint: bool,
}

impl WorkbenchApp {
    /// Creates the window over a fresh workbench, optionally opening a script path in tab one.
    ///
    /// DOC-004/005: the launch reads the settings file, restores the last session's tabs (by
    /// reading their files again, never by executing them) and offers any recovery records the
    /// previous run left behind.
    pub fn new(initial_path: Option<PathBuf>) -> WorkbenchApp {
        WorkbenchApp::launch(AppStorage::from_application_dir(), initial_path)
    }

    /// Launches over an explicit storage location (the test seam for DOC-004/005).
    fn launch(storage: AppStorage, initial_path: Option<PathBuf>) -> WorkbenchApp {
        let mut settings = match &storage.settings_path {
            Some(path) => Settings::load(path),
            None => Settings::new(),
        };
        let settings_warnings = settings.take_warnings();
        let mut workbench = Workbench::new();
        // DOC-004: a recent path whose file was deleted between two launches is pruned before the
        // File bar can offer it.
        let before_recent = settings.recent_files.len();
        let pruned = settings.prune_missing_recent();
        workbench.set_recent_files(settings.recent_files.clone());
        settings.recent_files = workbench.recent_files().to_vec();

        // DOC-005: whatever an interrupted run journaled is read before anything else, so the
        // prompt can offer it and no save can slip in first.
        let scan = match &storage.recovery_root {
            Some(root) => recovery::scan(root),
            None => RecoveryScan::default(),
        };
        let mut consumed: Vec<u64> = Vec::new();
        if let Some(session) = settings.session.clone() {
            let report = workbench.restore_session(&session, &scan.records);
            consumed = report.recovered;
        }
        for warning in settings_warnings {
            workbench.log_active(LogKind::Diagnostic, format!("Settings: {warning}"));
        }
        if pruned > 0 {
            workbench.log_active(
                LogKind::Info,
                format!("Pruned {pruned} recent file(s) that no longer exist."),
            );
        }
        let _ = before_recent;
        for warning in &scan.warnings {
            workbench.log_active(LogKind::Diagnostic, format!("Recovery: {warning}"));
        }
        // A path named on the command line wins over the restored session: it is selected when the
        // session already had it, otherwise it opens in its own tab.
        if let Some(path) = &initial_path {
            match workbench
                .documents()
                .iter()
                .position(|document| document.path.as_deref() == Some(path.as_path()))
            {
                Some(index) => workbench.set_active(index),
                None => {
                    if let Err(message) = workbench.open(path) {
                        workbench.log_active(LogKind::Diagnostic, message);
                    }
                }
            }
        }

        let pending: Vec<RecoveryRecord> = scan
            .records
            .iter()
            .filter(|record| !consumed.contains(&record.document_id))
            .cloned()
            .collect();

        let mut app = WorkbenchApp::with_storage(workbench, storage);
        app.settings = settings;
        // VIS-002: the persisted theme wins over the dark default; the first frame applies it.
        app.theme = app.settings.theme;
        // A11Y-014: the persisted ASCII/Unicode choice, applied from the first frame.
        app.glyph_mode = GlyphMode::from_unicode(app.settings.unicode_glyphs);
        app.apply_editor_settings();
        // A record the session restore consumed is exactly the text now in its tab, so the journal
        // keeps tracking it instead of rewriting it on the first frame; it is deleted when the
        // document is saved, closed or the window exits cleanly.
        for document_id in consumed {
            if let Some(index) = app.workbench.document_index(DocumentId(document_id)) {
                let revision = app.workbench.documents()[index].revision;
                app.recovery.track(document_id, revision, Instant::now());
            }
        }
        if !pending.is_empty() {
            app.recovery_prompt = Some(RecoveryPrompt {
                records: pending,
                error: None,
                focus_before: None,
            });
        }
        // The first frame writes the settings file, so the session is current from the moment the
        // window appears: a crash later in the run restores the tabs that were actually open.
        app.persisted_session = None;
        app
    }

    /// Creates the window over an existing workbench (the test seam).
    #[cfg_attr(not(test), allow(dead_code))]
    fn with_workbench(workbench: Workbench) -> WorkbenchApp {
        WorkbenchApp::with_storage(workbench, AppStorage::default())
    }

    /// Creates the window over an existing workbench and an explicit storage location.
    fn with_storage(workbench: Workbench, storage: AppStorage) -> WorkbenchApp {
        let editors = workbench
            .documents()
            .iter()
            .map(|document| {
                // DOC-005: a restored buffer comes back with the caret and selection the journal
                // recorded, so recovery does not merely restore the text.
                let mut editor =
                    TextEditor::from_source_with_newline(&document.source, document.newline);
                editor.set_caret(document.caret.0, document.caret.1);
                editor.selection = document.selection.map(|(anchor, _end)| anchor);
                editor
            })
            .collect();
        let editor_ids = workbench.documents().iter().map(|doc| doc.id).collect();
        let loaded_revisions = workbench
            .documents()
            .iter()
            .map(|document| document.revision)
            .collect();
        let view = workbench.active_view();
        let variables_table = VariableTable::new(&view.state);
        let functions_table = FunctionTable::new(&view.state);
        let workbench_precision = workbench.active_document().precision;
        let recovery = match &storage.recovery_root {
            Some(root) => RecoveryJournal::new(root.clone()),
            None => RecoveryJournal::disabled(),
        };
        WorkbenchApp {
            workbench,
            settings: Settings::new(),
            theme: theme::ThemeId::Dark,
            applied_theme: None,
            theme_button: None,
            focus_before_interaction: None,
            last_stable_focus: None,
            storage,
            recovery,
            recovery_prompt: None,
            recovery_buttons: Vec::new(),
            persisted_session: None,
            settings_error: None,
            view,
            editors,
            editor_ids,
            loaded_revisions,
            dialogs: Box::new(NativeFileDialogs),
            close: None,
            window_close_approved: false,
            window_title: String::new(),
            pending_focus: None,
            prompt_buttons: Vec::new(),
            file_buttons: Vec::new(),
            find_buttons: Vec::new(),
            plot: Plot::new(),
            loaded_plot_svg: None,
            plot_export: None,
            plot_export_status: None,
            goto_line: None,
            find_symbol: None,
            editor_settings_open: false,
            find_focus: false,
            editor_frac: 0.40,
            workspace_frac: 0.20,
            logs_frac: 0.18,
            filter: String::new(),
            variables_table,
            functions_table,
            expansion: ExpansionState::new(),
            inspector_open: false,
            inspector_expansion: InspectorExpansion::new(),
            inspector_toggle_rects: Vec::new(),
            workspace_view_tabs: [None; 2],
            result_toggle_rects: Vec::new(),
            variable_header_rects: [None; 3],
            function_header_rects: [None; 3],
            quick_eval: QuickEval::new(),
            quick_eval_rect: None,
            precision_input: workbench_precision.to_string(),
            precision_error: None,
            precision_seen: workbench_precision,
            layout: None,
            // A11Y-001: the editor owns the keyboard at launch, as it always has.
            focused_pane: Pane::Editor,
            pane_focus: None,
            run_button_id: None,
            plot_fit_id: None,
            workspace_tab_ids: [None; 2],
            workspace_filter_id: None,
            shortcuts_open: false,
            shortcuts_focus: false,
            filter_focus: false,
            // A11Y-014: `launch` re-reads this from the loaded settings; the constructor's
            // default is the settings default (ASCII, the engine's own notation).
            glyph_mode: GlyphMode::Ascii,
            glyph_coverage: std::collections::HashMap::new(),
            focus_requested: false,
            trace_pending: false,
            trace_after_paint: false,
        }
    }

    /// A11Y-014: localizes the frame's one view in the active ASCII/Unicode notation.
    ///
    /// The font probe is the shipped font stack (via [`a11y::font_has_glyph`]) and its answers
    /// are cached for the session, so a character is measured once and every later frame reuses
    /// the answer. A glyph the font lacks is replaced by its documented fallback in the same
    /// call, which is why no pane can ever paint a tofu box.
    fn localize_glyphs(&mut self, ctx: &egui::Context) {
        self.glyph_mode = GlyphMode::from_unicode(self.settings.unicode_glyphs);
        let mode = self.glyph_mode;
        let coverage = &mut self.glyph_coverage;
        let view = &mut self.view;
        let mut probe = |c: char| -> bool {
            if let Some(known) = coverage.get(&c) {
                return *known;
            }
            let known = a11y::font_has_glyph(ctx, c);
            coverage.insert(c, known);
            known
        };
        a11y::localize_view(view, mode, &mut probe);
    }

    /// Applies queued events and re-loads the editors whose document changed elsewhere.
    fn sync(&mut self, ctx: &egui::Context) {
        self.workbench.poll();
        // DOC-002: New, Open and Close change the tab list; keep one editor per open document.
        self.sync_editors();
        // DOC-005: the caret and selection belong to the editor, so they are copied into the
        // document before the journal pass that writes them into the recovery record.
        for (index, editor) in self.editors.iter().enumerate() {
            let selection = editor
                .selection
                .map(|anchor| (anchor, (editor.caret_row, editor.caret_col)));
            self.workbench
                .set_caret(index, (editor.caret_row, editor.caret_col), selection);
        }
        // DOC-005: journal every dirty document, at most once per debounce window. A write failure
        // is a diagnostic line, never a crash and never a lost buffer. Once the window close has
        // been approved nothing may journal or persist again: the clean exit has already recorded
        // the final state on disk.
        if !self.window_close_approved {
            let report = self
                .recovery
                .sync(self.workbench.documents(), Instant::now());
            for failure in report.failed {
                self.workbench
                    .log_active(LogKind::Diagnostic, format!("Recovery journal: {failure}"));
            }
            // DOC-004: a changed tab list, active tab, recent list or precision is persisted at
            // once, so a crash leaves a current session rather than the one from the last exit.
            self.persist_state(false);
        }
        // RUN-003: assemble the frame's pane state once, from the same committed snapshot the
        // poll above applied. Every pane below renders this value, not a fresh workbench lookup.
        self.view = self.workbench.active_view();
        // A11Y-014: the ASCII/Unicode setting is applied to that one view here, before any pane
        // or table reads it, so the result, the workspace, the logs and the inspector cannot
        // disagree about the same value (and a missing glyph takes its documented fallback).
        self.localize_glyphs(ctx);
        // RUN-020: a `setprecision(n)` statement moves the document precision; when it differs
        // from the value the control was last synchronised from, the control is rewritten. The
        // user's in-progress text is only replaced when the *document* actually changed, so an
        // invalid entry is not silently overwritten on the next frame.
        if self.view.precision != self.precision_seen {
            self.precision_seen = self.view.precision;
            self.precision_input = self.view.precision.to_string();
            self.precision_error = None;
        }
        // WKS-001/WKS-002: the tables are rebuilt from the same committed capture the frame
        // renders, keeping the user's chosen sort across runs and tab switches.
        self.variables_table.rebuild(&self.view.state);
        self.functions_table.rebuild(&self.view.state);
        // The workbench gates a diagnostic on the revision it was produced for; copying it here
        // makes every editor decoration appear with the failure and disappear on the next edit.
        for (index, editor) in self.editors.iter_mut().enumerate() {
            editor.diagnostic = self.workbench.diagnostic(index).cloned();
        }
        let current_svg = self.view.plot.as_ref().map(|plot| plot.svg.clone());
        if current_svg != self.loaded_plot_svg {
            match current_svg {
                Some(svg) => {
                    self.plot.set_svg(&svg);
                    self.loaded_plot_svg = Some(svg);
                }
                None => {
                    self.plot.clear();
                    self.loaded_plot_svg = None;
                }
            }
        }
    }

    /// Keeps `editors`, `loaded_revisions` and `editor_ids` in step with the workbench's tabs.
    ///
    /// A tab that is still open keeps its editor object, so its caret, selection and undo history
    /// survive another tab opening or closing. A document whose revision advanced elsewhere (a
    /// file load, a reload) gets its text and newline style re-loaded; the fresh tab of a document
    /// created since the last frame gets a new editor at the start of its text.
    fn sync_editors(&mut self) {
        let preferences = self.settings.editor;
        let ids: Vec<DocumentId> = self
            .workbench
            .documents()
            .iter()
            .map(|document| document.id)
            .collect();
        if self.editor_ids != ids {
            let mut previous: Vec<(DocumentId, TextEditor, u64)> = self
                .editor_ids
                .iter()
                .copied()
                .zip(self.editors.drain(..))
                .zip(self.loaded_revisions.drain(..))
                .map(|((id, editor), revision)| (id, editor, revision))
                .collect();
            let mut editors = Vec::with_capacity(ids.len());
            let mut revisions = Vec::with_capacity(ids.len());
            for (index, id) in ids.iter().enumerate() {
                let document = &self.workbench.documents()[index];
                let (caret, selection) = (document.caret, document.selection);
                match previous.iter().position(|(existing, _, _)| existing == id) {
                    Some(position) => {
                        let (_, mut editor, loaded) = previous.remove(position);
                        if loaded != document.revision {
                            editor.load_with_newline(&document.source, document.newline);
                        }
                        // DOC-005: a loaded or recovered buffer puts the journaled caret back.
                        editor.set_caret(caret.0, caret.1);
                        editor.selection = selection.map(|(anchor, _end)| anchor);
                        editor.preferences = preferences;
                        editors.push(editor);
                        revisions.push(document.revision);
                    }
                    None => {
                        let mut editor = TextEditor::from_source_with_newline(
                            &document.source,
                            document.newline,
                        );
                        editor.set_caret(caret.0, caret.1);
                        editor.selection = selection.map(|(anchor, _end)| anchor);
                        editor.preferences = preferences;
                        editors.push(editor);
                        revisions.push(document.revision);
                    }
                }
            }
            self.editors = editors;
            self.loaded_revisions = revisions;
            self.editor_ids = ids;
            return;
        }
        let active = self.workbench.active();
        let revision = self.workbench.documents()[active].revision;
        if self.loaded_revisions[active] != revision {
            let document = &self.workbench.documents()[active];
            let (source, newline) = (document.source.clone(), document.newline);
            let (caret, selection) = (document.caret, document.selection);
            self.editors[active].load_with_newline(&source, newline);
            self.editors[active].set_caret(caret.0, caret.1);
            self.editors[active].selection = selection.map(|(anchor, _end)| anchor);
            self.loaded_revisions[active] = revision;
        }
    }

    // -----------------------------------------------------------------------------------------
    // DOC-004 / DOC-005: the persisted session, the recovery journal and the recovery prompt.
    // -----------------------------------------------------------------------------------------

    /// A string that changes exactly when something the settings file stores changes.
    fn session_signature(&self) -> String {
        let session = self.workbench.session_state();
        let recent = self.workbench.recent_files();
        let precision = self.workbench.active_document().precision;
        format!("{session:?}|{recent:?}|{precision}|{:?}", self.settings.editor)
    }

    /// EDT-008: copies the persisted editor settings into every open editor.
    ///
    /// The settings are the single source of truth for indentation width, tabs-vs-spaces and
    /// auto-close; a change in the settings window applies to every tab in the same frame.
    fn apply_editor_settings(&mut self) {
        let preferences = self.settings.editor;
        for editor in &mut self.editors {
            editor.preferences = preferences;
        }
    }

    /// Writes the session, the recent list and the active precision when they changed.
    ///
    /// `force` is used at a clean exit. A failure is logged once and never blocks the window: a
    /// read-only or missing application directory must not make the editor unusable.
    fn persist_state(&mut self, force: bool) {
        let Some(path) = self.storage.settings_path.clone() else {
            return;
        };
        let signature = self.session_signature();
        if !force && self.persisted_session.as_deref() == Some(signature.as_str()) {
            return;
        }
        self.settings.set_session(self.workbench.session_state());
        self.settings.recent_files = self.workbench.recent_files().to_vec();
        self.settings.active_precision = self.workbench.active_document().precision;
        match self.settings.save(&path) {
            Ok(()) => {
                self.persisted_session = Some(signature);
                self.settings_error = None;
            }
            Err(error) => {
                let message = format!("Cannot save settings to {}: {error}", path.display());
                if self.settings_error.as_deref() != Some(message.as_str()) {
                    self.workbench
                        .log_active(LogKind::Diagnostic, message.clone());
                    self.settings_error = Some(message);
                }
            }
        }
    }

    /// DOC-005: deletes the records of every open document at a clean exit.
    ///
    /// Called only once the window is actually closing, after every dirty document has been saved
    /// or discarded, so nothing recoverable is left behind by a normal quit.
    fn finish_clean_exit(&mut self) {
        self.persist_state(true);
        let report = self.recovery.clean_exit(self.workbench.documents());
        for failure in report.failed {
            self.workbench
                .log_active(LogKind::Diagnostic, format!("Recovery journal: {failure}"));
        }
    }

    /// DOC-005: puts one record's text back into a tab.
    ///
    /// A record whose document is already open (the session restore usually recreated the tab from
    /// disk) rewrites that tab's buffer; otherwise the text arrives in a new tab. Either way the
    /// document is dirty against what is on disk, nothing is executed and nothing is written.
    fn recover_record(&mut self, record: RecoveryRecord, ctx: &egui::Context) {
        let report = self.workbench.recover(&record);
        if report.document_id.0 == record.document_id {
            // The record file is now exactly the text in the tab, so the journal tracks it instead
            // of writing it again on this frame.
            let revision = self.workbench.documents()[report.index].revision;
            self.recovery
                .track(record.document_id, revision, Instant::now());
        } else if let Some(message) = self.recovery.forget(record.document_id) {
            // Recovered into a different tab identity: the old record is superseded. The new tab
            // is dirty and untracked, so the next journal pass writes its record immediately.
            self.workbench
                .log_active(LogKind::Diagnostic, format!("Recovery journal: {message}"));
        }
        self.remove_recovery_record(record.document_id);
        self.sync(ctx);
        self.pending_focus = Some(surface_id());
    }

    /// DOC-005: deletes one record without restoring it.
    fn discard_record(&mut self, record: RecoveryRecord) {
        if let Some(message) = self.recovery.forget(record.document_id) {
            self.workbench
                .log_active(LogKind::Diagnostic, format!("Recovery journal: {message}"));
        }
        self.remove_recovery_record(record.document_id);
        self.pending_focus = Some(surface_id());
    }

    /// Removes one answered record from the prompt, closing the prompt when none are left.
    fn remove_recovery_record(&mut self, document_id: u64) {
        let Some(prompt) = self.recovery_prompt.as_mut() else {
            return;
        };
        prompt
            .records
            .retain(|record| record.document_id != document_id);
        if prompt.records.is_empty() {
            if let Some(prompt) = self.recovery_prompt.take() {
                self.pending_focus = Some(prompt.focus_before.unwrap_or_else(surface_id));
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // DOC-002: the file lifecycle. New / Open / Save / Save As / Close all go through the
    // workbench, which owns the document state and the bytes; the window only asks the native
    // dialogs for a path and renders the result.
    // -----------------------------------------------------------------------------------------

    /// Ctrl+N / File > New: add an empty document tab and select it.
    fn new_document(&mut self) {
        self.workbench.new_document();
    }

    /// Ctrl+O / File > Open: ask the native Open dialog, then open the chosen file in a tab.
    fn open_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.dialogs.open() else {
            return;
        };
        // A refusal (a non-UTF-8 file, for example) is logged by the workbench with its reason and
        // leaves every tab alone; the editor keeps the focus it already had.
        if let Ok(index) = self.workbench.open(&path) {
            self.sync_after_tab_change(index, ctx);
        }
    }

    /// Ctrl+S / File > Save, or Ctrl+Shift+S / File > Save As.
    ///
    /// `ask_path` forces the Save As dialog; Save asks only when the document has no path yet.
    /// A cancelled dialog changes nothing at all.
    fn save_active(&mut self, ask_path: bool) {
        let active = self.workbench.active();
        let _ = self.write_document_through_dialogs(active, ask_path);
    }

    /// Writes one document, asking the native Save As dialog when needed.
    ///
    /// Returns `None` when the user cancelled the dialog (nothing was written and the document is
    /// unchanged), otherwise the workbench's outcome.
    fn write_document_through_dialogs(
        &mut self,
        index: usize,
        ask_path: bool,
    ) -> Option<SaveOutcome> {
        let (has_path, suggested, initial_dir) = {
            let document = self.workbench.document(index)?;
            (
                document.path.is_some(),
                file_dialogs::suggested_name(&document.title),
                document
                    .path
                    .as_ref()
                    .and_then(|path| path.parent().map(PathBuf::from)),
            )
        };
        if !ask_path && has_path {
            return Some(self.workbench.save(index));
        }
        let path = self.dialogs.save_as(&suggested, initial_dir.as_deref())?;
        Some(self.workbench.save_as(index, path))
    }

    /// Makes `index` the active tab and reconciles the editors immediately, before any pane in
    /// this frame indexes them.
    fn sync_after_tab_change(&mut self, index: usize, ctx: &egui::Context) {
        self.workbench.set_active(index);
        self.sync(ctx);
    }

    // -----------------------------------------------------------------------------------------
    // DOC-003: the unsaved-changes prompt.
    // -----------------------------------------------------------------------------------------

    /// Starts closing `target`, or closes it immediately when nothing in it is dirty.
    fn request_close(&mut self, target: CloseTarget, ctx: &egui::Context) {
        // The DOC-005 recovery prompt is answered before any close: a quit must not race the
        // question of whether journaled text should be recovered.
        if self.close.is_some() || self.recovery_prompt.is_some() {
            return;
        }
        let pending: Vec<DocumentId> = match target {
            CloseTarget::Tab(id) => match self.workbench.document_index(id) {
                Some(index) if self.workbench.documents()[index].dirty() => vec![id],
                Some(_) => Vec::new(),
                None => return,
            },
            CloseTarget::Window => self.workbench.dirty_documents(),
        };
        if pending.is_empty() {
            self.perform_close(target, ctx);
            return;
        }
        let total = pending.len();
        let focus_before = ctx.memory(|memory| memory.focused());
        self.close = Some(ClosePrompt {
            target,
            pending,
            total,
            focus_before,
            error: None,
        });
    }

    /// Carries out a close that has been decided: a tab is removed, the window asks the OS to
    /// close and the next close request is allowed through.
    fn perform_close(&mut self, target: CloseTarget, ctx: &egui::Context) {
        match target {
            CloseTarget::Tab(id) => {
                // DOC-005: a tab that is closed (saved or discarded) no longer has unsaved work,
                // so its recovery record must not outlive it.
                if let Some(message) = self.recovery.forget(id.0) {
                    self.workbench
                        .log_active(LogKind::Diagnostic, format!("Recovery journal: {message}"));
                }
                self.workbench.close_document(id);
                // The tab list changed under this frame's editors; reconcile now so no pane draws
                // a document that is gone.
                self.sync(ctx);
                self.pending_focus = Some(surface_id());
            }
            CloseTarget::Window => {
                // DOC-004/005: the last session is written and the journal's records of the open
                // documents are deleted; the quit is only approved once both have happened.
                self.finish_clean_exit();
                self.window_close_approved = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Applies one answer to the prompt.
    ///
    /// Cancel is deliberately the cheapest branch: it drops the prompt, restores the focus that
    /// was taken when it opened, and touches no editor, no buffer and no caret.
    fn answer_close(&mut self, answer: UnsavedAnswer, ctx: &egui::Context) {
        let Some(prompt) = self.close.clone() else {
            return;
        };
        match answer {
            UnsavedAnswer::Cancel => {
                self.close = None;
                // Restored on the next pass, when the modal's layer no longer blocks the editor.
                self.pending_focus = Some(prompt.focus_before.unwrap_or_else(surface_id));
            }
            UnsavedAnswer::Discard => self.advance_close(ctx),
            UnsavedAnswer::Save => {
                let Some(index) = self.workbench.document_index(prompt.current()) else {
                    self.advance_close(ctx);
                    return;
                };
                match self.write_document_through_dialogs(index, false) {
                    Some(SaveOutcome::Saved { .. }) => self.advance_close(ctx),
                    Some(SaveOutcome::Failed(message)) => {
                        // The write failed: keep asking, and say why in the prompt.
                        if let Some(prompt) = self.close.as_mut() {
                            prompt.error = Some(message);
                        }
                    }
                    Some(SaveOutcome::NeedsPath) => {
                        // Unreachable: the dialog supplies a path before this outcome can occur.
                        if let Some(prompt) = self.close.as_mut() {
                            prompt.error = Some("This document has no path yet.".to_string());
                        }
                    }
                    None => {
                        // The Save As dialog was cancelled. The prompt stays open so the user can
                        // still choose Discard or Cancel.
                    }
                }
            }
        }
    }

    /// Removes the answered document from the queue and closes when nothing is left to ask about.
    fn advance_close(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.close.as_mut() else {
            return;
        };
        prompt.pending.remove(0);
        prompt.error = None;
        if prompt.pending.is_empty() {
            let target = prompt.target;
            self.close = None;
            self.perform_close(target, ctx);
        }
    }

    /// Interprets the OS close request (title-bar X, Alt+F4).
    ///
    /// A dirty workbench refuses the close for this frame and asks instead; nothing is written and
    /// nothing is discarded until the prompt is answered. A clean workbench lets the close through.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.window_close_approved {
            return;
        }
        // DOC-005: the recovery question is answered first; the window stays open until it is.
        if self.recovery_prompt.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return;
        }
        if self.close.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return;
        }
        if self.workbench.dirty_documents().is_empty() {
            self.finish_clean_exit();
            self.window_close_approved = true;
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.request_close(CloseTarget::Window, ctx);
    }

    /// Paints the prompt and applies the answer.
    ///
    /// The caller reads Escape/Enter before this runs and strips every key and text event from the
    /// frame, so no keystroke can reach an editor while the question is open. The modal is drawn in
    /// egui's foreground layer, so it is painted above the panes even though it is built first.
    fn close_prompt_ui(&mut self, ctx: &egui::Context, escape: bool, enter: bool) {
        self.prompt_buttons.clear();
        let Some(prompt) = self.close.clone() else {
            return;
        };
        let Some(index) = self.workbench.document_index(prompt.current()) else {
            // The document disappeared while the prompt was up; there is nothing to save.
            self.close = None;
            return;
        };
        let title = self.workbench.documents()[index].title.clone();
        let total = prompt.total;
        let remaining = prompt.pending.len();
        let error = prompt.error.clone();
        let mut answer: Option<UnsavedAnswer> = None;
        let modal = egui::Modal::new(egui::Id::unique("unsaved-changes-prompt")).show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.label(
                RichText::new("Unsaved changes")
                    .strong()
                    .size(16.0)
                    .color(theme::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Save changes to {title} before closing?"))
                    .color(theme::text())
                    .size(14.0),
            );
            if total > 1 {
                ui.label(
                    RichText::new(format!(
                        "Document {} of {} with unsaved changes.",
                        total - remaining + 1,
                        total
                    ))
                    .small()
                    .color(theme::dim()),
                );
            }
            ui.label(
                RichText::new("If you discard, what you changed since the last save is lost.")
                    .small()
                    .color(theme::dim()),
            );
            if let Some(error) = &error {
                ui.add_space(2.0);
                ui.label(RichText::new(error).small().color(theme::error()));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for choice in [
                    UnsavedAnswer::Save,
                    UnsavedAnswer::Discard,
                    UnsavedAnswer::Cancel,
                ] {
                    let text = RichText::new(choice.label()).size(14.0);
                    let text = if choice == UnsavedAnswer::Save {
                        text.strong()
                    } else {
                        text
                    };
                    let response = ui.add(egui::Button::new(text).min_size(vec2(96.0, 26.0)));
                    if response.clicked() {
                        answer = Some(choice);
                    }
                    self.prompt_buttons.push((choice, response.rect));
                }
            });
        });
        if answer.is_none() {
            if escape {
                answer = Some(UnsavedAnswer::Cancel);
            } else if enter {
                answer = Some(UnsavedAnswer::Save);
            } else if modal.should_close() {
                answer = Some(UnsavedAnswer::Cancel);
            }
        }
        if let Some(answer) = answer {
            self.answer_close(answer, ctx);
        }
    }

    /// Paints the DOC-005 recovery prompt and applies the answer.
    ///
    /// Like the close prompt it is a foreground modal and the caller strips every key and text
    /// event, so the journaled text is decided before a save can overwrite anything. Escape (or a
    /// click outside) keeps the question for the next launch instead of deleting the records.
    fn recovery_prompt_ui(&mut self, ctx: &egui::Context, escape: bool) {
        self.recovery_buttons.clear();
        let Some(prompt) = self.recovery_prompt.clone() else {
            return;
        };
        let mut answer: Option<(usize, RecoveryAnswer)> = None;
        let modal = egui::Modal::new(egui::Id::unique("recovery-prompt")).show(ctx, |ui| {
            ui.set_max_width(760.0);
            ui.label(
                RichText::new("Recover unsaved work")
                    .strong()
                    .size(16.0)
                    .color(theme::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The last session did not close cleanly. These buffers were journaled before \
                     it ended:",
                )
                .color(theme::text())
                .size(14.0),
            );
            ui.label(
                RichText::new(
                    "Recovering puts the text back into a tab before anything is saved, and runs \
                     nothing. Discarding deletes the record.",
                )
                .small()
                .color(theme::dim()),
            );
            if let Some(error) = &prompt.error {
                ui.add_space(2.0);
                ui.label(RichText::new(error).small().color(theme::error()));
            }
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for (row, record) in prompt.records.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&record.title).monospace().color(theme::accent()));
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {} bytes",
                                    record.path_label(),
                                    record.buffer_bytes()
                                ))
                                .small()
                                .color(theme::dim()),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                for choice in [RecoveryAnswer::Discard, RecoveryAnswer::Recover] {
                                    let text = RichText::new(choice.label()).size(14.0);
                                    let text = if choice == RecoveryAnswer::Recover {
                                        text.strong()
                                    } else {
                                        text
                                    };
                                    let response =
                                        ui.add(egui::Button::new(text).min_size(vec2(96.0, 26.0)));
                                    if response.clicked() {
                                        answer = Some((row, choice));
                                    }
                                    self.recovery_buttons.push((row, choice, response.rect));
                                }
                            });
                        });
                        ui.separator();
                    }
                });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Escape keeps these records for the next launch.")
                    .small()
                    .color(theme::dim()),
            );
        });
        if answer.is_none() && (escape || modal.should_close()) {
            // Keep everything: the records stay on disk and are offered again next launch.
            self.recovery_prompt = None;
            self.pending_focus = Some(prompt.focus_before.unwrap_or_else(surface_id));
            return;
        }
        if let Some((row, choice)) = answer {
            let Some(record) = prompt.records.get(row).cloned() else {
                return;
            };
            match choice {
                RecoveryAnswer::Recover => self.recover_record(record, ctx),
                RecoveryAnswer::Discard => self.discard_record(record),
            }
        }
    }

    /// Sends the active document's name and dirty marker to the native window title.
    fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = format!(
            "Lovelace Desktop - {}",
            self.workbench.active_document().title_with_marker()
        );
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Reads this frame's file shortcut, if any (DOC-002's Ctrl+N/O/S, Ctrl+Shift+S, Ctrl+W).
    fn file_shortcut(&self, ui: &Ui) -> Option<FileCommand> {
        ui.input(|input| {
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    if !(modifiers.ctrl || modifiers.command) || modifiers.alt {
                        continue;
                    }
                    let letter = match key {
                        Key::N => 'n',
                        Key::O => 'o',
                        Key::S => 's',
                        Key::W => 'w',
                        _ => continue,
                    };
                    if let Some(command) =
                        keyboard::file_command_for_shortcut(letter, true, modifiers.shift)
                    {
                        return Some(command);
                    }
                }
            }
            None
        })
    }

    /// Executes one file command through the same entry points the buttons use.
    fn apply_file_command(&mut self, command: FileCommand, ctx: &egui::Context) {
        // Trace mode only: the file command's stable name, beside the run trace marks.
        crate::run_trace::mark("file_command", command.label());
        match command {
            FileCommand::New => self.new_document(),
            FileCommand::Open => self.open_dialog(ctx),
            FileCommand::Save => self.save_active(false),
            FileCommand::SaveAs => self.save_active(true),
            FileCommand::CloseTab => {
                let id = self.workbench.active_document().id;
                self.request_close(CloseTarget::Tab(id), ctx);
            }
        }
    }

    /// Executes one EDT-010 editor navigation command.
    ///
    /// Tab navigation changes the workbench's active document (the editors are reconciled by
    /// `sync`), the bracket command acts on the active editor, and the two prompts are opened as
    /// keyboard-focused in-window fields. Every path returns keyboard focus to the editor when it
    /// completes, so the whole sequence is mouse-free.
    fn apply_editor_command(&mut self, command: keyboard::EditorCommand, ctx: &egui::Context) {
        crate::run_trace::mark("editor_command", command.label());
        match command {
            keyboard::EditorCommand::NextTab => {
                self.workbench.select_next_tab();
                self.focus_editor(ctx);
            }
            keyboard::EditorCommand::PreviousTab => {
                self.workbench.select_previous_tab();
                self.focus_editor(ctx);
            }
            keyboard::EditorCommand::GoToMatchingBracket => {
                let active = self.workbench.active();
                if self.editors[active].jump_to_matching_bracket() {
                    self.focus_editor(ctx);
                }
            }
            keyboard::EditorCommand::GoToLine => {
                self.find_symbol = None;
                self.goto_line = Some(GoToLinePrompt {
                    text: String::new(),
                    error: None,
                    focus: true,
                });
            }
            keyboard::EditorCommand::FindSymbol => {
                self.goto_line = None;
                self.find_symbol = Some(SymbolPrompt {
                    query: String::new(),
                    selected: 0,
                    focus: true,
                });
            }
            keyboard::EditorCommand::Find => {
                // EDT-009: Ctrl+F opens the docked find bar over the active document and focuses
                // its find field. The bar edits the same `FindSession` the renderer highlights,
                // so the first typed character is already highlighted.
                self.goto_line = None;
                self.find_symbol = None;
                let active = self.workbench.active();
                self.editors[active].find_open();
                self.find_focus = true;
            }
        }
    }

    /// Requests keyboard focus for the editor surface on the next pass.
    fn focus_editor(&mut self, ctx: &egui::Context) {
        for editor in &mut self.editors {
            editor.completion.dismiss();
        }
        ctx.memory_mut(|memory| memory.request_focus(surface_id()));
    }

    // ---------------------------------------------------------------------------------------
    // A11Y-001: pane focus and the table-driven dispatcher.
    // ---------------------------------------------------------------------------------------

    /// The focus group the keyboard is actually in, derived from egui's focus owner.
    ///
    /// The window stores which group the user last moved to, but a pointer click or Tab can move
    /// focus inside a group at any time, so F6 continues from the widget that really owns the
    /// keyboard rather than from a stale label.
    fn current_pane(&self, ctx: &egui::Context) -> Pane {
        let Some(id) = ctx.memory(|memory| memory.focused()) else {
            return self.focused_pane;
        };
        if id == surface_id() {
            return Pane::Editor;
        }
        if Some(id) == self.run_button_id {
            return Pane::Toolbar;
        }
        if Some(id) == self.plot_fit_id {
            return Pane::Plot;
        }
        if self.workspace_tab_ids.contains(&Some(id)) || Some(id) == self.workspace_filter_id {
            return Pane::Workspace;
        }
        if id == quick_eval_id() {
            return Pane::Logs;
        }
        self.focused_pane
    }

    /// Moves keyboard focus into one focus group's first control.
    ///
    /// The id is normally known from the last painted pass; on the very first pass it is not, so
    /// `pane_focus` waits for the pane to paint and requests focus then. That is what makes a
    /// keyboard journey start from a fresh window without a pointer.
    fn request_pane_focus(&mut self, pane: Pane, ctx: &egui::Context) {
        self.focused_pane = pane;
        let known = match pane {
            Pane::Toolbar => self.run_button_id,
            Pane::Editor => Some(surface_id()),
            Pane::Plot => self.plot_fit_id,
            Pane::Workspace => self.workspace_tab_ids[0],
            Pane::Logs => Some(quick_eval_id()),
        };
        match known {
            Some(id) => {
                self.pane_focus = None;
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
            None => self.pane_focus = Some(pane),
        }
    }

    /// F6 / Shift+F6: the next (or previous) focus group, wrapping.
    fn move_pane_focus(&mut self, forward: bool, ctx: &egui::Context) {
        let current = self.current_pane(ctx);
        let next = if forward {
            current.next()
        } else {
            current.previous()
        };
        self.request_pane_focus(next, ctx);
    }

    /// A11Y-007: opens the Help > Keyboard Shortcuts dialog and focuses its Close button.
    fn open_shortcuts(&mut self) {
        self.shortcuts_open = true;
        self.shortcuts_focus = true;
    }

    /// A11Y-014: switches the window between ASCII and Unicode mathematical notation.
    ///
    /// The setting is persisted immediately and the log records the change, so the switch is
    /// observable without a re-run: the next frame re-localizes the committed view in place.
    fn set_unicode_glyphs(&mut self, enabled: bool, ctx: &egui::Context) {
        if self.settings.unicode_glyphs == enabled {
            return;
        }
        self.settings.unicode_glyphs = enabled;
        self.glyph_mode = GlyphMode::from_unicode(enabled);
        self.workbench.log_active(
            LogKind::Info,
            format!(
                "Mathematical glyphs: {} notation",
                self.glyph_mode.name()
            ),
        );
        self.persist_state(true);
        ctx.request_repaint();
    }

    /// Dispatches one registered binding from the table.
    ///
    /// Returns true when this call executed the action. The actions the window already executes
    /// at a point of its own (`Run`/`Cancel`, the file commands, the editor navigation commands,
    /// the editor's own history keys, quick-eval focus, editor settings and F8) return false so
    /// they are not run twice; each of those paths is itself table-driven through
    /// [`keyboard::binding_for`] / the legacy mappers. The mapping from action to handler is
    /// exhaustive, so a newly registered action cannot be silently dead.
    fn dispatch_binding(&mut self, binding: &'static keyboard::Binding, ctx: &egui::Context) -> bool {
        crate::run_trace::mark(
            "binding",
            format!("{} {}", binding.keys.chord(), binding.action.label()),
        );
        match binding.action {
            Action::NextPane => {
                self.move_pane_focus(true, ctx);
                true
            }
            Action::PreviousPane => {
                self.move_pane_focus(false, ctx);
                true
            }
            Action::FocusPane(pane) => {
                self.request_pane_focus(pane, ctx);
                true
            }
            Action::WorkspaceResultView => {
                self.inspector_open = false;
                self.request_pane_focus(Pane::Workspace, ctx);
                if let Some(id) = self.workspace_tab_ids[0] {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
                true
            }
            Action::WorkspaceInspectorView => {
                self.inspector_open = true;
                self.request_pane_focus(Pane::Workspace, ctx);
                if let Some(id) = self.workspace_tab_ids[1] {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
                true
            }
            Action::WorkspaceFilterFocus => {
                self.filter_focus = true;
                self.request_pane_focus(Pane::Workspace, ctx);
                true
            }
            Action::ClearVariables => {
                if self.view.has_snapshot && !self.workbench.active_is_running() {
                    self.workbench.clear_variables_active();
                }
                true
            }
            Action::PlotFit => {
                self.plot.fit();
                true
            }
            Action::PlotActualSize => {
                self.plot.actual_size();
                true
            }
            Action::PlotZoomIn => {
                self.plot.zoom_step(plot::WHEEL_STEP);
                true
            }
            Action::PlotZoomOut => {
                self.plot.zoom_step(1.0 / plot::WHEEL_STEP);
                true
            }
            Action::PlotExportPng => {
                self.export_plot_png_beside_capture();
                true
            }
            Action::ShortcutsHelp => {
                self.open_shortcuts();
                true
            }
            Action::ToggleUnicodeGlyphs => {
                let enabled = !self.settings.unicode_glyphs;
                self.set_unicode_glyphs(enabled, ctx);
                true
            }
            // VIS-002 + A11Y-001: the theme switch is a command like any other, so it has a
            // binding; `set_theme` is the same entry point the toolbar button uses.
            Action::ToggleTheme => {
                let next = self.theme.toggled();
                self.set_theme(next, ctx);
                true
            }
            Action::Quit => {
                self.request_close(CloseTarget::Window, ctx);
                true
            }
            // Executed by the window's dedicated table-driven blocks or by the editor.
            Action::Workbench(_)
            | Action::File(_)
            | Action::Editor(_)
            | Action::Undo
            | Action::Redo
            | Action::Completion
            | Action::QuickEvalFocus
            | Action::EditorSettings
            | Action::JumpToDiagnostic => false,
        }
    }

    /// The table's chord for `action` was pressed this frame.
    ///
    /// The dedicated blocks (quick-eval focus, editor settings, F8) ask this instead of matching
    /// key constants, so changing a chord in [`keyboard::BINDINGS`] moves the behaviour and the
    /// Help reference together.
    fn pressed_action(&self, ui: &Ui, action: Action) -> bool {
        ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(shortcut_of_event)
                .any(|shortcut| keyboard::action_for(shortcut) == Some(action))
        })
    }

    /// EDT-016: the active editor, for the `--measure-large` driver.
    ///
    /// The measurement mode is a wrapper around this same app; it drives the editor's own caret
    /// and scroll fields rather than a second rendering path, so the numbers describe the
    /// shipped widget.
    pub(crate) fn measure_editor(&mut self) -> &mut TextEditor {
        let active = self.workbench.active();
        &mut self.editors[active]
    }

    /// EDT-010: the go-to-line prompt.
    ///
    /// A single-line field accepts `line` or `line:column` (1-based). Enter jumps and closes,
    /// Escape closes without moving the caret, and a rejected value stays in the field with its
    /// reason visible.
    fn goto_line_window(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.goto_line.take() else {
            return;
        };
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Go to line")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 96.0))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Line or line:column, 1-based. Enter jumps; Esc cancels.")
                        .color(theme::dim()),
                );
                let field = ui
                    .add(
                        egui::TextEdit::singleline(&mut prompt.text)
                            .id_salt("goto_line_input")
                            .desired_width(180.0)
                            .hint_text("42 or 42:8"),
                    )
                    .on_hover_text("The line to move the caret to");
                label_input(ui, &field, "Go to line");
                if prompt.focus {
                    // Retry until the field really owns the keyboard focus: the window's first
                    // pass or a UIA Invoke can leave another widget focused for a frame.
                    field.request_focus();
                }
                if field.has_focus() {
                    prompt.focus = false;
                }
                if let Some(error) = &prompt.error {
                    ui.label(RichText::new(error).color(theme::error()).monospace());
                }
                submit = ui.input(|input| input.key_pressed(Key::Enter));
                cancel = ui.input(|input| input.key_pressed(Key::Escape));
            });
        if cancel {
            self.focus_editor(ctx);
            return;
        }
        if submit {
            let active = self.workbench.active();
            match self.editors[active].go_to_line(&prompt.text) {
                Ok(_) => {
                    self.focus_editor(ctx);
                }
                Err(reason) => {
                    prompt.error = Some(reason);
                    prompt.focus = true;
                    self.goto_line = Some(prompt);
                }
            }
        } else {
            self.goto_line = Some(prompt);
        }
    }

    /// EDT-010: the find-symbol prompt.
    ///
    /// The list is the active document's identifiers filtered by the query; the highlighted row
    /// is keyboard-movable with Up/Down and Enter jumps to its first occurrence. Nothing here
    /// needs the pointer: the field takes focus when the prompt opens.
    fn find_symbol_window(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.find_symbol.take() else {
            return;
        };
        let active = self.workbench.active();
        let hits = self.editors[active].find_symbols(&prompt.query);
        if hits.is_empty() {
            prompt.selected = 0;
        } else {
            prompt.selected = prompt.selected.min(hits.len() - 1);
        }
        let mut jump: Option<usize> = None;
        let mut cancel = false;
        let mut submit = false;
        egui::Window::new("Find symbol in file")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 96.0))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Filter the document's identifiers. Enter jumps; Esc cancels.")
                        .color(theme::dim()),
                );
                let field = ui
                    .add(
                        egui::TextEdit::singleline(&mut prompt.query)
                            .id_salt("find_symbol_input")
                            .desired_width(240.0)
                            .hint_text("filter"),
                    )
                    .on_hover_text("Case-insensitive filter over the document's identifiers");
                label_input(ui, &field, "Find symbol");
                if prompt.focus {
                    // Retry until the field really owns the keyboard focus, exactly like the
                    // go-to-line prompt.
                    field.request_focus();
                }
                if field.has_focus() {
                    prompt.focus = false;
                }
                let down = ui.input(|input| input.key_pressed(Key::ArrowDown));
                let up = ui.input(|input| input.key_pressed(Key::ArrowUp));
                if down && !hits.is_empty() {
                    prompt.selected = (prompt.selected + 1).min(hits.len() - 1);
                }
                if up && !hits.is_empty() {
                    prompt.selected = prompt.selected.saturating_sub(1);
                }
                ui.separator();
                if hits.is_empty() {
                    ui.label(RichText::new("No identifier matches.").color(theme::dim()));
                }
                egui::ScrollArea::vertical()
                    .id_salt("find_symbol_list")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (index, hit) in hits.iter().enumerate() {
                            let label = format!(
                                "{}   {}:{}",
                                hit.name,
                                hit.row + 1,
                                hit.col + 1
                            );
                            let selected = index == prompt.selected;
                            if ui.selectable_label(selected, label).clicked() {
                                jump = Some(index);
                            }
                        }
                    });
                submit = ui.input(|input| input.key_pressed(Key::Enter));
                cancel = ui.input(|input| input.key_pressed(Key::Escape));
            });
        if cancel {
            self.focus_editor(ctx);
            return;
        }
        if let Some(index) = jump {
            let hit = hits[index].clone();
            if self.editors[active].jump_to_symbol(&hit) {
                self.focus_editor(ctx);
            }
            return;
        }
        if submit {
            if let Some(hit) = hits.get(prompt.selected).cloned() {
                if self.editors[active].jump_to_symbol(&hit) {
                    self.focus_editor(ctx);
                }
            }
            return;
        }
        prompt.selected = prompt.selected.min(hits.len().saturating_sub(1));
        self.find_symbol = Some(prompt);
    }

    /// EDT-008: the editor settings window.
    ///
    /// The controls edit a copy of the persisted settings; a change applies to every open editor
    /// in the same frame and is written to the settings file immediately, so the choice survives
    /// a restart and the tests can pin the value the editor actually uses.
    fn editor_settings_window(&mut self, ctx: &egui::Context) {
        if !self.editor_settings_open {
            return;
        }
        let mut preferences = self.settings.editor;
        let mut changed = false;
        let mut ui_scale_percent = self.settings.ui_scale_percent;
        let mut theme_follows_system = self.settings.theme_follows_system;
        let mut changed_scale = false;
        let mut changed_follows = false;
        let mut open = self.editor_settings_open;
        egui::Window::new("Editor settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 96.0))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Indentation and typing behaviour for every document")
                        .color(theme::dim()),
                );
                changed |= ui
                    .checkbox(&mut preferences.use_spaces, "Indent with spaces")
                    .on_hover_text("Off: Tab and smart Enter write a literal tab")
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut preferences.indent_width)
                            .range(settings::MIN_INDENT_WIDTH..=settings::MAX_INDENT_WIDTH)
                            .speed(1.0),
                    )
                    .on_hover_text("Columns per indentation level and the displayed tab width")
                    .changed();
                changed |= ui
                    .checkbox(&mut preferences.auto_close, "Auto-close brackets and quotes")
                    .on_hover_text(
                        "Typing ( [ { \" or ' inserts the closing character; typing the closing \
                         character steps over the one already there",
                    )
                    .changed();
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Tab writes {}",
                        if preferences.use_spaces {
                            format!("{} spaces", preferences.indent_width)
                        } else {
                            "one tab".to_string()
                        }
                    ))
                    .monospace()
                    .color(theme::accent()),
                );
                ui.separator();
                // A11Y-014: the ASCII/Unicode mathematical-notation setting. It applies to the
                // committed values in place (no re-run) and is persisted immediately.
                let mut unicode = self.settings.unicode_glyphs;
                if ui
                    .checkbox(&mut unicode, "Unicode mathematical glyphs (√ × π ≤)")
                    .on_hover_text(format!(
                        "Off: the engine's ASCII notation (sqrt, *, pi, <=). On: Unicode. A glyph \
                         the font lacks falls back to its ASCII spelling ({}).",
                        keyboard::chord_of(Action::ToggleUnicodeGlyphs)
                    ))
                    .changed()
                {
                    self.set_unicode_glyphs(unicode, ctx);
                }
                ui.label(
                    RichText::new(format!(
                        "Values render as {}",
                        self.glyph_mode.label()
                    ))
                    .monospace()
                    .color(theme::dim()),
                );
                ui.separator();
                // A11Y-005: the configurable editor typography. Every choice repaints in this
                // same frame because the editor derives its metrics from these settings.
                ui.label(RichText::new("Editor typography").strong());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Font").color(theme::dim()));
                    egui::ComboBox::from_id_salt("editor_font_family")
                        .selected_text(preferences.typography.font.label())
                        .show_ui(ui, |ui| {
                            for font in settings::EditorFont::ALL {
                                let available = font.available();
                                let text = if available {
                                    font.label().to_string()
                                } else {
                                    format!("{} (not installed)", font.label())
                                };
                                ui.add_enabled_ui(available, |ui| {
                                    ui.selectable_value(
                                        &mut preferences.typography.font,
                                        font,
                                        text,
                                    );
                                });
                            }
                        });
                });
                changed |= ui
                    .add(
                        egui::Slider::new(
                            &mut preferences.typography.font_size_pt,
                            settings::MIN_FONT_SIZE_PT..=settings::MAX_FONT_SIZE_PT,
                        )
                        .text("pt"),
                    )
                    .on_hover_text("Editor font size, 12-24 points")
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(
                            &mut preferences.typography.line_height_percent,
                            settings::MIN_LINE_HEIGHT_PERCENT
                                ..=settings::MAX_LINE_HEIGHT_PERCENT,
                        )
                        .text("line height %"),
                    )
                    .on_hover_text("Row height as a percentage of the font size")
                    .changed();
                changed |= ui
                    .checkbox(&mut preferences.typography.ligatures, "Ligatures")
                    .on_hover_text(
                        "Paint coding operator pairs as their single glyph (-> as an arrow, != \
                         as a not-equal sign, ...). The buffer, the caret column and every \
                         decoration keep their positions.",
                    )
                    .changed();
                ui.label(
                    RichText::new(typography::Typography::new(&preferences.typography).summary())
                        .monospace()
                        .color(theme::dim()),
                );
                ui.label(
                    RichText::new(format!(
                        "Row height {:.1} pt (never below the font's own row)",
                        preferences.typography.row_height()
                    ))
                    .small()
                    .color(theme::dim()),
                );
                ui.separator();
                // A11Y-004: the interface scale is applied on top of the OS DPI scale.
                ui.label(RichText::new("Interface scale").strong());
                changed_scale |= ui
                    .add(
                        egui::Slider::new(
                            &mut ui_scale_percent,
                            settings::MIN_UI_SCALE_PERCENT..=settings::MAX_UI_SCALE_PERCENT,
                        )
                        .suffix(" %")
                        .text("on top of the OS DPI"),
                    )
                    .on_hover_text(
                        "Enlarge every control and label by this percentage; the window keeps \
                         its size and the layout reflows",
                    )
                    .changed();
                ui.label(
                    RichText::new(format!(
                        "The OS DPI scale is honoured; this adds {}% on top of it.",
                        ui_scale_percent.saturating_sub(100)
                    ))
                    .small()
                    .color(theme::dim()),
                );
                ui.separator();
                // A11Y-006: follow the OS application theme, or keep the explicit override.
                ui.label(RichText::new("Theme").strong());
                changed_follows |= ui
                    .checkbox(
                        &mut theme_follows_system,
                        "Follow the OS light/dark theme",
                    )
                    .on_hover_text(
                        "On: an OS application-theme change repaints the window, keeping the \
                         document, the caret, the run and the focus. Off: the toolbar's Dark/Light \
                         choice is an explicit override.",
                    )
                    .changed();
                ui.label(
                    RichText::new(if theme_follows_system {
                        format!("Following the OS theme (currently {})", self.theme_button_label())
                    } else {
                        format!("Explicit override: {}", self.theme_button_label())
                    })
                    .small()
                    .color(theme::dim()),
                );
            });
        self.editor_settings_open = open;
        if changed {
            self.settings.editor = preferences;
            self.apply_editor_settings();
            self.persist_state(true);
        }
        if changed_scale {
            self.settings.ui_scale_percent = ui_scale_percent;
            self.persist_state(true);
        }
        if changed_follows {
            self.settings.theme_follows_system = theme_follows_system;
            self.persist_state(true);
        }
    }

    /// The file bar: New / Open / Save / Save As / Recent / Close / Quit and the active
    /// document's encoding and path.
    fn file_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.file_buttons.clear();
        // A11Y-004: the file bar wraps instead of running under the window edge at a high scale.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("File").small().color(theme::dim()));
            if self.file_bar_button(ui, "New", "New document (Ctrl+N)") {
                self.new_document();
            }
            if self.file_bar_button(ui, "Open...", "Open a Lovelace script from disk (Ctrl+O)") {
                self.open_dialog(&ctx);
            }
            if self.file_bar_button(
                ui,
                "Save",
                "Save the active document (Ctrl+S); asks for a path when it has none",
            ) {
                self.save_active(false);
            }
            if self.file_bar_button(
                ui,
                "Save As...",
                "Save the active document under a new name (Ctrl+Shift+S)",
            ) {
                self.save_active(true);
            }
            let recent = self.workbench.recent_files().to_vec();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Recent", |ui| {
                    ui.set_min_width(360.0);
                    for path in &recent {
                        if ui.button(path).clicked() {
                            if let Ok(index) = self.workbench.open(std::path::Path::new(path)) {
                                self.sync_after_tab_change(index, &ctx);
                            }
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Files opened or saved in this session");
            });
            ui.separator();
            if self.file_bar_button(
                ui,
                "Close",
                "Close the active tab (Ctrl+W); prompts when it has unsaved changes",
            ) {
                let id = self.workbench.active_document().id;
                self.request_close(CloseTarget::Tab(id), &ctx);
            }
            if self.file_bar_button(
                ui,
                "Quit",
                "Quit Lovelace Desktop; prompts for every unsaved document",
            ) {
                self.request_close(CloseTarget::Window, &ctx);
            }
            ui.separator();
            let document = self.workbench.active_document();
            ui.label(
                RichText::new(document.encoding_label())
                    .monospace()
                    .color(theme::dim()),
            )
            .on_hover_text("The encoding and newline style this document saves with");
            match &document.path {
                Some(path) => {
                    ui.label(
                        RichText::new(path.display().to_string())
                            .monospace()
                            .small()
                            .color(theme::dim()),
                    );
                }
                None => {
                    ui.label(RichText::new("no file yet").small().color(theme::dim()));
                }
            }
        });
    }

    /// One File bar button: records its rectangle for the pointer tests and reports a click.
    fn file_bar_button(&mut self, ui: &mut Ui, label: &'static str, hover: &str) -> bool {
        let response = ui.button(label).on_hover_text(hover);
        self.file_buttons.push((label, response.rect));
        response.clicked()
    }

    /// VIS-002: switches the window to `theme` at runtime.
    ///
    /// Only the token selection and egui's style slots change. The workbench, the editors (with
    /// their carets, selections and undo history), the plot and the focused widget are not
    /// touched, which is what makes the switch lossless. The choice is persisted immediately.
    fn set_theme(&mut self, theme: theme::ThemeId, ctx: &egui::Context) {
        self.theme = theme;
        theme::apply(ctx, theme);
        self.applied_theme = Some(theme);
        self.settings.theme = theme;
        self.persist_state(true);
        // VIS-002: a click on the toolbar button moves egui's focus to that button. Hand it back
        // to whatever owned it before the pass (normally the editor) so the switch costs neither
        // the caret nor the focused control.
        if let Some(focus) = self.focus_before_interaction {
            self.pending_focus = Some(focus);
            ctx.memory_mut(|memory| memory.request_focus(focus));
        }
        ctx.request_repaint();
    }

    /// VIS-002: the toolbar toggle's visible label, which is also its UIA accessible name.
    fn theme_button_label(&self) -> String {
        format!(
            "Theme: {}",
            match self.theme {
                theme::ThemeId::Dark => "Dark",
                theme::ThemeId::Light => "Light",
            }
        )
    }

    fn ui_root(&mut self, ui: &mut Ui) {
        // A11Y-005: the editor font family is registered before any text is laid out, so the
        // first frame already draws the configured face.
        typography::ensure_installed(ui.ctx(), &self.settings.editor.typography);
        // A11Y-004: the persisted interface scale is applied as egui's zoom factor on top of the
        // OS DPI scale. The workbench never replaces the OS scale, so honouring the system DPI is
        // the default and the setting only adds accessibility zoom.
        let scale = typography::scale_factor(self.settings.ui_scale_percent);
        if (ui.ctx().zoom_factor() - scale).abs() > 1e-4 {
            ui.ctx().set_zoom_factor(scale);
        }
        // A11Y-006: an OS application-theme change is followed when the user chose to follow it;
        // an explicit override ignores it. Only the token set changes here — the workbench, the
        // editors (caret, selection, history), the plot and the focused widget are untouched, so
        // the switch cannot lose a document, a run or the focus.
        if self.settings.theme_follows_system {
            if let Some(system) = ui.ctx().system_theme() {
                let wanted = match system {
                    egui::Theme::Dark => theme::ThemeId::Dark,
                    egui::Theme::Light => theme::ThemeId::Light,
                };
                if wanted != self.theme {
                    self.theme = wanted;
                    self.settings.theme = wanted;
                    theme::apply(ui.ctx(), wanted);
                    self.applied_theme = Some(wanted);
                    self.persist_state(true);
                }
            }
        }
        // VIS-002: every pass starts with the active theme's tokens applied to this Ui's style,
        // so no widget can inherit egui's default palette. When the theme changed, the context's
        // style slots are rewritten too, which is what windows, menus and tooltips created
        // through `ctx` (rather than through this Ui) read.
        theme::set_active(self.theme);
        theme::apply_style(ui.style_mut(), self.theme);
        if self.applied_theme != Some(self.theme) {
            theme::apply(ui.ctx(), self.theme);
            self.applied_theme = Some(self.theme);
        }
        // VIS-002: remember the focus owner before any widget can steal it, so the theme toggle
        // can hand focus back when it has finished switching. A press can clear focus in the same
        // frame, so the last pass that began focused is remembered too.
        let current_focus = ui.ctx().memory(|memory| memory.focused());
        if current_focus.is_some() {
            self.last_stable_focus = current_focus;
        }
        self.focus_before_interaction = current_focus.or(self.last_stable_focus);
        // A11Y-001: refresh the focus group from the real focus owner, so a pointer click or Tab
        // inside a pane moves the F6 sequence on from where the user actually is.
        self.focused_pane = self.current_pane(ui.ctx());
        // RUN-001 trace: the frame after the Running frame begins only once that frame has been
        // painted and presented, so its start bounds the paint time.
        if self.trace_after_paint {
            crate::run_trace::mark(
                "frame_after_paint_begin",
                format!("pass={}", ui.ctx().cumulative_pass_nr()),
            );
            self.trace_after_paint = false;
        }
        if crate::run_trace::enabled() && !self.workbench.is_running() {
            crate::run_trace::mark(
                "frame_begin",
                format!("pass={}", ui.ctx().cumulative_pass_nr()),
            );
        }

        let ctx = ui.ctx().clone();
        // A focus request made while the modal was up is applied every pass until it takes
        // effect: the modal's layer is still the top layer for one pass after it stops being
        // painted, so the editor is only interactable again on the pass after that.
        if let Some(id) = self.pending_focus {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        // DOC-003: the OS close request (title-bar X, Alt+F4) is either refused with a prompt or
        // allowed through. It is handled before anything else so a close can never race a pane.
        self.handle_close_request(&ctx);
        // While a prompt is open nothing in this frame may consume a keystroke: read the keys the
        // prompt itself uses, then strip every key and text event so no editor can be edited
        // behind the modal. The modal's buttons still receive pointer events, and the prompts are
        // drawn in egui's foreground layer even though they are built before the panes.
        if self.close.is_some() || self.recovery_prompt.is_some() {
            let (escape, enter) = ctx.input(|input| {
                (
                    input.key_pressed(Key::Escape),
                    input.key_pressed(Key::Enter),
                )
            });
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(
                        event,
                        egui::Event::Text(_) | egui::Event::Key { .. } | egui::Event::Paste(_)
                    )
                });
            });
            // The recovery prompt is the outermost question (DOC-005): it is asked before a save
            // can offer to overwrite anything.
            if self.recovery_prompt.is_some() {
                self.recovery_prompt_ui(&ctx, escape);
            } else {
                self.close_prompt_ui(&ctx, escape, enter);
            }
        }

        // Start with keyboard focus in the editor so a typed script lands there without a click.
        if !self.focus_requested {
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(surface_id()));
            self.focus_requested = true;
        }

        // LOG-003: Ctrl+Shift+E is the keyboard way into the quick-eval field. The field is also
        // focusable by clicking it (it is an ordinary egui `TextEdit`); this shortcut only moves
        // keyboard focus, it never submits or edits. The chord comes from the binding table
        // (A11Y-007), so the Help reference and this block cannot disagree.
        if self.pressed_action(ui, Action::QuickEvalFocus) {
            for editor in &mut self.editors {
                editor.completion.dismiss();
            }
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(quick_eval_id()));
        }

        // While the quick-eval field owns focus, Esc means "leave the field", not "cancel the
        // document's run". The field itself surrenders focus later in this same frame.
        let quick_eval_focused =
            ui.ctx().memory(|memory| memory.focused()) == Some(quick_eval_id());

        // Ctrl+Enter and Esc are window-global shortcuts: they work while any pane has focus.
        // Read the key events themselves as well as the frame's modifier state, so a shortcut
        // works even when the modifier tick and the key press arrive in the same frame.
        let shortcut = ui.input(|input| {
            let mut enter = false;
            let mut escape = false;
            let mut enter_command = false;
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    match key {
                        Key::Enter => {
                            enter = true;
                            enter_command |= modifiers.command || modifiers.ctrl;
                        }
                        Key::Escape => escape = true,
                        _ => {}
                    }
                }
            }
            // An open completion popup owns Escape for dismissal; it must not also cancel the
            // document's run. The editor closes the popup later in this same frame. An open
            // go-to-line, find-symbol or find bar also owns Escape for closing itself.
            let completion_open = self.editors[self.workbench.active()].completion.is_open();
            let prompt_open = self.goto_line.is_some() || self.find_symbol.is_some();
            let find_open = self.editors[self.workbench.active()].find.is_open();
            // While the quick-eval field owns focus, a bare Enter belongs to the field, not to
            // Run. The fallback to the frame's modifier state is only for the editor/other
            // panes: a stale Ctrl bit must not turn the field's submit key into Ctrl+Enter.
            let command_modifier = enter_command
                || (!quick_eval_focused && (input.modifiers.command || input.modifiers.ctrl));
            keyboard::command_for_shortcut(
                enter,
                escape && !completion_open && !quick_eval_focused && !prompt_open && !find_open,
                command_modifier,
            )
        });
        if let Some(command) = shortcut {
            if command == keyboard::WorkbenchCommand::Run && crate::run_trace::enabled() {
                // Trace mode only: measure this frame to its painted end.
                crate::run_trace::mark("key_event_handled", "Ctrl+Enter");
                self.trace_pending = true;
            }
            self.workbench.command(command);
        }

        // EDT-009: Escape closes the find bar from anywhere in the window (the find field, the
        // editor or a toolbar button) and returns keyboard focus to the editor. It is consumed
        // here, before any pane, so it cannot also reach the buffer or cancel a run. When no
        // find bar is open, the same press dismisses the EDT-004 signature popup for the call it
        // belongs to.
        if ui.input(|input| input.key_pressed(Key::Escape))
            && self.close.is_none()
            && self.recovery_prompt.is_none()
        {
            let active = self.workbench.active();
            if self.editors[active].find.is_open() {
                self.editors[active].find_close();
                self.find_focus = false;
                self.focus_editor(ui.ctx());
            } else {
                self.editors[active].dismiss_signature();
            }
        }

        // DOC-002: the file shortcuts are window-level commands because each of them may open a
        // native dialog. Ctrl+N/O/S, Ctrl+Shift+S and Ctrl+W are mapped in `keyboard`.
        if let Some(command) = self.file_shortcut(ui) {
            self.apply_file_command(command, ui.ctx());
        }

        // EDT-010: editor navigation shortcuts. They are read from the same key events, before
        // any pane consumes them, so go-to-line, go-to-bracket, find-symbol and next/previous tab
        // all work without a pointer.
        let editor_shortcut = ui.input(|input| {
            input.events.iter().find_map(|event| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    return None;
                };
                let letter = match key {
                    Key::Tab => '\t',
                    Key::F => 'F',
                    Key::G => 'G',
                    Key::M => 'M',
                    Key::O => 'O',
                    _ => return None,
                };
                keyboard::editor_command_for_shortcut(
                    letter,
                    modifiers.command || modifiers.ctrl,
                    modifiers.shift,
                )
            })
        });
        if let Some(command) = editor_shortcut {
            self.apply_editor_command(command, ui.ctx());
        }
        // A11Y-001/A11Y-007: every remaining registered binding — pane focus (F6, Ctrl+1..5),
        // the workspace and plot actions, the plot export, Help > Keyboard Shortcuts, the
        // ASCII/Unicode toggle and Quit — is dispatched through the same table the Help dialog
        // renders. The chords the window already executes at a point of its own come back as
        // false from `dispatch_binding` and run exactly once, in their own block.
        let window_bindings: Vec<&'static keyboard::Binding> = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(shortcut_of_event)
                .filter_map(keyboard::binding_for)
                .collect()
        });
        for binding in window_bindings {
            self.dispatch_binding(binding, ui.ctx());
        }
        // EDT-008: Ctrl+, opens the editor settings window. The chord is looked up in the
        // binding table, never spelled here.
        if self.pressed_action(ui, Action::EditorSettings) {
            self.editor_settings_open = true;
        }

        self.sync(ui.ctx());
        // EDT-008: Tab belongs to the code editor (indent / outdent / accept a completion), not
        // to egui's focus traversal. Handling it here, before any toolbar widget registers as a
        // focus candidate, is what keeps the very first Tab from moving focus out of the editor
        // and indenting nothing. The event is removed so the editor's own loop cannot run the
        // same action twice; focus returns on the next pass.
        let tab_shift = ui.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key: Key::Tab,
                    pressed: true,
                    modifiers,
                    ..
                } => Some(modifiers.shift),
                _ => None,
            })
        });
        if let Some(shift) = tab_shift {
            let editor_focused =
                ui.ctx().memory(|memory| memory.focused()) == Some(surface_id());
            if editor_focused {
                let active = self.workbench.active();
                self.editors[active].handle_tab(shift);
                // The edit happened before `editor_pane` measured the editor's edit counter, so
                // publish the buffer to the workbench here instead of relying on that pass.
                let source = self.editors[active].source();
                self.workbench.set_source(active, source);
                self.loaded_revisions[active] = self.workbench.documents()[active].revision;
                self.pending_focus = Some(surface_id());
                ui.ctx().input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Key {
                                key: Key::Tab,
                                pressed: true,
                                ..
                            }
                        )
                    });
                });
            }
        }
        if self.trace_pending {
            crate::run_trace::mark("sync_done", "event queue polled and document synced");
        }
        // DOC-005: while a dirty document's newest revision is still inside the debounce window,
        // ask for a frame when that window elapses. The trailing write then happens even if the
        // user stops typing, so a forced kill never loses more than the window.
        if let Some(delay) = self
            .recovery
            .pending_delay(self.workbench.documents(), Instant::now())
        {
            ui.ctx().request_repaint_after(delay);
        }

        // F8 is window-global keyboard navigation to the last failure: it works whether the
        // caret is in the editor or focus sits on the toolbar. The jump is revision-gated by the
        // workbench, so it can never move the caret to a line that no longer exists. The chord is
        // the table's (A11Y-007).
        if self.pressed_action(ui, Action::JumpToDiagnostic) {
            let active = self.workbench.active();
            if self.editors[active].jump_to_diagnostic() {
                ui.ctx()
                    .memory_mut(|memory| memory.request_focus(surface_id()));
            }
        }

        egui::Panel::top("filebar").show(ui, |ui| {
            ui.scope_builder(
                UiBuilder::new()
                    .accessibility_role(egui::Role::Pane)
                    .accessibility_label("File bar"),
                |ui| self.file_bar(ui),
            );
        });
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.scope_builder(
                UiBuilder::new()
                    .accessibility_role(egui::Role::Pane)
                    .accessibility_label("Toolbar"),
                |ui| self.toolbar(ui),
            );
        });
        if self.trace_pending {
            crate::run_trace::mark("toolbar_drawn", "");
        }
        egui::Panel::top("tabstrip").show(ui, |ui| {
            ui.scope_builder(
                UiBuilder::new()
                    .accessibility_role(egui::Role::TabList)
                    .accessibility_label("Document tabs"),
                |ui| self.tab_strip(ui),
            );
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.scope_builder(
                UiBuilder::new()
                    .accessibility_role(egui::Role::Pane)
                    .accessibility_label("Status bar"),
                |ui| self.status_bar(ui),
            );
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.scope_builder(
                UiBuilder::new()
                    .accessibility_role(egui::Role::Pane)
                    .accessibility_label("Workbench panes"),
                |ui| {
                    let full = ui.available_rect_before_wrap();
                    self.panes(ui, full);
                },
            );
        });
        // PLT-006: the PNG options window sits over every pane, not inside the plot pane, so a
        // click on it cannot also reach the plot canvas underneath.
        self.plot_export_window(ui.ctx());
        // EDT-010 / EDT-008: the keyboard navigation prompts and the editor settings window.
        self.goto_line_window(ui.ctx());
        self.find_symbol_window(ui.ctx());
        self.editor_settings_window(ui.ctx());
        // A11Y-007: Help > Keyboard Shortcuts, generated from the binding table.
        self.shortcuts_window(ui.ctx());
        // A11Y-003: the window's one keyboard-focus ring, painted last so it sits above every
        // pane and dialog and around whatever widget really owns the keyboard.
        paint_focus_ring(ui.ctx());
        // DOC-002: the native window title carries the active file name and its dirty marker.
        self.update_window_title(ui.ctx());
        // A queued focus request is cleared only once the editor actually holds the focus; while
        // the modal's layer is still the top layer the request cannot take effect, so it is kept
        // and re-requested on the following pass.
        if let Some(id) = self.pending_focus {
            if ctx.memory(|memory| memory.focused()) == Some(id) {
                self.pending_focus = None;
            }
        }
        if self.trace_pending {
            crate::run_trace::mark("panes_drawn", "");
        }

        if self.workbench.is_running() {
            // Poll again soon so a fast worker's terminal event is applied promptly.
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
        if self.trace_pending {
            let active = self.workbench.active();
            let (line, column) = self.editors[active].line_col_1based();
            let status = self.workbench.status_text(line, column);
            crate::run_trace::mark("frame_end_after_key", format!("status={status:?}"));
            // Mark "drawn": no trace work remains in this frame after this point. Ask for an
            // immediate repaint so the next frame start proves the paint completed.
            self.trace_pending = false;
            self.trace_after_paint = true;
            ui.ctx().request_repaint();
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        // A11Y-004: at 125-200% the toolbar's controls wrap onto extra rows instead of being
        // clipped by the window edge.
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Lovelace Desktop")
                    .strong()
                    .color(theme::text())
                    .size(15.0),
            );
            ui.label(RichText::new(self.workbench.engine_name()).color(theme::dim()));
            ui.separator();

            let toolbar = self.workbench.toolbar_state();
            let run = ui
                .add_enabled(
                    toolbar.run_enabled,
                    egui::Button::new(RichText::new("Run").strong()),
                )
                .on_hover_text(
                    "Run (Ctrl+Enter): evaluate the active document on a worker (the window never blocks)",
                );
            // A11Y-001: the toolbar group's first control. Its id is remembered so F6 / Ctrl+1 can
            // put the keyboard here, and a focus request that arrived before this pane painted is
            // satisfied now.
            self.run_button_id = Some(run.id);
            if self.pane_focus == Some(Pane::Toolbar) {
                self.pane_focus = None;
                run.request_focus();
            }
            if run.clicked() {
                self.workbench.run_active();
            }
            if toolbar.cancel_visible {
                let cancel = ui
                    .add_enabled(
                        toolbar.cancel_enabled,
                        egui::Button::new(RichText::new("Cancel").color(theme::error())),
                    )
                    .on_hover_text("Cancel (Esc): stop the active run at the next statement boundary");
                if cancel.clicked() {
                    self.workbench.cancel_active();
                }
            }
            ui.separator();

            // RUN-019: one visibly labelled precision input. It accepts text so an invalid value
            // is rejected with guidance instead of being silently clamped by a spinner; the
            // document precision is only changed when the value is valid.
            ui.label(RichText::new("Precision").color(theme::dim()));
            let field = ui
                .add(
                    egui::TextEdit::singleline(&mut self.precision_input)
                        .desired_width(64.0)
                        .hint_text("digits"),
                )
                .on_hover_text(format!(
                    "Display precision for the active document: a whole number of digits from {MIN_PRECISION} to {MAX_PRECISION}"
                ));
            label_input(ui, &field, "Precision (digits)");
            let apply = ui
                .add(egui::Button::new("Apply"))
                .on_hover_text("Apply the typed precision to the active document");
            let submitted = apply.clicked()
                || (field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            if submitted {
                match self
                    .workbench
                    .set_active_precision_text(&self.precision_input)
                {
                    Ok(decimals) => {
                        self.precision_seen = decimals;
                        self.precision_input = decimals.to_string();
                        self.precision_error = None;
                    }
                    Err(guidance) => self.precision_error = Some(guidance),
                }
            }
            ui.label(
                RichText::new(format!("{} digits", self.view.precision)).color(theme::dim()),
            )
            .on_hover_text("The active document's applied display precision");
            if let Some(guidance) = &self.precision_error {
                ui.label(RichText::new(guidance).color(theme::error()))
                    .on_hover_text("The precision was not changed; the run uses the last valid value.");
            }
            ui.separator();

            if ui
                .button("Editor settings")
                .on_hover_text(
                    "Indent width, tabs vs spaces and bracket/quote auto-close (Ctrl+,)",
                )
                .clicked()
            {
                self.editor_settings_open = true;
            }
            if ui
                .button("Go to line")
                .on_hover_text("Jump the caret to a 1-based line or line:column (Ctrl+G)")
                .clicked()
            {
                self.find_symbol = None;
                self.goto_line = Some(GoToLinePrompt {
                    text: String::new(),
                    error: None,
                    focus: true,
                });
            }
            if ui
                .button("Find symbol")
                .on_hover_text("Filter the active document's identifiers (Ctrl+Shift+O)")
                .clicked()
            {
                self.goto_line = None;
                self.find_symbol = Some(SymbolPrompt {
                    query: String::new(),
                    selected: 0,
                    focus: true,
                });
            }
            if ui
                .button("Find...")
                .on_hover_text(
                    "Find and replace in the active document: next/previous, replace, replace all, \
                     case/whole-word/regex toggles (Ctrl+F)",
                )
                .clicked()
            {
                let active = self.workbench.active();
                self.editors[active].find_open();
                self.find_focus = true;
            }
            ui.separator();

            // A11Y-007: Help > Keyboard Shortcuts. A menu (not only the F1 key) so the surface is
            // discoverable by a user who does not know the shortcut yet; the menu's contents are
            // the binding table, rendered by the same `reference_rows` the dialog uses.
            ui.menu_button("Help", |ui| {
                if ui
                    .button(format!(
                        "Keyboard Shortcuts ({})",
                        keyboard::chord_of(Action::ShortcutsHelp)
                    ))
                    .clicked()
                {
                    self.open_shortcuts();
                    ui.close();
                }
                ui.separator();
                let mut unicode = self.settings.unicode_glyphs;
                if ui
                    .checkbox(&mut unicode, "Unicode mathematical glyphs")
                    .on_hover_text(format!(
                        "Render values as √ × π ≤ ∞ instead of the engine's ASCII notation \
                         ({}). A glyph the font lacks falls back to its ASCII spelling.",
                        keyboard::chord_of(Action::ToggleUnicodeGlyphs)
                    ))
                    .changed()
                {
                    self.set_unicode_glyphs(unicode, ui.ctx());
                }
            })
            .response
            .on_hover_text("Keyboard shortcuts and the mathematical-glyph setting");
            ui.separator();

            // VIS-002: the runtime theme toggle. Its label carries the render state, so a UIA
            // capture names the control ("Theme: Dark") and the same name opens the next theme.
            let theme_response = ui
                .button(self.theme_button_label())
                .on_hover_text(
                    "Switch between the dark and light theme at runtime; the open documents, \
                     carets, selection and keyboard focus are preserved",
                );
            self.theme_button = Some(theme_response.rect);
            if theme_response.clicked() {
                let next = self.theme.toggled();
                self.set_theme(next, ui.ctx());
                // Apply the new tokens to this Ui immediately so the rest of this pass (panes and
                // status strip) paints in the new theme instead of a mixed frame.
                theme::apply_style(ui.style_mut(), next);
            }
            ui.separator();

            let active = self.workbench.active();
            let editor = &self.editors[active];
            ui.label(
                RichText::new(format!(
                    "{} lines / {} bytes",
                    editor.line_count(),
                    editor.bytes()
                ))
                .color(theme::dim()),
            );
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "laid out {} / {} visible rows",
                    editor.metrics.laid_out_rows, editor.metrics.visible_rows
                ))
                .color(theme::dim()),
            );
        });
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let tabs: Vec<String> = self
                .workbench
                .documents()
                .iter()
                .map(|document| document.tab_label())
                .collect();
            for (index, title) in tabs.iter().enumerate() {
                let selected = index == self.workbench.active();
                let response = ui.selectable_label(selected, title);
                // A11Y-002: a document tab is a `Tab` with a selected state, not an unnamed
                // toggle button: a reader announces "round-43-a11y.lv, tab, selected" and the
                // tab strip's `TabList` parent tells it how many tabs there are.
                let name = title.clone();
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Tab, true, name.clone())
                });
                // The selected state is the tab's own property, not a checked toggle.
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_selected(selected);
                });
                if response.clicked() {
                    self.workbench.set_active(index);
                    // A tab switch is a pointer click outside the editor: drop every popup so a
                    // stale one cannot reappear when the user switches back.
                    for editor in &mut self.editors {
                        editor.completion.dismiss();
                    }
                }
            }
            ui.separator();
            let active = self.workbench.active();
            if let Some(document) = self.workbench.documents().get(active) {
                ui.label(
                    RichText::new(format!(
                        "document {} · revision {}",
                        document.id.0, document.revision
                    ))
                    .color(theme::dim()),
                );
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let active = self.workbench.active();
        let (line, column) = self.editors[active].line_col_1based();
        let status = self.workbench.status_text(line, column);
        // A11Y-012: the run outcome carries a text marker, not only a colour. `[ERR]` and `[OK]`
        // are readable in greyscale, by a screen reader, and in the UIA name below.
        let marker = match self.view.run_state {
            crate::run_state::RunState::Failed => Some("[ERR]"),
            crate::run_state::RunState::Finished => Some("[OK]"),
            crate::run_state::RunState::Stale => Some("[STALE]"),
            _ => None,
        };
        let diagnostic = self.view.diagnostic.clone();
        let mut label = match marker {
            Some(marker) => format!("{marker} {status}"),
            None => status.clone(),
        };
        if let Some(diagnostic) = &diagnostic {
            label.push_str(&format!(
                " | error at Ln {}, Col {}: {}",
                diagnostic.line, diagnostic.column, diagnostic.message
            ));
        }
        // A11Y-002: the status strip is one `Status` node whose name is the whole state line
        // (marker, run state, caret and, after a failure, the error's location).
        label_ui_node_as(ui, egui::Role::Status, &label);
        if self.trace_pending {
            crate::run_trace::mark("status_bar_drawn", format!("status={status:?}"));
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(marker) = marker {
                ui.label(
                    RichText::new(marker)
                        .strong()
                        .color(status_state_color(self.view.run_state)),
                )
                .on_hover_text(if diagnostic.is_some() {
                    "The last run failed; the log names its line and column"
                } else {
                    "The last run finished"
                });
            }
            // VIS-002: the run state's semantic colour is a theme token, not a fixed accent:
            // success is positive, an in-flight run progresses, failure is the error colour and
            // a stale/cancelled result is a warning.
            ui.label(
                RichText::new(&status)
                    .monospace()
                    .color(status_state_color(self.view.run_state)),
            );
            // A11Y-012: a failed run's status names the error's location in text. The colour is
            // redundant with the marker and the words.
            if let Some(diagnostic) = &diagnostic {
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "error at Ln {}, Col {}",
                        diagnostic.line, diagnostic.column
                    ))
                    .monospace()
                    .color(theme::error()),
                );
            }
            // A11Y-002: the committed progress is a real `ProgressIndicator` with a numeric
            // value, not text a reader has to parse out of the status string. egui fills the
            // node's value with a percentage; the explicit range below states the numbers a
            // screen reader can act on (committed statements out of the run's total).
            ui.separator();
            let total = self.view.total.max(1);
            let committed = self.view.committed_steps.min(total);
            let progress = ui.add(
                egui::ProgressBar::new(committed as f32 / total as f32)
                    .desired_width(76.0)
                    .text(format!("{committed}/{total}")),
            );
            ui.ctx()
                .accesskit_node_builder(progress.id, |node| {
                    node.set_min_numeric_value(0.0);
                    node.set_max_numeric_value(total as f64);
                    node.set_numeric_value(committed as f64);
                });
            progress.on_hover_text("Committed statements of the last run's snapshot");
            ui.separator();
            if let Some(document) = self.workbench.documents().get(active) {
                ui.label(RichText::new(format!("{} ms last run", document.elapsed_ms)).color(theme::dim()));
            }
            if self.workbench.active_is_running() {
                ui.separator();
                ui.label(RichText::new("Cancel is available in the toolbar.").color(theme::warning()));
            }
            // A11Y-001: the status strip names the focus group, so a keyboard-only user (and a
            // UIA capture) can tell which pane F6 is in without reading the ring's position.
            ui.separator();
            ui.label(
                RichText::new(format!("focus: {}", self.focused_pane.label()))
                    .small()
                    .color(theme::dim()),
            )
            .on_hover_text(
                "The pane group the keyboard is in (F6 moves it; Ctrl+1..5 jump directly)",
            );
        });
    }

    fn panes(&mut self, ui: &mut Ui, full: Rect) {
        let layout = compute_layout(full, self.editor_frac, self.workspace_frac, self.logs_frac);
        self.layout = Some(layout);
        ui.painter()
            .rect_filled(full, 0.0, theme::app_bg());

        // RUN-003: one view for the whole frame. The clone is what a pane closure can borrow
        // while another pane mutates `self`; no pane reads pane data from the workbench.
        let view = self.view.clone();
        self.editor_pane(ui, layout.editor, &view);
        self.plot_pane(ui, layout.plot, &view);
        self.workspace_pane(ui, layout.workspace, &view);
        self.logs_pane(ui, layout.logs, &view);

        let drag_a = splitter(
            ui,
            layout.split_a,
            "split_editor",
            true,
            "Editor / plot splitter",
        );
        let drag_b = splitter(
            ui,
            layout.split_b,
            "split_workspace",
            true,
            "Plot / workspace splitter",
        );
        let drag_c = splitter(
            ui,
            layout.split_c,
            "split_logs",
            false,
            "Workspace / logs splitter",
        );

        if drag_a != 0.0 {
            self.editor_frac =
                (self.editor_frac + drag_a / full.width().max(1.0)).clamp(0.02, 0.95);
        }
        if drag_b != 0.0 {
            self.workspace_frac =
                (self.workspace_frac - drag_b / full.width().max(1.0)).clamp(0.02, 0.95);
        }
        if drag_c != 0.0 {
            self.logs_frac = (self.logs_frac - drag_c / full.height().max(1.0)).clamp(0.02, 0.9);
        }
    }

    fn editor_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        // A11Y-001: F6 / Ctrl+2 focus the editor surface. Requesting it before the editor paints
        // means the ring and the caret are already in place when the keys are read.
        if self.pane_focus == Some(Pane::Editor) {
            self.pane_focus = None;
            ui.ctx().memory_mut(|memory| memory.request_focus(surface_id()));
        }
        let active = self.workbench.active();
        let (title, encoding) = match self.workbench.document(active) {
            Some(document) => (document.title.clone(), document.encoding_label()),
            None => return,
        };
        let lines = self.editors[active].line_count();
        let bytes = self.editors[active].bytes();
        // A run failure is also stated in the pane header, so the position and message are
        // readable (and visible to accessibility clients) without hovering the underline. The
        // frame's assembled view already applied the revision gate.
        let diagnostic = view.diagnostic.clone();
        // The live catalogue: keywords, the engine's registered functions and the active
        // document's last committed variables/user functions. Built fresh each pass so an
        // engine snapshot landing or a tab switch is reflected immediately. The same capture is
        // handed to the editor, whose highlighter uses its builtin flags without the catalogue's
        // shadowing dedup (a variable named like a builtin must not hide that builtin's colour).
        let engine_state = self.workbench.completion_state();
        let catalogue = crate::completion::build_catalogue(
            crate::language::KEYWORDS,
            &engine_state.functions,
            &engine_state.variables,
        );
        let pane_title = format!("Editor - {title}");
        // The header's natural width decides how many rows it is given; the strings are built
        // once here so the estimate and the painted labels cannot disagree.
        let header_lines = format!("{lines} lines / {bytes} bytes");
        // A11Y-005: the header states the resolved typography, so a UIA capture proves which
        // font, size, line height and ligature choice the frame is painted with. At a narrow
        // pane it is the first label dropped: it is the least important header fact and keeping
        // it would force a fourth header row at 200% interface scale.
        let typography_line = (rect.width() >= 420.0)
            .then(|| typography::Typography::new(&self.settings.editor.typography).summary());
        let diagnostic_line = diagnostic
            .as_ref()
            .map(|diagnostic| format!("[ERR] {}", diagnostic_summary(diagnostic)));
        let completion_line = self.editors[active].completion_summary();
        let bracket_line = self.editors[active].bracket_status();
        let signature_line = self.editors[active].signature_summary();
        let find_line = self.editors[active].find_summary();
        let mut header_hint = header_text_width(&title)
            + header_text_width(&header_lines)
            + header_text_width(&encoding);
        if let Some(line) = &typography_line {
            header_hint += header_text_width(line);
        }
        // The editing-state summaries (completion, bracket, signature, find) come and go with
        // every pass; their widths are deliberately not measured, because a hint that changed
        // when a status appeared would resize the header and shift the whole pane between two
        // frames (which also moves a pointer target out from under a click). A fixed allowance
        // covers all four at once.
        header_hint += 280.0;
        if let Some(extra) = diagnostic_line.as_deref() {
            header_hint += 8.0 + header_text_width(extra);
        }
        with_pane(ui, rect, &pane_title, header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                ui.label(RichText::new(&title).color(theme::accent()).monospace());
                ui.label(RichText::new(&header_lines).color(theme::dim()));
                ui.label(RichText::new(&encoding).monospace().small().color(theme::dim()))
                    .on_hover_text(
                        "The document's encoding and newline style; Save writes them back \
                         unchanged unless the file is edited",
                    );
                if let Some(line) = &typography_line {
                    ui.label(RichText::new(line).monospace().small().color(theme::dim()))
                        .on_hover_text(
                            "Editor typography (Ctrl+,): font family, size in points, line height \
                             and ligature choice",
                        );
                }
                if let Some(line) = &diagnostic_line {
                    ui.separator();
                    ui.label(RichText::new(line).monospace().color(theme::error()))
                        .on_hover_text(
                            diagnostic
                                .as_ref()
                                .map(Diagnostic::render_fully)
                                .unwrap_or_default(),
                        );
                }
                // The popup itself is painted; the header states its selected signature so the
                // completion also exists in the accessibility tree and in a UIA capture.
                if let Some(summary) = &completion_line {
                    ui.separator();
                    ui.label(RichText::new(summary).monospace().color(theme::accent()));
                }
                // EDT-007: the matching-bracket state is also stated in the pane header, so both
                // the matched pair and the distinct unmatched state are readable (and visible in
                // a UIA capture) without inspecting painted pixel colours.
                if let Some(status) = &bracket_line {
                    ui.separator();
                    let color = if status.starts_with("Unmatched") {
                        theme::error()
                    } else {
                        theme::accent()
                    };
                    ui.label(RichText::new(status).monospace().color(color))
                        .on_hover_text(
                            "The bracket next to the caret: matched pairs name both positions; \
                             an unmatched bracket is a distinct red state",
                        );
                }
                // EDT-004: the signature popup is painted over the text; the header states the
                // same signature and active parameter for accessibility clients and UIA captures.
                if let Some(status) = &signature_line {
                    ui.separator();
                    ui.label(RichText::new(status).monospace().color(theme::accent()))
                        .on_hover_text(
                            "The engine-registered signature of the call the caret is inside; \
                             the active parameter is the argument being typed",
                        );
                }
                // EDT-009: the match count and the toggles are restated in the header so a UIA
                // capture can assert them; the bar itself paints the same text.
                if let Some(status) = &find_line {
                    ui.separator();
                    let color = if status.contains("invalid regex") {
                        theme::error()
                    } else {
                        theme::accent()
                    };
                    ui.label(RichText::new(status).monospace().color(color));
                }
            });
            let mut body = body;
            if self.editors[active].find.is_open() {
                // EDT-009: the bar is docked at the top of the editor body, not a floating
                // window, so it never covers the text it searches. The editor lays out the rest
                // of the body, and its virtualisation keeps the reduced viewport cheap.
                let bar = Rect::from_min_size(body.min, vec2(body.width(), 60.0));
                self.find_bar(ui, bar, active);
                let top = bar.bottom() + 2.0;
                body = Rect::from_min_max(pos2(body.left(), top), body.max);
            }
            let before = self.editors[active].edits;
            let metrics = self.editors[active].ui(ui, body, &catalogue, &engine_state);
            if self.editors[active].edits != before {
                // An edit makes the old position describe text that is gone. Drop it now, not
                // only on the next frame, so the paint pass cannot show a stale underline.
                self.editors[active].diagnostic = None;
                let source = self.editors[active].source();
                self.workbench.set_source(active, source);
                self.loaded_revisions[active] = self.workbench.documents()[active].revision;
            }
            if metrics.laid_out_rows == 0 {
                ui.painter().text(
                    body.center(),
                    Align2::CENTER_CENTER,
                    "This document is empty. Type a Lovelace script.",
                    FontId::proportional(13.0),
                    theme::dim(),
                );
            }
        });
    }

    /// EDT-009: the docked find/replace bar.
    ///
    /// The bar is drawn inside the editor pane body, so it never covers the text and the match
    /// count is visible beside the fields. Every edit it makes goes through the editor's own
    /// `replace_current` / `replace_all`, which record exactly one history step each; the bar
    /// then republishes the buffer through the same path a typed edit uses.
    fn find_bar(&mut self, ui: &mut Ui, rect: Rect, active: usize) {
        if !self.editors[active].find.is_open() {
            return;
        }
        let ctx = ui.ctx().clone();
        self.find_buttons.clear();
        let mut next = false;
        let mut previous = false;
        let mut replace = false;
        let mut replace_all = false;
        let mut close = false;
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::LEFT)),
            |ui| {
                ui.painter().rect_filled(rect, 3.0, theme::header_bg());
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(1.0, theme::border()),
                    StrokeKind::Inside,
                );
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Find").small().color(theme::dim()));
                    let find = &mut self.editors[active].find;
                    let query = ui
                        .add(
                            egui::TextEdit::singleline(&mut find.query)
                                .id_salt("find_query")
                                .desired_width(190.0)
                                .hint_text("Find text"),
                        )
                        .on_hover_text("The text or pattern to find");
                    label_input(ui, &query, "Find text");
                    if query.changed() {
                        find.query_changed();
                    }
                    if self.find_focus {
                        // Retry until the field really owns the keyboard focus, the same rule
                        // the go-to-line prompt uses.
                        query.request_focus();
                        if query.has_focus() {
                            self.find_focus = false;
                        }
                    }
                    if query.has_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                        if ui.input(|input| input.modifiers.shift) {
                            previous = true;
                        } else {
                            next = true;
                        }
                    }
                    let mut case = find.options.case_sensitive;
                    let mut whole = find.options.whole_word;
                    let mut regex = find.options.regex;
                    let case_button = ui
                        .selectable_label(case, "Aa")
                        .on_hover_text("Case sensitive");
                    self.find_buttons.push(("Aa", case_button.rect));
                    if case_button.clicked() {
                        case = !case;
                    }
                    let whole_button = ui
                        .selectable_label(whole, "W")
                        .on_hover_text("Whole word: both ends must be identifier boundaries");
                    self.find_buttons.push(("W", whole_button.rect));
                    if whole_button.clicked() {
                        whole = !whole;
                    }
                    let regex_button = ui
                        .selectable_label(regex, ".*")
                        .on_hover_text("Regular expression (invalid patterns show an error)");
                    self.find_buttons.push((".*", regex_button.rect));
                    if regex_button.clicked() {
                        regex = !regex;
                    }
                    find.set_option(case, whole, regex);
                    ui.separator();
                    match find.error() {
                        Some(reason) => {
                            let mut short: String = reason.chars().take(58).collect();
                            if reason.chars().count() > 58 {
                                short.push('…');
                            }
                            ui.label(
                                RichText::new(format!("Invalid regex: {short}"))
                                    .monospace()
                                    .small()
                                    .color(theme::error()),
                            )
                            .on_hover_text(reason);
                        }
                        None => {
                            let text = find.count_text().unwrap_or_default();
                            ui.label(RichText::new(text).monospace().small().color(theme::dim()));
                        }
                    }
                    let previous_button = ui
                        .button("Previous")
                        .on_hover_text("Previous match, wrapping at the start (Shift+Enter)");
                    self.find_buttons.push(("Previous", previous_button.rect));
                    if previous_button.clicked() {
                        previous = true;
                    }
                    let next_button = ui
                        .button("Next")
                        .on_hover_text("Next match, wrapping at the end (Enter)");
                    self.find_buttons.push(("Next", next_button.rect));
                    if next_button.clicked() {
                        next = true;
                    }
                    let close_button = ui
                        .button("Close")
                        .on_hover_text("Close the find bar and return to the editor (Esc)");
                    self.find_buttons.push(("Close", close_button.rect));
                    if close_button.clicked() {
                        close = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Replace").small().color(theme::dim()));
                    let find = &mut self.editors[active].find;
                    let replacement = ui
                        .add(
                            egui::TextEdit::singleline(&mut find.replacement)
                                .id_salt("find_replace")
                                .desired_width(190.0)
                                .hint_text("Replace with"),
                        )
                        .on_hover_text("The literal replacement text");
                    label_input(ui, &replacement, "Replacement text");
                    if replacement.has_focus()
                        && ui.input(|input| input.key_pressed(Key::Enter))
                    {
                        replace = true;
                    }
                    let replace_button = ui
                        .button("Replace")
                        .on_hover_text("Replace the highlighted match (one undo step)");
                    self.find_buttons.push(("Replace", replace_button.rect));
                    if replace_button.clicked() {
                        replace = true;
                    }
                    let replace_all_button = ui.button("Replace all").on_hover_text(
                        "Replace every match in one undo step; each match is rewritten once",
                    );
                    self.find_buttons.push(("Replace all", replace_all_button.rect));
                    if replace_all_button.clicked() {
                        replace_all = true;
                    }
                });
            },
        );
        if close {
            self.editors[active].find_close();
            self.find_focus = false;
            self.focus_editor(&ctx);
            return;
        }
        let before = self.editors[active].edits;
        if previous {
            self.editors[active].find_previous();
        } else if next {
            self.editors[active].find_next();
        }
        if replace {
            self.editors[active].replace_current();
        }
        if replace_all {
            self.editors[active].replace_all();
        }
        self.editors[active].find_refresh();
        if self.editors[active].edits != before {
            // The same publication a typed edit uses: the workbench revision advances, and the
            // result/run state sees the buffer change.
            self.editors[active].diagnostic = None;
            let source = self.editors[active].source();
            self.workbench.set_source(active, source);
            self.loaded_revisions[active] = self.workbench.documents()[active].revision;
        }
        if replace || replace_all || next || previous {
            self.find_focus = true;
        }
    }

    fn plot_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        let plot_title = view.plot.as_ref().map(|plot| plot.title.clone());
        let empty = self.plot.is_empty();
        let zoom_label = format!("{:.0}%", self.plot.zoom_percent());
        let mut header_hint = 36.0
            + 48.0
            + 78.0
            + 72.0
            + 8.0
            + header_text_width(&zoom_label)
            + 8.0
            + header_text_width(plot_title.as_deref().unwrap_or("no engine plot"))
            + 8.0
            + 100.0
            + 105.0;
        if let Some((status, _)) = &self.plot_export_status {
            header_hint += header_text_width(status);
        }
        with_pane(ui, rect, "Plot - engine SVG", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                // PLT-004: Fit restores the whole plot; 100% is the natural-size reset (one SVG
                // point per logical screen pixel) and is a different view on a pane whose fit
                // scale is not 1:1. Both are one click, and the wheel/drag handlers in
                // `plot.rs` apply the same clamped transform.
                // A11Y-001: Fit is the plot pane's first control, so F6 / Ctrl+3 lands here.
                let fit = ui
                    .button("Fit")
                    .on_hover_text("Show the whole plot in the pane (Ctrl+0)");
                self.plot_fit_id = Some(fit.id);
                if self.pane_focus == Some(Pane::Plot) {
                    self.pane_focus = None;
                    fit.request_focus();
                }
                if fit.clicked() {
                    self.plot.fit();
                }
                if ui
                    .button("100%")
                    .on_hover_text("Natural size: one SVG point per screen pixel")
                    .clicked()
                {
                    self.plot.actual_size();
                }
                if ui
                    .button("Zoom out")
                    .on_hover_text("Zoom out one step around the pointer")
                    .clicked()
                {
                    self.plot.zoom_step(1.0 / plot::WHEEL_STEP);
                }
                if ui
                    .button("Zoom in")
                    .on_hover_text("Zoom in one step around the pointer")
                    .clicked()
                {
                    self.plot.zoom_step(plot::WHEEL_STEP);
                }
                ui.label(
                    RichText::new(&zoom_label)
                        .monospace()
                        .color(theme::dim()),
                )
                .on_hover_text("Zoom as a percentage of natural size");
                ui.separator();
                match &plot_title {
                    Some(title) => {
                        ui.label(RichText::new(title).color(theme::accent()));
                    }
                    None => {
                        ui.label(RichText::new("no engine plot").color(theme::dim()));
                    }
                }
                ui.separator();
                // PLT-006: both exports go through the native save dialog. SVG copies the
                // engine's captured bytes; PNG opens an in-window options step first.
                if ui
                    .add_enabled(!empty, egui::Button::new("Export SVG..."))
                    .on_hover_text("Save the engine's original SVG bytes unchanged")
                    .clicked()
                {
                    self.export_plot_svg();
                }
                if ui
                    .add_enabled(!empty, egui::Button::new("Export PNG..."))
                    .on_hover_text("Rasterise at a chosen scale and background")
                    .clicked()
                {
                    self.plot_export = Some(PngExport::default());
                }
                if let Some((status, error)) = &self.plot_export_status {
                    ui.label(RichText::new(status).small().color(if *error {
                        theme::error()
                    } else {
                        theme::accent()
                    }));
                }
            });
            if empty {
                self.plot.ui(ui, body);
                return;
            }
            self.plot.title = plot_title.clone();
            let info = format!(
                "raster {}x{} @ {:.2}x · {:.2} ms raster · {:.2} ms upload",
                self.plot.raster_px[0],
                self.plot.raster_px[1],
                self.plot.raster_scale,
                self.plot.last_raster_ms,
                self.plot.last_upload_ms
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(Rect::from_min_size(body.min, vec2(body.width(), 18.0)))
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.label(RichText::new(info).small().color(theme::dim()));
                },
            );
            let canvas = Rect::from_min_max(pos2(body.left(), body.top() + 18.0), body.max);
            self.plot.ui(ui, canvas);
        });
    }

    /// PLT-006: writes the captured `PlotCapture.svg` through the native save dialog.
    fn export_plot_svg(&mut self) {
        let title = self.plot_title();
        let suggested = plot::export_file_name(title.as_deref(), "svg");
        let Some(path) = plot::save_plot_dialog(
            "Export plot as SVG",
            plot::SVG_FILTER,
            "svg",
            &suggested,
            self.plot_export_dir().as_deref(),
        ) else {
            self.plot_export_status = Some(("SVG export cancelled".to_string(), false));
            return;
        };
        self.plot_export_status = Some(match self.plot.export_svg_to(&path) {
            Ok(bytes) => (
                format!(
                    "SVG: {bytes} bytes -> {}",
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                ),
                false,
            ),
            Err(error) => (error, true),
        });
    }

    /// PLT-006: rasterises at the chosen scale/background and writes it through the dialog.
    fn export_plot_png(&mut self, scale: f32, background: PngBackground) {
        let title = self.plot_title();
        let suggested = plot::export_file_name(title.as_deref(), "png");
        let Some(path) = plot::save_plot_dialog(
            "Export plot as PNG",
            plot::PNG_FILTER,
            "png",
            &suggested,
            self.plot_export_dir().as_deref(),
        ) else {
            self.plot_export_status = Some(("PNG export cancelled".to_string(), false));
            return;
        };
        self.plot_export_status = Some(match self.plot.export_png_to(&path, scale, background) {
            Ok([width, height]) => (
                format!(
                    "PNG: {width}x{height} px {} -> {}",
                    background.label(),
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                ),
                false,
            ),
            Err(error) => (error, true),
        });
    }

    fn plot_title(&self) -> Option<String> {
        self.view.plot.as_ref().map(|plot| plot.title.clone())
    }

    /// The folder the save dialog starts in: where the engine wrote this plot's capture.
    ///
    /// The engine has already created and written that directory, so it is writable; starting
    /// the dialog there avoids a shell default folder this process cannot write (which would
    /// turn every export into a permission prompt) and keeps the exported file next to the
    /// capture it came from.
    fn plot_export_dir(&self) -> Option<PathBuf> {
        self.view
            .plot
            .as_ref()
            .map(|plot| PathBuf::from(&plot.path))
            .and_then(|path| path.parent().map(|directory| directory.to_path_buf()))
            .filter(|directory| directory.is_dir())
    }

    /// PLT-006: the PNG export options window, drawn over the workbench.
    fn plot_export_window(&mut self, ctx: &egui::Context) {
        let Some(mut options) = self.plot_export else {
            return;
        };
        let mut open = true;
        let mut close = false;
        let mut save = false;
        let dimensions = match plot::export_pixels(self.plot.svg_w, self.plot.svg_h, options.scale)
        {
            Ok((width, height)) => format!("{width} x {height} px"),
            Err(error) => error,
        };
        egui::Window::new("Export PNG")
            .id(Id::unique("plot_export_png"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new("Scale").color(theme::dim()));
                ui.horizontal(|ui| {
                    for scale in [1.0f32, 2.0, 4.0] {
                        let selected = (options.scale - scale).abs() < f32::EPSILON;
                        let label = format!("{scale:.0}x");
                        if ui
                            .add(egui::Button::new(if selected {
                                RichText::new(label).strong().color(theme::accent())
                            } else {
                                RichText::new(label)
                            }))
                            .clicked()
                        {
                            options.scale = scale;
                        }
                    }
                });
                ui.label(RichText::new("Background").color(theme::dim()));
                ui.horizontal(|ui| {
                    for background in [PngBackground::Opaque, PngBackground::Transparent] {
                        let selected = options.background == background;
                        let label = background.label();
                        if ui
                            .add(egui::Button::new(if selected {
                                RichText::new(label).strong().color(theme::accent())
                            } else {
                                RichText::new(label)
                            }))
                            .clicked()
                        {
                            options.background = background;
                        }
                    }
                });
                ui.separator();
                ui.label(format!("Output: {dimensions}"));
                ui.label(
                    RichText::new(format!(
                        "Selected: {:.0}x, {}",
                        options.scale,
                        options.background.label()
                    ))
                    .color(theme::dim()),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save PNG...").clicked() {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        self.plot_export = if open && !close && !save {
            Some(options)
        } else {
            None
        };
        if save {
            self.export_plot_png(options.scale, options.background);
        }
    }

    /// A11Y-001: Ctrl+Shift+P writes a PNG next to the engine's SVG capture, without a dialog.
    ///
    /// A keyboard user cannot complete a native Save dialog with the keyboard alone on every
    /// platform, so the plot pane gets one export that needs no dialog at all. The file lands in
    /// the directory the engine chose for the capture (the same writable directory the dialog
    /// opens in), and the result is reported in the plot header exactly like a dialog export.
    fn export_plot_png_beside_capture(&mut self) {
        if self.plot.is_empty() {
            self.plot_export_status =
                Some(("PNG export: no engine plot to export yet".to_string(), true));
            return;
        }
        let Some(directory) = self.plot_export_dir() else {
            self.plot_export_status = Some((
                "PNG export: the engine plot capture has no writable directory".to_string(),
                true,
            ));
            return;
        };
        let name = plot::export_file_name(self.plot_title().as_deref(), "png");
        let path = directory.join(&name);
        self.plot_export_status = Some(match self.plot.export_png_to(
            &path,
            2.0,
            PngBackground::Opaque,
        ) {
            Ok([width, height]) => (
                format!(
                    "PNG: {width}x{height} px {} -> {}",
                    PngBackground::Opaque.label(),
                    name
                ),
                false,
            ),
            Err(error) => (error, true),
        });
    }

    /// A11Y-007: the Help > Keyboard Shortcuts dialog.
    ///
    /// Every row is a projection of [`keyboard::BINDINGS`] (`reference_rows`), grouped by the
    /// scope the binding declares, so the dialog cannot list a shortcut the window does not
    /// register or omit one it does. The dialog is keyboard-complete: it opens with its Close
    /// button focused and Esc closes it.
    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            return;
        }
        let mut open = self.shortcuts_open;
        let mut close = false;
        let grab_focus = self.shortcuts_focus;
        self.shortcuts_focus = false;
        let unicode_chord = keyboard::chord_of(Action::ToggleUnicodeGlyphs);
        let mode = self.glyph_mode;
        egui::Window::new("Keyboard shortcuts")
            .id(Id::unique("a11y_shortcuts"))
            .collapsible(false)
            .resizable(true)
            // The reference uses the window: two chord/description pairs per row and an explicit
            // size, wide and tall enough that the whole table is on one screen (a reference that
            // has to be scrolled to find a key is not a reference).
            .default_size(vec2(
                (ctx.content_rect().width() - 160.0).clamp(640.0, 1400.0),
                (ctx.content_rect().height() - 120.0).clamp(360.0, 900.0),
            ))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Generated from the {} registered bindings in keyboard.rs. \
                         Mathematical glyphs: {} ({}).",
                        keyboard::BINDINGS.len(),
                        mode.label(),
                        unicode_chord
                    ))
                    .small()
                    .color(theme::dim()),
                );
                egui::ScrollArea::vertical()
                    .id_salt("a11y_shortcuts_scroll")
                    // The reference is one table: the dialog asks for the height it needs and the
                    // window's own scroll handles a screen too small to show it (no inner max
                    // height, so a row is never culled from the painted tree by nesting).
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        for scope in keyboard::Scope::ALL {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(scope.label().to_uppercase())
                                    .strong()
                                    .color(theme::accent()),
                            );
                            let rows: Vec<keyboard::ReferenceRow> = keyboard::reference_rows()
                                .into_iter()
                                .filter(|row| row.scope == scope)
                                .collect();
                            // Two bindings per row: the whole reference fits on one screen, which
                            // is what makes it readable (and wholly present in a UIA dump)
                            // without scrolling.
                            egui::Grid::new(("a11y_shortcuts_grid", scope.label()))
                                .num_columns(4)
                                .striped(true)
                                .min_col_width(110.0)
                                .show(ui, |ui| {
                                    for pair in rows.chunks(2) {
                                        for row in pair {
                                            ui.label(
                                                RichText::new(row.chord.clone())
                                                    .monospace()
                                                    .color(theme::text()),
                                            );
                                            ui.label(
                                                RichText::new(row.description).color(theme::text()),
                                            );
                                        }
                                        for _ in pair.len()..2 {
                                            ui.label("");
                                            ui.label("");
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                ui.separator();
                let close_button = ui.button("Close");
                if close_button.clicked() {
                    close = true;
                }
                if grab_focus {
                    close_button.request_focus();
                }
            });
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            close = true;
        }
        self.shortcuts_open = open && !close;
    }

    fn workspace_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        let has_snapshot = view.has_snapshot;
        // RUN-003: the pane states the revision and committed prefix it is showing, from the
        // same DocumentView as the variables below it. The status strip reports the live buffer
        // revision; this line reports the committed snapshot's revision. RUN-012: a snapshot
        // whose revision is older than the buffer's (or a display the state machine marked Stale)
        // is labelled, never silently presented as the current result.
        let stale = view.run_state == crate::run_state::RunState::Stale
            || (has_snapshot && view.revision.0 < view.buffer_revision.0);
        let state_line = if has_snapshot {
            format!(
                "rev {} · committed {}/{}{}",
                view.revision.0,
                view.committed_steps,
                view.total,
                if stale {
                    " · STALE: not the current source"
                } else {
                    ""
                }
            )
        } else {
            format!(
                "rev {} · no committed statement{}",
                view.revision.0,
                if stale { " · STALE" } else { "" }
            )
        };
        let mut filter = self.filter.clone();
        // WKS-001/WKS-002: the table rows, their counts and their sort state all come from the
        // frame's one committed capture. The counts are the table row counts, so a displayed
        // count cannot disagree with the rows beside it or with the snapshot's registry capture.
        let variable_count = self.variables_table.rows.len();
        let function_count = self.functions_table.rows.len();
        let variable_rows = self.variables_table.rows.clone();
        let function_rows = self.functions_table.rows.clone();
        let mut variable_sort = self.variables_table.sort;
        let mut function_sort = self.functions_table.sort;
        let mut variable_header_rects: [Option<Rect>; 3] = [None; 3];
        let mut function_header_rects: [Option<Rect>; 3] = [None; 3];
        // RES-004: the RESULT tree's collapse state and the headers this pass paints. The state
        // lives on the window, not in the paint pass, so a repaint or an unrelated update (a
        // quick eval, an edit) cannot reset it; `paint_result_section` clears it only when the
        // committed result changes.
        let expansion = &mut self.expansion;
        let inspector_expansion = &mut self.inspector_expansion;
        let mut inspector_open = self.inspector_open;
        let mut inspector_toggle_rects: Vec<(String, Rect)> = Vec::new();
        let mut workspace_view_tabs: [Option<Rect>; 2] = [None; 2];
        let mut workspace_view_tab_ids: [Option<Id>; 2] = [None; 2];
        let mut workspace_filter_id: Option<Id> = None;
        let mut result_toggle_rects: Vec<(String, Rect)> = Vec::new();
        let pane_focus = self.pane_focus;
        let header_hint = 40.0 + 140.0 + header_text_width(&format!("{variable_count} vars · {function_count} funcs"));
        with_pane(ui, rect, "Workspace", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                let label = ui.label("Filter");
                let filter_field = ui
                    .add(
                        egui::TextEdit::singleline(&mut filter)
                            .hint_text("variable or function")
                            .desired_width(130.0),
                    )
                    .labelled_by(label.id)
                    .on_hover_text("Filter the variables and functions by name (Ctrl+Alt+F)");
                label_input(ui, &filter_field, "Filter variables and functions");
                workspace_filter_id = Some(filter_field.id);
                ui.label(
                    RichText::new(format!("{variable_count} vars · {function_count} funcs"))
                        .color(theme::dim()),
                )
                .on_hover_text(format!(
                    "{variable_count} variables · {function_count} functions"
                ));
            });
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(body.shrink(8.0))
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    // RES-001: constrain the result tree to this pane's width. The tree's
                    // symbolic renderings are long, and a ScrollArea left to size itself from
                    // its widest child would let them widen the content and clip at the border.
                    // The 96 px reserve covers the nested record/array indentation: egui's
                    // indents move the cursor without shrinking the wrap width, so a label
                    // wrapping at the full pane width would still end under the border.
                    let result_width = (body.width() - 96.0).max(120.0);
                    // SYM-001: the Workspace pane has two views over the same committed
                    // snapshot. The Inspector tab never reads the editor buffer; it renders
                    // only `view.result_inspection`, which came from the last commit.
                    // A11Y-004: the view tabs and the revision line wrap instead of running
                    // under the pane border when the interface scale narrows the pane.
                    ui.horizontal_wrapped(|ui| {
                        let result_tab = ui.selectable_label(!inspector_open, "Result");                        workspace_view_tab_ids[0] = Some(result_tab.id);
                        if result_tab.clicked() {
                            inspector_open = false;
                        }
                        workspace_view_tabs[0] = Some(result_tab.rect);
                        let inspector_tab = ui.selectable_label(inspector_open, "Inspector");
                        workspace_view_tab_ids[1] = Some(inspector_tab.id);
                        if inspector_tab.clicked() {
                            inspector_open = true;
                        }
                        workspace_view_tabs[1] = Some(inspector_tab.rect);
                        ui.label(RichText::new(&state_line).small().color(theme::accent()));
                    });
                    ui.add_space(4.0);
                    if inspector_open {
                        egui::ScrollArea::vertical()
                            .id_salt("workspace_inspector_scroll")
                            .show(ui, |ui| {
                                paint_inspector(
                                    ui,
                                    view,
                                    result_width,
                                    inspector_expansion,
                                    &mut inspector_toggle_rects,
                                );
                            });
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("workspace_scroll")
                            .show(ui, |ui| {
                                ui.add_space(4.0);

                                // RES-001: the last committed result's typed tree, built only from
                                // the engine's structured projection. The display text is a caption;
                                // it is never parsed for structure.
                                paint_result_section(
                                    ui,
                                    view,
                                    result_width,
                                    expansion,
                                    &mut result_toggle_rects,
                                );

                            ui.add_space(6.0);
                            ui.separator();
                            // WKS-004: Clear Variables removes the globals and keeps every
                            // function, including the user's `func` definitions. It belongs
                            // beside the variables it clears (the pane header is too narrow for
                            // a fourth control) and is disabled while a run is in flight or
                            // before a run published a workspace.
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("VARIABLES · {variable_count}"))
                                        .small()
                                        .color(theme::dim()),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let can_clear =
                                        has_snapshot && !self.workbench.active_is_running();
                                    if ui
                                        .add_enabled(
                                            can_clear,
                                            egui::Button::new(
                                                RichText::new("Clear Variables").small(),
                                            ),
                                        )
                                        .on_hover_text(
                                            "Remove every variable; function definitions are retained",
                                        )
                                        .clicked()
                                    {
                                        self.workbench.clear_variables_active();
                                    }
                                });
                            });
                            let widths = variable_column_widths(ui.available_width());
                            ui.horizontal(|ui| {
                                for column in VariableColumn::ALL {
                                    let response = ui.add_sized(
                                        [widths[column.index()], 18.0],
                                        egui::Button::new(
                                            RichText::new(
                                                variable_sort.header_text(column, column.label()),
                                            )
                                            .small(),
                                        )
                                        .frame(false),
                                    );
                                    let direction = if variable_sort.is_active(column) {
                                        variable_sort.direction_label()
                                    } else {
                                        "not sorted"
                                    };
                                    variable_header_rects[column.index()] = Some(response.rect);
                                    response.clone().on_hover_text(format!(
                                        "Sort variables by {} ({direction})",
                                        column.label().to_lowercase()
                                    ));
                                    if response.clicked() {
                                        variable_sort.clicked(column);
                                    }
                                }
                            });
                            ui.add_space(1.0);

                            let visible_variables: Vec<&crate::workspace_table::VariableRow> =
                                ordered_variables(&variable_rows, variable_sort)
                                    .into_iter()
                                    .map(|index| &variable_rows[index])
                                    .filter(|row| {
                                        filter.is_empty()
                                            || row.name.contains(filter.as_str())
                                    })
                                    .collect();
                            let shown = visible_variables.len();
                            // A11Y-002: the Variables grid is a `Table` whose rows are `Row`
                            // nodes named after their cells and whose sort headers are
                            // `ColumnHeader`s carrying the sort state.
                            table(
                                ui,
                                format!("Variables table · {shown} rows"),
                                shown,
                                VariableColumn::ALL.len(),
                                |ui| {
                                    ui.horizontal(|ui| {
                                        for column in VariableColumn::ALL {
                                            let direction = if variable_sort.is_active(column) {
                                                variable_sort.direction_label()
                                            } else {
                                                "not sorted"
                                            };
                                            let response = column_header(
                                                ui,
                                                widths[column.index()],
                                                &variable_sort
                                                    .header_text(column, column.label()),
                                                variable_sort.is_active(column),
                                            );
                                            variable_header_rects[column.index()] =
                                                Some(response.rect);
                                            response.clone().on_hover_text(format!(
                                                "Sort variables by {} ({direction})",
                                                column.label().to_lowercase()
                                            ));
                                            if response.clicked() {
                                                variable_sort.clicked(column);
                                            }
                                        }
                                    });
                                    ui.add_space(1.0);
                                    for row in &visible_variables {
                                        let label = format!(
                                            "{} | {} | {}",
                                            row.name,
                                            row.kind,
                                            row.display_cell()
                                        );
                                        table_row(ui, label, |ui| {
                                            table_cell(
                                                ui,
                                                widths[0],
                                                RichText::new(cell_text(&row.name))
                                                    .monospace()
                                                    .color(theme::accent()),
                                                &row.name,
                                            );
                                            table_cell(
                                                ui,
                                                widths[1],
                                                RichText::new(cell_text(&row.kind))
                                                    .monospace()
                                                    .color(theme::dim()),
                                                &row.kind,
                                            );
                                            // WKS-001: a large display value is cut in the cell;
                                            // the full value stays available as the hover text.
                                            table_cell(
                                                ui,
                                                widths[2],
                                                RichText::new(row.display_cell())
                                                    .monospace()
                                                    .color(theme::text()),
                                                &row.display,
                                            );
                                        });
                                    }
                                },
                            );
                            if shown == 0 {
                                let message = if !has_snapshot {
                                    "No run has published a workspace yet."
                                } else if variable_rows.is_empty() {
                                    "The last run defined no variables."
                                } else {
                                    "No variables match the filter."
                                };
                                ui.label(RichText::new(message).color(theme::dim()));
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.label(
                                RichText::new(format!("FUNCTIONS · {function_count}"))
                                    .small()
                                    .color(theme::dim()),
                            );
                            let widths = function_column_widths(ui.available_width());
                            ui.horizontal(|ui| {
                                for column in FunctionColumn::ALL {
                                    let response = ui.add_sized(
                                        [widths[column.index()], 18.0],
                                        egui::Button::new(
                                            RichText::new(
                                                function_sort.header_text(column, column.label()),
                                            )
                                            .small(),
                                        )
                                        .frame(false),
                                    );
                                    let direction = if function_sort.is_active(column) {
                                        function_sort.direction_label()
                                    } else {
                                        "not sorted"
                                    };
                                    function_header_rects[column.index()] = Some(response.rect);
                                    response.clone().on_hover_text(format!(
                                        "Sort functions by {} ({direction})",
                                        column.label().to_lowercase()
                                    ));
                                    if response.clicked() {
                                        function_sort.clicked(column);
                                    }
                                }
                            });
                            ui.add_space(1.0);

                            let visible_functions: Vec<&crate::workspace_table::FunctionRow> =
                                ordered_functions(&function_rows, function_sort)
                                    .into_iter()
                                    .map(|index| &function_rows[index])
                                    .filter(|row| {
                                        filter.is_empty()
                                            || row.name.contains(filter.as_str())
                                    })
                                    .collect();
                            let shown = visible_functions.len();
                            table(
                                ui,
                                format!("Functions table · {shown} rows"),
                                shown,
                                FunctionColumn::ALL.len(),
                                |ui| {
                                    ui.horizontal(|ui| {
                                        for column in FunctionColumn::ALL {
                                            let direction = if function_sort.is_active(column) {
                                                function_sort.direction_label()
                                            } else {
                                                "not sorted"
                                            };
                                            let response = column_header(
                                                ui,
                                                widths[column.index()],
                                                &function_sort
                                                    .header_text(column, column.label()),
                                                function_sort.is_active(column),
                                            );
                                            function_header_rects[column.index()] =
                                                Some(response.rect);
                                            response.clone().on_hover_text(format!(
                                                "Sort functions by {} ({direction})",
                                                column.label().to_lowercase()
                                            ));
                                            if response.clicked() {
                                                function_sort.clicked(column);
                                            }
                                        }
                                    });
                                    ui.add_space(1.0);
                                    for row in &visible_functions {
                                        let parameters = row.parameters_text();
                                        let label = format!(
                                            "{} | {} | {}",
                                            row.name,
                                            parameters,
                                            row.origin_label()
                                        );
                                        table_row(ui, label, |ui| {
                                            table_cell(
                                                ui,
                                                widths[0],
                                                RichText::new(cell_text(&row.name))
                                                    .monospace()
                                                    .color(theme::accent()),
                                                &row.name,
                                            );
                                            table_cell(
                                                ui,
                                                widths[1],
                                                RichText::new(cell_text(&parameters))
                                                    .monospace()
                                                    .color(theme::text()),
                                                &parameters,
                                            );
                                            let origin_hover = match &row.plugin {
                                                Some(plugin) => format!("plugin: {plugin}"),
                                                None => row.origin_label().to_string(),
                                            };
                                            table_cell(
                                                ui,
                                                widths[2],
                                                RichText::new(row.origin_label())
                                                    .monospace()
                                                    .color(origin_color(row.origin)),
                                                &origin_hover,
                                            );
                                        });
                                        // WKS-002: the registry's summary, when it publishes one,
                                        // is a second line under the function; it is cut like any
                                        // other cell and the full summary is the hover text.
                                        if let (Some(summary_cell), Some(summary)) =
                                            (row.summary_cell(), row.summary.as_deref())
                                        {
                                            ui.horizontal(|ui| {
                                                ui.add_space(6.0);
                                                ui.allocate_ui_with_layout(
                                                    vec2(
                                                        (ui.available_width() - 6.0).max(40.0),
                                                        14.0,
                                                    ),
                                                    Layout::left_to_right(Align::Center),
                                                    |ui| {
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(summary_cell)
                                                                    .small()
                                                                    .color(theme::dim()),
                                                            )
                                                            .truncate(),
                                                        )
                                                        .on_hover_text(summary.to_string());
                                                    },
                                                );
                                            });
                                        }
                                    }
                                },
                            );
                            if shown == 0 {
                                let message = if !has_snapshot {
                                    "No run has published a workspace yet."
                                } else if function_rows.is_empty() {
                                    "The engine published no functions."
                                } else {
                                    "No functions match the filter."
                                };
                                ui.label(RichText::new(message).color(theme::dim()));
                            }
                        });
                    }
                },
            );
        });
        self.filter = filter;
        self.variables_table.sort = variable_sort;
        self.functions_table.sort = function_sort;
        self.variable_header_rects = variable_header_rects;
        self.function_header_rects = function_header_rects;
        self.result_toggle_rects = result_toggle_rects;
        self.inspector_open = inspector_open;
        self.inspector_toggle_rects = inspector_toggle_rects;
        self.workspace_view_tabs = workspace_view_tabs;
        // A11Y-001: the workspace group's first control is the Result tab; Ctrl+Alt+F targets the
        // filter. Both requests are satisfied on the pass that painted the widget.
        self.workspace_tab_ids = workspace_view_tab_ids;
        self.workspace_filter_id = workspace_filter_id;
        if pane_focus == Some(Pane::Workspace) {
            if let Some(id) = self.workspace_tab_ids[0] {
                self.pane_focus = None;
                ui.ctx()
                    .memory_mut(|memory| memory.request_focus(id));
            }
        }
        if self.filter_focus {
            if let Some(id) = self.workspace_filter_id {
                self.filter_focus = false;
                ui.ctx()
                    .memory_mut(|memory| memory.request_focus(id));
            }
        }
    }

    fn logs_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        // A11Y-001: the logs group's first control is the quick-eval field.
        if self.pane_focus == Some(Pane::Logs) {
            self.pane_focus = None;
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(quick_eval_id()));
        }
        let active = self.workbench.active();
        let legend = log_kind_legend();
        let header_line = {
            let (line, column) = self.editors[active].line_col_1based();
            format!(
                "run log · caret Ln {line}, Col {column} · engine {}",
                self.workbench.engine_name()
            )
        };
        let header_hint = header_text_width(&header_line) + 8.0 + 60.0;
        with_pane(ui, rect, "Logs", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                ui.label(RichText::new(&header_line).color(theme::dim()))
                    .on_hover_text(format!("Log kinds: {legend}"));
            });
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(body.shrink2(vec2(10.0, 4.0)))
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    // RUN-002: the quick-eval one-liner lives above the log list, so it stays
                    // visible with an empty log and cannot be confused with a history row.
                    self.quick_eval_row(ui);
                    let logs = &view.logs;
                    if logs.is_empty() {
                        ui.label(RichText::new("No log entries yet. Run the document.").color(theme::dim()));
                        // The legend names every kind and marker before the first line exists,
                        // so the vocabulary is discoverable even in an empty pane.
                        ui.label(RichText::new(legend.clone()).small().color(theme::dim()));
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("logs_scroll")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for entry in logs {
                                // LOG-002: the marker is part of the line's rendered model, so
                                // the pane paints exactly the string the unit tests pin.
                                let color = match entry.kind {
                                    LogKind::Echo => theme::log_echo(),
                                    LogKind::Stdout => theme::text(),
                                    LogKind::Result => theme::accent(),
                                    LogKind::Diagnostic => theme::error(),
                                    LogKind::Timing => theme::warning(),
                                    LogKind::Progress => theme::progress(),
                                    LogKind::Info => theme::dim(),
                                };
                                ui.label(RichText::new(entry.render()).monospace().color(color));
                            }
                        });
                },
            );
        });
    }

    /// The one-line quick-eval field and its keyboard contract (RUN-002 / LOG-003).
    ///
    /// The label is visible and the hint text is the reference placeholder. Enter submits; Esc
    /// leaves the field without submitting and without editing the document; Up/Down restore
    /// submitted expressions newest first. Only an accepted submission clears the field.
    fn quick_eval_row(&mut self, ui: &mut Ui) {
        let mut submit = false;
        let mut navigate: i8 = 0;
        ui.horizontal(|ui| {
            let label = ui.label(RichText::new("Quick eval").strong().color(theme::text()));
            let id = quick_eval_id();
            let response = ui
                .add(
                    egui::TextEdit::singleline(self.quick_eval.text_mut())
                        .id(id)
                        .hint_text("quick eval — one-liner, press Enter")
                        .desired_width(340.0),
                )
                .labelled_by(label.id);
            label_input(ui, &response, "Quick eval expression");
            self.quick_eval_rect = Some(response.rect);

            if response.has_focus() {
                // Keep Up/Down and Esc for this field. Otherwise egui's focus navigation would
                // move focus before the history handler below can act, and Esc could surrender
                // focus without the field observing it (the DEF-002 class the editor locks its
                // own arrows for). The filter applies to the NEXT pass, so it is installed as
                // soon as the field holds focus.
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        id,
                        egui::EventFilter {
                            tab: false,
                            escape: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                        },
                    );
                });
                if ui.input(|input| input.key_pressed(Key::Escape)) {
                    // Esc leaves the field without submitting and without clearing its text.
                    response.surrender_focus();
                } else if key_pressed_without_modifiers(ui, Key::ArrowUp) {
                    navigate = -1;
                } else if key_pressed_without_modifiers(ui, Key::ArrowDown) {
                    navigate = 1;
                }
            }

            // A single-line egui TextEdit surrenders focus on a bare Enter, so `lost_focus` plus
            // the Enter key is the submit edge. Ctrl+Enter keeps focus and remains the global
            // Run shortcut, which is why this looks at the key event's own modifiers.
            if response.lost_focus() && key_pressed_without_modifiers(ui, Key::Enter) {
                submit = true;
            }

            ui.label(RichText::new("Ctrl+Shift+E").small().color(theme::dim()))
                .on_hover_text("Focus quick eval");
            ui.label(
                RichText::new("Enter runs · Esc leaves the field")
                    .small()
                    .color(theme::dim()),
            );
        });

        if navigate < 0 {
            self.quick_eval.navigate_up();
        } else if navigate > 0 {
            self.quick_eval.navigate_down();
        }
        if submit {
            self.submit_quick_eval();
            // Enter already surrendered the TextEdit's focus; return it so a sequence of quick
            // evaluations needs no click and a refused expression stays editable.
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(quick_eval_id()));
        }
        ui.add_space(2.0);
    }

    /// Submits the field's expression; clears it only when the workbench accepted the request.
    fn submit_quick_eval(&mut self) {
        let Some(expression) = self.quick_eval.expression().map(str::to_string) else {
            return;
        };
        if self.workbench.quick_eval_active(&expression) {
            self.quick_eval.accept(&expression);
        }
        // On refusal the workbench already logged its [info] advisory; the field is left exactly
        // as typed so the expression can be retried.
    }
}

impl eframe::App for WorkbenchApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.ui_root(ui);
    }
}

/// The INSPECTOR view: the engine's six symbolic sections for the last committed result.
///
/// Every value painted here comes from `view.result_inspection`, the payload the engine adapter
/// captured beside the result when it was committed. The editor buffer is not read, and no
/// section is reconstructed from a display string. A section the engine did not supply paints
/// the engine's (or the adapter's) specific reason instead of an empty box.
fn paint_inspector(
    ui: &mut Ui,
    view: &DocumentView,
    width: f32,
    expansion: &mut InspectorExpansion,
    toggles: &mut Vec<(String, Rect)>,
) {
    ui.set_max_width(width.max(120.0));
    ui.label(
        RichText::new("INSPECTOR · last committed result")
            .small()
            .color(theme::dim()),
    );
    if !view.has_snapshot {
        ui.label(RichText::new("No run has published a result yet.").color(theme::dim()));
        return;
    }
    let Some(payload) = view.result_inspection.as_ref() else {
        ui.label(
            RichText::new(
                "The engine published no inspection payload for this result. It supplies no \
                 pretty, canonical, tree, assumptions, trace or MathIR for a result it did not \
                 inspect.",
            )
            .small()
            .color(theme::warning()),
        );
        return;
    };
    // SYM-001: bind the tree's collapse map to this payload. The payload is committed data, so
    // an edit to the buffer cannot clear the map while a new committed result can.
    expansion.bind(payload);
    let Some(inspection) = Inspection::from_json(payload) else {
        ui.label(
            RichText::new(
                "The engine's inspection payload was not readable (unexpected desktop schema); \
                 no section is shown.",
            )
            .small()
            .color(theme::warning()),
        );
        return;
    };

    let subject = &inspection.subject;
    let detail = subject
        .detail
        .as_deref()
        .map(|detail| format!(" · {detail}"))
        .unwrap_or_default();
    ui.label(
        RichText::new(format!(
            "subject  {} · {}{detail}",
            subject.kind, subject.path
        ))
        .monospace()
        .color(theme::accent()),
    );

    paint_inspector_heading(ui, "Pretty", "");
    match &inspection.pretty {
        Section::Supplied(text) => paint_inspector_text(ui, text),
        Section::Missing(reason) => paint_inspector_reason(ui, reason),
    }

    paint_inspector_heading(ui, "Canonical", "");
    match &inspection.canonical {
        Section::Supplied(text) => paint_inspector_text(ui, text),
        Section::Missing(reason) => paint_inspector_reason(ui, reason),
    }

    match &inspection.tree {
        Section::Supplied(tree) => {
            paint_inspector_heading(
                ui,
                "Expression tree",
                &count_label(tree.count(), "node", "nodes"),
            );
            paint_inspector_node(ui, tree, TREE_ROOT_PATH, expansion, toggles);
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "Expression tree", "");
            paint_inspector_reason(ui, reason);
        }
    }

    match &inspection.assumptions {
        Section::Supplied(assumptions) => {
            paint_inspector_heading(
                ui,
                "Assumptions",
                &count_label(assumptions.len(), "row", "rows"),
            );
            if assumptions.is_empty() {
                ui.label(
                    RichText::new(
                        "No active assumptions constrain the free symbols of this result.",
                    )
                    .small()
                    .color(theme::dim()),
                );
            }
            for assumption in assumptions {
                paint_inspector_assumption(ui, assumption);
            }
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "Assumptions", "");
            paint_inspector_reason(ui, reason);
        }
    }

    match &inspection.trace {
        Section::Supplied(steps) => {
            paint_inspector_heading(
                ui,
                "Simplification trace",
                &count_label(steps.len(), "step", "steps"),
            );
            if steps.is_empty() {
                ui.label(
                    RichText::new(
                        "The engine's TransformResult recorded 0 rewrite steps; the expression \
                         was already in its target form.",
                    )
                    .small()
                    .color(theme::dim()),
                );
            }
            for (index, step) in steps.iter().enumerate() {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("{}. {}", index + 1, step.rule_id))
                        .monospace()
                        .color(theme::accent()),
                );
                ui.label(
                    RichText::new(format!("classification  {}", step.classification))
                        .small()
                        .color(theme::dim()),
                );
                ui.label(
                    RichText::new(format!("before  {}", step.before))
                        .monospace()
                        .color(theme::text()),
                );
                if !step.before_canonical.is_empty() {
                    ui.label(
                        RichText::new(format!("canonical  {}", step.before_canonical))
                            .small()
                            .monospace()
                            .color(theme::dim()),
                    );
                }
                ui.label(
                    RichText::new(format!("after   {}", step.after))
                        .monospace()
                        .color(theme::text()),
                );
                if !step.after_canonical.is_empty() {
                    ui.label(
                        RichText::new(format!("canonical  {}", step.after_canonical))
                            .small()
                            .monospace()
                            .color(theme::dim()),
                    );
                }
                if step.conditions.is_empty() {
                    ui.label(
                        RichText::new("requires  none")
                            .small()
                            .monospace()
                            .color(theme::dim()),
                    );
                } else {
                    ui.label(RichText::new("requires").small().color(theme::dim()));
                    for condition in &step.conditions {
                        paint_inspector_assumption(ui, condition);
                    }
                }
            }
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "Simplification trace", "");
            paint_inspector_reason(ui, reason);
        }
    }

    match &inspection.mathir {
        Section::Supplied(mathir) => {
            let result_type = mathir
                .result_type
                .as_deref()
                .map(|kind| format!(" · result {kind}"))
                .unwrap_or_default();
            paint_inspector_heading(
                ui,
                "MathIR",
                &format!(
                    "compile · parameters [{}]{result_type}",
                    mathir.parameters.join(", ")
                ),
            );
            // SYM-007 groundwork: monospaced, selectable, and scrollable both ways. The outer
            // pane scrolls vertically; this inner area bounds the IR's own height and width.
            egui::ScrollArea::both()
                .id_salt("inspector_mathir")
                .max_height(240.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&mathir.text).monospace().color(theme::text()))
                            .selectable(true)
                            .extend(),
                    );
                });
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "MathIR", "");
            paint_inspector_reason(ui, reason);
        }
    }
}

/// `1 node` / `5 nodes`, so a one-item section does not read as a count bug.
fn count_label(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn paint_inspector_heading(ui: &mut Ui, title: &str, detail: &str) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(title).small().color(theme::dim()));
        if !detail.is_empty() {
            ui.label(RichText::new(detail).small().color(theme::accent()));
        }
    });
}

fn paint_inspector_text(ui: &mut Ui, text: &str) {
    ui.add(
        egui::Label::new(RichText::new(text).monospace().color(theme::text()))
            .selectable(true)
            .wrap(),
    );
}

fn paint_inspector_reason(ui: &mut Ui, reason: &str) {
    ui.label(RichText::new(reason).small().color(theme::warning()));
}

fn paint_inspector_assumption(ui: &mut Ui, assumption: &crate::inspector::Assumption) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("[{}]", assumption.source))
                .small()
                .color(theme::dim()),
        );
        ui.label(RichText::new(assumption.type_label()).small().color(theme::accent()));
        ui.label(RichText::new(&assumption.label).monospace().color(theme::text()));
    });
    if let Some(canonical) = &assumption.canonical {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("canonical").small().color(theme::dim()));
            ui.label(RichText::new(canonical).small().monospace().color(theme::dim()));
        });
    }
}

/// Paints one expression-tree node. A node with children is a collapsible button header whose
/// accessible name carries the node kind, the engine pretty rendering, the node path and the
/// state; a leaf is a single indented row. Child order is the engine's child order.
fn paint_inspector_node(
    ui: &mut Ui,
    node: &crate::inspector::TreeNode,
    path: &str,
    expansion: &mut InspectorExpansion,
    toggles: &mut Vec<(String, Rect)>,
) {
    let label = node.header();
    if node.children.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(label).monospace().color(theme::text()));
        });
        return;
    }
    let visible = format!("{} {}", expansion.glyph(path), label);
    let response = ui
        .push_id(path, |ui| {
            ui.add(egui::Button::new(RichText::new(visible).monospace().color(theme::accent())).frame(false))
        })
        .inner;
    if response.clicked() {
        expansion.toggle(path);
        ui.memory_mut(|memory| memory.request_focus(response.id));
        ui.ctx().request_repaint();
    }
    let accessible = expansion.accessible_name(path, &label);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, accessible.clone()));
    let action = if expansion.is_collapsed(path) {
        "expand"
    } else {
        "collapse"
    };
    response.clone().on_hover_text(format!(
        "{path} · {} — click, or press Space/Enter, to {action}",
        expansion.state_word(path)
    ));
    toggles.push((path.to_string(), response.rect));
    if !expansion.is_collapsed(path) {
        ui.indent(path, |ui| {
            for (index, child) in node.children.iter().enumerate() {
                paint_inspector_node(ui, child, &tree_path(path, index), expansion, toggles);
            }
        });
    }
}

/// The RESULT section: the engine's display text and the typed tree beside it.
///
/// The tree is built by [`ResultProjection::tree`] from `view.result_structured` alone; the
/// display string is painted as a caption and is never parsed. An engine that publishes no
/// projection gets an explicit empty state rather than a display-derived guess.
///
/// RES-004: `expansion` is the per-path collapse state of this result. It is bound to the
/// projection before painting, so a repaint of the same committed value keeps every decision
/// while a new result clears the map. `toggles` receives every painted header's path and
/// rectangle; the pointer tests click those rectangles and the pane can use them for focus.
fn paint_result_section(
    ui: &mut Ui,
    view: &DocumentView,
    width: f32,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    ui.set_max_width(width.max(120.0));
    ui.label(
        RichText::new("RESULT · structured projection")
            .small()
            .color(theme::dim()),
    );
    if !view.has_snapshot {
        ui.label(RichText::new("No run has published a result yet.").color(theme::dim()));
        return;
    }
    let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
    // RES-004: the one place the expansion map is scoped to its result. A repaint of the same
    // committed snapshot returns false and keeps every path; a commit that publishes a different
    // value replaces the map instead of applying paths that name the old tree.
    expansion.bind(&projection);
    match projection.tree() {
        Some(tree) => {
            // The display caption stays one line: the full display is in the Logs pane and on
            // hover. It is painted beside the tree, never read by it.
            let full = projection.display.clone();
            let caption: String = full.chars().take(72).collect();
            let caption = if full.chars().count() > 72 {
                format!("{caption}…")
            } else {
                caption
            };
            ui.label(
                RichText::new(format!("display: {caption}"))
                    .small()
                    .monospace()
                    .color(theme::dim()),
            )
            .on_hover_text(full);
            paint_structured_node(ui, &tree, "result", expansion, toggles);
        }
        None => {
            ui.label(
                RichText::new(
                    "The engine published no structured value for this result; only the display \
                     text is available.",
                )
                .small()
                .color(theme::dim()),
            );
        }
    }
}

/// Paints one collapsible header row and returns true when its node is open after the click.
///
/// The header is an egui button, so a pointer click and Space/Enter on a focused header arrive
/// as the same `clicked()` and go through the same [`ExpansionState::toggle`] call. A click also
/// moves keyboard focus onto the header it changed, so the next Space/Enter lands on the node
/// the user just touched instead of typing into the document.
///
/// The visible text carries the ▸/▾ indicator; the accessible name carries the same label, the
/// node's path and the state word, so a UIA client can tell which node is collapsed.
fn paint_toggle(
    ui: &mut Ui,
    expansion: &mut ExpansionState,
    path: &str,
    label: &str,
    toggles: &mut Vec<(String, Rect)>,
) -> bool {
    let visible = expansion.header_text(path, label);
    let response = ui
        .push_id(path, |ui| {
            ui.add(egui::Button::new(RichText::new(visible).monospace().color(theme::accent())).frame(false))
        })
        .inner;
    if response.clicked() {
        expansion.toggle(path);
        ui.memory_mut(|memory| memory.request_focus(response.id));
        // The indicator and the state word are painted from the map, so ask for the frame that
        // renders the new state now that the input has been consumed.
        ui.ctx().request_repaint();
    }
    let accessible = expansion.accessible_name(path, label);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, accessible.clone()));
    let action = if expansion.is_collapsed(path) {
        "expand"
    } else {
        "collapse"
    };
    response.clone().on_hover_text(format!(
        "{path} · {} — click, or press Space/Enter, to {action}",
        expansion.state_word(path)
    ));
    toggles.push((path.to_string(), response.rect));
    !expansion.is_collapsed(path)
}

/// Paints one structured node with its toggle header. Scalars and the absent value have no
/// children, so they stay a single row and are never given a toggle.
fn paint_structured_node(
    ui: &mut Ui,
    node: &StructuredNode,
    path: &str,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    match node {
        StructuredNode::Scalar(scalar) => paint_scalar(ui, scalar),
        StructuredNode::Null => paint_absent(ui),
        other => {
            if paint_toggle(ui, expansion, path, &other.headline(), toggles) {
                ui.indent(path, |ui| {
                    paint_node_body(ui, other, path, expansion, toggles);
                });
            }
        }
    }
}

/// Paints a node's children without a header of its own: the caller paints the header (a record
/// field, an array row or a matrix cell already names the node).
fn paint_node_body(
    ui: &mut Ui,
    node: &StructuredNode,
    path: &str,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    match node {
        StructuredNode::Scalar(scalar) => paint_scalar(ui, scalar),
        StructuredNode::Null => paint_absent(ui),
        StructuredNode::Symbolic(symbolic) => paint_symbolic_body(ui, symbolic),
        StructuredNode::Array(array) => paint_array_body(ui, array, path, expansion, toggles),
        StructuredNode::Record(record) => paint_record_body(ui, record, path, expansion, toggles),
    }
}

/// One scalar row: the projection kind, the value, then the remaining published fields.
fn paint_scalar(ui: &mut Ui, scalar: &crate::results_tree::ScalarNode) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&scalar.kind).monospace().color(theme::dim()));
        ui.label(RichText::new(&scalar.value).monospace().color(theme::text()));
        for (key, value) in &scalar.details {
            ui.label(RichText::new(format!("{key}={value}")).small().color(theme::dim()));
        }
    });
}

/// The absent value, painted where a node would be.
fn paint_absent(ui: &mut Ui) {
    ui.label(RichText::new("Null - no value").monospace().color(theme::dim()));
}

/// Paints one matrix cell. A scalar (the common case) is compact on the cell's row; a symbolic
/// cell is itself a collapsible node, so its pretty and canonical renderings hide behind the
/// same ▸/▾ header every other node uses.
fn paint_cell(
    ui: &mut Ui,
    node: &StructuredNode,
    path: &str,
    column: usize,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    match node {
        StructuredNode::Scalar(scalar) => paint_scalar(ui, scalar),
        StructuredNode::Null => {
            ui.label(RichText::new("Null").monospace().color(theme::dim()));
        }
        other => {
            let label = format!("[{column}] {}", other.headline());
            if paint_toggle(ui, expansion, path, &label, toggles) {
                ui.indent(path, |ui| {
                    paint_node_body(ui, other, path, expansion, toggles);
                });
            }
        }
    }
}

/// Paints a symbolic node's published renderings and metadata.
fn paint_symbolic_body(ui: &mut Ui, symbolic: &crate::results_tree::SymbolicNode) {
    ui.add(
        egui::Label::new(
            RichText::new(format!("pretty     {}", symbolic.pretty))
                .monospace()
                .color(theme::text()),
        )
        .wrap(),
    );
    ui.add(
        egui::Label::new(
            RichText::new(format!("canonical  {}", symbolic.canonical))
                .monospace()
                .color(theme::text()),
        )
        .wrap(),
    );
    if let Some(domain) = &symbolic.domain {
        ui.label(
            RichText::new(format!("domain     {domain}"))
                .small()
                .color(theme::dim()),
        );
    }
    if !symbolic.free_symbols.is_empty() {
        ui.label(
            RichText::new(format!("free       {}", symbolic.free_symbols.join(", ")))
                .small()
                .color(theme::dim()),
        );
    }
    if let Some(count) = &symbolic.node_count {
        ui.label(
            RichText::new(format!("nodes      {count}"))
                .small()
                .color(theme::dim()),
        );
    }
    if symbolic.truncated {
        ui.label(
            RichText::new("this rendering was shortened by a print budget")
                .small()
                .color(theme::warning()),
        );
    }
    for (key, value) in &symbolic.details {
        ui.label(RichText::new(format!("{key}={value}")).small().color(theme::dim()));
    }
}

/// Paints an array's body. Every row is a collapsible group: a 2-D array's rows hold the cells
/// laid out as columns; a 1-D array's rows hold one element each. A higher-rank array names its
/// shape and lists its row-major elements exactly like a flat array.
fn paint_array_body(
    ui: &mut Ui,
    array: &ArrayNode,
    path: &str,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    let rows = array.rows();
    if array.is_matrix() && !rows.is_empty() {
        let columns = array.shape[1].max(1);
        let cell_width = ((ui.available_width() - 44.0) / columns as f32).max(52.0);
        for (row_index, row) in rows.iter().enumerate() {
            let row_path = element_path(path, row_index);
            let label = format!(
                "[{row_index}] {} cell{}",
                row.len(),
                if row.len() == 1 { "" } else { "s" }
            );
            if paint_toggle(ui, expansion, &row_path, &label, toggles) {
                ui.indent(&row_path, |ui| {
                    egui::Grid::new(format!("{row_path}.cells"))
                        .num_columns(columns)
                        .spacing([6.0, 3.0])
                        .show(ui, |ui| {
                            for (column_index, cell) in row.iter().enumerate() {
                                let cell_path = element_path(&row_path, column_index);
                                ui.allocate_ui_with_layout(
                                    vec2(cell_width, 14.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.set_width(cell_width);
                                        paint_cell(
                                            ui,
                                            cell,
                                            &cell_path,
                                            column_index,
                                            expansion,
                                            toggles,
                                        );
                                    },
                                );
                            }
                            ui.end_row();
                        });
                });
            }
        }
        return;
    }
    if array.elements.is_empty() {
        ui.label(RichText::new("empty").small().color(theme::dim()));
        return;
    }
    for (index, element) in array.elements.iter().enumerate() {
        let child_path = element_path(path, index);
        let label = match element {
            StructuredNode::Scalar(_) | StructuredNode::Null => format!("[{index}]"),
            other => format!("[{index}] {}", other.headline()),
        };
        if paint_toggle(ui, expansion, &child_path, &label, toggles) {
            ui.indent(&child_path, |ui| {
                paint_node_body(ui, element, &child_path, expansion, toggles);
            });
        }
    }
}

/// Paints a record body: every published field is one collapsible node. Its header names the
/// field and, when the value is structured, the value's headline; its body is the value. A
/// scalar value is a row inside the field's body, so collapsing a scalar field hides its value
/// without dropping it from the model.
fn paint_record_body(
    ui: &mut Ui,
    record: &crate::results_tree::RecordNode,
    path: &str,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    if record.fields.is_empty() {
        ui.label(RichText::new("no fields").small().color(theme::dim()));
        return;
    }
    for field in &record.fields {
        let child_path = field_path(path, &field.name);
        let label = match &field.value {
            StructuredNode::Scalar(_) | StructuredNode::Null => format!("{}:", field.name),
            other => format!("{}: {}", field.name, other.headline()),
        };
        if paint_toggle(ui, expansion, &child_path, &label, toggles) {
            ui.indent(&child_path, |ui| {
                paint_node_body(ui, &field.value, &child_path, expansion, toggles);
            });
        }
    }
}

/// A11Y-001: the table's chord for one egui key event, if the event names a key the table knows.
///
/// The conversion is the only place the window's key names meet the table's platform-independent
/// names, so every binding (old and new) is matched by the same rules.
fn shortcut_of_event(event: &egui::Event) -> Option<Shortcut> {
    let egui::Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    let key = match key {
        Key::Enter => KeyName::Enter,
        Key::Escape => KeyName::Escape,
        Key::Tab => KeyName::Tab,
        Key::Space => KeyName::Space,
        Key::Comma => KeyName::Comma,
        Key::F1 => KeyName::F1,
        Key::F6 => KeyName::F6,
        Key::F8 => KeyName::F8,
        // Both the `=` key and the `+` key it shifts to are the registered "zoom in" chord.
        Key::Equals | Key::Plus => KeyName::Plus,
        Key::Minus => KeyName::Minus,
        Key::Num0 => KeyName::Digit(0),
        Key::Num1 => KeyName::Digit(1),
        Key::Num2 => KeyName::Digit(2),
        Key::Num3 => KeyName::Digit(3),
        Key::Num4 => KeyName::Digit(4),
        Key::Num5 => KeyName::Digit(5),
        Key::A => KeyName::Char('a'),
        Key::B => KeyName::Char('b'),
        Key::C => KeyName::Char('c'),
        Key::D => KeyName::Char('d'),
        Key::E => KeyName::Char('e'),
        Key::F => KeyName::Char('f'),
        Key::G => KeyName::Char('g'),
        Key::H => KeyName::Char('h'),
        Key::I => KeyName::Char('i'),
        Key::J => KeyName::Char('j'),
        Key::K => KeyName::Char('k'),
        Key::L => KeyName::Char('l'),
        Key::M => KeyName::Char('m'),
        Key::N => KeyName::Char('n'),
        Key::O => KeyName::Char('o'),
        Key::P => KeyName::Char('p'),
        Key::Q => KeyName::Char('q'),
        Key::R => KeyName::Char('r'),
        Key::S => KeyName::Char('s'),
        Key::T => KeyName::Char('t'),
        Key::U => KeyName::Char('u'),
        Key::V => KeyName::Char('v'),
        Key::W => KeyName::Char('w'),
        Key::X => KeyName::Char('x'),
        Key::Y => KeyName::Char('y'),
        Key::Z => KeyName::Char('z'),
        _ => return None,
    };
    Some(Shortcut::new(
        key,
        modifiers.ctrl || modifiers.command,
        modifiers.shift,
        modifiers.alt,
    ))
}

/// Paints one keyboard-focus ring for the whole window (A11Y-003).
fn paint_focus_ring(ctx: &egui::Context) {
    a11y::paint_focus_ring(ctx);
}

/// The fixed widths of the variables table's three columns, fitted to the pane.
fn variable_column_widths(available: f32) -> [f32; 3] {
    let usable = (available - 16.0).max(96.0);
    let name = (usable * 0.34).max(40.0);
    let kind = (usable * 0.26).max(34.0);
    let display = (usable - name - kind).max(40.0);
    [name, kind, display]
}

/// The fixed widths of the functions table's three columns, fitted to the pane.
fn function_column_widths(available: f32) -> [f32; 3] {
    let usable = (available - 16.0).max(120.0);
    let name = (usable * 0.36).max(44.0);
    let parameters = (usable * 0.32).max(38.0);
    let origin = (usable - name - parameters).max(44.0);
    [name, parameters, origin]
}

/// One aligned table cell: the (already truncated) text, with the full value as its tooltip.
///
/// The cell allocates a fixed width so every row lines up under its header; `Label::truncate`
/// is the second clip for a value that is still wider than the column (the character cut in
/// [`crate::workspace_table::cell_text`] is the deterministic one the tests pin).
fn table_cell(ui: &mut Ui, width: f32, text: RichText, full: &str) {
    ui.allocate_ui_with_layout(
        vec2(width, 16.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add(egui::Label::new(text).truncate())
                .on_hover_text(full.to_string());
        },
    );
}

/// The visible origin colour: core builtins are quiet, plugin builtins warn-coloured and user
/// definitions take the accent so the three rows are distinguishable at a glance.
fn origin_color(origin: FunctionOrigin) -> Color32 {
    match origin {
        FunctionOrigin::Builtin => theme::dim(),
        FunctionOrigin::Plugin => theme::warning(),
        FunctionOrigin::User => theme::accent(),
    }
}

/// VIS-002: the semantic colour of the status strip's run-state text.
///
/// Every branch is a theme token, so the status strip carries the positive/error/warning/progress
/// semantic roles in both themes instead of one fixed accent colour.
fn status_state_color(state: crate::run_state::RunState) -> Color32 {
    use crate::run_state::RunState;
    match state {
        RunState::Idle => theme::accent(),
        RunState::Queued | RunState::Running | RunState::Cancelling => theme::progress(),
        RunState::Finished => theme::positive(),
        RunState::Failed => theme::error(),
        RunState::Cancelled | RunState::Stale => theme::warning(),
    }
}

/// The stable egui id of the one-line quick-eval field.
///
/// The window uses it for the Ctrl+Shift+E focus shortcut and for the focus-lock filter that
/// keeps Up/Down in the field for history navigation; the unit tests use it to assert focus.
pub fn quick_eval_id() -> Id {
    Id::unique("quick_eval_input")
}

/// True when this pass contains a press of `key` carrying no modifiers.
///
/// The key event's own modifier mask is used, not the frame's aggregate state: a modifier that
/// was pressed for the focus shortcut and is released in the same pass must not make a later
/// bare Enter look like Ctrl+Enter, and the event mask is what describes the key press itself.
fn key_pressed_without_modifiers(ui: &Ui, key: Key) -> bool {
    ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: pressed_key,
                    pressed: true,
                    modifiers,
                    ..
                } if *pressed_key == key && !modifiers.any()
            )
        })
    })
}

/// A11Y-002: names one text field's accessibility node.
///
/// A field whose visible label is a separate `Label` node has no name of its own in the raw tree
/// (the relation is `labelled_by`, which the platform adapter resolves but a raw dump does not).
/// Setting the label on the field's own node makes the name explicit for a reader and for the
/// audit.
fn label_input(ui: &Ui, response: &Response, label: &str) {
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
}

/// A11Y-002: gives a panel's node a role as well as a name (the status strip is a `Status`, the
/// tab strip a `TabList`).
fn label_ui_node_as(ui: &Ui, role: egui::Role, label: &str) {
    ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
        node.set_role(role);
        node.set_label(label);
    });
}

/// A11Y-002: one table row as a `Row` node with a name built from its cells.
///
/// A table row painted as bare text was the A11Y-002 defect: a screen reader announced each cell
/// as an unrelated line and never said which record a value belonged to. The accessible name
/// states the whole row (`name | kind | value`), so one arrow-key stop reads one record, and the
/// cell `Text` nodes stay inside it.
fn table_row<R>(ui: &mut Ui, label: String, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(
        UiBuilder::new()
            .accessibility_role(egui::Role::Row)
            .accessibility_label(label),
        |ui| ui.horizontal(|ui| add(ui)).inner,
    )
    .inner
}

/// A11Y-002: a table container with its row and column counts.
fn table<R>(
    ui: &mut Ui,
    label: String,
    rows: usize,
    columns: usize,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.scope_builder(
        UiBuilder::new()
            .accessibility_role(egui::Role::Table)
            .accessibility_label(label),
        |ui| {
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_row_count(rows);
                node.set_column_count(columns);
            });
            add(ui)
        },
    )
    .inner
}

/// A11Y-002: a sortable column header.
///
/// The role is `ColumnHeader` (UIA `DataItem`) instead of `Button`, and the selected state carries
/// the sort direction, so a reader says "Name, column header, sorted ascending" rather than an
/// unlabelled button.
fn column_header(ui: &mut Ui, width: f32, label: &str, sorted: bool) -> Response {
    let response = ui.add_sized(
        [width, 18.0],
        egui::Button::new(RichText::new(label).small()).frame(false),
    );
    let name = label.to_string();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColumnHeader, true, name.clone()));
    // `WidgetInfo::selected` fills the *toggled* property, which is the check-box state; a
    // column header's sort state is its `selected` property in the platform tree.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_selected(sorted));
    response
}

/// A rough natural width for a header label, used only to decide how many header rows a pane
/// needs. It over-estimates slightly so a wrapped header is never allocated one row too few.
fn header_text_width(text: &str) -> f32 {
    text.chars().count() as f32 * 7.0 + 28.0
}

/// A11Y-004: the header height in points for a pane of `width` whose header wants `hint` points.
///
/// At 200% interface scale the window holds fewer points, so a header that fits one row at 100%
/// needs several at 200%. `horizontal_wrapped` breaks rows at whole widgets, so the row count is
/// estimated with a 1.35 allowance over the ideal fill; the header grows instead of clipping, and
/// the body starts below it. Five rows is the cap (the plot header at 200% is the widest).
fn header_height(width: f32, hint: f32) -> f32 {
    let available = (width - 16.0).max(80.0);
    let rows = (1.35 * hint / available).ceil().clamp(1.0, 5.0);
    // 20 points per painted row (the header text is 16-20 points tall) plus a 4 point pad.
    4.0 + rows * 20.0
}

fn header_ui(header: Rect) -> UiBuilder {
    // A top-down parent is what makes `horizontal_wrapped` start at the header's top edge: in a
    // left-to-right parent the wrapped row would be centred on its 18 pt initial height inside
    // the header's full height, which pushes the content down by a row and overflows the body.
    UiBuilder::new()
        .max_rect(header.shrink2(vec2(8.0, 3.0)))
        .layout(Layout::top_down(Align::Min))
}

/// A11Y-004: the wrapping header row every pane uses.
///
/// `horizontal_wrapped` is the one-line change that lets the same controls flow onto a second or
/// third row when the interface scale leaves the pane narrower, instead of running under the
/// neighbouring pane's border.
fn header_row(ui: &mut Ui, header: Rect, add: impl FnOnce(&mut Ui)) {
    ui.scope_builder(header_ui(header), |ui| {
        ui.horizontal_wrapped(add);
    });
}

/// The kind vocabulary, as `label: marker` pairs, for the pane header hover and the empty pane.
fn log_kind_legend() -> String {
    LogKind::ALL
        .iter()
        .map(|kind| format!("{}: {}", kind.label(), kind.marker()))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// One-line summary of a failed run's diagnostic for the editor pane header.
///
/// RUN-014: the header carries every field the row names — code, category, message, line and
/// column — compactly enough for a pane header. Suggestions are stated in the hover text and in
/// the log, where there is room for the whole list.
fn diagnostic_summary(diagnostic: &Diagnostic) -> String {
    let message = diagnostic.message.lines().next().unwrap_or_default();
    let mut short: String = message.chars().take(70).collect();
    if message.chars().count() > 70 {
        short.push('…');
    }
    let class = match (&diagnostic.code, &diagnostic.category) {
        (Some(code), Some(category)) => format!("{code} ({category}) · "),
        (Some(code), None) => format!("{code} · "),
        (None, Some(category)) => format!("({category}) · "),
        (None, None) => String::new(),
    };
    format!(
        "Diagnostic {class}Ln {}, Col {}: {}",
        diagnostic.line, diagnostic.column, short
    )
}

fn with_pane<R>(
    ui: &mut Ui,
    rect: Rect,
    title: &str,
    header_hint: f32,
    add: impl FnOnce(&mut Ui, Rect, Rect) -> R,
) -> R {
    let header_h = header_height(rect.width(), header_hint);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .accessibility_role(egui::Role::Pane)
            .accessibility_label(title),
        |ui| {
            ui.painter().rect_filled(rect, 4.0, theme::panel_bg());
            ui.painter()
                .rect_stroke(rect, 4.0, Stroke::new(1.0, theme::border()), StrokeKind::Inside);
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), header_h));
            ui.painter().rect_filled(header, 4.0, theme::header_bg());
            let body = Rect::from_min_max(
                pos2(rect.left() + 1.0, header.bottom() + 1.0),
                pos2(rect.right() - 1.0, rect.bottom() - 1.0),
            );
            add(ui, header, body)
        },
    )
    .inner
}

fn splitter(ui: &mut Ui, rect: Rect, salt: &str, horizontal_axis: bool, label: &str) -> f32 {
    let id = Id::unique(salt);
    let response = ui.interact(rect, id, Sense::click_and_drag());
    if response.clicked() || response.drag_started() || response.is_pointer_button_down_on() {
        response.request_focus();
    }
    let focused = response.has_focus();
    let hot = response.hovered() || response.dragged() || focused;
    ui.painter()
        .rect_filled(rect, 0.0, if hot { theme::splitter_hot() } else { theme::splitter_bg() });
    let grip = if horizontal_axis {
        Rect::from_center_size(rect.center(), vec2(2.0, rect.height()))
    } else {
        Rect::from_center_size(rect.center(), vec2(rect.width(), 2.0))
    };
    ui.painter()
        .rect_filled(grip, 1.0, if hot { theme::accent() } else { theme::control_border() });

    if hot {
        ui.ctx().set_cursor_icon(if horizontal_axis {
            CursorIcon::ResizeHorizontal
        } else {
            CursorIcon::ResizeVertical
        });
    }

    let mut delta = if response.dragged() {
        if horizontal_axis {
            response.drag_delta().x
        } else {
            response.drag_delta().y
        }
    } else {
        0.0
    };

    if focused {
        let step = if ui.input(|input| input.modifiers.shift) {
            32.0
        } else {
            8.0
        };
        if horizontal_axis {
            if ui.input(|input| input.key_pressed(Key::ArrowRight)) {
                delta += step;
            }
            if ui.input(|input| input.key_pressed(Key::ArrowLeft)) {
                delta -= step;
            }
            if ui.input(|input| input.key_pressed(Key::Home)) {
                delta -= 1000.0;
            }
            if ui.input(|input| input.key_pressed(Key::End)) {
                delta += 1000.0;
            }
        } else {
            if ui.input(|input| input.key_pressed(Key::ArrowDown)) {
                delta += step;
            }
            if ui.input(|input| input.key_pressed(Key::ArrowUp)) {
                delta -= step;
            }
        }
    }

    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, label));
    delta
}

/// The native window options shared by the workbench and the EDT-016 measurement mode.
pub(crate) fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Lovelace Desktop"),
        ..Default::default()
    }
}

/// Opens the native workbench window and runs it until the user closes it.
pub fn run(initial_path: Option<PathBuf>) -> i32 {
    // Printed at startup so a launch with redirected stderr shows where the engine will write.
    eprintln!(
        "lovelace-desktop: plot directory {}",
        crate::engine::default_plot_dir().display()
    );
    let options = native_options();
    match eframe::run_native(
        "Lovelace Desktop",
        options,
        Box::new(move |creation| {
            // VIS-002: the dark token set is the shipped default. A persisted light preference
            // is applied by the first frame's `ui_root` before any pane paints.
            theme::apply(&creation.egui_ctx, theme::ThemeId::Dark);
            Ok(Box::new(WorkbenchApp::new(initial_path)))
        }),
    ) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("lovelace-desktop: could not open the workbench window: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::CompletionKind;
    use crate::engine::{EngineFunction, EngineState, Evaluator};
    use crate::glyphs;
    use crate::log_kind::LogLine;
    use crate::run_state::RunState;
    use crate::workbench::Workbench;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Instant;

    /// An engine that sleeps in every evaluation, so a run stays in flight long enough for the
    /// window to paint it and for a shortcut to reach it.
    struct SlowEngine {
        delay: Duration,
    }

    impl Evaluator for SlowEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            std::thread::sleep(self.delay);
            Ok("ok".to_string())
        }

        fn variables(&self) -> Vec<(String, String)> {
            vec![("ran".to_string(), "yes".to_string())]
        }
    }

    fn slow_workbench() -> Workbench {
        Workbench::with_factory(Box::new(|| {
            Box::new(SlowEngine {
                delay: Duration::from_millis(30),
            })
        }))
    }

    /// A gate the worker's engine factory waits on, so the first frame after Ctrl+Enter can
    /// deterministically observe `run Queued` before the worker reports `RunStarted`.
    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        condvar: Condvar,
    }

    impl Gate {
        fn wait(&self) {
            let mut open = self.open.lock().expect("gate");
            while !*open {
                open = self.condvar.wait(open).expect("gate wait");
            }
        }

        fn release(&self) {
            let mut open = self.open.lock().expect("gate");
            *open = true;
            self.condvar.notify_all();
        }
    }

    fn gated_workbench(gate: Arc<Gate>) -> Workbench {
        // `with_factory` calls the factory once to learn the engine name; use the named variant
        // so the gate blocks the run worker only, not the test thread.
        Workbench::with_named_factory(
            Box::new(move || {
                gate.wait();
                Box::new(SlowEngine {
                    delay: Duration::from_millis(200),
                })
            }),
            "gated-slow",
        )
    }

    fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn frame(ctx: &egui::Context, app: &mut WorkbenchApp, events: Vec<egui::Event>) {
        frame_sized(ctx, app, vec2(1280.0, 800.0), events);
    }

    /// One headless pass of the whole window at an explicit size. The window-proof layout uses
    /// 2500x1380, the same frame the round-27 captures were taken in, so the Workspace pane lays
    /// the result tree out exactly where the pointer tests click it.
    fn frame_sized(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
        events: Vec<egui::Event>,
    ) {
        let _ = frame_capturing(ctx, app, screen, events, Default::default());
    }

    /// One headless pass with an explicit viewport map, returning everything the integration would
    /// apply (DOC-003's close commands are read from the output).
    fn frame_capturing(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
        events: Vec<egui::Event>,
        viewports: egui::ViewportIdMap<egui::ViewportInfo>,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), screen)),
            events,
            ..Default::default()
        };
        if !viewports.is_empty() {
            input.viewports = viewports;
        }
        let mut output = ctx.run_ui(input, |ui| app.ui_root(ui));
        // A headless pass has no painter to upload the font texture to; mark the deltas handled.
        output.textures_delta.clear();
        output
    }

    /// Delivers one OS close request (the title-bar X) to the window, returning the frame output.
    fn close_request(ctx: &egui::Context, app: &mut WorkbenchApp) -> egui::FullOutput {
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                events: vec![egui::ViewportEvent::Close],
                ..Default::default()
            },
        );
        frame_capturing(ctx, app, vec2(1280.0, 800.0), Vec::new(), viewports)
    }

    /// True when the frame asked the integration to cancel a close (DOC-003's refusal).
    fn cancelled_close(output: &egui::FullOutput) -> bool {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|viewport| {
                viewport
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
            })
            .unwrap_or(false)
    }

    /// True when the frame asked the integration to close the window (DOC-003's approval).
    fn approved_close(output: &egui::FullOutput) -> bool {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|viewport| {
                viewport
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::Close))
            })
            .unwrap_or(false)
    }

    /// The center of the prompt button the test wants to click.
    fn prompt_button(app: &WorkbenchApp, choice: UnsavedAnswer) -> egui::Pos2 {
        app.prompt_buttons
            .iter()
            .find(|(painted, _)| *painted == choice)
            .map(|(_, rect)| rect.center())
            .unwrap_or_else(|| panic!("the {choice:?} button was not painted"))
    }

    /// The buttons the open prompt painted, in their on-screen order.
    fn prompt_choices(app: &WorkbenchApp) -> Vec<&'static str> {
        app.prompt_buttons
            .iter()
            .map(|(choice, _)| choice.label())
            .collect()
    }

    /// A full pointer click at `pos`: move, press, release (one pass each), the same sequence a
    /// user's mouse produces.
    fn pointer_click(ctx: &egui::Context, app: &mut WorkbenchApp, pos: egui::Pos2) {
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                }],
            );
        }
    }

    /// VIS-002: clicking the toolbar theme control switches the whole window's token set at
    /// runtime without touching the document, its caret, its revision, its selection or the
    /// focused widget, and the next pass paints with egui visuals derived from the new tokens.
    #[test]
    fn the_theme_toggle_switches_at_runtime_and_preserves_the_document_and_focus() {
        let mut app = app_with_source("value: 41 + 1\nother: value * 2", 0, 13);
        let ctx = egui::Context::default();
        let screen = vec2(1600.0, 1000.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        // A real edit before the switch makes the preserved state non-trivial: a dirty buffer,
        // a moved caret and an editor that owns focus.
        frame(&ctx, &mut app, vec![egui::Event::Text("7".to_string())]);
        // A non-empty selection is part of "document state": the caret stays after the typed
        // character and the selection anchor stays at column 8 of line 1.
        app.editors[0].selection = Some((0, 8));
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let before_source = app.workbench.documents()[0].source.clone();
        let before_caret = app.editors[0].line_col_1based();
        let before_selection = app.editors[0].selection;
        let before_focus = ctx.memory(|memory| memory.focused());
        let before_revision = app.workbench.documents()[0].revision;
        let before_dirty = app.workbench.documents()[0].dirty();
        assert_eq!(app.theme, theme::ThemeId::Dark, "dark is the shipped default");
        assert_eq!(before_focus, Some(surface_id()), "the editor owns focus");
        assert!(app.workbench.documents()[0].source.contains("41 + 17"));
        assert_eq!(
            ctx.global_style().visuals.panel_fill,
            theme::tokens_for(theme::ThemeId::Dark).app_bg,
            "the first pass must already be token-themed"
        );

        let button = app
            .theme_button
            .expect("the toolbar must paint the theme control");
        assert!(
            Rect::from_min_size(pos2(0.0, 0.0), screen).contains_rect(button),
            "the theme control is off-screen: {button:?}"
        );
        pointer_click(&ctx, &mut app, button.center());

        assert_eq!(app.theme, theme::ThemeId::Light, "the toggle switched");
        assert_eq!(app.settings.theme, theme::ThemeId::Light, "the choice persists");
        assert_eq!(
            app.workbench.documents()[0].source, before_source,
            "the document text must survive the theme switch"
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            before_caret,
            "the caret must survive the theme switch"
        );
        assert_eq!(
            app.editors[0].selection, before_selection,
            "the text selection must survive the theme switch"
        );
        assert_eq!(
            app.workbench.documents()[0].revision, before_revision,
            "the theme switch must not advance the document revision"
        );
        assert_eq!(
            app.workbench.documents()[0].dirty(),
            before_dirty,
            "the theme switch must not change the dirty state"
        );

        // One more pass settles egui's context style; both the explicit paint tokens and the
        // widget visuals must now be the light set. The same pass re-focuses the editor: the
        // release click moved focus to the toolbar, and the toggle handed it back.
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            before_focus,
            "keyboard focus must survive the theme switch"
        );
        let light = theme::tokens_for(theme::ThemeId::Light);
        assert_eq!(ctx.global_style().visuals.panel_fill, light.app_bg);
        assert_eq!(ctx.global_style().visuals.window_fill, light.window_bg);
        assert_eq!(ctx.global_style().visuals.widgets.inactive.bg_fill, light.button_bg);
        assert!(!ctx.global_style().visuals.dark_mode, "light text transfer function");

        // Switching back restores the dark tokens and still preserves the buffer and focus.
        let button = app.theme_button.expect("still painted");
        pointer_click(&ctx, &mut app, button.center());
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert_eq!(app.theme, theme::ThemeId::Dark);
        assert_eq!(app.workbench.documents()[0].source, before_source);
        assert_eq!(app.editors[0].line_col_1based(), before_caret);
        assert_eq!(app.editors[0].selection, before_selection);
        assert_eq!(ctx.memory(|memory| memory.focused()), before_focus);
        assert_eq!(
            ctx.global_style().visuals.panel_fill,
            theme::tokens_for(theme::ThemeId::Dark).app_bg
        );
    }

    /// VIS-002: the theme control must be visible in the shipped 1280x800 window, not only in
    /// the 1600x1000 capture frame.
    #[test]
    fn the_theme_control_is_inside_the_default_window() {
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let button = app.theme_button.expect("the theme control is painted");
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
        assert!(
            screen.contains_rect(button),
            "the theme control is clipped at the default window size: {button:?}"
        );
    }

    /// VIS-002: the status strip paints each run state with its semantic token.
    #[test]
    fn the_status_strip_uses_the_semantic_status_tokens() {
        use crate::run_state::RunState;
        frame(&egui::Context::default(), &mut app_with_source("1", 0, 0), Vec::new());
        let dark = theme::tokens_for(theme::ThemeId::Dark);
        assert_eq!(status_state_color(RunState::Finished), dark.positive);
        assert_eq!(status_state_color(RunState::Failed), dark.error);
        assert_eq!(status_state_color(RunState::Running), dark.progress);
        assert_eq!(status_state_color(RunState::Stale), dark.warning);
        assert_eq!(status_state_color(RunState::Idle), dark.accent);
    }

    #[test]
    fn focus_starts_in_the_editor_and_typed_text_lands_without_a_click() {
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        let ctx = egui::Context::default();

        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "the editor must own keyboard focus on the first frame"
        );

        let lines_before = app.editors[0].line_count();
        frame(&ctx, &mut app, vec![egui::Event::Text("a = 1".to_string())]);

        assert!(
            app.workbench.documents()[0].source.starts_with("a = 1"),
            "typed text did not reach the document: {:?}",
            &app.workbench.documents()[0].source
                [..20.min(app.workbench.documents()[0].source.len())]
        );
        assert_eq!(
            app.editors[0].line_count(),
            lines_before,
            "plain text typed into the editor must not change the line count"
        );
    }

    /// Builds an app whose editor holds `source`, with the caret placed at `(row, col)`.
    fn app_with_source(source: &str, row: usize, col: usize) -> WorkbenchApp {
        let mut workbench = slow_workbench();
        workbench.set_source(0, source.to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.editors[0].set_caret(row, col);
        app
    }

    #[test]
    fn ctrl_tab_and_ctrl_shift_tab_change_the_active_document_from_the_keyboard() {
        let mut workbench = Workbench::new();
        let initial = workbench.documents().len();
        workbench.new_document();
        workbench.new_document();
        let count = initial + 2;
        let last = count - 1;
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.workbench.active(), last, "the newest tab is active");

        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Tab, ctrl)],
        );
        assert_eq!(app.workbench.active(), 0, "Ctrl+Tab wraps to the first tab");
        assert_eq!(app.editors.len(), count);

        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            command: true,
            shift: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(ctrl_shift),
                key(Key::Tab, ctrl_shift),
            ],
        );
        assert_eq!(
            app.workbench.active(),
            last,
            "Ctrl+Shift+Tab wraps back to the last tab"
        );
        assert_eq!(
            app.workbench.active_document().id,
            app.workbench.documents()[last].id
        );
    }

    #[test]
    fn ctrl_g_go_to_line_prompt_moves_the_caret_to_the_exact_line() {
        let mut app = app_with_source("one\ntwo\nthree\nfour", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::G, ctrl)],
        );
        assert!(
            app.goto_line.is_some(),
            "Ctrl+G must open the go-to-line prompt"
        );

        // The prompt's field takes focus on the pass after the window appears.
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("3".to_string())]);
        assert_eq!(
            app.goto_line.as_ref().map(|prompt| prompt.text.as_str()),
            Some("3"),
            "typed digits must land in the prompt, not the document"
        );
        assert_eq!(app.editors[0].source(), "one\ntwo\nthree\nfour");

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].line_col_1based(), (3, 1));
        assert_eq!(app.editors[0].source(), "one\ntwo\nthree\nfour");
        assert!(app.goto_line.is_none(), "a successful jump closes the prompt");
    }

    #[test]
    fn ctrl_m_jumps_to_the_matching_bracket_from_the_keyboard() {
        let mut app = app_with_source("f(ab[cd]e)", 0, 1);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::M, ctrl)],
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (1, 10),
            "Ctrl+M moves the caret from '(' to its exact mate ')'"
        );
        assert_eq!(app.editors[0].source(), "f(ab[cd]e)");
    }

    #[test]
    fn tab_and_shift_tab_indent_the_selected_block_through_the_key_events() {
        let mut app = app_with_source("a\nb\nc", 0, 0);
        app.editors[0].select_to(2, 1);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Tab, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "    a\n    b\n    c");
        assert_eq!(app.workbench.documents()[0].source, "    a\n    b\n    c");

        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(shift), key(Key::Tab, shift)],
        );
        assert_eq!(app.editors[0].source(), "a\nb\nc");
    }

    #[test]
    fn enter_after_an_opening_brace_indents_the_new_line_through_the_key_event() {
        let mut app = app_with_source("    if (x) {", 0, 12);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "    if (x) {\n        ");
        assert_eq!(app.editors[0].line_col_1based(), (2, 9));
    }

    #[test]
    fn persisted_editor_settings_are_applied_to_the_editors() {
        let (storage, directory) = app_storage("editor-settings");
        let mut settings = Settings::new();
        settings.editor = settings::EditorSettings {
            indent_width: 2,
            use_spaces: false,
            auto_close: false,
            ..settings::EditorSettings::default()
        };
        settings
            .save(storage.settings_path.as_ref().expect("settings path"))
            .expect("save the settings");
        let mut app = WorkbenchApp::launch(storage, None);
        assert_eq!(
            app.settings.editor,
            settings::EditorSettings {
                indent_width: 2,
                use_spaces: false,
                auto_close: false,
                ..settings::EditorSettings::default()
            },
            "the settings file is the source of truth"
        );
        assert_eq!(
            app.editors[0].preferences, app.settings.editor,
            "every editor receives the persisted preferences"
        );
        let empty = app
            .editors
            .iter()
            .position(|editor| editor.source().is_empty())
            .expect("the launch has an untitled empty document");
        app.editors[empty].indent();
        assert_eq!(
            app.editors[empty].source(),
            "\t",
            "the editor honours use_spaces=false and writes one literal tab"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn ctrl_shift_o_find_symbol_filters_the_file_and_enter_jumps() {
        let mut app = app_with_source("alpha = 1\nbeta = alpha + 2\n", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            command: true,
            shift: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(ctrl_shift),
                key(Key::O, ctrl_shift),
            ],
        );
        assert!(
            app.find_symbol.is_some(),
            "Ctrl+Shift+O must open the find-symbol prompt"
        );
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("be".to_string())]);
        assert_eq!(
            app.find_symbol.as_ref().map(|prompt| prompt.query.as_str()),
            Some("be"),
            "the filter is typed into the prompt"
        );

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (2, 1),
            "Enter jumps to beta's first occurrence on line 2"
        );
        assert!(app.find_symbol.is_none());
    }

    fn find_bar_button(app: &WorkbenchApp, label: &str) -> egui::Pos2 {
        app.find_buttons
            .iter()
            .find(|(painted, _)| *painted == label)
            .map(|(_, rect)| rect.center())
            .unwrap_or_else(|| panic!("the {label} button was not painted"))
    }

    #[test]
    fn ctrl_f_opens_the_find_bar_and_escape_closes_it_with_focus_on_the_editor() {
        let mut app = app_with_source("alpha beta alpha", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::F, ctrl)],
        );
        assert!(
            app.editors[0].find.is_open(),
            "Ctrl+F must open the docked find bar"
        );

        // The bar's find field takes focus on the pass after it appears, exactly like the
        // go-to-line prompt; the query then lands in the field and not in the document.
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("alpha".to_string())]);
        assert_eq!(app.editors[0].find.query, "alpha");
        assert_eq!(app.editors[0].find.matches().len(), 2);
        assert_eq!(
            app.editors[0].find.count_text().as_deref(),
            Some("1 of 2"),
            "the bar shows a sensible match count"
        );
        assert_eq!(
            app.editors[0].source(),
            "alpha beta alpha",
            "typing in the find field must not edit the document"
        );
        assert!(
            app.find_buttons.iter().any(|(label, _)| *label == "Replace all"),
            "the bar paints the replace actions"
        );

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        assert!(
            !app.editors[0].find.is_open(),
            "Esc closes the find bar instead of cancelling a run"
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "Esc returns keyboard focus to the editor"
        );
        assert_eq!(app.editors[0].source(), "alpha beta alpha");
    }

    #[test]
    fn find_next_wraps_through_the_bar_buttons() {
        let mut app = app_with_source("alpha beta alpha", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::F, ctrl)],
        );
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("alpha".to_string())]);
        assert_eq!(app.editors[0].find.selected_index(), 0);

        let next = find_bar_button(&app, "Next");
        pointer_click(&ctx, &mut app, next);
        assert_eq!(app.editors[0].find.selected_index(), 1);
        assert_eq!(app.editors[0].line_col_1based(), (1, 12));

        let next = find_bar_button(&app, "Next");
        pointer_click(&ctx, &mut app, next);
        assert_eq!(
            app.editors[0].find.selected_index(),
            0,
            "the second Next wraps back to the first match"
        );
        assert_eq!(app.editors[0].line_col_1based(), (1, 1));
    }

    #[test]
    fn find_bar_replace_all_rewrites_the_buffer_and_one_undo_restores_it() {
        let mut app = app_with_source("one two one two one", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::F, ctrl)],
        );
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("one".to_string())]);
        app.editors[0].find.set_replacement("1");
        let revision_before = app.workbench.documents()[0].revision;

        let replace_all = find_bar_button(&app, "Replace all");
        pointer_click(&ctx, &mut app, replace_all);
        assert_eq!(app.editors[0].source(), "1 two 1 two 1");
        assert_eq!(
            app.workbench.documents()[0].source, "1 two 1 two 1",
            "the bar publishes the rewritten buffer through the same path as a typed edit"
        );
        assert_eq!(
            app.workbench.documents()[0].revision,
            revision_before + 1,
            "one replace-all advances the document revision exactly once"
        );

        // Esc closes the bar and restores editor focus; then one Ctrl+Z restores the buffer.
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), ctrl_z()],
        );
        assert_eq!(
            app.editors[0].source(),
            "one two one two one",
            "one undo step restores the pre-replace buffer"
        );
        assert_eq!(app.editors[0].find.matches().len(), 0, "the bar is closed");
    }

    #[test]
    fn an_invalid_regex_shows_the_error_in_the_bar_and_never_edits_the_buffer() {
        let mut app = app_with_source("alpha beta alpha", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::F, ctrl)],
        );
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("(".to_string())]);
        let regex = find_bar_button(&app, ".*");
        pointer_click(&ctx, &mut app, regex);
        assert_eq!(
            app.editors[0].find.error().map(|error| {
                error.contains("Invalid regular expression")
            }),
            Some(true),
            "the invalid pattern is a visible error state"
        );
        assert!(app.editors[0]
            .find_summary()
            .expect("open")
            .starts_with("Find: invalid regex"));
        assert_eq!(app.editors[0].find.count_text(), None);

        // Replace all with an invalid pattern must be a no-op, not a panic.
        let replace_all = find_bar_button(&app, "Replace all");
        pointer_click(&ctx, &mut app, replace_all);
        assert_eq!(app.editors[0].source(), "alpha beta alpha");
        assert_eq!(app.workbench.documents()[0].source, "alpha beta alpha");
    }

    #[test]
    fn signature_popup_for_sqrt_and_compile_reads_the_engine_catalogue() {
        let mut app = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        for ch in "sqrt(".chars() {
            frame(&ctx, &mut app, vec![egui::Event::Text(ch.to_string())]);
        }
        assert_eq!(
            app.editors[0].signature_summary().as_deref(),
            Some("Signature: sqrt(x) - parameter 1/1 'x'"),
            "the popup and the header carry the engine's sqrt(x) signature"
        );

        let mut compile = completion_app(
            "",
            0,
            0,
            vec![builtin("compile", &["f", "params"])],
        );
        let ctx = egui::Context::default();
        frame(&ctx, &mut compile, Vec::new());
        for ch in "compile(".chars() {
            frame(&ctx, &mut compile, vec![egui::Event::Text(ch.to_string())]);
        }
        assert_eq!(
            compile.editors[0].signature_summary().as_deref(),
            Some("Signature: compile(f, params) - parameter 1/2 'f'")
        );
        for ch in "x^2, ".chars() {
            frame(&ctx, &mut compile, vec![egui::Event::Text(ch.to_string())]);
        }
        assert_eq!(
            compile.editors[0].signature_summary().as_deref(),
            Some("Signature: compile(f, params) - parameter 2/2 'params'"),
            "the comma advances the active parameter to params"
        );

        // Escape dismisses the popup for this call without touching the buffer.
        frame(
            &ctx,
            &mut compile,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        assert_eq!(compile.editors[0].signature_summary(), None);
        assert_eq!(
            compile.editors[0].source(),
            "compile(x^2, )",
            "Escape dismisses the popup without editing the buffer"
        );
    }

    #[test]
    fn backspace_that_joins_two_lines_keeps_the_paint_pass_in_bounds() {
        // DEF-001: pressing Backspace at the start of line 2 joins it with line 1. The line
        // count shrank, and the paint loop used to index `lines[1]` of a one-line buffer.
        let mut app = app_with_source("a\nb", 1, 0);
        let ctx = egui::Context::default();

        frame(&ctx, &mut app, Vec::new()); // gives the editor keyboard focus
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Backspace, egui::Modifiers::default())],
        );

        assert_eq!(
            app.editors[0].source(),
            "ab",
            "Backspace at the start of line 2 must join it with line 1"
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (1, 2),
            "the caret must sit after the joined text"
        );
        assert_eq!(
            app.workbench.documents()[0].source,
            "ab",
            "the joined buffer must be the document's committed source"
        );
    }

    #[test]
    fn arrow_up_moves_the_caret_without_surrendering_editor_focus() {
        // DEF-002: egui's cardinal focus navigation moves focus on a bare Up arrow before the
        // editor can consume it; the editor must lock the arrow keys while it has focus.
        let mut app = app_with_source("a\nb", 1, 0);
        let ctx = egui::Context::default();

        frame(&ctx, &mut app, Vec::new()); // gives the editor keyboard focus
        frame(&ctx, &mut app, Vec::new()); // lets the focus lock filter install
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );

        assert_eq!(
            app.editors[0].line_col_1based(),
            (1, 1),
            "Up must move the caret to line 1"
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "Up must not move keyboard focus out of the editor"
        );

        frame(&ctx, &mut app, vec![egui::Event::Text("c".to_string())]);
        assert_eq!(
            app.editors[0].source(),
            "ca\nb",
            "typing immediately after Up must land in the editor"
        );
    }

    /// Builds an app whose editor holds `source` and whose engine is the deterministic fake.
    fn fake_app_with_source(source: &str) -> WorkbenchApp {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::FakeEngine::new())));
        workbench.set_source(0, source.to_string());
        WorkbenchApp::with_workbench(workbench)
    }

    #[test]
    fn failed_run_decorates_the_reported_line_and_f8_jumps_to_it_and_an_edit_clears_it() {
        let mut app = fake_app_with_source("alpha = 1\n1 / 0\ngamma = 3\n");
        let ctx = egui::Context::default();
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        assert_eq!(app.workbench.documents()[0].state(), RunState::Failed);

        frame(&ctx, &mut app, Vec::new());
        let diagnostic = app.editors[0]
            .diagnostic
            .as_ref()
            .expect("the failed run's diagnostic must reach the editor");
        assert_eq!((diagnostic.line, diagnostic.column), (2, 1));
        let range = app.editors[0]
            .diagnostic_range()
            .expect("the offending line is in the buffer");
        assert_eq!(
            (range.line, range.start_char, range.end_char),
            (2, 0, 5),
            "the underline covers line 2 from the reported column to end-of-line"
        );
        assert_eq!(
            (range.start_byte, range.end_byte),
            (0, 5),
            "the range is byte-exact for the ASCII line"
        );

        // F8 is the keyboard path to the failure: caret on the exact 1-based line and column.
        app.editors[0].set_caret(0, 0);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::F8, egui::Modifiers::default())],
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (2, 1),
            "F8 must jump to the reported position"
        );

        // Delete the line break before the offending line: the buffer changes and the old
        // diagnostic must not be painted over the joined text.
        app.editors[0].set_caret(1, 0);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Backspace, egui::Modifiers::default())],
        );
        assert!(
            app.editors[0].diagnostic.is_none(),
            "an edit clears the decoration in the same pass"
        );
        assert!(
            app.workbench.diagnostic(0).is_none(),
            "the workbench also hides a diagnostic whose revision is no longer current"
        );
    }

    /// RUN-003 through the window: after a four-statement run, the view the frame renders is
    /// the workbench's one assembled object and its variables, revision and log tail agree.
    #[test]
    fn a_frame_renders_the_same_assembled_view_the_workbench_published() {
        let mut app = fake_app_with_source("a = 1\nb = a + 1\nprint(\"p\")\nb\n");
        let ctx = egui::Context::default();
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        frame(&ctx, &mut app, Vec::new());

        let view = app.workbench.active_view();
        assert_eq!(
            app.view, view,
            "the frame must render the workbench's assembled view, not a second assembly"
        );
        assert_eq!(view.revision, crate::execution::SourceRevision(1));
        assert_eq!(view.committed_steps, 4);
        assert_eq!(
            view.variables,
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ]
        );
        assert_eq!(view.last_committed_log_statement(), Some(4));
        assert_eq!(view.last_log_statement(), Some(4));
    }

    /// WKS-001/WKS-002 through the window: both table headers are real clickable controls. The
    /// first click on a column sorts ascending, the second descending, and the rows the window
    /// paints are the committed snapshot's rows (so the header counts are the snapshot counts).
    #[test]
    fn workspace_header_clicks_sort_both_tables_and_the_counts_are_the_snapshot_counts() {
        let mut app = fake_app_with_source("c = 3\na = 1\nb = 2\n");
        let ctx = egui::Context::default();
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        frame(&ctx, &mut app, Vec::new());

        let variable_names = |app: &WorkbenchApp| -> Vec<String> {
            app.variables_table
                .ordered()
                .into_iter()
                .map(|index| app.variables_table.rows[index].name.clone())
                .collect()
        };
        let function_names = |app: &WorkbenchApp| -> Vec<String> {
            app.functions_table
                .ordered()
                .into_iter()
                .map(|index| app.functions_table.rows[index].name.clone())
                .collect()
        };

        // Default: ordinal name ascending for both tables, and the painted rows are exactly the
        // committed snapshot's rows.
        assert_eq!(variable_names(&app), vec!["a", "b", "c"]);
        assert!(app.variables_table.sort.ascending);
        assert!(app.functions_table.sort.ascending);
        assert_eq!(
            app.variables_table.rows.len(),
            app.view.state.variables.len(),
            "the variables table must show every captured variable"
        );
        assert_eq!(
            app.functions_table.rows.len(),
            app.view.functions.len(),
            "the displayed function count must equal the snapshot's function count"
        );
        assert_eq!(
            app.functions_table.rows.len(),
            app.view.state.functions.len(),
            "and that count must be the registry capture the engine published"
        );

        // First click on Name keeps the default ascending order; the second click descends.
        let name_header = app.variable_header_rects[VariableColumn::Name.index()]
            .expect("the variables Name header was painted");
        pointer_click(&ctx, &mut app, name_header.center());
        assert!(
            app.variables_table.sort.ascending,
            "the first header click must sort ascending"
        );
        let name_header = app.variable_header_rects[VariableColumn::Name.index()]
            .expect("the Name header stays painted");
        pointer_click(&ctx, &mut app, name_header.center());
        assert!(!app.variables_table.sort.ascending);
        assert_eq!(variable_names(&app), vec!["c", "b", "a"]);

        // A new column starts ascending again.
        let kind_header = app.variable_header_rects[VariableColumn::Kind.index()]
            .expect("the variables Kind header was painted");
        pointer_click(&ctx, &mut app, kind_header.center());
        assert_eq!(app.variables_table.sort.key, VariableColumn::Kind);
        assert!(app.variables_table.sort.ascending);

        // Functions obey the same click contract: the first click on Name is ascending, the
        // second reverses the whole registry slice without losing a row.
        let function_header = app.function_header_rects[FunctionColumn::Name.index()]
            .expect("the functions Name header was painted");
        pointer_click(&ctx, &mut app, function_header.center());
        assert!(app.functions_table.sort.ascending);
        let ascending = function_names(&app);
        assert_eq!(ascending.len(), app.view.state.functions.len());
        let function_header = app.function_header_rects[FunctionColumn::Name.index()]
            .expect("the Name header stays painted");
        pointer_click(&ctx, &mut app, function_header.center());
        assert!(!app.functions_table.sort.ascending);
        let mut reversed = ascending.clone();
        reversed.reverse();
        assert_eq!(
            function_names(&app),
            reversed,
            "descending reverses every row"
        );
        assert_eq!(app.functions_table.rows.len(), ascending.len());
    }

    #[test]
    fn ctrl_enter_runs_and_escape_cancels_with_the_visible_running_state() {
        let gate = Arc::new(Gate::default());
        let mut app = WorkbenchApp::with_workbench(gated_workbench(Arc::clone(&gate)));
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());

        let source_before = app.workbench.documents()[0].source.clone();
        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );

        assert!(
            app.workbench.active_is_running(),
            "Ctrl+Enter did not start a run"
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Queued,
            "the frame that pressed Ctrl+Enter must submit and show Queued"
        );
        assert!(
            app.workbench.status_text(1, 1).contains("run Queued"),
            "status strip after submit was {:?}",
            app.workbench.status_text(1, 1)
        );
        assert_eq!(
            app.workbench.documents()[0].source,
            source_before,
            "Ctrl+Enter must not also insert a newline into the document"
        );
        let toolbar = app.workbench.toolbar_state();
        assert!(!toolbar.run_enabled, "Run must be disabled while queued");
        assert!(
            toolbar.cancel_visible,
            "Cancel must be visible while queued"
        );

        // Release the worker and let the window apply the typed RunStarted event.
        gate.release();
        let deadline = Instant::now() + Duration::from_secs(2);
        while app.workbench.documents()[0].state() == RunState::Queued {
            app.workbench.poll();
            assert!(
                Instant::now() < deadline,
                "the worker never reported RunStarted"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Running,
            "RunStarted must move Queued to Running"
        );
        frame(&ctx, &mut app, Vec::new());
        assert!(
            app.workbench.status_text(1, 1).contains("run Running"),
            "status strip was {:?}",
            app.workbench.status_text(1, 1)
        );

        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::default()),
                key(Key::Escape, egui::Modifiers::default()),
            ],
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Cancelling,
            "Esc must move the visible state to Cancelling immediately"
        );
        assert!(
            app.workbench.status_text(1, 1).contains("run Cancelling"),
            "status strip after Esc was {:?}",
            app.workbench.status_text(1, 1)
        );
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(2)),
            "the run did not reach a terminal state after Esc"
        );
        assert_eq!(app.workbench.documents()[0].state(), RunState::Cancelled);
        assert!(
            app.workbench.status_text(1, 1).contains("run Cancelled"),
            "status strip was {:?}",
            app.workbench.status_text(1, 1)
        );
        assert_eq!(
            app.workbench.documents()[0]
                .transitions()
                .iter()
                .map(|transition| transition.to)
                .collect::<Vec<_>>(),
            [
                RunState::Queued,
                RunState::Running,
                RunState::Cancelling,
                RunState::Cancelled
            ],
            "the window's own frame loop must log the whole visible sequence"
        );
        let toolbar = app.workbench.toolbar_state();
        assert!(toolbar.run_enabled, "Run must return after the run stops");
        assert!(!toolbar.cancel_visible, "Cancel must leave with the run");
    }

    fn builtin(name: &str, parameters: &[&str]) -> EngineFunction {
        EngineFunction {
            name: name.to_string(),
            parameters: parameters.iter().map(|p| p.to_string()).collect(),
            is_builtin: true,
            plugin: None,
            summary: None,
        }
    }

    fn sqrt_function() -> EngineFunction {
        builtin("sqrt", &["x"])
    }

    /// An app whose editor holds `source`, with the caret at `(row, col)` and a registry that
    /// only contains the functions the test names.
    fn completion_app(
        source: &str,
        row: usize,
        col: usize,
        functions: Vec<EngineFunction>,
    ) -> WorkbenchApp {
        let mut workbench = slow_workbench();
        workbench.set_engine_state(EngineState {
            variables: Vec::new(),
            functions,
        });
        workbench.set_source(0, source.to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.editors[0].set_caret(row, col);
        app
    }

    #[test]
    fn typing_sq_opens_completion_with_the_engine_function_and_enter_replaces_only_the_prefix() {
        let mut app = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // first frame: focus the editor
        frame(&ctx, &mut app, vec![egui::Event::Text("sq".to_string())]);

        let session = &app.editors[0].completion;
        assert!(session.is_open(), "typing `sq` must open the popup");
        assert_eq!(session.items().len(), 1);
        let item = session.selected().expect("a highlighted entry");
        assert_eq!(item.label, "sqrt");
        assert_eq!(
            item.detail, "sqrt(x)",
            "the signature comes from the engine's own function snapshot"
        );
        assert_eq!(item.kind, CompletionKind::Builtin);
        assert_eq!(
            app.editors[0].completion_summary().as_deref(),
            Some("Completion: sq -> sqrt(x) [builtin] 1/1")
        );
        assert_eq!(app.workbench.documents()[0].source, "sq");

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(
            app.editors[0].source(),
            "sqrt",
            "Enter must replace exactly the typed prefix"
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (1, 5),
            "the caret must sit after the inserted text"
        );
        assert!(!app.editors[0].completion.is_open());
        assert_eq!(app.workbench.documents()[0].source, "sqrt");
    }

    #[test]
    fn arrow_keys_move_the_completion_selection_and_tab_accepts_it() {
        let mut app = completion_app(
            "",
            0,
            0,
            vec![builtin("sq", &["x"]), builtin("sqrt", &["x"])],
        );
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("sq".to_string())]);
        assert_eq!(
            app.editors[0]
                .completion
                .items()
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["sq", "sqrt"],
            "the exact label is offered before the longer prefix match"
        );
        assert_eq!(app.editors[0].completion.selected_index(), 0);

        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowDown, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].completion.selected_index(), 1);
        assert_eq!(app.editors[0].source(), "sq", "navigation must not edit");
        assert_eq!(app.editors[0].line_col_1based(), (1, 3));

        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].completion.selected_index(), 0);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );
        assert_eq!(
            app.editors[0].completion.selected_index(),
            0,
            "Up on the first entry stays on it"
        );
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowDown, egui::Modifiers::default())],
        );
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Tab, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "sqrt", "Tab accepts the selection");
        assert_eq!(app.editors[0].line_col_1based(), (1, 5));
        assert!(!app.editors[0].completion.is_open());
    }

    #[test]
    fn escape_dismisses_completion_without_changing_the_buffer_or_accepting() {
        let mut app = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("sq".to_string())]);
        assert!(app.editors[0].completion.is_open());

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        assert!(!app.editors[0].completion.is_open(), "Esc must dismiss");
        assert_eq!(
            app.editors[0].source(),
            "sq",
            "Esc must not edit the buffer"
        );
        assert_eq!(app.editors[0].line_col_1based(), (1, 3));
        assert_eq!(app.workbench.documents()[0].source, "sq");

        // A dismissed popup must not accept anything: Enter is an ordinary newline again.
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "sq\n");
        assert_eq!(app.editors[0].line_col_1based(), (2, 1));
    }

    #[test]
    fn ctrl_space_opens_the_whole_catalogue_and_strings_and_comments_suppress_it() {
        let mut app = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Space, ctrl)],
        );
        assert!(
            app.editors[0].completion.is_open(),
            "Ctrl+Space must open the popup even with no typed prefix"
        );
        assert_eq!(
            app.editors[0]
                .completion
                .selected()
                .map(|item| item.label.as_str()),
            Some("break"),
            "with an empty prefix the catalogue starts at the first keyword in name order"
        );
        assert!(
            app.editors[0]
                .completion
                .items()
                .iter()
                .any(|item| item.label == "sqrt" && item.detail == "sqrt(x)"),
            "the whole catalogue includes the engine's registered functions"
        );
        assert_eq!(
            app.editors[0].source(),
            "",
            "Ctrl+Space must not insert a space"
        );

        // Inside a string literal the popup stays closed.
        let mut quoted = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut quoted, Vec::new());
        frame(
            &ctx,
            &mut quoted,
            vec![egui::Event::Text("\"sq".to_string())],
        );
        assert!(
            !quoted.editors[0].completion.is_open(),
            "a string literal must suppress completion"
        );

        // Inside a comment the popup stays closed too.
        let mut commented = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut commented, Vec::new());
        frame(
            &ctx,
            &mut commented,
            vec![egui::Event::Text("# sq".to_string())],
        );
        assert!(
            !commented.editors[0].completion.is_open(),
            "a comment must suppress completion"
        );
    }

    fn ctrl_modifiers() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        }
    }

    fn ctrl_z() -> egui::Event {
        key(Key::Z, ctrl_modifiers())
    }

    fn ctrl_y() -> egui::Event {
        key(Key::Y, ctrl_modifiers())
    }

    fn ctrl_shift_z() -> egui::Event {
        key(
            Key::Z,
            egui::Modifiers {
                shift: true,
                ..ctrl_modifiers()
            },
        )
    }

    #[test]
    fn ctrl_z_undoes_a_typing_burst_and_ctrl_y_redoes_it() {
        let mut app = app_with_source("", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor

        // Three character events, one per pass: they are one burst because nothing else
        // happened between them and the pause is far below the coalescing gap.
        frame(&ctx, &mut app, vec![egui::Event::Text("a".to_string())]);
        frame(&ctx, &mut app, vec![egui::Event::Text("b".to_string())]);
        frame(&ctx, &mut app, vec![egui::Event::Text("c".to_string())]);
        assert_eq!(app.editors[0].source(), "abc");
        assert_eq!(app.editors[0].line_col_1based(), (1, 4));

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(
            app.editors[0].source(),
            "",
            "Ctrl+Z restores the empty buffer"
        );
        assert_eq!(
            app.editors[0].line_col_1based(),
            (1, 1),
            "with the caret at 0"
        );
        assert_eq!(
            app.workbench.documents()[0].source,
            "",
            "the document must be committed from the undone buffer"
        );

        frame(&ctx, &mut app, vec![ctrl_y()]);
        assert_eq!(app.editors[0].source(), "abc", "Ctrl+Y restores the burst");
        assert_eq!(app.editors[0].line_col_1based(), (1, 4));
        assert_eq!(app.workbench.documents()[0].source, "abc");

        // Ctrl+Shift+Z is the second redo spelling.
        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.editors[0].source(), "");
        frame(&ctx, &mut app, vec![ctrl_shift_z()]);
        assert_eq!(app.editors[0].source(), "abc");
    }

    #[test]
    fn a_letter_key_event_does_not_split_a_typing_burst_but_an_arrow_does() {
        let mut app = app_with_source("", 0, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor

        // A real backend delivers a Key event and the character it produced. The Key event for
        // a letter is not a non-typing action, so it must not split the burst; only the Text
        // events are the edits.
        frame(
            &ctx,
            &mut app,
            vec![
                key(Key::A, egui::Modifiers::default()),
                egui::Event::Text("a".to_string()),
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![
                key(Key::B, egui::Modifiers::default()),
                egui::Event::Text("b".to_string()),
            ],
        );
        assert_eq!(app.editors[0].source(), "ab");
        assert_eq!(
            app.editors[0].history.undo_steps(),
            1,
            "letter Key events must not split a typing burst"
        );

        // A caret move is a non-typing action and does start a new step.
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowLeft, egui::Modifiers::default())],
        );
        frame(
            &ctx,
            &mut app,
            vec![
                key(Key::X, egui::Modifiers::default()),
                egui::Event::Text("X".to_string()),
            ],
        );
        assert_eq!(app.editors[0].source(), "aXb");
        assert_eq!(app.editors[0].history.undo_steps(), 2);

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.editors[0].source(), "ab");
        assert_eq!(app.editors[0].line_col_1based(), (1, 2));
        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.editors[0].source(), "");
    }

    #[test]
    fn ctrl_z_restores_a_deleted_selection() {
        let mut app = app_with_source("abcdef", 0, 1);
        app.editors[0].select_to(0, 4);
        assert_eq!(app.editors[0].selected_text().as_deref(), Some("bcd"));
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Delete, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "aef");
        assert_eq!(app.editors[0].line_col_1based(), (1, 2));
        assert_eq!(app.workbench.documents()[0].source, "aef");

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.editors[0].source(), "abcdef");
        assert_eq!(
            app.editors[0].selection_range(),
            Some(((0, 1), (0, 4))),
            "the undo restores the selection, not only the deleted text"
        );
        assert_eq!(app.editors[0].selected_text().as_deref(), Some("bcd"));
        assert_eq!(app.workbench.documents()[0].source, "abcdef");
    }

    #[test]
    fn ctrl_z_restores_a_backspace_line_join() {
        let mut app = app_with_source("a\nb", 1, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Backspace, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "ab");
        assert_eq!(app.workbench.documents()[0].source, "ab");

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.editors[0].source(), "a\nb");
        assert_eq!(
            app.editors[0].line_col_1based(),
            (2, 1),
            "the caret is back at the start of line 2"
        );
        assert_eq!(app.workbench.documents()[0].source, "a\nb");
    }

    #[test]
    fn undo_in_one_document_never_changes_the_other() {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::FakeEngine::new())));
        workbench.set_source(0, String::new());
        workbench.set_source(1, String::new());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus document A

        frame(&ctx, &mut app, vec![egui::Event::Text("abc".to_string())]);
        assert_eq!(app.workbench.documents()[0].source, "abc");

        // Switch to document B: its editor has its own history.
        app.workbench.set_active(1);
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![egui::Event::Text("xy".to_string())]);
        assert_eq!(app.workbench.documents()[1].source, "xy");
        assert_eq!(app.workbench.documents()[0].source, "abc");

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.workbench.documents()[1].source, "");
        assert_eq!(
            app.workbench.documents()[0].source,
            "abc",
            "an undo in B must not change A"
        );

        // Switch back to A: its own history still holds its typing burst.
        app.workbench.set_active(0);
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(app.workbench.documents()[0].source, "");
        assert_eq!(app.workbench.documents()[1].source, "");
    }

    #[test]
    fn a_completion_insertion_is_its_own_undo_step_in_the_window() {
        let mut app = completion_app("", 0, 0, vec![sqrt_function()]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor
        frame(&ctx, &mut app, vec![egui::Event::Text("sq".to_string())]);
        assert!(app.editors[0].completion.is_open());

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(app.editors[0].source(), "sqrt");
        assert_eq!(app.editors[0].line_col_1based(), (1, 5));

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(
            app.editors[0].source(),
            "sq",
            "one undo removes only the completion insertion"
        );
        assert_eq!(app.editors[0].line_col_1based(), (1, 3));
        assert!(!app.editors[0].completion.is_open());
        assert_eq!(app.workbench.documents()[0].source, "sq");

        frame(&ctx, &mut app, vec![ctrl_z()]);
        assert_eq!(
            app.editors[0].source(),
            "",
            "the typing that produced the prefix is the next step"
        );
        assert_eq!(app.workbench.documents()[0].source, "");
    }

    fn ctrl_shift_modifiers() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Default::default()
        }
    }

    /// RUN-002 / LOG-003: Ctrl+Shift+E puts keyboard focus in the one-line field, typing there
    /// does not touch the editor buffer, and Enter submits the expression, clears the field only
    /// after the request was accepted, and leaves the document's source and revision unchanged.
    #[test]
    fn quick_eval_shortcut_focuses_the_field_and_enter_runs_without_editing_the_buffer() {
        let mut app = fake_app_with_source("a = 1\n");
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // editor focus on the first frame
        let source_before = app.workbench.documents()[0].source.clone();
        let revision_before = app.workbench.documents()[0].revision;

        let shortcut = ctrl_shift_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(shortcut),
                key(Key::E, shortcut),
            ],
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(quick_eval_id()),
            "Ctrl+Shift+E must focus the quick-eval field"
        );
        // Release the shortcut's modifiers before using Enter, as the real backend does.
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );

        frame(&ctx, &mut app, vec![egui::Event::Text("6 * 7".to_string())]);
        assert_eq!(app.quick_eval.text(), "6 * 7");
        assert_eq!(
            app.editors[0].source(),
            "a = 1\n",
            "typing in quick eval must not reach the editor"
        );
        assert_eq!(app.workbench.documents()[0].source, source_before);

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert_eq!(
            app.quick_eval.text(),
            "",
            "an accepted submission clears the field"
        );
        assert!(
            app.workbench.active_is_running(),
            "Enter in the field must submit a run"
        );
        assert_eq!(
            app.workbench.documents()[0].source,
            source_before,
            "quick eval must not change the document buffer"
        );
        assert_eq!(
            app.workbench.documents()[0].revision,
            revision_before,
            "quick eval must not bump the document revision"
        );
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        assert_eq!(app.workbench.documents()[0].state(), RunState::Finished);

        let document = &app.workbench.documents()[0];
        let echoes: Vec<&LogLine> = document
            .logs
            .iter()
            .filter(|line| line.kind == LogKind::Echo)
            .collect();
        assert_eq!(echoes.len(), 1, "exactly one echo line per submission");
        assert_eq!(echoes[0].render(), "[echo] >> 6 * 7");
        assert!(
            document
                .logs
                .iter()
                .any(|line| line.kind == LogKind::Result && line.text == "= 42")
        );
    }

    /// LOG-003: Up/Down on the focused field walk accepted submissions newest first and stop at
    /// the oldest; the restored text really lands back in the field.
    #[test]
    fn quick_eval_history_returns_previous_expressions_newest_first_and_stops_at_the_oldest() {
        let mut app = fake_app_with_source("a = 1\n");
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());

        let shortcut = ctrl_shift_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(shortcut),
                key(Key::E, shortcut),
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );
        frame(&ctx, &mut app, vec![egui::Event::Text("6 * 7".to_string())]);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        frame(&ctx, &mut app, Vec::new()); // repaint the cleared field and install its key lock

        frame(&ctx, &mut app, vec![egui::Event::Text("2 + 3".to_string())]);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(quick_eval_id()),
            "the field keeps focus after a submission"
        );

        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );
        assert_eq!(app.quick_eval.text(), "2 + 3", "the first Up is the newest");
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );
        assert_eq!(app.quick_eval.text(), "6 * 7");
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowUp, egui::Modifiers::default())],
        );
        assert_eq!(
            app.quick_eval.text(),
            "6 * 7",
            "Up at the oldest keeps the oldest expression"
        );
        frame(
            &ctx,
            &mut app,
            vec![key(Key::ArrowDown, egui::Modifiers::default())],
        );
        assert_eq!(app.quick_eval.text(), "2 + 3");
        assert_eq!(
            app.editors[0].source(),
            "a = 1\n",
            "history navigation must not edit the editor"
        );
    }

    /// RUN-002: the field is also focusable by pointer, not only by its shortcut.
    #[test]
    fn the_quick_eval_field_takes_keyboard_focus_from_a_mouse_click() {
        let mut app = fake_app_with_source("a = 1\n");
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let rect = app
            .quick_eval_rect
            .expect("the quick-eval field was painted on the first frame");
        let pos = rect.center();
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                }],
            );
        }
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(quick_eval_id()),
            "clicking the field must give it keyboard focus"
        );
    }

    /// LOG-003: Esc leaves the quick-eval field without submitting, without clearing its text
    /// and without starting a run.
    #[test]
    fn escape_leaves_the_quick_eval_field_without_submitting_or_clearing_it() {
        let mut app = fake_app_with_source("a = 1\n");
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());

        let shortcut = ctrl_shift_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(shortcut),
                key(Key::E, shortcut),
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );
        frame(&ctx, &mut app, vec![egui::Event::Text("6 * 7".to_string())]);
        assert_eq!(app.quick_eval.text(), "6 * 7");

        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );

        assert_eq!(
            app.quick_eval.text(),
            "6 * 7",
            "Esc must leave the text in the field"
        );
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(quick_eval_id()),
            "Esc must clear focus from the field"
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Idle,
            "Esc in the field must not start a run"
        );
        assert!(
            !app.workbench
                .active_logs()
                .iter()
                .any(|line| line.kind == LogKind::Echo),
            "Esc must not echo or submit"
        );
        assert_eq!(app.editors[0].source(), "a = 1\n");
    }

    /// LOG-003: when the host refuses (a run is in flight), the typed expression stays in the
    /// field, no echo line is faked, and the `[info]` advisory is logged.
    #[test]
    fn a_refused_quick_eval_keeps_the_typed_text_and_logs_info_without_an_echo() {
        let gate = Arc::new(Gate::default());
        let mut app = WorkbenchApp::with_workbench(gated_workbench(Arc::clone(&gate)));
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());

        let ctrl = ctrl_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Queued,
            "the document run must be in flight"
        );

        let shortcut = ctrl_shift_modifiers();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(shortcut),
                key(Key::E, shortcut),
            ],
        );
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(quick_eval_id()));
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );
        frame(&ctx, &mut app, vec![egui::Event::Text("6 * 7".to_string())]);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );

        assert_eq!(
            app.quick_eval.text(),
            "6 * 7",
            "a refused submission must keep the input text"
        );
        let logs = app.workbench.active_logs();
        assert!(
            !logs.iter().any(|line| line.kind == LogKind::Echo),
            "a refused submission must not fake an echo line"
        );
        assert!(
            logs.iter().any(|line| {
                line.kind == LogKind::Info
                    && line.text.contains("Quick eval not submitted")
                    && line.text.contains("kept")
            }),
            "logs were {logs:?}"
        );

        gate.release();
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
    }

    /// One assembled view whose result is the given display text plus a 2x2 projection.
    fn result_view(display: &str) -> DocumentView {
        use lovelace_json::Json;
        let cell = |value: &str| {
            Json::obj([
                ("kind", Json::Str("Natural".to_string())),
                ("value", Json::Str(value.to_string())),
                ("exact", Json::Bool(true)),
            ])
        };
        let structured = Json::obj([
            ("kind", Json::Str("Array".to_string())),
            ("type", Json::Str("Array".to_string())),
            ("shape", Json::num_array([2, 2])),
            (
                "elements",
                Json::Arr(vec![cell("1"), cell("2"), cell("3"), cell("4")]),
            ),
        ]);
        DocumentView {
            revision: crate::execution::SourceRevision(1),
            has_snapshot: true,
            committed_steps: 1,
            total: 1,
            result: display.to_string(),
            result_structured: Some(structured),
            result_inspection: None,
            variables: Vec::new(),
            functions: Vec::new(),
            state: EngineState::default(),
            plot: None,
            output: Vec::new(),
            logs: Vec::new(),
            diagnostic: None,
            run_state: RunState::Idle,
            buffer_revision: crate::execution::SourceRevision(1),
            progress: String::new(),
            precision: crate::engine::DEFAULT_PRECISION,
            elapsed_ms: 0,
        }
    }

    fn collect_shape_text(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_shape_text(shape, out);
                }
            }
            _ => {}
        }
    }

    /// RES-001 at the paint layer: the workspace RESULT section paints the tree from the
    /// structured projection. The display is deliberately contradictory (it names a record and
    /// a flat list while the projection is a 2x2 array); no record header or field label may be
    /// painted, and the 2x2 rows and scalar values must be.
    #[test]
    fn the_result_section_paints_the_projection_and_never_parses_the_display() {
        let ctx = egui::Context::default();
        let view = result_view("DivRemResult(quotient: 9, remainder: 9) [1, 2, 3, 4]");
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
            });
        });
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }

        assert!(
            texts
                .iter()
                .any(|text| text == "\u{25be} Array 2 x 2 - 4 elements"),
            "the array headline must state the projection's shape: {texts:?}"
        );
        for value in ["1", "2", "3", "4"] {
            assert!(
                texts.iter().any(|text| text == value),
                "the matrix must paint element {value}: {texts:?}"
            );
        }
        assert!(
            texts.iter().any(|text| text == "Natural"),
            "the scalar kind must come from the projection: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.contains("DivRemResult - ")),
            "the record-looking display must not create a record header: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.ends_with("quotient:")),
            "the record-looking display must not create a field label: {texts:?}"
        );
    }

    /// One nested `SolveResult` view at the paint layer: its `solutions` field is a vector of two
    /// `Solution` records with a symbolic value. It is the shape `solve_full(x^2 - 4 == 0, x)`
    /// publishes.
    fn solve_result_view(values: [&str; 2]) -> DocumentView {
        use lovelace_json::Json;
        let symbolic = |pretty: &str| {
            Json::obj([
                ("kind", Json::Str("Symbolic".to_string())),
                ("pretty", Json::Str(pretty.to_string())),
                ("canonical", Json::Str(format!("(sym {pretty})"))),
                ("freeSymbols", Json::str_array(["x"])),
                ("nodeCount", Json::Str("1".to_string())),
            ])
        };
        let solution = |value: &str| {
            Json::obj([
                ("kind", Json::Str("Record".to_string())),
                ("type", Json::Str("Solution".to_string())),
                (
                    "fields",
                    Json::Arr(vec![
                        Json::obj([
                            ("name", Json::Str("value".to_string())),
                            ("value", symbolic(value)),
                        ]),
                        Json::obj([
                            ("name", Json::Str("multiplicity".to_string())),
                            (
                                "value",
                                Json::obj([
                                    ("kind", Json::Str("Integer".to_string())),
                                    ("value", Json::Str("1".to_string())),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ])
        };
        let structured = Json::obj([
            ("kind", Json::Str("Record".to_string())),
            ("type", Json::Str("SolveResult".to_string())),
            (
                "fields",
                Json::Arr(vec![
                    Json::obj([
                        ("name", Json::Str("status".to_string())),
                        (
                            "value",
                            Json::obj([
                                ("kind", Json::Str("Enum".to_string())),
                                ("value", Json::Str("Solved".to_string())),
                                ("type", Json::Str("SolveStatus".to_string())),
                            ]),
                        ),
                    ]),
                    Json::obj([
                        ("name", Json::Str("solutions".to_string())),
                        (
                            "value",
                            Json::obj([
                                ("kind", Json::Str("Array".to_string())),
                                ("type", Json::Str("Vector".to_string())),
                                ("shape", Json::num_array([2])),
                                (
                                    "elements",
                                    Json::Arr(values.iter().map(|value| solution(value)).collect()),
                                ),
                            ]),
                        ),
                    ]),
                ]),
            ),
        ]);
        let mut view = result_view("SolveResult(status: Solved, solutions: [...])");
        view.result_structured = Some(structured);
        view
    }

    /// Paints one RESULT section pass and returns every Text shape in paint order. The toggle
    /// list is cleared first, so it always describes the pass that was just painted.
    fn paint_result_frame(
        ctx: &egui::Context,
        view: &DocumentView,
        expansion: &mut ExpansionState,
        toggles: &mut Vec<(String, Rect)>,
        events: Vec<egui::Event>,
    ) -> Vec<String> {
        toggles.clear();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                paint_result_section(ui, view, 420.0, expansion, toggles);
            });
        });
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        texts
    }

    /// The rectangle the last pass painted for one header path.
    fn toggle_rect(toggles: &[(String, Rect)], path: &str) -> Rect {
        toggles
            .iter()
            .find(|(recorded, _)| recorded == path)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("no painted header for {path}: {toggles:?}"))
    }

    /// One pointer button event at `pos`.
    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// RES-004 at the paint layer: collapsing a nested record field hides its children, keeps
    /// them in the model, and re-expanding paints the identical content.
    #[test]
    fn the_result_tree_collapses_a_nested_field_and_restores_identical_content() {
        let ctx = egui::Context::default();
        let view = solve_result_view(["-2", "2"]);
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();

        let expanded = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            expanded
                .iter()
                .any(|text| text.ends_with("solutions: Vector 2 - 2 elements")),
            "the field header names the field and its value: {expanded:?}"
        );
        assert!(
            expanded
                .iter()
                .any(|text| text.ends_with("[0] Solution - 2 fields")),
            "the expanded field paints its nested rows: {expanded:?}"
        );
        assert!(
            expanded.iter().any(|text| text == "pretty     -2"),
            "the expanded field paints the nested symbolic value: {expanded:?}"
        );
        // The header's recorded path is the key the map uses.
        let _ = toggle_rect(&toggles, "result.solutions");

        expansion.set_collapsed("result.solutions", true);
        let collapsed = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            collapsed
                .iter()
                .any(|text| text == "\u{25b8} solutions: Vector 2 - 2 elements"),
            "the collapsed field keeps its header and shows the right-pointing indicator: {collapsed:?}"
        );
        assert!(
            !collapsed
                .iter()
                .any(|text| text.contains("Solution - 2 fields")),
            "collapsing hides the nested record rows: {collapsed:?}"
        );
        assert!(
            !collapsed.iter().any(|text| text == "pretty     -2"),
            "collapsing hides the nested symbolic value: {collapsed:?}"
        );

        // The children the pass stopped drawing are still in the model.
        let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
        let tree = projection.tree().expect("the view's projection is a tree");
        let StructuredNode::Record(record) = &tree else {
            panic!("the projection is a record");
        };
        let StructuredNode::Array(solutions) = record
            .field("solutions")
            .expect("the record publishes its solutions")
        else {
            panic!("solutions is a vector");
        };
        assert_eq!(
            solutions.elements.len(),
            2,
            "a collapsed field's children stay in the model"
        );

        expansion.set_collapsed("result.solutions", false);
        let again = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert_eq!(
            again, expanded,
            "re-expanding must paint byte-identical content"
        );
    }

    /// RES-004: a pointer click and Space on the focused header change the same map entry. The
    /// click also gives the header keyboard focus, so Space acts on the node the user clicked.
    #[test]
    fn a_click_and_space_on_a_result_header_change_the_same_state() {
        let ctx = egui::Context::default();
        let view = solve_result_view(["-2", "2"]);
        let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();

        let _ = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        let pos = toggle_rect(&toggles, "result.solutions").center();
        for event in [
            egui::Event::PointerMoved(pos),
            pointer_button(pos, true),
            pointer_button(pos, false),
        ] {
            let _ = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, vec![event]);
        }
        assert!(
            expansion.is_collapsed("result.solutions"),
            "a pointer click collapses the field it hits"
        );

        let mut clicked = ExpansionState::new();
        clicked.bind(&projection);
        clicked.toggle("result.solutions");
        assert_eq!(
            expansion, clicked,
            "a click writes the same path entry as a toggle"
        );

        // The click moved focus to the header; Space is the keyboard route to the same toggle.
        let _ = paint_result_frame(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![key(Key::Space, egui::Modifiers::default())],
        );
        assert!(
            !expansion.is_collapsed("result.solutions"),
            "Space on the focused header expands it again"
        );
        let mut clicked_twice = ExpansionState::new();
        clicked_twice.bind(&projection);
        clicked_twice.toggle("result.solutions");
        clicked_twice.toggle("result.solutions");
        assert_eq!(
            expansion, clicked_twice,
            "click then Space leaves the same state as two clicks"
        );

        // Enter is the other keyboard activation egui maps to the same click.
        let _ = paint_result_frame(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![key(Key::Enter, egui::Modifiers::default())],
        );
        assert!(
            expansion.is_collapsed("result.solutions"),
            "Enter on the focused header collapses it again"
        );
        let mut clicked_three_times = ExpansionState::new();
        clicked_three_times.bind(&projection);
        for _ in 0..3 {
            clicked_three_times.toggle("result.solutions");
        }
        assert_eq!(
            expansion, clicked_three_times,
            "click, Space and Enter leave the same state as three clicks"
        );
    }

    /// RES-004: the collapse state follows the committed result, not the paint pass. An edit
    /// (a new revision) and a quick eval (new logs/output, no published snapshot) keep it; a new
    /// result clears it and paints expanded.
    #[test]
    fn a_result_collapse_survives_edits_and_a_quick_eval_and_a_new_result_clears_it() {
        let ctx = egui::Context::default();
        let view = solve_result_view(["-2", "2"]);
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let _ = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        expansion.set_collapsed("result.solutions", true);

        // An unrelated update: the buffer moved to revision 99 and the frame carries a quick
        // eval's output and a new run state. The committed result itself is unchanged.
        let mut edited = view.clone();
        edited.revision = crate::execution::SourceRevision(99);
        edited.output.push("6 * 7".to_string());
        edited.elapsed_ms = 3;
        edited.run_state = RunState::Running;
        let texts = paint_result_frame(&ctx, &edited, &mut expansion, &mut toggles, Vec::new());
        assert!(
            expansion.is_collapsed("result.solutions"),
            "an edit and a quick eval must not reset the tree"
        );
        assert!(
            !texts
                .iter()
                .any(|text| text.contains("Solution - 2 fields")),
            "the collapsed field stays collapsed across the repaint: {texts:?}"
        );

        // A new result replaces the old one: the map is cleared and the new tree paints expanded.
        let new_view = solve_result_view(["-3", "3"]);
        let texts = paint_result_frame(&ctx, &new_view, &mut expansion, &mut toggles, Vec::new());
        assert!(expansion.is_empty(), "a new result clears the path map");
        assert!(!expansion.is_collapsed("result.solutions"));
        assert!(
            texts
                .iter()
                .any(|text| text.contains("[0] Solution - 2 fields")),
            "the new result paints expanded: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "pretty     -3"),
            "and paints its own values: {texts:?}"
        );
    }

    /// One symbolic root view at the paint layer: `Symbolic x^2 - 1` with both renderings.
    fn symbolic_result_view() -> DocumentView {
        use lovelace_json::Json;
        let structured = Json::obj([
            ("kind", Json::Str("Symbolic".to_string())),
            ("pretty", Json::Str("x^2 - 1".to_string())),
            (
                "canonical",
                Json::Str("(add (pow (sym x) (rat 2 1)) (rat -1 1))".to_string()),
            ),
            ("freeSymbols", Json::str_array(["x"])),
            ("nodeCount", Json::Str("5".to_string())),
        ]);
        let mut view = result_view("Symbolic x^2 - 1");
        view.result_structured = Some(structured);
        view
    }

    /// RES-004: a symbolic node is collapsible like any other node; collapsing hides both
    /// renderings, the model keeps them, and re-expanding is byte-identical.
    #[test]
    fn the_result_tree_collapses_a_symbolic_result_without_losing_it() {
        let ctx = egui::Context::default();
        let view = symbolic_result_view();
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let expanded = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            expanded
                .iter()
                .any(|text| text == "\u{25be} Symbolic  x^2 - 1"),
            "the symbolic header names the pretty rendering: {expanded:?}"
        );
        assert!(
            expanded.iter().any(|text| text == "pretty     x^2 - 1"),
            "the expanded symbolic node paints its pretty rendering: {expanded:?}"
        );

        expansion.set_collapsed("result", true);
        let collapsed = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            collapsed
                .iter()
                .any(|text| text == "\u{25b8} Symbolic  x^2 - 1"),
            "the collapsed symbol keeps a header with the right-pointing indicator: {collapsed:?}"
        );
        assert!(
            !collapsed.iter().any(|text| text.starts_with("pretty")),
            "collapsing hides the pretty rendering: {collapsed:?}"
        );
        assert!(
            !collapsed.iter().any(|text| text.starts_with("canonical")),
            "collapsing hides the canonical rendering: {collapsed:?}"
        );

        let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
        let tree = projection.tree().expect("the view's projection is a tree");
        let StructuredNode::Symbolic(symbolic) = &tree else {
            panic!("the projection is a symbolic node");
        };
        assert_eq!(symbolic.pretty, "x^2 - 1");
        assert!(!symbolic.canonical.is_empty());

        expansion.set_collapsed("result", false);
        let again = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert_eq!(
            again, expanded,
            "re-expanding the symbolic node paints byte-identical content"
        );
    }

    /// RES-004: an array row is a collapsible group of its own. Collapsing one matrix/vector row
    /// hides that row's children and leaves its sibling untouched.
    #[test]
    fn an_array_row_collapses_independently_of_its_sibling() {
        let ctx = egui::Context::default();
        let view = solve_result_view(["-2", "2"]);
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let expanded = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            expanded
                .iter()
                .any(|text| text.ends_with("[0] Solution - 2 fields")),
            "the vector paints one header per row: {expanded:?}"
        );
        assert!(
            expanded.iter().any(|text| text == "pretty     -2"),
            "row 0 paints its symbolic value: {expanded:?}"
        );
        assert!(
            expanded.iter().any(|text| text == "pretty     2"),
            "row 1 paints its symbolic value: {expanded:?}"
        );

        expansion.set_collapsed("result.solutions[0]", true);
        let collapsed = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            collapsed
                .iter()
                .any(|text| text == "\u{25b8} [0] Solution - 2 fields"),
            "row 0 is collapsed: {collapsed:?}"
        );
        assert!(
            !collapsed.iter().any(|text| text == "pretty     -2"),
            "collapsing row 0 hides its value: {collapsed:?}"
        );
        assert!(
            collapsed.iter().any(|text| text == "pretty     2"),
            "row 1 keeps its value: {collapsed:?}"
        );

        let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
        let tree = projection.tree().expect("the view's projection is a tree");
        let StructuredNode::Record(record) = &tree else {
            panic!("the projection is a record");
        };
        let StructuredNode::Array(solutions) = record.field("solutions").unwrap() else {
            panic!("solutions is a vector");
        };
        assert_eq!(solutions.elements.len(), 2, "both rows stay in the model");

        expansion.set_collapsed("result.solutions[0]", false);
        let again = paint_result_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert_eq!(
            again, expanded,
            "re-expanding a row paints byte-identical content"
        );
    }

    /// RES-004 end to end through the shipped window layout: a pointer click on a painted field
    /// header collapses that path in the live tree, and editing the buffer (which advances the
    /// revision and marks the result stale without publishing a new one) leaves the map alone.
    #[test]
    #[cfg(feature = "real-engine")]
    fn a_window_click_collapses_a_field_and_an_edit_keeps_it() {
        let mut workbench = Workbench::new();
        workbench.set_source(
            0,
            "x = symbol(\"x\")\nsolve_full(x^2 - 4 == 0, x)".to_string(),
        );
        let mut app = WorkbenchApp::with_workbench(workbench);
        assert!(app.workbench.run_active(), "the test run must start");
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(15)),
            "the solve run must finish"
        );

        let ctx = egui::Context::default();
        let screen = vec2(2500.0, 1380.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let rect = app
            .result_toggle_rects
            .iter()
            .find(|(path, _)| path == "result.solutions")
            .map(|(_, rect)| *rect)
            .expect("the window paints a header for result.solutions");
        let pos = rect.center();
        frame_sized(&ctx, &mut app, screen, vec![egui::Event::PointerMoved(pos)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, true)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, false)]);
        assert!(
            app.expansion.is_collapsed("result.solutions"),
            "the click on the painted header must collapse the field"
        );

        // An edit advances the buffer revision and marks the displayed result stale; it
        // publishes no new result, so the tree keeps every expansion decision.
        app.workbench.set_source(
            0,
            "x = symbol(\"x\")\nsolve_full(x^2 - 4 == 0, x)\n# edited".to_string(),
        );
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert!(
            app.expansion.is_collapsed("result.solutions"),
            "editing the buffer must not reset the result tree"
        );
    }

    /// SYM-001: one inspection payload in the exact shape `engine::real::inspection` publishes.
    /// `mathir` is either the supplied IR object or a `{reason}` object.
    fn inspector_payload(mathir: lovelace_json::Json) -> lovelace_json::Json {
        use lovelace_json::Json;
        let relation = Json::obj([
            ("source", Json::Str("session".to_string())),
            ("kind", Json::Str("Relation".to_string())),
            ("op", Json::Str("gt".to_string())),
            ("opLabel", Json::Str(">".to_string())),
            ("left", Json::Str("x".to_string())),
            ("right", Json::Str("0".to_string())),
            ("label", Json::Str("x > 0".to_string())),
            ("canonical", Json::Str("(gt (sym x) (rat 0 1))".to_string())),
        ]);
        let leaf = |kind: &str, label: &str| {
            Json::obj([
                ("kind", Json::Str(kind.to_string())),
                ("label", Json::Str(label.to_string())),
                ("children", Json::Arr(Vec::new())),
            ])
        };
        let finite = Json::obj([
            ("source", Json::Str("required".to_string())),
            ("kind", Json::Str("FiniteCondition".to_string())),
            (
                "label",
                Json::Str("FiniteCondition(expression: log(x))".to_string()),
            ),
        ]);
        Json::obj([
            (
                "subject",
                Json::obj([
                    ("kind", Json::Str("Symbolic".to_string())),
                    ("path", Json::Str("result".to_string())),
                ]),
            ),
            (
                "sections",
                Json::obj([
                    (
                        "pretty",
                        Json::obj([("text", Json::Str("3*x^2".to_string()))]),
                    ),
                    (
                        "canonical",
                        Json::obj([(
                            "text",
                            Json::Str("(mul (rat 3 1) (pow (sym x) (rat 2 1)))".to_string()),
                        )]),
                    ),
                    (
                        "tree",
                        Json::obj([
                            ("nodes", Json::Num("5".to_string())),
                            (
                                "root",
                                Json::obj([
                                    ("kind", Json::Str("Mul".to_string())),
                                    ("label", Json::Str("3*x^2".to_string())),
                                    (
                                        "children",
                                        Json::Arr(vec![
                                            leaf("Num", "3"),
                                            Json::obj([
                                                ("kind", Json::Str("Pow".to_string())),
                                                ("label", Json::Str("x^2".to_string())),
                                                (
                                                    "children",
                                                    Json::Arr(vec![
                                                        leaf("Sym", "x"),
                                                        leaf("Num", "2"),
                                                    ]),
                                                ),
                                            ]),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                    ),
                    (
                        "assumptions",
                        Json::obj([("items", Json::Arr(vec![relation]))]),
                    ),
                    (
                        "trace",
                        Json::obj([(
                            "steps",
                            Json::Arr(vec![Json::obj([
                                ("ruleId", Json::Str("logexp.exp-log".to_string())),
                                ("classification", Json::Str("Conditional".to_string())),
                                (
                                    "before",
                                    Json::obj([
                                        ("pretty", Json::Str("exp(log(x))".to_string())),
                                        (
                                            "canonical",
                                            Json::Str("(fun exp (fun log (sym x)))".to_string()),
                                        ),
                                    ]),
                                ),
                                (
                                    "after",
                                    Json::obj([
                                        ("pretty", Json::Str("x".to_string())),
                                        ("canonical", Json::Str("(sym x)".to_string())),
                                    ]),
                                ),
                                ("conditions", Json::Arr(vec![finite])),
                            ])]),
                        )]),
                    ),
                    ("mathir", mathir),
                ]),
            ),
        ])
    }

    fn inspector_view(mathir: lovelace_json::Json) -> DocumentView {
        let mut view = result_view("Symbolic 3*x^2");
        view.result = "3*x^2".to_string();
        view.result_structured = Some(lovelace_json::Json::obj([
            ("kind", lovelace_json::Json::Str("Symbolic".to_string())),
            ("pretty", lovelace_json::Json::Str("3*x^2".to_string())),
            (
                "canonical",
                lovelace_json::Json::Str("(mul (rat 3 1) (pow (sym x) (rat 2 1)))".to_string()),
            ),
        ]));
        view.result_inspection = Some(inspector_payload(mathir));
        view
    }

    fn supplied_mathir() -> lovelace_json::Json {
        use lovelace_json::Json;
        Json::obj([
            (
                "text",
                Json::Str(
                    "#!mathir 2\nparam x\nParameter 0 0\nPowInt 0 0,1\nMul 0 0,2".to_string(),
                ),
            ),
            ("parameters", Json::str_array(["x"])),
            ("resultType", Json::Str("Integer".to_string())),
        ])
    }

    fn missing_mathir() -> lovelace_json::Json {
        use lovelace_json::Json;
        Json::obj([(
            "reason",
            Json::Str("Node kind Relation cannot be lowered to MathIR.".to_string()),
        )])
    }

    /// Paints one inspector pass and returns every Text shape in paint order.
    fn paint_inspector_frame(
        ctx: &egui::Context,
        view: &DocumentView,
        expansion: &mut InspectorExpansion,
        toggles: &mut Vec<(String, Rect)>,
        events: Vec<egui::Event>,
    ) -> Vec<String> {
        toggles.clear();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 1200.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                paint_inspector(ui, view, 420.0, expansion, toggles);
            });
        });
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        texts
    }

    /// SYM-001 at the paint layer: all six sections come from the payload, not from the display.
    #[test]
    fn the_inspector_paints_all_six_sections_from_the_engine_payload() {
        let ctx = egui::Context::default();
        let view = inspector_view(supplied_mathir());
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let texts = paint_inspector_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());

        assert!(
            texts
                .iter()
                .any(|text| text == "INSPECTOR · last committed result"),
            "the inspector heading: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "Pretty"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "3*x^2"),
            "the engine's pretty text: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "Canonical"), "{texts:?}");
        assert!(
            texts
                .iter()
                .any(|text| text == "(mul (rat 3 1) (pow (sym x) (rat 2 1)))"),
            "the engine's canonical text: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "Expression tree"),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.ends_with("Mul · 3*x^2")),
            "the tree root's node kind and engine rendering: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "Num · 3"),
            "a leaf child in engine child order: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "Assumptions"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "relation >"),
            "the assumption is a typed relation, not prose: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "x > 0"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "Simplification trace"),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "1. logexp.exp-log"),
            "{texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "classification  Conditional"),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "before  exp(log(x))"),
            "{texts:?}"
        );
        assert!(texts.iter().any(|text| text == "after   x"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "FiniteCondition"),
            "the required condition's engine record kind: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "MathIR"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text.contains("#!mathir 2")),
            "the engine IR text is painted: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.contains("display:")),
            "the inspector never paints the result display caption: {texts:?}"
        );
    }

    /// SYM-009: a subject that cannot lower to MathIR keeps its reason while every other
    /// supplied section stays usable.
    #[test]
    fn a_missing_mathir_shows_the_reason_and_leaves_the_other_sections_usable() {
        let ctx = egui::Context::default();
        let view = inspector_view(missing_mathir());
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let texts = paint_inspector_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());

        assert!(
            texts
                .iter()
                .any(|text| text == "Node kind Relation cannot be lowered to MathIR."),
            "the engine's specific refusal: {texts:?}"
        );
        assert!(texts.iter().any(|text| text == "MathIR"), "{texts:?}");
        assert!(
            texts.iter().any(|text| text == "3*x^2"),
            "pretty stays usable: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "(mul (rat 3 1) (pow (sym x) (rat 2 1)))"),
            "canonical stays usable: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.ends_with("Mul · 3*x^2")),
            "the tree stays usable: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "relation >"),
            "assumptions stay usable: {texts:?}"
        );
    }

    /// SYM-002: the inspector renders the last committed result, not the editor buffer. An edit
    /// changes neither the payload nor the painted inspector; a new committed result does both.
    #[test]
    #[cfg(feature = "real-engine")]
    fn the_inspector_is_bound_to_the_committed_result_not_the_buffer() {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::RealEngine::new())));
        workbench.set_source(
            0,
            "x = symbol(\"x\")\nassume(x > 0)\ndiff(x^3, x)".to_string(),
        );
        assert!(workbench.run_active(), "the first run must start");
        assert!(
            workbench.wait_until_idle(Duration::from_secs(15)),
            "the first run must finish"
        );
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        let screen = vec2(2500.0, 1380.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());

        let before = app.view.clone();
        assert!(
            before.result_inspection.is_some(),
            "the real engine published an inspection payload"
        );
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let first = paint_inspector_frame(&ctx, &before, &mut expansion, &mut toggles, Vec::new());
        assert!(
            first.iter().any(|text| text == "3*x^2"),
            "the committed diff result is inspected: {first:?}"
        );
        assert!(
            first.iter().any(|text| text == "x > 0"),
            "the session assumption is inspected: {first:?}"
        );

        // Editing advances the buffer revision and marks the result stale, but publishes no new
        // result: the inspector must not move.
        app.workbench
            .set_source(0, "x = symbol(\"x\")\n1 + 1".to_string());
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert_eq!(
            app.view.result_inspection, before.result_inspection,
            "editing the buffer must not change the inspected payload"
        );
        let after_edit =
            paint_inspector_frame(&ctx, &app.view, &mut expansion, &mut toggles, Vec::new());
        assert_eq!(
            after_edit, first,
            "editing the buffer must paint the same inspector"
        );

        // A new committed result replaces the payload, the tree and the rendered text.
        app.workbench.set_source(
            0,
            "x = symbol(\"x\")\nassume(x > 0)\ndiff(x^4, x)".to_string(),
        );
        assert!(app.workbench.run_active(), "the second run must start");
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(15)),
            "the second run must finish"
        );
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert_ne!(
            app.view.result_inspection, before.result_inspection,
            "a new committed result must publish a different inspection"
        );
        let second =
            paint_inspector_frame(&ctx, &app.view, &mut expansion, &mut toggles, Vec::new());
        assert!(
            second.iter().any(|text| text == "4*x^3"),
            "the new result's pretty form is painted: {second:?}"
        );
        assert!(
            !second.iter().any(|text| text == "3*x^2"),
            "the old result's pretty form is gone: {second:?}"
        );
    }

    /// SYM-001 through the shipped window layout: the Workspace pane's Inspector tab is a real
    /// pointer target, and selecting it swaps the Result view for the inspector.
    #[test]
    #[cfg(feature = "real-engine")]
    fn the_workspace_inspector_tab_swaps_the_view_and_the_result_tab_restores_it() {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::RealEngine::new())));
        workbench.set_source(
            0,
            "x = symbol(\"x\")\nassume(x > 0)\ndiff(x^3, x)".to_string(),
        );
        assert!(workbench.run_active());
        assert!(workbench.wait_until_idle(Duration::from_secs(15)));
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        let screen = vec2(2500.0, 1380.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert!(!app.inspector_open, "the Result view is the default");

        let inspector_tab =
            app.workspace_view_tabs[1].expect("the window painted the Inspector tab");
        let pos = inspector_tab.center();
        frame_sized(&ctx, &mut app, screen, vec![egui::Event::PointerMoved(pos)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, true)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, false)]);
        assert!(
            app.inspector_open,
            "clicking the Inspector tab must select it"
        );
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let mut texts = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), screen)),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.ui_root(ui));
        output.textures_delta.clear();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            texts
                .iter()
                .any(|text| text == "INSPECTOR · last committed result"),
            "the inspector heading is painted: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "3*x^2"),
            "the committed pretty form is painted: {texts:?}"
        );
        assert!(
            !texts
                .iter()
                .any(|text| text == "RESULT · structured projection"),
            "the Result view is replaced, not stacked: {texts:?}"
        );

        let result_tab = app.workspace_view_tabs[0].expect("the window painted the Result tab");
        let pos = result_tab.center();
        frame_sized(&ctx, &mut app, screen, vec![egui::Event::PointerMoved(pos)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, true)]);
        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, false)]);
        assert!(
            !app.inspector_open,
            "clicking Result must restore the result view"
        );
    }

    // -----------------------------------------------------------------------------------------
    // DOC-002 / DOC-003: the window's file lifecycle and the unsaved-changes prompt.
    // -----------------------------------------------------------------------------------------

    /// A file in the test scratch directory, unique per test and process.
    fn fixture(name: &str, bytes: &[u8]) -> PathBuf {
        let directory = std::env::temp_dir().join("lovelace-desktop-window-file-tests");
        std::fs::create_dir_all(&directory).expect("scratch directory");
        let path = directory.join(format!("{name}-{}.lv", std::process::id()));
        std::fs::write(&path, bytes).expect("fixture bytes");
        path
    }

    /// A workbench with the fixture opened in its own tab, which becomes active.
    fn opened_workbench(path: &std::path::Path) -> Workbench {
        let mut workbench = slow_workbench();
        workbench.open(path).expect("open the fixture");
        workbench
    }

    /// Answers both dialogs from a queue, so Open and Save As can run headlessly.
    #[derive(Default)]
    struct ScriptedDialogs {
        open: std::collections::VecDeque<Option<PathBuf>>,
        save: std::collections::VecDeque<Option<PathBuf>>,
    }

    impl ScriptedDialogs {
        fn opening(paths: impl IntoIterator<Item = Option<PathBuf>>) -> ScriptedDialogs {
            ScriptedDialogs {
                open: paths.into_iter().collect(),
                save: std::collections::VecDeque::new(),
            }
        }

        fn saving(paths: impl IntoIterator<Item = Option<PathBuf>>) -> ScriptedDialogs {
            ScriptedDialogs {
                open: std::collections::VecDeque::new(),
                save: paths.into_iter().collect(),
            }
        }
    }

    impl FileDialogs for ScriptedDialogs {
        fn open(&mut self) -> Option<PathBuf> {
            self.open.pop_front().flatten()
        }

        fn save_as(
            &mut self,
            _suggested: &str,
            _initial_dir: Option<&std::path::Path>,
        ) -> Option<PathBuf> {
            self.save.pop_front().flatten()
        }
    }

    /// One pass with no events, used to settle the frame after a state change.
    fn settle(ctx: &egui::Context, app: &mut WorkbenchApp) {
        frame(ctx, app, Vec::new());
        frame(ctx, app, Vec::new());
    }

    #[test]
    fn the_prompt_names_the_document_and_offers_save_discard_and_cancel() {
        let path = fixture("prompt-buttons", b"a = 1\nb = 2\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 3\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        assert!(app.close.is_some(), "a dirty tab must ask before closing");
        settle(&ctx, &mut app);

        assert_eq!(
            prompt_choices(&app),
            vec!["Save", "Discard", "Cancel"],
            "the prompt offers exactly the three answers, in that order"
        );
        let prompt = app.close.as_ref().expect("prompt");
        assert!(prompt.error.is_none());
        assert_eq!(prompt.pending.len(), 1);
        assert_eq!(prompt.total, 1);
        // The document the question names is the dirty one.
        assert_eq!(
            app.workbench
                .document_index(prompt.current())
                .map(|index| app.workbench.documents()[index].title_with_marker()),
            Some(format!("{} *", path.file_name().unwrap().to_string_lossy()))
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cancel_keeps_the_buffer_the_caret_the_tab_and_the_focus() {
        let path = fixture("prompt-cancel", b"a = 1\nb = 2\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 3\n".to_string());
        let tabs = workbench.documents().len();
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        app.editors[index].set_caret(1, 3);
        let caret = app.editors[index].caret();
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "the editor owns the focus before the prompt"
        );

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert!(app.close.is_some(), "the prompt must be open");
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            None,
            "the prompt owns the keyboard: the editor is not focused while it is open"
        );

        // Escape is the prompt's Cancel.
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );

        assert!(app.close.is_none(), "Cancel dismisses the prompt");
        assert_eq!(
            app.workbench.documents().len(),
            tabs,
            "Cancel closes nothing"
        );
        assert_eq!(
            app.workbench.documents()[index].source,
            "a = 1\nb = 3\n",
            "Cancel keeps the edited buffer"
        );
        assert!(
            app.workbench.documents()[index].dirty(),
            "it is still unsaved"
        );
        assert_eq!(app.editors[index].caret(), caret, "the caret has not moved");
        // The modal's layer blocks the editor for one pass after it stops being painted, so the
        // focus it had before the prompt is granted back on the pass after that.
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "focus is back in the editor, where it was"
        );
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\nb = 2\n",
            "Cancel writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn discard_closes_the_tab_without_writing_to_the_file() {
        let path = fixture("prompt-discard", b"x = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "x = 2\n".to_string());
        let tabs = workbench.documents().len();
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        settle(&ctx, &mut app);

        let discard = prompt_button(&app, UnsavedAnswer::Discard);
        pointer_click(&ctx, &mut app, discard);

        assert!(app.close.is_none(), "Discard finishes the close");
        assert_eq!(
            app.workbench.documents().len(),
            tabs - 1,
            "the tab is closed"
        );
        assert_eq!(app.editors.len(), tabs - 1, "its editor is gone too");
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"x = 1\n",
            "Discard writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_in_the_prompt_writes_the_edit_then_closes_the_tab() {
        let path = fixture("prompt-save", b"x = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "x = 2\n".to_string());
        let tabs = workbench.documents().len();
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        settle(&ctx, &mut app);

        let save = prompt_button(&app, UnsavedAnswer::Save);
        pointer_click(&ctx, &mut app, save);

        assert!(app.close.is_none(), "Save finishes the close");
        assert_eq!(app.workbench.documents().len(), tabs - 1);
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"x = 2\n",
            "Save in the prompt writes the edited buffer"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_clean_tab_closes_without_a_prompt() {
        let path = fixture("clean-close", b"x = 1\n");
        let workbench = opened_workbench(&path);
        let tabs = workbench.documents().len();
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);

        assert!(app.close.is_none(), "a clean tab must not ask anything");
        assert_eq!(app.workbench.documents().len(), tabs - 1);
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"x = 1\n",
            "closing a clean tab writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_window_close_request_with_unsaved_changes_is_refused_and_prompts() {
        let path = fixture("window-close", b"a = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 2\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        let output = close_request(&ctx, &mut app);

        assert!(
            cancelled_close(&output),
            "the window must refuse the close while a document is dirty"
        );
        assert!(app.close.is_some(), "the refusal is a prompt");
        assert!(
            !app.window_close_approved,
            "the window has not been allowed to close"
        );

        // Cancel the prompt with Escape: the window stays open, nothing was written.
        settle(&ctx, &mut app);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        assert!(app.close.is_none());
        assert!(!app.window_close_approved);
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\n",
            "refusing the close writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_window_close_request_on_a_clean_workbench_is_allowed() {
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        let output = close_request(&ctx, &mut app);

        assert!(
            !cancelled_close(&output),
            "a clean workbench does not refuse a close"
        );
        assert!(
            !approved_close(&output),
            "no close command is needed: the request is simply allowed through"
        );
        assert!(app.window_close_approved);
        assert!(app.close.is_none());
    }

    #[test]
    fn quit_from_the_file_bar_prompts_and_discard_closes_the_window() {
        let path = fixture("quit-prompt", b"a = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 2\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        let quit = app
            .file_buttons
            .iter()
            .find(|(label, _)| *label == "Quit")
            .expect("the File bar paints Quit")
            .1
            .center();
        pointer_click(&ctx, &mut app, quit);

        assert!(app.close.is_some(), "Quit asks about the dirty document");
        settle(&ctx, &mut app);
        let discard = prompt_button(&app, UnsavedAnswer::Discard);
        pointer_click(&ctx, &mut app, discard);

        assert!(app.close.is_none());
        assert!(
            app.window_close_approved,
            "Discard on Quit approves closing the window"
        );
        // The next OS close request is allowed through instead of asking again.
        let output = close_request(&ctx, &mut app);
        assert!(!cancelled_close(&output));
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\n",
            "Discard from Quit writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saving_a_pathless_document_from_the_prompt_asks_for_a_path_then_closes() {
        let path = fixture("prompt-save-as", b"");
        let _ = std::fs::remove_file(&path);
        let mut workbench = slow_workbench();
        let index = workbench.new_document();
        workbench.set_source(index, "q = 5\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.dialogs = Box::new(ScriptedDialogs::saving([Some(path.clone())]));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert!(app.close.is_some(), "the dirty tab prompts first");
        assert!(
            prompt_choices(&app).contains(&"Save"),
            "the prompt offers Save"
        );

        // Answer Save: the workbench has no path, so the scripted Save As dialog supplies one.
        let save = prompt_button(&app, UnsavedAnswer::Save);
        pointer_click(&ctx, &mut app, save);

        assert!(app.close.is_none(), "the flow finished");
        assert_eq!(std::fs::read(&path).expect("read back"), b"q = 5\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ctrl_n_adds_a_tab_and_ctrl_w_closes_the_clean_one() {
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        assert_eq!(app.workbench.documents().len(), 2);

        frame(&ctx, &mut app, vec![key(Key::N, ctrl_modifiers())]);

        assert_eq!(app.workbench.documents().len(), 3, "Ctrl+N adds a tab");
        assert_eq!(app.editors.len(), 3, "and an editor for it");
        assert_eq!(app.workbench.active(), 2);
        assert_eq!(
            app.workbench.documents()[2].title_with_marker(),
            "untitled-2.lv"
        );
        assert_eq!(app.editors[2].source(), "", "the new tab starts empty");
        assert!(
            app.window_title.ends_with("untitled-2.lv"),
            "the window title names the active document: {:?}",
            app.window_title
        );

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);

        assert_eq!(app.workbench.documents().len(), 2, "Ctrl+W closes it");
        assert_eq!(app.editors.len(), 2);
        assert!(app.close.is_none(), "a clean tab does not prompt");
    }

    #[test]
    fn ctrl_shift_s_asks_for_a_path_and_ctrl_s_then_writes_to_it() {
        let path = fixture("save-as-binding", b"");
        let _ = std::fs::remove_file(&path);
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        app.dialogs = Box::new(ScriptedDialogs::saving([Some(path.clone())]));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::S, ctrl_shift_modifiers())]);

        assert_eq!(
            app.workbench.documents()[0].path.as_deref(),
            Some(path.as_path()),
            "Ctrl+Shift+S adopts the chosen path"
        );
        assert_eq!(
            app.workbench.documents()[0].title,
            path.file_name().unwrap().to_string_lossy(),
            "Save As renames the tab"
        );
        let written = std::fs::read(&path).expect("the starter script was written");
        assert!(
            written.starts_with(b"# Lovelace Desktop."),
            "Save As wrote the buffer: {:?}",
            String::from_utf8_lossy(&written)
        );

        // One edit, then Ctrl+S: the document has a path, so no dialog may be needed. The
        // scripted queue is empty, so a dialog would return None and nothing would be written.
        frame(&ctx, &mut app, vec![egui::Event::Text("w".to_string())]);
        assert!(app.workbench.documents()[0].dirty());
        assert!(app.window_title.ends_with(" *"), "{:?}", app.window_title);
        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);

        let written = std::fs::read(&path).expect("read back");
        assert!(
            written.starts_with(b"w# Lovelace Desktop."),
            "Ctrl+S wrote the edit to the document's own path: {:?}",
            String::from_utf8_lossy(&written)
        );
        assert!(
            !app.workbench.documents()[0].dirty(),
            "the marker is cleared"
        );
        assert!(!app.window_title.ends_with(" *"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ctrl_o_opens_the_file_the_dialog_returns_and_the_tab_shows_its_name() {
        let path = fixture("open-binding", b"u = 7\r\n");
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        app.dialogs = Box::new(ScriptedDialogs::opening([Some(path.clone())]));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::O, ctrl_modifiers())]);

        assert_eq!(app.workbench.documents().len(), 3, "Open added a tab");
        assert_eq!(app.editors.len(), 3, "with an editor");
        let document = &app.workbench.documents()[2];
        assert_eq!(document.title, path.file_name().unwrap().to_string_lossy());
        assert_eq!(document.source, "u = 7\r\n", "the CRLF text is intact");
        assert_eq!(document.newline, crate::document_io::NewlineStyle::CrLf);
        assert!(!document.dirty());
        assert!(
            app.window_title.ends_with(&document.title),
            "the window title names the opened file: {:?}",
            app.window_title
        );
        assert_eq!(
            app.editors[2].lines,
            vec!["u = 7", ""],
            "the editor shows no phantom carriage return"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn opening_a_non_utf8_file_through_the_dialog_is_refused_without_a_new_tab() {
        let path = fixture("open-bad-utf8", b"u = \xE9\n");
        let mut app = WorkbenchApp::with_workbench(slow_workbench());
        app.dialogs = Box::new(ScriptedDialogs::opening([Some(path.clone())]));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::O, ctrl_modifiers())]);

        assert_eq!(
            app.workbench.documents().len(),
            2,
            "a refused file must not create a tab"
        );
        let logged = app
            .workbench
            .active_document()
            .logs
            .iter()
            .any(|line| line.kind == LogKind::Diagnostic && line.text.contains("not UTF-8"));
        assert!(logged, "the refusal is in the log with its reason");
        assert_eq!(
            std::fs::read(&path).expect("the file is untouched"),
            b"u = \xE9\n"
        );
        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------------------------
    // DOC-004 / DOC-005: the persisted session, the recovery journal and the recovery prompt.
    // -----------------------------------------------------------------------------------------

    /// Scratch storage for a launch test: a settings file and a recovery directory.
    fn app_storage(name: &str) -> (AppStorage, PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "lovelace-desktop-launch-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch directory");
        (
            AppStorage {
                settings_path: Some(directory.join("desktop.json")),
                recovery_root: Some(directory.join("recovery")),
            },
            directory,
        )
    }

    /// Writes the state a forced kill leaves: a session naming the file and a journaled edit.
    fn crash_state(
        storage: &AppStorage,
        file: &std::path::Path,
        source: &str,
        caret: (usize, usize),
    ) -> RecoveryRecord {
        let record = RecoveryRecord {
            document_id: 1,
            revision: 1,
            title: file.file_name().unwrap().to_string_lossy().into_owned(),
            path: Some(file.to_path_buf()),
            bom: false,
            caret,
            selection: None,
            source_mtime: None,
            saved_at: std::time::SystemTime::UNIX_EPOCH,
            source: source.to_string(),
        };
        let root = storage.recovery_root.as_ref().expect("recovery root");
        record
            .write_atomic(&recovery::record_path(root, 1))
            .expect("write the recovery record");
        let mut settings = Settings::new();
        settings.set_session(crate::settings::SessionState::new(
            vec![crate::settings::SessionTab {
                path: Some(file.to_string_lossy().into_owned()),
                document_id: 1,
            }],
            0,
        ));
        settings
            .save(storage.settings_path.as_ref().expect("settings path"))
            .expect("save the settings");
        record
    }

    /// The centre of a recovery prompt button painted in the last pass.
    fn recovery_button(app: &WorkbenchApp, choice: RecoveryAnswer) -> egui::Pos2 {
        app.recovery_buttons
            .iter()
            .find(|(_, painted, _)| *painted == choice)
            .map(|(_, _, rect)| rect.center())
            .unwrap_or_else(|| panic!("the {choice:?} button was not painted"))
    }

    /// The recovery prompt buttons in their on-screen order.
    fn recovery_choices(app: &WorkbenchApp) -> Vec<&'static str> {
        app.recovery_buttons
            .iter()
            .map(|(_, choice, _)| choice.label())
            .collect()
    }

    #[test]
    fn the_recovery_prompt_offers_a_crashed_buffer_and_recovering_restores_it_before_any_save() {
        let (storage, directory) = app_storage("recover");
        let file = directory.join("notes.lv");
        std::fs::write(&file, b"a = 1\nb = 2\n").expect("fixture");
        let record = crash_state(&storage, &file, "a = 1\nb = 3\n", (1, 3));
        let record_path = recovery::record_path(storage.recovery_root.as_ref().unwrap(), 1);
        assert!(record_path.exists());

        let mut app = WorkbenchApp::launch(storage.clone(), None);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        // The session restored the file from disk; the missing edit is only in the journal.
        assert_eq!(app.workbench.documents().len(), 1);
        assert_eq!(app.workbench.documents()[0].source, "a = 1\nb = 2\n");
        assert_eq!(
            std::fs::read(&file).expect("their file"),
            b"a = 1\nb = 2\n",
            "restoring reads the file and writes nothing"
        );
        let prompt = app
            .recovery_prompt
            .as_ref()
            .expect("a surviving record is offered at launch");
        assert_eq!(prompt.records.len(), 1);
        assert_eq!(
            prompt.records[0], record,
            "the prompt lists the journaled record"
        );
        assert_eq!(
            prompt.records[0].path_label(),
            file.display().to_string(),
            "the prompt reports the original path"
        );
        assert_eq!(recovery_choices(&app), vec!["Discard", "Recover"]);

        let recover_at = recovery_button(&app, RecoveryAnswer::Recover);
        pointer_click(&ctx, &mut app, recover_at);
        settle(&ctx, &mut app);

        assert!(
            app.recovery_prompt.is_none(),
            "the answer closes the prompt"
        );
        let index = app.workbench.active();
        let document = &app.workbench.documents()[index];
        assert_eq!(
            document.source, "a = 1\nb = 3\n",
            "the exact journaled text is back"
        );
        assert_eq!(document.path.as_deref(), Some(file.as_path()));
        assert_eq!(document.caret, (1, 3), "the journaled caret is back");
        assert!(document.dirty(), "the recovered text is unsaved work");
        assert_eq!(
            app.editors[index].source(),
            "a = 1\nb = 3\n",
            "the editor shows the recovered text"
        );
        assert_eq!(
            std::fs::read(&file).expect("their file"),
            b"a = 1\nb = 2\n",
            "recovering never writes the document file"
        );

        // Saving is the first action that may overwrite the file, and it retires the record.
        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert_eq!(
            std::fs::read(&file).expect("the saved file"),
            b"a = 1\nb = 3\n"
        );
        assert!(!app.workbench.documents()[index].dirty());
        assert!(
            !record_path.exists(),
            "a clean document has no recovery record"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn discarding_a_recovery_record_deletes_it_and_leaves_the_file_and_buffer_alone() {
        let (storage, directory) = app_storage("discard");
        let file = directory.join("notes.lv");
        std::fs::write(&file, b"a = 1\nb = 2\n").expect("fixture");
        crash_state(&storage, &file, "a = 1\nb = 9\n", (0, 0));
        let record_path = recovery::record_path(storage.recovery_root.as_ref().unwrap(), 1);

        let mut app = WorkbenchApp::launch(storage.clone(), None);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        assert!(app.recovery_prompt.is_some());

        let discard_at = recovery_button(&app, RecoveryAnswer::Discard);
        pointer_click(&ctx, &mut app, discard_at);
        settle(&ctx, &mut app);

        assert!(app.recovery_prompt.is_none());
        assert!(!record_path.exists(), "discarding deletes the record");
        assert_eq!(std::fs::read(&file).expect("their file"), b"a = 1\nb = 2\n");
        assert_eq!(
            app.workbench.documents()[0].source,
            "a = 1\nb = 2\n",
            "discarding changes no buffer"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_corrupt_recovery_record_is_skipped_with_a_warning_and_the_launch_continues() {
        let (storage, directory) = app_storage("corrupt");
        let file = directory.join("notes.lv");
        std::fs::write(&file, b"a = 1\n").expect("fixture");
        crash_state(&storage, &file, "a = 1\nb = 3\n", (0, 0));
        let root = storage.recovery_root.as_ref().unwrap();
        let corrupt = recovery::record_path(root, 9);
        std::fs::write(&corrupt, b"LOVELACE-RECOVERY 1\nid: 9\nbytes: 100\n\nshort")
            .expect("corrupt record");

        let mut app = WorkbenchApp::launch(storage.clone(), None);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        assert_eq!(
            app.recovery_prompt
                .as_ref()
                .map(|prompt| prompt.records.len()),
            Some(1),
            "only the valid record is offered"
        );
        let warned = app
            .workbench
            .documents()
            .iter()
            .flat_map(|document| document.logs.iter())
            .any(|line| {
                line.kind == LogKind::Diagnostic
                    && line.text.contains("corrupt recovery record")
                    && line.text.contains("doc-9.lvrec")
            });
        assert!(warned, "the skipped record is a warning naming the file");
        assert_eq!(std::fs::read(&file).expect("their file"), b"a = 1\n");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_session_is_written_at_a_clean_quit_and_restored_on_the_next_launch() {
        let (storage, directory) = app_storage("session");
        let first = directory.join("first.lv");
        let second = directory.join("second.lv");
        std::fs::write(&first, b"a = 1\n").expect("fixture");
        std::fs::write(&second, b"b = 2\n").expect("fixture");

        let ctx = egui::Context::default();
        let mut app = WorkbenchApp::launch(storage.clone(), None);
        app.dialogs = Box::new(ScriptedDialogs::opening([
            Some(first.clone()),
            Some(second.clone()),
        ]));
        settle(&ctx, &mut app);
        frame(&ctx, &mut app, vec![key(Key::O, ctrl_modifiers())]);
        frame(&ctx, &mut app, vec![key(Key::O, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert_eq!(app.workbench.documents().len(), 4);
        let first_tab = app
            .workbench
            .documents()
            .iter()
            .position(|document| document.path.as_deref() == Some(first.as_path()))
            .expect("the first file has a tab");
        app.workbench.set_active(first_tab);
        settle(&ctx, &mut app);

        let output = close_request(&ctx, &mut app);
        assert!(
            !cancelled_close(&output),
            "a clean workbench does not refuse the close request"
        );
        assert!(app.window_close_approved);

        let saved = Settings::load(storage.settings_path.as_ref().expect("settings path"));
        assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
        let session = saved.session.expect("the clean quit wrote the session");
        assert_eq!(session.documents.len(), 4, "every tab is in the session");
        assert_eq!(
            session.active, first_tab,
            "the active tab is in the session"
        );
        assert_eq!(
            session.documents[2].path.as_deref(),
            Some(first.to_string_lossy().as_ref()),
            "tab order is the tab order"
        );
        assert_eq!(
            session.documents[3].path.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );
        assert!(
            session.documents[0].path.is_none(),
            "a pathless tab keeps its place with no path"
        );

        let mut relaunched = WorkbenchApp::launch(storage.clone(), None);
        settle(&ctx, &mut relaunched);
        let titles: Vec<&str> = relaunched
            .workbench
            .documents()
            .iter()
            .map(|document| document.title.as_str())
            .collect();
        assert_eq!(
            titles,
            vec!["first.lv", "second.lv"],
            "the file tabs come back in their original order"
        );
        assert_eq!(
            relaunched.workbench.active_document().title,
            "first.lv",
            "the active tab comes back"
        );
        assert!(
            relaunched
                .workbench
                .documents()
                .iter()
                .all(|document| !document.dirty() && document.state() == RunState::Idle),
            "a restored session is clean and has not run anything"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn discarding_at_the_window_close_does_not_recreate_the_record() {
        let (storage, directory) = app_storage("discard-quit");
        let file = directory.join("quit.lv");
        std::fs::write(&file, b"a = 1\n").expect("fixture");
        let mut app = WorkbenchApp::launch(storage.clone(), Some(file.clone()));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        let index = app.workbench.active();
        app.workbench.set_source(index, "a = 2\n".to_string());
        settle(&ctx, &mut app);
        let record_path = recovery::record_path(
            storage.recovery_root.as_ref().expect("recovery root"),
            app.workbench.documents()[index].id.0,
        );
        assert!(record_path.exists(), "the dirty buffer is journaled");

        let output = close_request(&ctx, &mut app);
        assert!(
            cancelled_close(&output),
            "a dirty window refuses the close and asks first"
        );
        settle(&ctx, &mut app);
        assert!(app.close.is_some());

        let discard_at = prompt_button(&app, UnsavedAnswer::Discard);
        pointer_click(&ctx, &mut app, discard_at);

        assert!(
            app.window_close_approved,
            "Discard approves the window close"
        );
        assert!(
            !record_path.exists(),
            "the clean exit deleted the record and the closing frame did not journal it again"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn an_edit_journals_the_buffer_and_the_save_retires_the_record() {
        let (storage, directory) = app_storage("journal");
        let file = directory.join("dirty.lv");
        std::fs::write(&file, b"a = 1\n").expect("fixture");

        let mut app = WorkbenchApp::launch(storage.clone(), Some(file.clone()));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        let index = app.workbench.active();
        assert_eq!(
            app.workbench.active_document().path.as_deref(),
            Some(file.as_path())
        );
        app.workbench.set_source(index, "a = 2\n".to_string());
        settle(&ctx, &mut app);

        let document_id = app.workbench.documents()[index].id.0;
        let record_path = recovery::record_path(
            storage.recovery_root.as_ref().expect("recovery root"),
            document_id,
        );
        assert!(
            record_path.exists(),
            "the dirty buffer is journaled within the debounce window"
        );
        let record = recovery::read_record(&record_path).expect("the record parses");
        assert_eq!(record.source, "a = 2\n", "the record is the buffer");
        assert_eq!(record.path, Some(file.clone()));
        assert_eq!(record.source.as_bytes(), b"a = 2\n");

        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert_eq!(std::fs::read(&file).expect("the saved file"), b"a = 2\n");
        assert!(
            !record_path.exists(),
            "a saved document's recovery record is deleted"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    // ======================================================================================
    // A11Y-001 / A11Y-003 / A11Y-007 / A11Y-014: accessibility acceptance.
    // ======================================================================================

    /// A scripted evaluator for the accessibility tests.
    ///
    /// It returns one fixed symbolic value for every statement, plus the structured projection,
    /// the inspector payload, a variable, one print line and a plot capture whose path is a
    /// scratch directory. The keyboard journey therefore asserts the *window's* keyboard path
    /// (painting, focus, export) without touching the user profile and without depending on
    /// engine arithmetic, which its own tests cover.
    struct A11yEngine {
        plot_dir: PathBuf,
    }

    impl A11yEngine {
        fn new(plot_dir: PathBuf) -> A11yEngine {
            let _ = std::fs::create_dir_all(&plot_dir);
            A11yEngine { plot_dir }
        }

        fn structured() -> lovelace_json::Json {
            use lovelace_json::Json;
            Json::obj([
                ("kind", Json::Str("Symbolic".to_string())),
                ("pretty", Json::Str("sqrt(2)*pi".to_string())),
                ("canonical", Json::Str("(mul (pow 2 1/2) pi)".to_string())),
                ("freeSymbols", Json::str_array(["x"])),
                ("nodeCount", Json::Num("3".to_string())),
            ])
        }

        fn inspection() -> lovelace_json::Json {
            use lovelace_json::Json;
            Json::obj([
                (
                    "subject",
                    Json::obj([
                        ("kind", Json::Str("Symbolic".to_string())),
                        ("path", Json::Str("result".to_string())),
                    ]),
                ),
                (
                    "sections",
                    Json::obj([
                        (
                            "pretty",
                            Json::obj([("text", Json::Str("sqrt(2)*pi".to_string()))]),
                        ),
                        (
                            "tree",
                            Json::obj([(
                                "root",
                                Json::obj([
                                    ("kind", Json::Str("Mul".to_string())),
                                    ("label", Json::Str("sqrt(2)*pi".to_string())),
                                    ("children", Json::Arr(Vec::new())),
                                ]),
                            )]),
                        ),
                    ]),
                ),
            ])
        }
    }

    impl Evaluator for A11yEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            Ok("sqrt(2)*pi".to_string())
        }

        fn last_structured(&self) -> Option<lovelace_json::Json> {
            Some(A11yEngine::structured())
        }

        fn last_inspection(&self) -> Option<lovelace_json::Json> {
            Some(A11yEngine::inspection())
        }

        fn variables(&self) -> Vec<(String, String)> {
            vec![("x".to_string(), "sqrt(2)*pi".to_string())]
        }

        fn state(&self) -> Option<EngineState> {
            Some(EngineState {
                variables: vec![crate::engine::EngineVariable {
                    name: "x".to_string(),
                    kind: "Symbolic".to_string(),
                    display: "sqrt(2)*pi".to_string(),
                    structured: Some(A11yEngine::structured()),
                }],
                functions: Vec::new(),
            })
        }

        fn take_output(&mut self) -> Vec<String> {
            vec!["printed sqrt(2)".to_string()]
        }

        fn last_plot(&self) -> Option<crate::engine::PlotCapture> {
            Some(crate::engine::PlotCapture {
                path: self.plot_dir.join("plot-1.svg").display().to_string(),
                title: "a11y journey plot".to_string(),
                svg: crate::svg::make_plot_svg("a11y journey plot", &[1, 3, 2]),
            })
        }
    }

    /// A scratch directory under the process temp directory for one accessibility test.
    fn scratch_dir(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "lovelace-desktop-a11y-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch directory");
        directory
    }

    /// A window over the scripted evaluator, with no persisted settings or recovery journal.
    fn a11y_app(plot_dir: PathBuf) -> WorkbenchApp {
        let workbench = Workbench::with_factory(Box::new(move || {
            Box::new(A11yEngine::new(plot_dir.clone()))
        }));
        WorkbenchApp::with_storage(workbench, AppStorage::default())
    }

    fn a11y_frame(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        frame_capturing(ctx, app, vec2(1600.0, 1000.0), events, Default::default())
    }

    /// Pumps `frames` passes and returns the last pass's painted text.
    fn pump_text(ctx: &egui::Context, app: &mut WorkbenchApp, frames: usize) -> Vec<String> {
        let mut texts = Vec::new();
        for _ in 0..frames.max(1) {
            let output = a11y_frame(ctx, app, Vec::new());
            texts.clear();
            for clipped in &output.shapes {
                collect_shape_text(&clipped.shape, &mut texts);
            }
        }
        texts
    }

    /// The egui key for one table key name.
    fn egui_key_of(name: KeyName) -> Key {
        match name {
            KeyName::Enter => Key::Enter,
            KeyName::Escape => Key::Escape,
            KeyName::Tab => Key::Tab,
            KeyName::Space => Key::Space,
            KeyName::Comma => Key::Comma,
            KeyName::F1 => Key::F1,
            KeyName::F6 => Key::F6,
            KeyName::F8 => Key::F8,
            KeyName::Plus => Key::Equals,
            KeyName::Minus => Key::Minus,
            KeyName::Digit(0) => Key::Num0,
            KeyName::Digit(1) => Key::Num1,
            KeyName::Digit(2) => Key::Num2,
            KeyName::Digit(3) => Key::Num3,
            KeyName::Digit(4) => Key::Num4,
            KeyName::Digit(5) => Key::Num5,
            KeyName::Digit(other) => panic!("no egui digit key for {other}"),
            KeyName::Char('n') => Key::N,
            KeyName::Char('q') => Key::Q,
            KeyName::Char('u') => Key::U,
            KeyName::Char('p') => Key::P,
            KeyName::Char('c') => Key::C,
            KeyName::Char('f') => Key::F,
            KeyName::Char('t') => Key::T,
            KeyName::Char('z') => Key::Z,
            KeyName::Char(other) => panic!("no egui key for {other:?}"),
        }
    }

    /// Sends one chord, then paints the pass in which its focus request takes effect.
    fn press_chord(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        chord: keyboard::Shortcut,
    ) -> Vec<String> {
        let modifiers = egui::Modifiers {
            ctrl: chord.ctrl,
            shift: chord.shift,
            alt: chord.alt,
            ..Default::default()
        };
        let _ = a11y_frame(ctx, app, vec![key(egui_key_of(chord.key), modifiers)]);
        // Two more passes: the pass that reads the chord and the pass that lays out whatever it
        // opened (a dialog measures its content on the first pass that paints it).
        pump_text(ctx, app, 2)
    }

    /// Sends the chord registered for `action` through the table.
    fn press_action(ctx: &egui::Context, app: &mut WorkbenchApp, action: Action) -> Vec<String> {
        let binding = keyboard::binding_record(action).expect("the action is registered");
        press_chord(ctx, app, binding.keys)
    }

    /// Runs the document through Ctrl+Enter and waits for the terminal state, painting frames
    /// exactly as the window does.
    fn run_from_keyboard(ctx: &egui::Context, app: &mut WorkbenchApp) -> Vec<String> {
        let mut texts = press_action(
            ctx,
            app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        for _ in 0..600 {
            if matches!(
                app.view.run_state,
                RunState::Finished | RunState::Failed | RunState::Cancelled
            ) {
                break;
            }
            texts = pump_text(ctx, app, 1);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            matches!(app.view.run_state, RunState::Finished),
            "the run did not finish: {:?}",
            app.view.run_state
        );
        texts
    }

    /// Collects the stroke colours of every rectangle shape in a pass.
    fn rect_stroke_colors(shape: &egui::Shape, out: &mut Vec<(Color32, f32)>) {
        match shape {
            egui::Shape::Rect(rect) => {
                if rect.stroke.width > 0.0 {
                    out.push((rect.stroke.color, rect.stroke.width));
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    rect_stroke_colors(shape, out);
                }
            }
            _ => {}
        }
    }

    /// A11Y-003: the ring is painted in the active theme's `focus_ring` token — in both themes.
    ///
    /// The assertion reads the painted `RectShape` stroke, so it proves what the window drew
    /// rather than what a constant says it would draw.
    #[test]
    fn the_focus_ring_is_painted_in_the_theme_token_in_both_themes() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("focus");
        let mut app = a11y_app(plot_dir.clone());
        // The editor owns focus from the first pass, so there is always a focused control.
        let mut output = a11y_frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "the editor owns the keyboard at launch"
        );
        let mut strokes = Vec::new();
        for clipped in &output.shapes {
            rect_stroke_colors(&clipped.shape, &mut strokes);
        }
        let ring = theme::tokens_for(theme::ThemeId::Dark).focus_ring;
        assert!(
            strokes.iter().any(|(color, width)| *color == ring
                && (*width - a11y::FOCUS_RING_WIDTH).abs() < f32::EPSILON),
            "the dark frame must paint the focus ring in the dark token: {strokes:?}"
        );

        // Switch to the light theme through the window's own control and paint again.
        app.set_theme(theme::ThemeId::Light, &ctx);
        output = a11y_frame(&ctx, &mut app, Vec::new());
        let mut strokes = Vec::new();
        for clipped in &output.shapes {
            rect_stroke_colors(&clipped.shape, &mut strokes);
        }
        let light_ring = theme::tokens_for(theme::ThemeId::Light).focus_ring;
        assert_ne!(ring, light_ring);
        assert!(
            strokes.iter().any(|(color, width)| *color == light_ring
                && (*width - a11y::FOCUS_RING_WIDTH).abs() < f32::EPSILON),
            "the light frame must paint the focus ring in the light token: {strokes:?}"
        );
        assert!(
            !strokes.iter().any(|(color, _)| *color == ring),
            "the dark ring colour must not survive the theme switch"
        );
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-003: the ring follows keyboard focus from pane to pane, so a keyboard user can see
    /// where they are. The ring's rectangle is the focused widget's rectangle.
    #[test]
    fn the_focus_ring_moves_with_the_keyboard_focus() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("ring-follow");
        let mut app = a11y_app(plot_dir.clone());
        a11y_frame(&ctx, &mut app, Vec::new());
        // F6 from the editor: the plot pane's Fit button takes focus.
        let _ = a11y_frame(&ctx, &mut app, vec![key(Key::F6, Default::default())]);
        let output = a11y_frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.focused_pane, Pane::Plot);
        let focus = ctx
            .memory(|memory| memory.focused())
            .expect("a focused control");
        assert_eq!(
            Some(focus),
            app.plot_fit_id,
            "the Fit button is the plot group's first control"
        );
        let ring_rect = ctx
            .read_response(focus)
            .map(|response| response.rect)
            .expect("the focused widget was painted");
        let mut strokes = Vec::new();
        for clipped in &output.shapes {
            if clipped.clip_rect.intersects(ring_rect.expand(a11y::FOCUS_RING_GAP + 4.0)) {
                rect_stroke_colors(&clipped.shape, &mut strokes);
            }
        }
        let ring = theme::tokens_for(theme::ThemeId::Dark).focus_ring;
        assert!(
            !strokes.is_empty(),
            "the focused widget's area must carry painted strokes"
        );
        assert!(
            strokes.iter().any(|(color, _)| *color == ring),
            "the ring must be painted around the focused widget: {strokes:?}"
        );
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-007: the Help > Keyboard Shortcuts dialog lists every registered binding, by chord
    /// and description, and Escape closes it.
    #[test]
    fn the_shortcut_reference_lists_every_registered_binding_and_closes_with_escape() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("shortcuts");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        assert!(!app.shortcuts_open);

        // F1 is the table's chord for the dialog; it is pressed as a key, not by calling the
        // opener, so the binding itself is what is under test.
        let binding = keyboard::binding_record(Action::ShortcutsHelp).expect("F1 is registered");
        let texts = press_chord(&ctx, &mut app, binding.keys);
        assert!(app.shortcuts_open, "F1 must open the reference");
        assert!(
            texts.iter().any(|text| text == "Keyboard shortcuts"),
            "the window title is the surface's name: {texts:?}"
        );
        let rows = keyboard::reference_rows();
        assert_eq!(rows.len(), keyboard::BINDINGS.len());
        let mut missing = Vec::new();
        for row in &rows {
            if !texts.iter().any(|text| text == &row.chord) {
                missing.push(format!("chord {}", row.chord));
            }
            if !texts.iter().any(|text| text == row.description) {
                missing.push(format!("description for {}", row.chord));
            }
        }
        assert!(
            missing.is_empty(),
            "the reference is missing rows from the table: {missing:?}\npainted: {texts:?}"
        );
        // Global and editor sections are both present.
        for scope in keyboard::Scope::ALL {
            assert!(
                texts.iter().any(|text| text == &scope.label().to_uppercase()),
                "the reference is missing the {} section: {texts:?}",
                scope.label()
            );
        }
        // The dialog lists no chord that is not registered (no hand-written extra). A chord is the
        // only painted text with no spaces that contains a modifier or is "Esc".
        let registered: Vec<String> = rows.iter().map(|row| row.chord.clone()).collect();
        for text in texts.iter().filter(|text| {
            !text.contains(' ')
                && (text.contains('+') || text.as_str() == "Esc")
                // Section headings and the window title contain no '+' and are not "Esc".
                && text.chars().next().is_some_and(|first| first.is_ascii_alphanumeric())
        }) {
            assert!(
                registered.contains(text),
                "the reference shows an unregistered chord {text:?}"
            );
        }

        let _ = press_chord(&ctx, &mut app, keyboard::Shortcut::plain(KeyName::Escape));
        assert!(!app.shortcuts_open, "Escape closes the reference");
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-001: F6 / Shift+F6 and Ctrl+1..5 reach every pane group without a pointer.
    #[test]
    fn f6_and_ctrl_digits_move_keyboard_focus_between_every_pane_group() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("panes");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        assert_eq!(app.focused_pane, Pane::Editor);

        // F6 walks the groups in the order the status strip prints, wrapping at the end.
        let mut seen = vec![app.focused_pane];
        for _ in 0..Pane::ALL.len() {
            let texts = press_action(&ctx, &mut app, Action::NextPane);
            seen.push(app.focused_pane);
            assert!(
                texts
                    .iter()
                    .any(|text| text == &format!("focus: {}", app.focused_pane.label())),
                "the status strip must name the focus group: {texts:?}"
            );
        }
        assert_eq!(
            seen,
            vec![
                Pane::Editor,
                Pane::Plot,
                Pane::Workspace,
                Pane::Logs,
                Pane::Toolbar,
                Pane::Editor
            ],
            "F6 visits every group and wraps"
        );

        // Ctrl+1..5 jump directly; Shift+F6 walks backwards.
        for (action, expected) in [
            (Action::FocusPane(Pane::Toolbar), Pane::Toolbar),
            (Action::FocusPane(Pane::Editor), Pane::Editor),
            (Action::FocusPane(Pane::Plot), Pane::Plot),
            (Action::FocusPane(Pane::Workspace), Pane::Workspace),
            (Action::FocusPane(Pane::Logs), Pane::Logs),
        ] {
            press_action(&ctx, &mut app, action);
            assert_eq!(app.focused_pane, expected, "{}", action.label());
        }
        press_action(&ctx, &mut app, Action::PreviousPane);
        assert_eq!(app.focused_pane, Pane::Workspace, "Shift+F6 goes back one");
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-001: the remaining window commands that used to be pointer-only are keyboard
    /// commands, dispatched through the same table.
    #[test]
    fn the_theme_and_view_commands_are_keyboard_reachable() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("commands");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        assert_eq!(app.theme, theme::ThemeId::Dark);

        // Ctrl+Shift+T: the theme toggle, the one VIS-002 control that had no key.
        press_action(&ctx, &mut app, Action::ToggleTheme);
        assert_eq!(app.theme, theme::ThemeId::Light, "the theme switch is a binding");
        press_action(&ctx, &mut app, Action::ToggleTheme);
        assert_eq!(app.theme, theme::ThemeId::Dark);

        // Ctrl+Alt+F: the workspace filter takes focus; Ctrl+Alt+C: the variables are cleared.
        let _ = press_action(&ctx, &mut app, Action::FocusPane(Pane::Workspace));
        let texts = press_action(&ctx, &mut app, Action::WorkspaceFilterFocus);
        assert!(
            ctx.memory(|memory| memory.focused()) == app.workspace_filter_id,
            "Ctrl+Alt+F focuses the filter: {texts:?}"
        );
        let _ = std::fs::remove_dir_all(&plot_dir);
    }
    /// A11Y-001: from a pane group's first control, Tab walks the group's other controls (and
    /// Enter/Space activates the focused button), so a button without a dedicated binding is
    /// still reachable without a pointer.
    #[test]
    fn tab_walks_the_focused_group_and_shift_tab_returns() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("tab");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        press_action(&ctx, &mut app, Action::FocusPane(Pane::Toolbar));
        assert_eq!(ctx.memory(|memory| memory.focused()), app.run_button_id);

        // Tab moves to another control painted in the window's chrome (file bar / toolbar rows).
        let _ = a11y_frame(&ctx, &mut app, vec![key(Key::Tab, Default::default())]);
        pump_text(&ctx, &mut app, 1);
        let after = ctx
            .memory(|memory| memory.focused())
            .expect("Tab must focus a control");
        assert_ne!(
            Some(after),
            app.run_button_id,
            "Tab leaves the Run button for the next control"
        );
        let rect = ctx
            .read_response(after)
            .map(|response| response.rect)
            .expect("the newly focused control is painted");
        assert!(
            rect.top() < 120.0,
            "the next control is in the window chrome, above the panes: {rect:?}"
        );

        // Shift+Tab walks back (the exact widget is egui's focus order, which the window keeps
        // as its registration order; the observable is that focus moves again and stays in the
        // chrome).
        let _ = a11y_frame(
            &ctx,
            &mut app,
            vec![key(
                Key::Tab,
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            )],
        );
        pump_text(&ctx, &mut app, 1);
        let back = ctx
            .memory(|memory| memory.focused())
            .expect("Shift+Tab must focus a control");
        assert_ne!(back, after, "Shift+Tab moves focus back");
        let rect = ctx
            .read_response(back)
            .map(|response| response.rect)
            .expect("the control is painted");
        assert!(rect.top() < 120.0, "focus stays in the chrome: {rect:?}");
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-001: the whole keyboard journey — new tab, edit, run, inspect, switch panes, export —
    /// with an observable asserted after every step.
    ///
    /// The transcript it prints is the one recorded in
    /// `docs/goal-desktop-v4/round-42/accessibility.md`.
    #[test]
    fn the_keyboard_journey_opens_edits_runs_inspects_switches_panes_and_exports() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("journey");
        let mut app = a11y_app(plot_dir.clone());
        let mut transcript: Vec<String> = Vec::new();
        macro_rules! step {
            ($label:expr, $detail:expr) => {{
                let line = format!("{}: {}", $label, $detail);
                println!("{line}");
                transcript.push(line);
            }};
        }

        // 1. Launch: the editor owns the keyboard with no pointer input at all.
        pump_text(&ctx, &mut app, 2);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(surface_id()));
        let status = app.workbench.status_text(1, 1);
        step!("launch", format!("focus=editor status={status:?}"));

        // 2. Ctrl+N: a fresh tab.
        let documents_before = app.workbench.documents().len();
        press_action(&ctx, &mut app, Action::File(FileCommand::New));
        assert_eq!(app.workbench.documents().len(), documents_before + 1);
        assert!(
            app.workbench.active_document().title.starts_with("untitled"),
            "a fresh tab is untitled: {:?}",
            app.workbench.active_document().title
        );
        step!(
            "ctrl+n",
            format!(
                "documents={} active={:?}",
                app.workbench.documents().len(),
                app.workbench.active_document().title
            )
        );

        // 3. Type the script with text events only.
        let source = "sqrt(2)*pi";
        let _ = a11y_frame(&ctx, &mut app, vec![egui::Event::Text(source.to_string())]);
        pump_text(&ctx, &mut app, 1);
        assert_eq!(app.editors[app.workbench.active()].source(), source);
        step!("type", format!("buffer={source:?}"));

        // 4. Ctrl+Enter: run, and wait for the terminal state.
        let texts = run_from_keyboard(&ctx, &mut app);
        assert!(
            texts.iter().any(|text| text.contains("Finished")),
            "the status strip must report the finished run: {texts:?}"
        );
        step!("ctrl+enter", format!("run_state={:?}", app.view.run_state));

        // 5. Ctrl+4: the workspace pane, then Ctrl+Alt+2: the Inspector view.
        press_action(&ctx, &mut app, Action::FocusPane(Pane::Workspace));
        assert_eq!(app.focused_pane, Pane::Workspace);
        let texts = press_action(&ctx, &mut app, Action::WorkspaceInspectorView);
        assert!(app.inspector_open, "Ctrl+Alt+2 opens the Inspector view");
        assert!(
            texts
                .iter()
                .any(|text| text == "INSPECTOR · last committed result"),
            "the inspector surface must be painted: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "sqrt(2)*pi"),
            "the inspector shows the committed value: {texts:?}"
        );
        step!("inspect", "inspector=open value=sqrt(2)*pi".to_string());

        // 6. Ctrl+Alt+1: back to the structured Result view; the tree and the variable table are
        //    both observable.
        let texts = press_action(&ctx, &mut app, Action::WorkspaceResultView);
        assert!(!app.inspector_open);
        assert!(
            texts.iter().any(|text| text == "RESULT · structured projection"),
            "the result surface must be painted: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "x"),
            "the workspace variable table must paint the variable: {texts:?}"
        );
        step!("result-view", "result=open variable=x".to_string());

        // 7. Ctrl+5: the logs pane; the value-bearing result line is painted.
        let texts = press_action(&ctx, &mut app, Action::FocusPane(Pane::Logs));
        assert_eq!(app.focused_pane, Pane::Logs);
        assert!(
            texts.iter().any(|text| text == "[result] = sqrt(2)*pi"),
            "the logs pane must paint the result line: {texts:?}"
        );
        step!("logs", "result_line=[result] = sqrt(2)*pi".to_string());

        // 8. Ctrl+3: the plot pane; the engine plot is loaded, zoom and fit work from keys.
        let texts = press_action(&ctx, &mut app, Action::FocusPane(Pane::Plot));
        assert_eq!(app.focused_pane, Pane::Plot);
        assert!(
            texts.iter().any(|text| text.contains("a11y journey plot")),
            "the plot pane must name the engine plot: {texts:?}"
        );
        press_action(&ctx, &mut app, Action::PlotZoomIn);
        assert!(
            app.plot.view.zoom > 1.0,
            "Ctrl+= zooms the plot in: {}",
            app.plot.view.zoom
        );
        press_action(&ctx, &mut app, Action::PlotFit);
        assert!(
            (app.plot.view.zoom - 1.0).abs() < f32::EPSILON,
            "Ctrl+0 fits"
        );
        step!(
            "plot",
            format!("plot={:?} zoom={:.2}", app.plot_title(), app.plot.view.zoom)
        );

        // 9. Ctrl+Shift+P: export the plot beside the engine capture, no dialog.
        let texts = press_action(&ctx, &mut app, Action::PlotExportPng);
        let exported: Vec<PathBuf> = std::fs::read_dir(&plot_dir)
            .expect("the plot directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
            .collect();
        assert_eq!(exported.len(), 1, "one PNG is written: {exported:?}");
        let bytes = std::fs::metadata(&exported[0])
            .expect("the PNG exists")
            .len();
        assert!(bytes > 1000, "the PNG has content: {bytes} bytes");
        assert!(
            texts.iter().any(|text| text.starts_with("PNG:")),
            "the plot header reports the export: {texts:?}"
        );
        step!(
            "export",
            format!(
                "png={} bytes={bytes}",
                exported[0]
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        );

        // 10. Ctrl+1: the toolbar; Enter activates the focused Run button.
        press_action(&ctx, &mut app, Action::FocusPane(Pane::Toolbar));
        assert_eq!(app.focused_pane, Pane::Toolbar);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            app.run_button_id,
            "the Run button owns the keyboard"
        );
        let _ = a11y_frame(&ctx, &mut app, vec![key(Key::Enter, Default::default())]);
        assert!(
            app.workbench.is_running() || app.view.run_state == RunState::Queued,
            "Enter on the focused Run button starts a run: {:?}",
            app.view.run_state
        );
        step!(
            "toolbar-enter",
            format!(
                "focused=run run_state={:?}",
                app.view.run_state
            )
        );

        // 11. Ctrl+Z: the editor's own history key, reached without the pointer.
        press_action(&ctx, &mut app, Action::FocusPane(Pane::Editor));
        let _ = press_chord(
            &ctx,
            &mut app,
            keyboard::Shortcut::new(KeyName::Char('z'), true, false, false),
        );
        assert_eq!(app.editors[app.workbench.active()].source(), "");
        step!("undo", "buffer=\"\"".to_string());

        // 12. Type once more (so the tab is dirty), then Ctrl+Q: the Save/Discard/Cancel prompt
        //     opens, and Escape cancels it without losing the buffer.
        let _ = a11y_frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("unsaved".to_string())],
        );
        pump_text(&ctx, &mut app, 1);
        assert_eq!(
            app.editors[app.workbench.active()].source(),
            "unsaved",
            "the buffer is dirty before the quit request"
        );
        let texts = press_action(&ctx, &mut app, Action::Quit);
        assert!(
            app.close.is_some(),
            "Ctrl+Q opens the Save/Discard/Cancel prompt"
        );
        assert!(
            texts.iter().any(|text| text == "Save"),
            "the prompt offers Save: {texts:?}"
        );
        let _ = press_chord(&ctx, &mut app, keyboard::Shortcut::plain(KeyName::Escape));
        assert!(app.close.is_none(), "Escape cancels the close");
        assert_eq!(
            app.editors[app.workbench.active()].source(),
            "unsaved",
            "cancelling the close keeps the buffer"
        );
        step!("quit", "prompt=cancelled buffer=unsaved".to_string());

        assert!(
            transcript.len() >= 12,
            "the journey records every step: {transcript:?}"
        );
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-014: Ctrl+Shift+U switches the notation and the same committed value is re-rendered
    /// on all four surfaces without a re-run.
    #[test]
    fn the_unicode_setting_updates_the_result_workspace_logs_and_inspector_without_a_rerun() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("unicode");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        run_from_keyboard(&ctx, &mut app);

        // The shipped font decides what the Unicode form actually paints: a glyph the font lacks
        // is replaced by its documented fallback, which is the point of the fallback rule.
        let expected = glyphs::render("sqrt(2)*pi", GlyphMode::Unicode, &mut |c| {
            a11y::font_has_glyph(&ctx, c)
        });
        assert_ne!(
            expected, "sqrt(2)*pi",
            "Unicode mode must change the notation: {expected:?}"
        );

        let ascii_texts = pump_text(&ctx, &mut app, 1);
        assert!(ascii_texts
            .iter()
            .any(|text| text == "[result] = sqrt(2)*pi"));
        assert!(
            !ascii_texts.iter().any(|text| text.contains(&expected)),
            "ASCII is the default mode: {ascii_texts:?}"
        );

        let texts = press_action(&ctx, &mut app, Action::ToggleUnicodeGlyphs);
        assert_eq!(app.glyph_mode, GlyphMode::Unicode);
        assert!(
            texts
                .iter()
                .any(|text| text == "[info] Mathematical glyphs: unicode notation"),
            "the switch is logged: {texts:?}"
        );
        // Structured result caption.
        assert!(
            texts
                .iter()
                .any(|text| text == &format!("display: {expected}")),
            "the result caption must use the same notation: {texts:?}"
        );
        // Workspace variable display cell.
        assert!(
            texts.iter().any(|text| text == &expected),
            "the workspace cell must use the same notation: {texts:?}"
        );
        // Logs result line.
        assert!(
            texts
                .iter()
                .any(|text| text == &format!("[result] = {expected}")),
            "the logs result line must use the same notation: {texts:?}"
        );
        // Inspector (switched open from the keyboard) paints the same value.
        let texts = press_action(&ctx, &mut app, Action::WorkspaceInspectorView);
        assert!(
            texts
                .iter()
                .any(|text| text.ends_with(&format!("· {expected}")) || text == &expected),
            "the inspector must use the same notation: {texts:?}"
        );

        // The switch is persisted: the settings file carries the Unicode choice.
        let persisted = app.settings.to_json();
        assert!(
            persisted.contains("\"unicode_glyphs\": true"),
            "the setting survives a restart: {persisted}"
        );

        // And back: the same value returns to the engine's ASCII notation.
        let texts = press_action(&ctx, &mut app, Action::ToggleUnicodeGlyphs);
        assert_eq!(app.glyph_mode, GlyphMode::Ascii);
        assert!(texts.iter().any(|text| text == "[result] = sqrt(2)*pi"));
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// A11Y-014: the no-tofu check for the glyph set the *engine* emits.
    ///
    /// A real engine evaluates a symbolic expression, the named constants, a relation and a plot
    /// with a Unicode title; every non-ASCII character in those display strings and in the plot's
    /// SVG is measured against the shipped font stack. Each must be drawable or have a documented
    /// fallback, and the rendered string must not contain a character that neither can draw.
    #[test]
    #[cfg(feature = "real-engine")]
    fn the_engine_glyph_set_is_renderable_or_mapped_in_the_shipped_font() {
        let mut engine = lovelace_lang::Engine::new();
        let plot_dir = scratch_dir("glyphs");
        engine.set_plot_dir(&plot_dir.display().to_string());
        let mut sample = String::new();
        for script in [
            "x = symbol(\"x\")",
            "3*x^2",
            "inf",
            "x <= 5",
            "x != 0",
            "sqrt(2)",
            "pi(20)",
            "plot([1, 4, 2], [4, 5, 6], \"\u{221a} \u{03c0} \u{2264} glyph check\")",
        ] {
            if let Ok(value) = engine.eval(script) {
                sample.push_str(&engine.format(&value));
                sample.push('\n');
            }
        }
        if let Some(plot) = engine.last_plot() {
            sample.push_str(&plot.svg);
        }
        // The window's Unicode additions plus one astral mathematical character.
        sample.push_str(glyphs::SAMPLE_GLYPHS);
        sample.push('\u{1D70B}');

        let unicode = glyphs::to_unicode(&sample);
        assert!(
            unicode.contains('\u{00d7}') && unicode.contains('\u{2264}'),
            "the sample must exercise × and ≤: {unicode:?}"
        );
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
        let report = a11y::glyph_report(&unicode, &mut |c| a11y::font_has_glyph(&ctx, c));
        assert!(!report.is_empty(), "the engine sample must emit glyphs");
        for (glyph, drawable, fallback) in &report {
            assert!(
                *drawable || fallback.is_some(),
                "U+{:04X} {glyph:?} is neither drawable nor mapped",
                *glyph as u32
            );
        }
        // No tofu: every character of the rendered form is drawable (fallbacks already applied).
        let rendered = glyphs::render(&unicode, GlyphMode::Unicode, &mut |c| {
            a11y::font_has_glyph(&ctx, c)
        });
        for glyph in glyphs::non_ascii(&rendered) {
            assert!(
                a11y::font_has_glyph(&ctx, glyph),
                "the rendered string still asks the font for U+{:04X}, which it cannot draw",
                glyph as u32
            );
        }
        assert!(!rendered.contains('\u{FFFD}'), "no replacement tofu");
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    // ---------------------------------------------------------------------------------------
    // A11Y-002 / A11Y-004 / A11Y-005 / A11Y-006 / A11Y-012 structural accessibility.
    // ---------------------------------------------------------------------------------------

    /// A deterministic engine that publishes both workspace tables and a plot, so the
    /// accessibility audit sees every surface without depending on the real engine's catalogue.
    struct StructuralEngine;

    impl Evaluator for StructuralEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            Ok("sqrt(2)*pi".to_string())
        }

        fn variables(&self) -> Vec<(String, String)> {
            vec![
                ("expr".to_string(), "3*x^2".to_string()),
                ("x".to_string(), "symbol(\"x\")".to_string()),
            ]
        }

        fn functions(&self) -> Vec<(String, Vec<String>)> {
            vec![
                ("my_func".to_string(), vec!["a".to_string()]),
                ("sqrt".to_string(), vec!["x".to_string()]),
            ]
        }

        fn state(&self) -> Option<EngineState> {
            Some(EngineState {
                variables: vec![
                    crate::engine::EngineVariable {
                        name: "expr".to_string(),
                        kind: "Symbolic".to_string(),
                        display: "3*x^2".to_string(),
                        structured: None,
                    },
                    crate::engine::EngineVariable {
                        name: "x".to_string(),
                        kind: "Symbolic".to_string(),
                        display: "symbol(\"x\")".to_string(),
                        structured: None,
                    },
                ],
                functions: vec![
                    EngineFunction {
                        name: "my_func".to_string(),
                        parameters: vec!["a".to_string()],
                        is_builtin: false,
                        plugin: None,
                        summary: Some("A user function.".to_string()),
                    },
                    EngineFunction {
                        name: "sqrt".to_string(),
                        parameters: vec!["x".to_string()],
                        is_builtin: true,
                        plugin: None,
                        summary: Some("Principal square root.".to_string()),
                    },
                ],
            })
        }

        fn last_plot(&self) -> Option<crate::engine::PlotCapture> {
            Some(crate::engine::PlotCapture {
                path: "round-43-a11y.svg".to_string(),
                title: "round 43 a11y plot".to_string(),
                svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"600\">\
                      <rect width=\"800\" height=\"600\" fill=\"#ffffff\"/>\
                      <text x=\"20\" y=\"40\" fill=\"#000000\">round 43 a11y plot</text></svg>"
                    .to_string(),
            })
        }
    }

    /// An engine that fails its second statement, so the frame carries a real diagnostic with a
    /// line and a column (the A11Y-012 error surfaces).
    struct FailingA11yEngine;

    impl Evaluator for FailingA11yEngine {
        fn eval(&mut self, src: &str) -> Result<String, String> {
            if src.contains("sqrt") {
                Err("unknown function 'sqrt'".to_string())
            } else {
                Ok("ok".to_string())
            }
        }

        fn last_error(&self) -> Option<crate::engine::EngineError> {
            Some(crate::engine::EngineError {
                code: "UnknownFunction".to_string(),
                category: "UnsupportedOperation".to_string(),
                suggestions: vec!["sqrt".to_string()],
                hints: Vec::new(),
            })
        }
    }

    /// The document the failing engine decorates: line 2 is the failed statement
    /// (`y = sqrt(x)`, brackets at columns 7 and 9) and line 3 carries a `->` pair for the
    /// ligature rendering. A 1-based caret column 9 is inside the pair.
    const STRUCTURAL_SOURCE: &str = "x = symbol(\"x\")\ny = sqrt(x)\nz = f(1)->2\n";

    fn structural_app() -> WorkbenchApp {
        let mut workbench = Workbench::with_factory(Box::new(|| Box::new(StructuralEngine)));
        workbench.set_source(0, STRUCTURAL_SOURCE.to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.settings.theme_follows_system = true;
        app
    }

    fn failing_structural_app() -> WorkbenchApp {
        let mut workbench = Workbench::with_factory(Box::new(|| Box::new(FailingA11yEngine)));
        workbench.set_source(0, STRUCTURAL_SOURCE.to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.settings.theme_follows_system = true;
        app
    }

    /// Paints frames until the run reaches a terminal state.
    fn pump_until_terminal(ctx: &egui::Context, app: &mut WorkbenchApp) {
        for _ in 0..600 {
            if matches!(
                app.view.run_state,
                RunState::Finished | RunState::Failed | RunState::Cancelled
            ) {
                return;
            }
            let _ = pump_text(ctx, app, 1);
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("the run did not reach a terminal state: {:?}", app.view.run_state);
    }

    /// Runs the active document and returns the accessibility rows of the frame that follows.
    fn accessibility_rows(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
    ) -> Vec<a11y::NodeRow> {
        ctx.enable_accesskit();
        let output = frame_capturing(ctx, app, screen, Vec::new(), Default::default());
        let update = output
            .platform_output
            .accesskit_update
            .expect("the window must publish an accessibility tree");
        a11y::accesskit_rows(&update)
    }

    /// A11Y-002: every control answers with a role, a name, a value and a state; the two tables
    /// expose `Table`/`Row`/`ColumnHeader` nodes instead of bare text rows, the tabs are `Tab`s
    /// with a selection state, the editor carries its text as its value and the status strip and
    /// progress are their own roles.
    #[test]
    fn a11y_002_every_control_exposes_role_name_value_and_state() {
        use egui::Role;
        let mut app = structural_app();
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let _ = press_action(
            &ctx,
            &mut app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&ctx, &mut app);
        let rows = accessibility_rows(&ctx, &mut app, vec2(1600.0, 1000.0));
        let dump = a11y::dump_table(&rows);
        assert!(
            rows.len() > 60,
            "the audit must see the whole tree, not a fragment: {} rows\n{dump}",
            rows.len()
        );

        // No interactive node may be unlabelled or have the wrong (generic) role.
        for row in rows.iter().filter(|row| row.interactive()) {
            assert!(
                !row.name.is_empty(),
                "an interactive {:?} node has no name at {:?}\n{dump}",
                row.role,
                row.bounds
            );
            assert_ne!(
                row.role,
                Role::GenericContainer,
                "an interactive node must not be a generic container: {:?}\n{dump}",
                row.role
            );
        }

        // Both workspace tables, each with a row count and named rows.
        let tables: Vec<&a11y::NodeRow> = rows.iter().filter(|row| row.role == Role::Table).collect();
        assert_eq!(tables.len(), 2, "variables and functions\n{dump}");
        for table in &tables {
            assert!(
                table.name.contains("table"),
                "a table must name itself: {}\n{dump}",
                table.name
            );
            let (row_count, column_count) = table
                .table_size
                .unwrap_or_else(|| panic!("table {} has no size\n{dump}", table.name));
            assert!(
                row_count > 0 && column_count > 0,
                "table {} reports {row_count} rows x {column_count} columns",
                table.name
            );
        }
        let data_rows: Vec<&a11y::NodeRow> = rows.iter().filter(|row| row.role == Role::Row).collect();
        assert!(
            data_rows.len() >= 4,
            "both tables' data rows must be Row nodes, got {}\n{dump}",
            data_rows.len()
        );
        for row in &data_rows {
            assert!(
                row.name.matches(" | ").count() >= 2 && !row.name.is_empty(),
                "a table row's name must carry its cells: {:?}",
                row.name
            );
        }
        let headers: Vec<&a11y::NodeRow> = rows
            .iter()
            .filter(|row| row.role == Role::ColumnHeader)
            .collect();
        assert!(
            headers.len() >= 6,
            "both tables' sort headers must be ColumnHeader nodes, got {}\n{dump}",
            headers.len()
        );
        for header in &headers {
            assert!(!header.name.is_empty(), "a column header has no name");
            assert!(
                header.selected.is_some(),
                "a column header must expose its sort state: {:?}",
                header.name
            );
        }

        // Document tabs: a Tab role with exactly one selected.
        let tabs: Vec<&a11y::NodeRow> = rows.iter().filter(|row| row.role == Role::Tab).collect();
        assert_eq!(
            tabs.len(),
            app.workbench.documents().len(),
            "every document tab is a Tab node\n{dump}"
        );
        for tab in &tabs {
            assert!(!tab.name.is_empty(), "a tab has no name");
            assert!(tab.selected.is_some(), "a tab must expose its selection state");
        }
        assert_eq!(
            tabs.iter().filter(|tab| tab.selected == Some(true)).count(),
            1,
            "exactly one tab is selected"
        );

        // Editor: role, name, value (the document text) and focusability.
        let editor = rows
            .iter()
            .find(|row| row.role == Role::MultilineTextInput)
            .unwrap_or_else(|| panic!("no editor node\n{dump}"));
        assert!(
            editor.name.starts_with("Script editor,"),
            "the editor name states the buffer: {}",
            editor.name
        );
        assert_eq!(
            editor.value.as_deref(),
            Some(STRUCTURAL_SOURCE),
            "the editor value is the buffer a reader can read"
        );
        assert!(editor.focusable, "the editor is keyboard focusable");

        // Plot: an Image with a name and the engine title as its value.
        let plot = rows
            .iter()
            .find(|row| row.role == Role::Image)
            .unwrap_or_else(|| panic!("no plot node\n{dump}"));
        assert!(plot.name.contains("SVG plot canvas"), "plot name: {}", plot.name);
        assert_eq!(plot.value.as_deref(), Some("round 43 a11y plot"));

        // Status: its own role, named with the run state and the success marker.
        let status = rows
            .iter()
            .find(|row| row.role == Role::Status)
            .unwrap_or_else(|| panic!("no status node\n{dump}"));
        assert!(
            status.name.contains("run Finished") && status.name.contains("[OK]"),
            "the status name must carry the outcome and its text marker: {}",
            status.name
        );

        // Progress: a numeric ProgressIndicator.
        let progress = rows
            .iter()
            .find(|row| row.role == Role::ProgressIndicator)
            .unwrap_or_else(|| panic!("no progress node\n{dump}"));
        assert_eq!(
            progress.name,
            format!("{}/{}", app.view.committed_steps, app.view.total)
        );
        assert_eq!(progress.numeric_value, Some(app.view.total as f64));

        // Splitters: the three pane dividers have a role and a name.
        let splitters: Vec<&a11y::NodeRow> =
            rows.iter().filter(|row| row.role == Role::Splitter).collect();
        assert_eq!(splitters.len(), 3, "three pane splitters\n{dump}");
        for splitter in &splitters {
            assert!(
                !splitter.name.is_empty(),
                "a splitter must name what it divides"
            );
        }

        // Every landmark pane either names itself or contains a named pane. egui wraps each
        // top-level panel in a structural Pane of its own; the *named* landmark is the region
        // painted inside it, and a reader navigating by landmarks reaches that one.
        for pane in rows
            .iter()
            .filter(|row| row.role == Role::Pane && row.bounds.is_positive())
        {
            let named = !pane.name.is_empty()
                || rows.iter().any(|child| {
                    child.role == Role::Pane
                        && !child.name.is_empty()
                        && a11y::is_descendant(&rows, child, pane.id)
                });
            assert!(
                named,
                "a Pane at {:?} has no name and no named pane inside it\n{dump}",
                pane.bounds
            );
        }

        // Every button is named (the audit covers buttons this test did not know about too).
        for button in rows.iter().filter(|row| row.role == Role::Button) {
            assert!(
                !button.name.is_empty(),
                "a Button at {:?} has no name\n{dump}",
                button.bounds
            );
        }

        // Keep the table as an artifact for the report when the test is run with --nocapture.
        if let Some(path) = std::env::var_os("LOVELACE_A11Y_DUMP") {
            let _ = std::fs::write(path, &dump);
        }
    }

    /// A11Y-004: at 125%, 150% and 200% interface scale (on top of the OS DPI), no control and
    /// no text node leaves its pane, measured from the same accessibility bounds the Windows UIA
    /// adapter publishes.
    #[test]
    fn a11y_004_interface_scale_keeps_controls_and_labels_inside_their_panes() {
        let mut app = structural_app();
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let _ = press_action(
            &ctx,
            &mut app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&ctx, &mut app);
        let physical = vec2(1600.0, 1000.0);
        let mut summary = String::new();
        for percent in [125u32, 150, 200] {
            app.settings.ui_scale_percent = percent;
            let scale = percent as f32 / 100.0;
            let screen = physical / scale;
            // egui applies a new zoom factor at the start of the following pass, so the setting
            // needs two passes before the layout is measured at it.
            frame(&ctx, &mut app, Vec::new());
            frame_sized(&ctx, &mut app, screen, Vec::new());
            assert!(
                (ctx.zoom_factor() - scale).abs() < 1e-4,
                "the setting must set the zoom factor: {} != {scale}",
                ctx.zoom_factor()
            );
            let rows = accessibility_rows(&ctx, &mut app, screen);
            let window = Rect::from_min_size(pos2(0.0, 0.0), screen).expand(0.5);
            // The four pane rectangles the frame painted. A `Ui` node's own accesskit bounds are
            // not filled in until a widget owns it, so containment is measured against the same
            // rectangles the panes paint into — the pane a control visibly sits in.
            let layout = app.layout.expect("the frame paints the pane layout");
            let panes = [
                ("Editor", layout.editor),
                ("Plot", layout.plot),
                ("Workspace", layout.workspace),
                ("Logs", layout.logs),
            ];
            let mut checked = 0usize;
            for row in &rows {
                if row.bounds == Rect::NOTHING || !row.bounds.is_positive() {
                    continue;
                }
                // A row inside a scroll area is reported at its layout position; it can be
                // scrolled out of the window and is clipped by the viewport, not by the layout.
                let scrollable = a11y::in_scroll_container(&rows, row);
                if !scrollable {
                    assert!(
                        window.contains_rect(row.bounds),
                        "{percent}%: {:?} '{}' at {:?} leaves the {:.0}x{:.0} window",
                        row.role,
                        row.name,
                        row.bounds,
                        screen.x,
                        screen.y
                    );
                }
                // Pane containment is measured on the controls. A scrolled text node in the
                // logs or workspace reports the layout position it had before the scroll
                // offset, which is not where the reader sees it; the painted-shape assertion
                // below covers text clipping instead.
                if !row.interactive() {
                    continue;
                }
                let Some((pane_name, pane)) = panes
                    .iter()
                    .find(|(_, pane)| pane.contains(row.bounds.center()))
                else {
                    continue;
                };
                // A control may not be cut at the pane's left or right edge. (A row inside the
                // workspace/logs list can be partly scrolled past the top or bottom edge; that
                // is ordinary scrolling, and the scroll viewport clips it cleanly. The editor and
                // plot panes have no scrolling controls, so they require full containment.)
                assert!(
                    row.bounds.left() >= pane.left() - 0.5
                        && row.bounds.right() <= pane.right() + 0.5,
                    "{percent}%: {:?} '{}' at {:?} is cut at the left/right edge of the {pane_name} pane at {:?}",
                    row.role,
                    row.name,
                    row.bounds,
                    pane
                );
                if *pane_name == "Editor" || *pane_name == "Plot" {
                    assert!(
                        pane.expand(0.5).contains_rect(row.bounds),
                        "{percent}%: {:?} '{}' at {:?} exceeds the {pane_name} pane at {:?}",
                        row.role,
                        row.name,
                        row.bounds,
                        pane
                    );
                }
                checked += 1;
            }
            // "No text is clipped" at the paint layer: every visible text shape must fit inside
            // the clip rectangle its pane or scroll area paints it through. A shape wider than
            // its clip would be cut mid-glyph.
            let painted_output =
                frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
            let mut text_shapes = 0usize;
            for clipped in &painted_output.shapes {
                let mut painted = Painted::default();
                collect_painted(&clipped.shape, &mut painted);
                for (text, rect) in painted.texts {
                    if !rect.intersects(window) || !rect.is_positive() {
                        continue;
                    }
                    text_shapes += 1;
                    // No text may be cut at the left or right edge: that is a label laid out
                    // wider than the region that paints it. (Vertical clipping at the top and
                    // bottom of a scroll viewport is ordinary scrolling, not a layout defect;
                    // when the clip rectangle *is* a pane, the vertical edges are asserted too.)
                    let clip = clipped.clip_rect;
                    assert!(
                        rect.left() >= clip.left() - 0.5 && rect.right() <= clip.right() + 0.5,
                        "{percent}%: the text {text:?} at {rect:?} is horizontally clipped by {clip:?}"
                    );
                    let pane_clip = panes.iter().any(|(_, pane)| {
                        (pane.left() - clip.left()).abs() < 1.0
                            && (pane.right() - clip.right()).abs() < 1.0
                            && (pane.top() - clip.top()).abs() < 1.0
                            && (pane.bottom() - clip.bottom()).abs() < 1.0
                    });
                    if pane_clip {
                        assert!(
                            clip.expand(0.5).contains_rect(rect),
                            "{percent}%: the text {text:?} at {rect:?} is clipped by its pane {clip:?}"
                        );
                    }
                }
            }
            assert!(
                text_shapes > 20,
                "{percent}%: only {text_shapes} text shapes were visible"
            );
            assert!(
                checked > 15,
                "{percent}%: only {checked} controls were checked; the tree is too small"
            );
            summary.push_str(&format!(
                "scale={percent}% window_points={:.0}x{:.0} rows={} checked={checked}\n",
                screen.x,
                screen.y,
                rows.len()
            ));
        }
        eprintln!("A11Y-004 layout:\n{summary}");
    }

    /// One painted shape's geometry, collected from a frame for the typography assertions.
    #[derive(Default)]
    struct Painted {
        texts: Vec<(String, Rect)>,
        rects: Vec<(Color32, Rect)>,
        lines: Vec<(Color32, [egui::Pos2; 2])>,
    }

    fn collect_painted(shape: &egui::Shape, out: &mut Painted) {
        match shape {
            egui::Shape::Text(text) => out.texts.push((
                text.galley.text().to_string(),
                Rect::from_min_size(text.pos, text.galley.size()),
            )),
            egui::Shape::Rect(rect) => out.rects.push((rect.fill, rect.rect)),
            egui::Shape::LineSegment { points, stroke } => {
                out.lines.push((stroke.color, *points));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_painted(shape, out);
                }
            }
            _ => {}
        }
    }

    /// A11Y-005: at 12, 18 and 24 pt, at three line heights and with ligatures on and off, the
    /// caret, the bracket cell, the diagnostic underline and the gutter number all stay on the
    /// text grid the editor measures, and the ligature rendering keeps the buffer's columns.
    #[test]
    fn a11y_005_editor_typography_keeps_every_decoration_on_the_text_grid() {
        use crate::settings::{EditorFont, TypographySettings};
        let cases = [
            (12u32, 100u32, false),
            (12, 200, true),
            (18, 125, false),
            (18, 150, true),
            (24, 100, false),
            (24, 200, true),
        ];
        for (size, line_height, ligatures) in cases {
            let mut app = failing_structural_app();
            app.settings.editor.typography = TypographySettings {
                font: EditorFont::Hack,
                font_size_pt: size,
                line_height_percent: line_height,
                ligatures,
            };
            app.apply_editor_settings();
            let ctx = egui::Context::default();
            frame(&ctx, &mut app, Vec::new());
            let _ = press_action(
                &ctx,
                &mut app,
                Action::Workbench(keyboard::WorkbenchCommand::Run),
            );
            pump_until_terminal(&ctx, &mut app);
            assert_eq!(
                app.view.run_state,
                RunState::Failed,
                "{size}pt: the failing engine must produce the diagnostic"
            );
            // Caret inside the bracket pair of line 2 (1-based line 2, column 9).
            app.editors[0].set_caret(1, 8);
            ctx.memory_mut(|memory| memory.request_focus(surface_id()));
            let _ = frame_capturing(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new(), Default::default());
            let output =
                frame_capturing(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new(), Default::default());
            let mut painted = Painted::default();
            for clipped in &output.shapes {
                collect_painted(&clipped.shape, &mut painted);
            }
            let metrics = app.editors[0].metrics;
            assert_eq!(metrics.font_size, size as f32, "the configured size is painted");
            assert_eq!(metrics.ligatures, ligatures);
            assert!(
                metrics.row_h + 0.01 >= size as f32 * line_height as f32 / 100.0,
                "{size}pt/{line_height}%: row height {} is below the line height",
                metrics.row_h
            );
            let cell = metrics.char_w;
            let text_left = metrics.text_left;

            // The caret is a vertical line segment on the caret's column, spanning one row.
            let caret = painted
                .lines
                .iter()
                .filter(|(_, points)| {
                    (points[0].x - points[1].x).abs() < 0.01
                        && (points[0].x - (text_left + 8.0 * cell)).abs() < 0.5
                })
                .max_by(|a, b| {
                    (a.1[1].y - a.1[0].y)
                        .partial_cmp(&(b.1[1].y - b.1[0].y))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or_else(|| {
                    panic!("{size}pt/{line_height}%: the focused editor paints a caret")
                });
            let expected_x = text_left + 8.0 * cell;
            assert!(
                (caret.1[0].x - expected_x).abs() < 0.5,
                "{size}pt/{line_height}%: caret x {} != text_left + 8 cells {expected_x}",
                caret.1[0].x
            );
            let row_top = caret.1[0].y - 1.0;
            assert!(
                ((caret.1[1].y - caret.1[0].y) - (metrics.row_h - 2.0)).abs() < 0.5,
                "{size}pt/{line_height}%: the caret must span one row ({:?} vs {})",
                caret.1,
                metrics.row_h
            );

            // The gutter number for row 2 starts at the same y as the text row.
            let gutter_number = painted
                .texts
                .iter()
                .find(|(text, rect)| {
                    text == "2" && rect.left() < text_left && (rect.top() - row_top).abs() < 0.5
                })
                .unwrap_or_else(|| panic!("{size}pt/{line_height}%: no gutter number for row 2"));
            assert!(
                (gutter_number.1.top() - row_top).abs() < 0.5,
                "{size}pt/{line_height}%: gutter number y {} != row top {row_top}",
                gutter_number.1.top()
            );

            // The matched bracket cells: a one-cell fill and its underline on the row, at the
            // cells columns 7 and 9 name. Colour is the theme tests' concern; these geometry
            // assertions are what the typography row is about.
            for column in [8usize, 10] {
                let x = text_left + column as f32 * cell;
                assert!(
                    painted.rects.iter().any(|(_, rect)| {
                        (rect.left() - x).abs() < 0.5
                            && (rect.width() - cell).abs() < 0.5
                            && rect.height() >= metrics.row_h - 3.0
                            && rect.top() >= row_top - 0.5
                            && rect.bottom() <= row_top + metrics.row_h + 0.5
                    }),
                    "{size}pt/{line_height}%: no bracket cell at column {column} (x={x})"
                );
                assert!(
                    painted.rects.iter().any(|(_, rect)| {
                        (rect.left() - x).abs() < 0.5
                            && (rect.width() - cell).abs() < 0.5
                            && rect.height() <= 3.0
                            && (rect.bottom() - (row_top + metrics.row_h - 1.0)).abs() < 0.5
                    }),
                    "{size}pt/{line_height}%: no bracket underline at column {column}"
                );
            }

            // The diagnostic underline covers the reported span of the failed line, at the
            // bottom edge of the same row.
            let line_two = "y = sqrt(x)";
            let underline = painted
                .rects
                .iter()
                .find(|(_, rect)| {
                    (rect.left() - text_left).abs() < 0.5
                        && (rect.center().y - (row_top + metrics.row_h - 2.0)).abs() < 1.5
                })
                .unwrap_or_else(|| panic!("{size}pt/{line_height}%: no diagnostic underline"));
            assert!(
                (underline.1.width() - line_two.chars().count() as f32 * cell).abs() < 0.5,
                "{size}pt/{line_height}%: underline width {} != {} cells",
                underline.1.width(),
                line_two.chars().count()
            );
            assert!(
                (underline.1.bottom() - (row_top + metrics.row_h - 1.0)).abs() < 0.5,
                "{size}pt/{line_height}%: underline bottom {} != row bottom {}",
                underline.1.bottom(),
                row_top + metrics.row_h - 1.0
            );
            assert!(
                painted.rects.iter().any(|(_, rect)| {
                    rect.right() <= text_left
                        && (8.0..=12.0).contains(&rect.width())
                        && (rect.center().y - (row_top + metrics.row_h * 0.5)).abs() < 1.0
                }),
                "{size}pt/{line_height}%: the gutter error badge must sit on the failed row"
            );

            // The ligature rendering: same number of characters as the buffer line, so every
            // column above still points at the same cell.
            let line_three = "z = f(1)->2";
            let painted_line = painted
                .texts
                .iter()
                .find(|(text, _)| text.contains("z = f(1)"))
                .map(|(text, _)| text.clone())
                .unwrap_or_else(|| panic!("{size}pt/{line_height}%: line 3 is painted"));
            assert_eq!(
                painted_line.chars().count(),
                line_three.chars().count(),
                "{size}pt/{line_height}%: the painted line must keep the buffer's columns: {painted_line:?}"
            );
            if ligatures {
                assert!(
                    painted_line.contains('\u{2192}'),
                    "{size}pt/{line_height}%: ligatures are on, so -> paints as an arrow: {painted_line:?}"
                );
                assert!(
                    !painted_line.contains("->"),
                    "{size}pt/{line_height}%: the pair is replaced: {painted_line:?}"
                );
            } else {
                assert_eq!(painted_line, line_three);
            }
        }
    }

    /// A11Y-006: an OS application-theme notification repaints the window and preserves the
    /// document, the caret, the run and the focused control; an explicit override ignores it.
    #[test]
    fn a11y_006_an_os_theme_change_preserves_focus_caret_run_and_document() {
        let mut app = structural_app();
        app.settings.theme = theme::ThemeId::Dark;
        app.theme = theme::ThemeId::Dark;
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let _ = press_action(
            &ctx,
            &mut app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&ctx, &mut app);
        app.editors[0].set_caret(2, 4);
        ctx.memory_mut(|memory| memory.request_focus(surface_id()));
        frame(&ctx, &mut app, Vec::new());

        let source = app.workbench.documents()[0].source.clone();
        let caret = app.editors[0].caret();
        let run_state = app.view.run_state;
        let focused = ctx.memory(|memory| memory.focused());
        let tabs = app.workbench.documents().len();

        // The simulated OS notification: one frame whose RawInput carries the system theme.
        let notify = |app: &mut WorkbenchApp, system: egui::Theme| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0))),
                system_theme: Some(system),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.ui_root(ui));
            output.textures_delta.clear();
        };
        assert_eq!(
            app.theme,
            theme::ThemeId::Dark,
            "the frame starts on the explicit dark theme"
        );
        notify(&mut app, egui::Theme::Light);
        assert_eq!(
            app.theme,
            theme::ThemeId::Light,
            "following the OS must switch the token set"
        );
        assert_eq!(theme::active(), theme::ThemeId::Light);
        assert_eq!(app.settings.theme, theme::ThemeId::Light, "the follow state persists");
        // Nothing else may have moved.
        assert_eq!(app.workbench.documents()[0].source, source, "the document survives");
        assert_eq!(app.editors[0].caret(), caret, "the caret survives");
        assert_eq!(app.view.run_state, run_state, "the run state survives");
        assert_eq!(ctx.memory(|memory| memory.focused()), focused, "the focus survives");
        assert_eq!(app.workbench.documents().len(), tabs, "the tabs survive");
        assert_eq!(app.editors.len(), tabs);

        notify(&mut app, egui::Theme::Dark);
        assert_eq!(app.theme, theme::ThemeId::Dark, "a second OS change is followed back");
        assert_eq!(app.editors[0].caret(), caret);
        assert_eq!(app.view.run_state, run_state);

        // An explicit override: the OS notification is ignored.
        app.settings.theme_follows_system = false;
        app.settings.theme = theme::ThemeId::Light;
        app.theme = theme::ThemeId::Light;
        notify(&mut app, egui::Theme::Dark);
        assert_eq!(
            app.theme,
            theme::ThemeId::Light,
            "an explicit override must ignore the OS theme"
        );
        assert_eq!(app.editors[0].caret(), caret);
        assert_eq!(app.view.run_state, run_state);
        assert_eq!(app.workbench.documents()[0].source, source);
    }

    /// A11Y-012: every error surface carries a text marker and a location, and the error tokens
    /// are distinguishable in greyscale (luminance contrast, no hue).
    #[test]
    fn a11y_012_errors_carry_a_marker_and_a_location_without_colour() {
        use egui::Role;
        let mut app = failing_structural_app();
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let _ = press_action(
            &ctx,
            &mut app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&ctx, &mut app);
        assert_eq!(app.view.run_state, RunState::Failed);
        app.editors[0].set_caret(1, 8);
        ctx.memory_mut(|memory| memory.request_focus(surface_id()));
        let output =
            frame_capturing(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new(), Default::default());
        let mut painted = Painted::default();
        for clipped in &output.shapes {
            collect_painted(&clipped.shape, &mut painted);
        }
        let rows = accessibility_rows(&ctx, &mut app, vec2(1600.0, 1000.0));
        let dump = a11y::dump_table(&rows);

        // Status: text marker + location.
        let status = rows
            .iter()
            .find(|row| row.role == Role::Status)
            .unwrap_or_else(|| panic!("no status node\n{dump}"));
        assert!(
            status.name.contains("[ERR]"),
            "the status must carry a non-colour marker: {}",
            status.name
        );
        assert!(
            status.name.contains("error at Ln 2, Col 1"),
            "the status must carry the error's location: {}",
            status.name
        );
        // Log line: the kind marker plus the rendered diagnostic's line and column.
        let log_line = rows
            .iter()
            .filter(|row| row.role == Role::Label)
            .map(|row| row.name.clone())
            .find(|name| name.starts_with("[diagnostic]") && name.contains("line 2, column 1"))
            .unwrap_or_else(|| panic!("no diagnostic log line\n{dump}"));
        assert!(
            log_line.contains("line 2, column 1"),
            "the log line names the location: {log_line}"
        );
        assert!(
            log_line.contains("[diagnostic]"),
            "the log line has its kind marker: {log_line}"
        );
        // Editor node: the diagnostic is its description, with the location.
        let editor = rows
            .iter()
            .find(|row| row.role == Role::MultilineTextInput)
            .unwrap_or_else(|| panic!("no editor node\n{dump}"));
        let description = editor.description.clone().unwrap_or_default();
        assert!(
            description.contains("Ln 2, Col 1"),
            "the editor's description names the diagnostic location: {description}"
        );
        // Decoration: the painted gutter badge is a shape (a filled square with a `!`), not a
        // colour-only underline; its position is the failed row's gutter cell.
        let metrics = app.editors[0].metrics;
        assert!(
            painted.rects.iter().any(|(_, rect)| {
                rect.right() <= metrics.text_left
                    && (8.0..=12.0).contains(&rect.width())
                    && (rect.height() - rect.width()).abs() < 1.0
            }),
            "the editor paints a square diagnostic badge in the gutter"
        );
        assert!(
            painted.texts.iter().any(|(text, _)| text == "!"),
            "the badge carries the exclamation glyph"
        );

        // Greyscale: the error badges/words remain distinguishable by luminance alone in both
        // themes. This is the ratio the desaturated capture is checked against.
        let mut ratios = String::new();
        for theme_id in [theme::ThemeId::Dark, theme::ThemeId::Light] {
            let tokens = theme::tokens_for(theme_id);
            let badge = a11y::luminance_contrast(tokens.diagnostic_marker, tokens.gutter_bg);
            let word = a11y::luminance_contrast(tokens.error, tokens.panel_bg);
            assert!(
                badge >= 3.0,
                "{theme_id:?}: the gutter badge is {badge:.2}:1 in greyscale (needs 3:1)"
            );
            assert!(
                word >= 4.5,
                "{theme_id:?}: the error word is {word:.2}:1 in greyscale (needs 4.5:1)"
            );
            let luma_marker = a11y::greyscale_luma(tokens.diagnostic_marker);
            let luma_gutter = a11y::greyscale_luma(tokens.gutter_bg);
            assert_ne!(
                luma_marker, luma_gutter,
                "{theme_id:?}: the desaturated badge must not equal the gutter"
            );
            ratios.push_str(&format!(
                "{theme_id:?}: badge {badge:.2}:1 (luma {luma_marker} on {luma_gutter}), word {word:.2}:1\n"
            ));
        }
        eprintln!("A11Y-012 greyscale luminance:\n{ratios}");

        // The success marker is text too: the same window after a successful run says [OK].
        let mut good = structural_app();
        let good_ctx = egui::Context::default();
        frame(&good_ctx, &mut good, Vec::new());
        let _ = press_action(
            &good_ctx,
            &mut good,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&good_ctx, &mut good);
        let good_rows = accessibility_rows(&good_ctx, &mut good, vec2(1600.0, 1000.0));
        let good_status = good_rows
            .iter()
            .find(|row| row.role == Role::Status)
            .expect("a status node");
        assert!(
            good_status.name.contains("[OK]"),
            "success must also be identifiable without hue: {}",
            good_status.name
        );
    }
}


