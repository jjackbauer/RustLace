//! The native workbench window: toolbar, tab strip, four panes and the status strip.
//!
//! The window is a thin view over [`crate::workbench::Workbench`]. It never evaluates a script:
//! Run only submits to the `ExecutionHost`, every frame polls the typed event stream, and every
//! value in the panes comes from the snapshot or the engine's plot capture. Panes with nothing
//! to show render their empty state instead of sample data.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, CursorIcon, Id, Key, Layout, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use egui::{FontId, TextFormat, TextStyle};

use crate::engine::FunctionOrigin;
use crate::execution::{Diagnostic, DocumentId};
use crate::glyphs::GlyphMode;
use crate::inspector::{
    Inspection, InspectorExpansion, Section, TREE_LEVELS_STEP, TreeNode, tree_rows,
};
use crate::keyboard::{self, Action, FileCommand, KeyName, Pane, Shortcut};
use crate::log_kind::LogKind;
use crate::quick_eval::QuickEval;
use crate::recovery::{self, RecoveryJournal, RecoveryRecord, RecoveryScan};
use crate::results_tree::{
    ArrayNode, ExpansionState, ResultProjection, StructuredNode, element_path, field_path,
};
use crate::settings::{
    self, CompactPaneSetting, MAX_PRECISION, MIN_PRECISION, Settings, WorkbenchLayout,
};
use crate::symbolic_status::{self, StatusBadge, Tone};
use crate::workbench::{DocumentView, FileConflict, SaveOutcome, Workbench};
use crate::workspace_table::{
    FunctionColumn, FunctionTable, VariableColumn, VariableTable, cell_text, ordered_functions,
    ordered_variables,
};

use super::a11y;
use super::control;
use super::editor::{TextEditor, surface_id};
use super::empty;
use super::file_dialogs::{self, FileDialogs, NativeFileDialogs};
use super::plot::{self, Plot, PngBackground};
use super::semantic;
use super::theme;
use super::type_scale;
use super::typography;

/// Width of a splitter, in pixels.
pub const SPLIT: f32 = 6.0;
const MIN_EDITOR: f32 = 320.0;
const MIN_PLOT: f32 = 240.0;
const MIN_WORKSPACE: f32 = 180.0;
const MIN_LOGS: f32 = 100.0;
const MIN_TOP: f32 = 240.0;
/// LAY-002: the Symbolic Inspector's column in the bottom strip, and the Logs pane beside it.
/// Both are wide enough for a scrollable text/tree body at the normal split; the dense split
/// shrinks them proportionally, the same rule the top columns use.
const MIN_INSPECTOR_WIDTH: f32 = 220.0;
const MIN_LOGS_WIDTH: f32 = 220.0;
const DENSE_MIN_INSPECTOR_WIDTH: f32 = 150.0;
const DENSE_MIN_LOGS_WIDTH: f32 = 150.0;

/// The shipped pane-size fractions. LAY-001: the editor gets 48% of the client (about 47.5% of
/// the workbench after the splitters), above the 45% rule, while the graph and workspace columns
/// keep their minimums. LAY-004 persists these values; a double-click or the reset command
/// returns to exactly these numbers.
pub const DEFAULT_EDITOR_FRACTION: f32 = 0.48;
/// The Workspace column's share of the client width.
pub const DEFAULT_WORKSPACE_FRACTION: f32 = 0.20;
/// The Logs strip's share of the client height.
pub const DEFAULT_LOGS_FRACTION: f32 = 0.22;
/// The Symbolic Inspector's share of the client width inside the bottom strip.
pub const DEFAULT_INSPECTOR_FRACTION: f32 = 0.30;

/// LAY-011: the smallest client (in points) that still fits three columns and a logs strip.
///
/// This is a *point* threshold, not a pixel one: 1280x800 at 100% (1280x800 points) and at 150%
/// (853x533 points) are above it, 900x600 at 100% (900x600 points) is above it, and the same
/// windows at 200% (640x400 and 450x300 points) are below it, where a split would have to squeeze
/// the editor and every pane below a usable width. The compact layout takes over there.
pub const MIN_SPLIT_WIDTH: f32 = 760.0;
/// The smallest client height (in points) that still fits a top row and a logs strip.
pub const MIN_SPLIT_HEIGHT: f32 = 340.0;
/// LAY-011: the editor's share of the workbench in the compact layout when the drawer is open.
/// The requirement is at least ~45%; the default leaves the editor the primary surface.
pub const COMPACT_EDITOR_DEFAULT: f32 = 0.55;
/// LAY-011: the compact edition never lets the drawer shrink the editor below this share.
pub const COMPACT_EDITOR_MIN: f32 = 0.48;
/// LAY-011: the height of the compact pane switcher's row.
pub const COMPACT_TAB_HEIGHT: f32 = 24.0;
/// LAY-011: the column minimums of the dense split (a large window at a scale that leaves fewer
/// than 760x340 points). They are smaller than the normal minimums because the dense split uses
/// the compact pane headers, which fit a narrower column.
const DENSE_MIN_EDITOR: f32 = 250.0;
const DENSE_MIN_PLOT: f32 = 170.0;
const DENSE_MIN_WORKSPACE: f32 = 130.0;
/// LAY-011: the editor's share in the dense split; 47% of the workbench keeps it above the 45%
/// rule while leaving the two ancillary columns enough width for their controls.
const DENSE_EDITOR_FRACTION: f32 = 0.47;

/// Which of the window layouts a pass uses (LAY-011 / VIS-016).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// The normal four-pane arrangement: editor | plot | workspace over logs.
    Split,
    /// The four-pane arrangement at a large window whose points are too few for the normal
    /// minimums (1280x800 at 200%): the panes stay side by side, but the chrome and the pane
    /// headers use their compact forms so the split keeps the editor above 45% and stays inside
    /// the client.
    DenseSplit,
    /// The minimum-window arrangement: the editor stays the primary surface and the ancillary
    /// panes collapse into one drawer reached from a compact tab row.
    Compact,
}

impl LayoutMode {
    /// The layout the window described by `window` (client size in points) and
    /// `pixels_per_point` (the interface scale over the OS DPI) must use.
    ///
    /// * At least 760x340 points: the normal split.
    /// * Fewer points but a large physical client (at least 1200x700 physical): the dense split,
    ///   the four panes at the compact chrome/header sizes.
    /// * Anything smaller (the minimum window at 150/200%): the compact drawer layout.
    pub fn for_window(window: Vec2, pixels_per_point: f32) -> LayoutMode {
        if window.x >= MIN_SPLIT_WIDTH && window.y >= MIN_SPLIT_HEIGHT {
            LayoutMode::Split
        } else if window.x * pixels_per_point >= 1200.0 && window.y * pixels_per_point >= 700.0 {
            LayoutMode::DenseSplit
        } else {
            LayoutMode::Compact
        }
    }

    /// True when the pane headers use their compact form.
    fn compact_headers(self) -> bool {
        self != LayoutMode::Split
    }
}

/// LAY-011 / LAY-002: the ancillary panes the compact layout can show in its drawer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactPane {
    /// The engine graph.
    Graph,
    /// The Workspace pane (the committed result and the variables/functions tables).
    Workspace,
    /// The run log and quick eval.
    Logs,
    /// LAY-002: the six-section Symbolic Inspector over the last committed result.
    Inspector,
}

impl CompactPane {
    /// The pane's tab label, which is also its accessible name.
    pub fn label(self) -> &'static str {
        match self {
            CompactPane::Graph => "Graph",
            CompactPane::Workspace => "Workspace",
            CompactPane::Logs => "Logs",
            CompactPane::Inspector => "Symbolic Inspector",
        }
    }
}

/// LAY-011: the rectangles of the compact layout.
#[derive(Clone, Copy, Debug)]
pub struct CompactLayout {
    /// The row of pane tabs, across the top of the workbench.
    pub tab_bar: Rect,
    /// The editor, always visible and always at least [`COMPACT_EDITOR_MIN`] of the width
    /// beside the drawer (or the whole width when the drawer is collapsed).
    pub editor: Rect,
    /// The open ancillary pane, when the drawer is open.
    pub drawer: Option<Rect>,
    /// The draggable divider between the editor and the drawer.
    pub splitter: Option<Rect>,
    /// Which pane the drawer shows; `None` means collapsed to the tab row.
    pub pane: Option<CompactPane>,
}

/// LAY-011: computes the compact layout from the workbench's client rect.
///
/// The tab row is always painted, so every collapsed pane is one pointer action away; the drawer
/// itself is optional. The editor keeps [`COMPACT_EDITOR_MIN`] of the width whether the drawer is
/// open or closed, and the whole area is clamped to `full` so no rectangle can leave the client.
pub fn compute_compact_layout(
    full: Rect,
    pane: Option<CompactPane>,
    editor_fraction: f32,
) -> CompactLayout {
    let width = full.width().max(0.0);
    let tab_h = COMPACT_TAB_HEIGHT.min(full.height().max(0.0));
    let tab_bar = Rect::from_min_size(full.min, vec2(width, tab_h));
    let top = (tab_bar.bottom() + 2.0).min(full.bottom());
    let area = Rect::from_min_max(pos2(full.left(), top), full.max);
    let height = area.height().max(0.0);
    match pane {
        None => CompactLayout {
            tab_bar,
            editor: area,
            drawer: None,
            splitter: None,
            pane,
        },
        Some(pane) => {
            let usable = (area.width() - SPLIT).max(0.0);
            let fraction = editor_fraction.clamp(COMPACT_EDITOR_MIN, 0.82);
            let editor_w = (usable * fraction).min(usable);
            let drawer_w = (usable - editor_w).max(0.0);
            let editor = Rect::from_min_size(area.min, vec2(editor_w, height));
            let splitter = Rect::from_min_size(
                pos2(editor.right(), area.top()),
                vec2(SPLIT.min(drawer_w + SPLIT), height),
            );
            let drawer =
                Rect::from_min_size(pos2(splitter.right(), area.top()), vec2(drawer_w, height));
            CompactLayout {
                tab_bar,
                editor,
                drawer: Some(drawer),
                splitter: Some(splitter),
                pane: Some(pane),
            }
        }
    }
}

/// Rectangles of every pane and splitter for one pass.
#[derive(Clone, Copy, Debug)]
pub struct PaneLayout {
    /// The workbench's client rectangle the panes were laid out in (the measurement seam the
    /// layout tests assert against).
    #[cfg_attr(not(test), allow(dead_code))]
    pub full: Rect,
    /// Which layout this pass used.
    #[cfg_attr(not(test), allow(dead_code))]
    pub mode: LayoutMode,
    /// Editor pane.
    pub editor: Rect,
    /// Graph pane (empty in the compact layout unless its drawer is open).
    pub plot: Rect,
    /// Workspace pane (empty in the compact layout unless its drawer is open).
    pub workspace: Rect,
    /// Logs pane (empty in the compact layout unless its drawer is open).
    pub logs: Rect,
    /// LAY-002: the Symbolic Inspector pane, beside the Logs pane in the bottom strip.
    pub inspector: Rect,
    /// Editor / graph splitter.
    pub split_a: Rect,
    /// Graph / workspace splitter.
    pub split_b: Rect,
    /// Top / bottom-strip splitter.
    pub split_c: Rect,
    /// Logs / Symbolic Inspector splitter.
    pub split_d: Rect,
    /// The compact rectangles, when [`PaneLayout::mode`] is [`LayoutMode::Compact`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub compact: Option<CompactLayout>,
}

/// Computes the five-pane layout from the saved fractions, honouring minimum sizes whenever the
/// window is large enough and compressing the minimums proportionally when it is not.
///
/// The top row is editor | graph | workspace. The bottom strip is split again: logs on the left,
/// the Symbolic Inspector on the right. `dense` selects the compact minimums and the compact
/// editor share used by [`LayoutMode::DenseSplit`]; the five panes are the same.
pub fn compute_layout(
    full: Rect,
    editor_frac: f32,
    workspace_frac: f32,
    logs_frac: f32,
    inspector_frac: f32,
    dense: bool,
) -> PaneLayout {
    let (min_editor_base, min_plot_base, min_workspace_base, editor_frac) = if dense {
        (
            DENSE_MIN_EDITOR,
            DENSE_MIN_PLOT,
            DENSE_MIN_WORKSPACE,
            editor_frac.max(DENSE_EDITOR_FRACTION),
        )
    } else {
        (MIN_EDITOR, MIN_PLOT, MIN_WORKSPACE, editor_frac)
    };
    let usable_w = (full.width() - 2.0 * SPLIT).max(0.0);
    let min_sum = min_editor_base + min_plot_base + min_workspace_base;
    let shrink_w = if usable_w >= min_sum {
        1.0
    } else {
        usable_w / min_sum
    };
    let min_editor = min_editor_base * shrink_w;
    let min_plot = min_plot_base * shrink_w;
    let min_workspace = min_workspace_base * shrink_w;

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

    // LAY-002: the bottom strip holds the Logs pane and the Symbolic Inspector. The inspector's
    // fraction is of the whole client width, the same measure the top fractions use.
    let (min_logs_w_base, min_inspector_w_base) = if dense {
        (DENSE_MIN_LOGS_WIDTH, DENSE_MIN_INSPECTOR_WIDTH)
    } else {
        (MIN_LOGS_WIDTH, MIN_INSPECTOR_WIDTH)
    };
    let usable_bottom_w = (full.width() - SPLIT).max(0.0);
    let bottom_min_sum = min_logs_w_base + min_inspector_w_base;
    let shrink_b = if usable_bottom_w >= bottom_min_sum {
        1.0
    } else {
        usable_bottom_w / bottom_min_sum
    };
    let min_logs_w = min_logs_w_base * shrink_b;
    let min_inspector_w = min_inspector_w_base * shrink_b;
    let max_inspector_w = (usable_bottom_w - min_logs_w).max(min_inspector_w);
    let inspector_w = (full.width() * inspector_frac).clamp(min_inspector_w, max_inspector_w);
    let logs_w = (usable_bottom_w - inspector_w).max(0.0);

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
    let bottom_top = split_c.bottom();
    let logs = Rect::from_min_size(pos2(full.left(), bottom_top), vec2(logs_w, logs_h));
    let split_d = Rect::from_min_size(
        pos2(logs.right(), bottom_top),
        vec2(SPLIT.min(full.right() - logs.right()).max(0.0), logs_h),
    );
    let inspector =
        Rect::from_min_size(pos2(split_d.right(), bottom_top), vec2(inspector_w, logs_h));

    PaneLayout {
        full,
        mode: if dense {
            LayoutMode::DenseSplit
        } else {
            LayoutMode::Split
        },
        editor,
        plot,
        workspace,
        logs,
        inspector,
        split_a,
        split_b,
        split_c,
        split_d,
        compact: None,
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

/// SYM-001 / SYM-003: one painted expression-tree row's seam.
///
/// The path names the node, the rectangle is where the header was painted (the pointer tests
/// click its centre) and the id is the header widget's own id, so a keyboard move can focus the
/// same control a click would.
#[derive(Clone, Debug)]
struct TreeRowSeam {
    /// The node's path in the tree (`inspect.tree`, `inspect.tree.0`).
    path: String,
    /// The painted header's rectangle.
    rect: Rect,
    /// The header widget's id.
    id: Id,
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

/// What pressing a button in the DOC-006 external-change prompt means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConflictAnswer {
    /// Discard the buffer and load the file as it stands on disk (confirmed first when dirty).
    Reload,
    /// Overwrite the file deliberately, after the user has seen both versions.
    KeepMine,
    /// Write nothing and leave the buffer, the caret and the focus exactly as they were.
    Cancel,
}

impl ConflictAnswer {
    /// The button label, which is also the button's accessible name in a UIA capture.
    fn label(self) -> &'static str {
        match self {
            ConflictAnswer::Reload => "Reload",
            ConflictAnswer::KeepMine => "Keep mine",
            ConflictAnswer::Cancel => "Cancel",
        }
    }
}

/// The second question a Reload asks when the buffer has unsaved changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReloadAnswer {
    /// Throw the buffer away and load the disk text.
    DiscardAndReload,
    /// Go back to the three-way conflict question.
    Back,
}

impl ReloadAnswer {
    fn label(self) -> &'static str {
        match self {
            ReloadAnswer::DiscardAndReload => "Reload and discard",
            ReloadAnswer::Back => "Back",
        }
    }
}

/// The DOC-006 external-change prompt.
///
/// It is opened by a refused Save / Save As, from the tab's conflict marker, or by the close flow's
/// Save. While it is open no keystroke reaches an editor, so the file cannot be overwritten behind
/// the question; and the only path that writes is the deliberate **Keep mine** button.
#[derive(Clone, Debug)]
struct ConflictPrompt {
    /// The document the save belongs to, by stable identity.
    document: DocumentId,
    /// The path the refused write would have used (the document's own path, or Save As' target).
    target: PathBuf,
    /// The recorded and current states the prompt shows.
    report: FileConflict,
    /// The widget that had focus when the prompt opened; restored on Cancel.
    focus_before: Option<Id>,
    /// True while the dirty-buffer Reload confirmation is on screen.
    confirm_reload: bool,
    /// Why the last Keep mine or Reload failed, shown in the prompt itself.
    error: Option<String>,
}

/// Field attributes of the open DOC-006 prompt, read by the window tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConflictSummary {
    /// The document's stable identity.
    pub document: DocumentId,
    /// The path the refused write would have used.
    pub target: PathBuf,
    /// True while the Reload confirmation is on screen.
    pub confirm_reload: bool,
    /// The headline the prompt shows.
    pub headline: String,
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
    /// DOC-006: the open external-change prompt, if any.
    conflict: Option<ConflictPrompt>,
    /// DOC-006 test seam: the conflict prompt buttons painted in the last pass.
    conflict_buttons: Vec<(ConflictAnswer, Rect)>,
    /// DOC-006 test seam: the Reload confirmation buttons painted in the last pass.
    reload_buttons: Vec<(ReloadAnswer, Rect)>,
    /// DOC-006: when the disk was last re-probed for every located document.
    last_disk_poll: Instant,
    /// DOC-006: whether the window had keyboard focus on the previous pass, so regaining it forces
    /// an immediate re-probe instead of waiting for the poll interval.
    window_focused: bool,
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
    /// PLT-005/PLT-009: the identity and bytes of the capture the pane is showing.
    ///
    /// The document id and run id are part of the key, so a tab switch reloads the pane even when
    /// two tabs happen to hold byte-identical captures: the pane can never keep painting one
    /// tab's chart while another tab is active.
    loaded_plot: Option<(u64, Option<u64>, String)>,
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
    /// LAY-002 / LAY-004: the Symbolic Inspector column's share of the client width, persisted
    /// with the other pane sizes.
    inspector_frac: f32,
    /// The layout fractions the settings file was last synchronised from/to. A splitter drag or
    /// a keyboard resize compares against this so the settings file is written when the layout
    /// actually changed, not on every painted frame.
    persisted_layout: WorkbenchLayout,
    /// LAY-011: the layout the current pass uses, decided from the window's client size.
    layout_mode: LayoutMode,
    /// LAY-011: which ancillary pane the compact drawer shows; `None` is the collapsed state.
    compact_pane: Option<CompactPane>,
    /// LAY-011: the editor's share of the compact workbench while the drawer is open.
    compact_editor_frac: f32,
    /// LAY-011: the compact pane tabs painted in the last pass (the pointer-test seam, the same
    /// pattern as `file_buttons`).
    compact_tabs: Vec<(CompactPane, Rect)>,
    /// LAY-011: the "Hide panes" button painted in the last compact pass.
    compact_hide: Option<Rect>,
    /// LAY-011: the Inspector tab painted in the last compact pass; it opens the workspace drawer
    /// in the Inspector view, so the inspector is one action away even when collapsed.
    compact_inspector: Option<Rect>,
    filter: String,
    /// The two Workspace tables, rebuilt from the frame's committed snapshot in [`Self::sync`].
    variables_table: VariableTable,
    functions_table: FunctionTable,
    /// WKS-006: the variable whose full value the Workspace pane shows in its detail view.
    ///
    /// `None` closes the view. The name is kept (not a row index) so a resort, a repaint or a
    /// rebuilt table cannot make the detail point at a different variable.
    variable_detail: Option<String>,
    /// WKS-003 / WKS-006: the per-row controls the variables table painted this pass, keyed
    /// `delete <name>` and `details <name>`.
    ///
    /// The window itself never reads this list; it is the acceptance seam a pointer test uses to
    /// click the control a user sees, exactly like `variable_header_rects`.
    variable_action_rects: Vec<(String, Rect)>,
    /// LOG-005: the log rows the user has selected, by index into the painted log.
    ///
    /// Selection is a range model with an anchor: a plain click selects one row, Shift+click
    /// extends from the anchor, Ctrl+click toggles one row. The set is cleared when the painted
    /// log is replaced (a document switch, a clear, an eviction) because an index would otherwise
    /// name a different line.
    log_selection: std::collections::BTreeSet<usize>,
    /// LOG-005: the row a range selection extends from.
    log_anchor: Option<usize>,
    /// LOG-005: the last copy/save outcome, painted in the pane header.
    log_status: Option<String>,
    /// LOG-005: the log rows the painted pane offered this pass, by index.
    ///
    /// The window itself never reads this list; pointer tests click the row a user sees through
    /// it, exactly like `variable_action_rects`.
    log_row_rects: Vec<(usize, Rect)>,
    /// The log length and discarded count the current selection was made against.
    log_seen: (usize, usize),
    /// LOG-010: the total record count (`logs.len() + logs_dropped`) the last painted pass saw.
    ///
    /// The retained length alone cannot report growth: once the cap evicts an old record for
    /// every new one, `logs.len()` is constant while the log keeps growing.
    log_follow_total: usize,
    /// LOG-010: true while the log view follows the newest record.
    ///
    /// It starts true (a launch paints the tail), stays true only while the viewport is at the
    /// bottom, and is ended by a user scroll away. New records never re-arm it: a user reading
    /// history is not yanked to the end. The [`Self::log_unseen`] counter and the footer's
    /// *Jump to newest* control are the way back.
    log_follow: bool,
    /// LOG-010: records appended since the view left the bottom, named by the jump affordance.
    log_unseen: usize,
    /// LOG-010: request the tail offset on this pass (launch, jump, growth, resize).
    log_force_follow: bool,
    /// LOG-010: how many consecutive passes have tried to reach the tail without getting there.
    /// It bounds the retry so a pathological scroll-area measurement cannot loop.
    log_follow_retries: u8,
    /// LOG-010: the log body height of the last pass, so a resize re-requests the tail.
    log_follow_body: f32,
    /// LOG-010 acceptance seam: hold the view scrolled back for the first layout passes.
    ///
    /// `LOVELACE_LOG_START_SCROLLED=1` sets this at launch. It is cleared by the first pass that
    /// has a real body and a scrollable list, so the state the capture sees is an ordinary
    /// scrolled-back view (no per-pass override); off unless the switch is set.
    log_force_scrolled: bool,
    /// LOG-010: the *Jump to newest* control the pane painted this pass (the pointer-test seam,
    /// the same pattern as `log_row_rects`). `None` when the view follows the tail, so a test can
    /// assert both the control's presence and its absence.
    log_jump_rect: Option<Rect>,
    /// LOG-004 / LOG-006: the pinned diagnostic/result rows the pane painted this pass, by index.
    ///
    /// The window never reads this list; pointer tests click the pinned row a user sees.
    log_pinned_rects: Vec<(usize, Rect)>,
    /// RES-004: the RESULT tree's per-path collapse state. It is bound to the committed result
    /// in `paint_result_section`, so repaints of the same value keep every decision and a new
    /// result clears the map.
    expansion: ExpansionState,
    /// SYM-001: true while the Workspace pane's Inspector view is selected. The view is a tab
    /// beside the Result view; it renders only the committed snapshot's inspection payload.
    inspector_open: bool,
    /// SYM-001: the expression tree's per-path collapse state, scoped to the inspection payload.
    inspector_expansion: InspectorExpansion,
    /// SYM-001: expression-tree headers painted in the last pass, the same test seam as
    /// `result_toggle_rects`. Each entry carries the header's widget id, which is what a keyboard
    /// move focuses.
    inspector_toggle_rects: Vec<TreeRowSeam>,
    /// LAY-002: expression-tree headers painted by the dedicated Symbolic Inspector pane in the
    /// last pass. Kept apart from the Workspace Inspector view's seam so a capture or test can
    /// address the pane's own tree.
    symbolic_inspector_toggle_rects: Vec<TreeRowSeam>,
    /// SYM-003 / SYM-007: the copy/export controls the Workspace Inspector view painted in the
    /// last pass (key and rectangle), the same pointer-test seam as `inspector_toggle_rects`.
    inspector_control_rects: Vec<(String, Rect)>,
    /// SYM-003 / SYM-007: the copy/export controls the dedicated Symbolic Inspector pane painted
    /// in the last pass.
    symbolic_inspector_control_rects: Vec<(String, Rect)>,
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
    /// PERF-011: the estimate dialog's buttons painted last pass (the pointer-test seam, keyed
    /// `Set precision` and `Cancel`).
    precision_confirm_rects: Vec<(String, Rect)>,
    /// PERF-011: the toolbar precision field and Apply button painted last pass (the same
    /// pointer-test seam as the dialog's buttons).
    precision_field_rect: Option<Rect>,
    precision_apply_rect: Option<Rect>,
    layout: Option<PaneLayout>,
    /// A11Y-001: the focus group the keyboard is in, for F6 / Ctrl+1..5 movement.
    focused_pane: Pane,
    /// A11Y-001: a focus request waiting for its pane to paint its first control.
    pane_focus: Option<Pane>,
    /// TST-011 / J-19: the run state whose transition the status strip last exposed as a live
    /// region. The status leaf is announced only on the pass the state changes, so moving the
    /// caret does not re-speak the whole status line to a screen reader.
    last_announced_run: Option<crate::run_state::RunState>,
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
    /// VIS-006: true while the Help > Control states matrix is open.
    control_matrix_open: bool,
    /// A11Y-001: Ctrl+Alt+F requests the workspace filter on the pass it is painted.
    filter_focus: bool,
    /// A11Y-014: the ASCII/Unicode notation in force this frame, mirrored from the settings.
    glyph_mode: GlyphMode,
    /// A11Y-014: font coverage per character, probed once from the shipped font stack.
    glyph_coverage: std::collections::HashMap<char, bool>,
    /// True once the window has moved keyboard focus into the editor on its first frame.
    focus_requested: bool,
    /// A11Y-008: a chord was withheld because an IME composition owned the keyboard, and the
    /// deferral is still in flight. Released by the first pass that carries neither a composition
    /// nor a command chord, so a deferred Ctrl+Enter can never be applied after the composition.
    ime_chord_deferred: bool,
    /// A11Y-008: a composition is in progress in the focused widget.
    ///
    /// Tracked from the window's IME events, not from the editor's own `preedit`: the quick-eval
    /// field is an `egui::TextEdit` that keeps its pre-edit private, and a chord must be deferred
    /// for any focused text widget. Retired by a commit, an empty pre-edit, a delete-surrounding,
    /// or by keyboard focus moving to another widget.
    ime_composition: bool,
    /// The widget that owned the composition when it started.
    ime_composition_owner: Option<Id>,
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

    /// Round-68 IME probe seam: the active document's buffer, the composition the editor is
    /// painting, the depth of its undo history, and which widget owns egui's keyboard focus.
    ///
    /// Read-only. `--ime-probe` records these values around a platform IME message sequence, so
    /// the report states what this window's editor actually received rather than what a sibling
    /// test module assumed.
    ///
    /// The five `ime_probe_*` seams exist in the Windows build (consumed by `src/ime_probe.rs`).
    /// `ime_probe_snapshot` and `ime_probe_focus_quick_eval` also exist on every platform for
    /// tests: the IME-deferral tests drive that focus/snapshot pair from a headless
    /// `egui::Context`, which keeps the Linux window's A11Y-008 behaviour covered without a Win32
    /// IME. The other three have no Linux reader, so they are compiled only into the Windows
    /// binary instead of being dead code there.
    #[cfg(any(windows, test))]
    pub(crate) fn ime_probe_snapshot(
        &self,
        ctx: &egui::Context,
    ) -> (String, Option<String>, usize, String, (usize, usize)) {
        let active = self.workbench.active();
        let (source, preedit, undo_steps, caret) = match self.editors.get(active) {
            Some(editor) => (
                editor.source().to_string(),
                editor.preedit.clone(),
                editor.history.undo_steps(),
                editor.line_col_1based(),
            ),
            None => (String::new(), None, 0, (0, 0)),
        };
        let focused = ctx.memory(|memory| memory.focused());
        let focus = if focused == Some(surface_id()) {
            "editor"
        } else if focused == Some(quick_eval_id()) {
            "quick-eval"
        } else if focused.is_some() {
            "other"
        } else {
            "none"
        };
        (source, preedit, undo_steps, focus.to_string(), caret)
    }

    /// Round-68 IME probe seam: puts the caret at the start of the buffer and gives the editor
    /// the keyboard focus, the state the first click produces.
    #[cfg(windows)]
    pub(crate) fn ime_probe_focus_editor(&mut self, ctx: &egui::Context) {
        let active = self.workbench.active();
        if let Some(editor) = self.editors.get_mut(active) {
            editor.set_caret(0, 0);
            editor.selection = None;
        }
        ctx.memory_mut(|memory| memory.request_focus(surface_id()));
    }

    /// Round-68 IME probe seam: gives the quick-eval field (an `egui::TextEdit`) the keyboard
    /// focus, which is what makes that widget request IME from the integration.
    #[cfg(any(windows, test))]
    pub(crate) fn ime_probe_focus_quick_eval(&mut self, ctx: &egui::Context) {
        ctx.memory_mut(|memory| memory.request_focus(quick_eval_id()));
    }

    /// Round-68 IME probe seam: the quick-eval field's text.
    #[cfg(windows)]
    pub(crate) fn ime_probe_quick_eval(&self) -> String {
        self.quick_eval.text().to_string()
    }

    /// Round-68 IME probe seam: the active document's run-state label.
    #[cfg(windows)]
    pub(crate) fn ime_probe_run_state(&self) -> String {
        self.view.run_state.label().to_string()
    }

    /// Launches over an explicit storage location (the test seam for DOC-004/005).
    fn launch(storage: AppStorage, initial_path: Option<PathBuf>) -> WorkbenchApp {
        let mut settings = match &storage.settings_path {
            Some(path) => Settings::load(path),
            None => Settings::new(),
        };
        let settings_warnings = settings.take_warnings();
        let mut workbench = Workbench::new();
        // LOG-010 acceptance seam: `LOVELACE_LOG_FLOOD=<n>` appends n stdout records and one fatal
        // diagnostic through the real log path, so a capture can show the virtualised pane and the
        // storage cap on a document no script could produce quickly. Off unless set, and no
        // script can reach it (the same pattern as `LOVELACE_ENGINE_FAULT` / `LOVELACE_PLOT_FAULT`).
        if let Ok(count) = std::env::var("LOVELACE_LOG_FLOOD") {
            if let Ok(count) = count.parse::<usize>() {
                workbench.flood_log_active(count);
            }
        }
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
        // LAY-004: the persisted pane sizes and the compact drawer's visible pane are applied
        // before the first frame, so a relaunch paints the layout the user left.
        app.apply_workbench_layout();
        // VIS-002: the persisted theme wins over the dark default; the first frame applies it.
        app.theme = app.settings.theme;
        // A11Y-014 / DOC-009: the persisted ASCII/Unicode choice is the default for documents
        // without a tab-level choice. A restored session carries each tab's own notation instead;
        // only a launch with no session (the starter tabs) takes the file-wide default.
        app.glyph_mode = GlyphMode::from_unicode(app.settings.unicode_glyphs);
        if app.settings.session.is_none() {
            for index in 0..app.workbench.documents().len() {
                app.workbench
                    .set_unicode_glyphs_quietly(index, app.settings.unicode_glyphs);
            }
        }
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
        // LOG-010 acceptance seam: `LOVELACE_LOG_START_SCROLLED=1` starts the log view away from
        // the bottom, which is the state a wheel gesture normally produces. The locked
        // workstation used for window captures accepts no wheel input at all, so without this
        // switch the capture cannot exercise the jump-to-newest control; the switch only sets the
        // starting state, and every control the user then presses is the real one. Off unless
        // set, unreachable from a script, and the same pattern as `LOVELACE_LOG_FLOOD`.
        if std::env::var("LOVELACE_LOG_START_SCROLLED").is_ok() {
            app.log_follow = false;
            app.log_force_follow = false;
            app.log_follow_retries = 0;
            app.log_force_scrolled = true;
        }
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
            conflict: None,
            conflict_buttons: Vec::new(),
            reload_buttons: Vec::new(),
            // Zero so the first pass probes every located document once.
            last_disk_poll: Instant::now(),
            window_focused: false,
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
            loaded_plot: None,
            plot_export: None,
            plot_export_status: None,
            goto_line: None,
            find_symbol: None,
            editor_settings_open: false,
            find_focus: false,
            editor_frac: DEFAULT_EDITOR_FRACTION,
            workspace_frac: DEFAULT_WORKSPACE_FRACTION,
            logs_frac: DEFAULT_LOGS_FRACTION,
            inspector_frac: DEFAULT_INSPECTOR_FRACTION,
            persisted_layout: WorkbenchLayout::default(),
            layout_mode: LayoutMode::Split,
            compact_pane: None,
            compact_editor_frac: COMPACT_EDITOR_DEFAULT,
            compact_tabs: Vec::new(),
            compact_hide: None,
            compact_inspector: None,
            filter: String::new(),
            variables_table,
            functions_table,
            variable_detail: None,
            variable_action_rects: Vec::new(),
            log_selection: std::collections::BTreeSet::new(),
            log_anchor: None,
            log_status: None,
            log_row_rects: Vec::new(),
            log_seen: (0, 0),
            log_follow_total: 0,
            log_follow: true,
            log_unseen: 0,
            log_force_follow: true,
            log_follow_retries: 0,
            log_follow_body: 0.0,
            log_force_scrolled: false,
            log_jump_rect: None,
            log_pinned_rects: Vec::new(),
            expansion: ExpansionState::new(),
            inspector_open: false,
            inspector_expansion: InspectorExpansion::new(),
            inspector_toggle_rects: Vec::new(),
            inspector_control_rects: Vec::new(),
            symbolic_inspector_control_rects: Vec::new(),
            symbolic_inspector_toggle_rects: Vec::new(),
            workspace_view_tabs: [None; 2],
            result_toggle_rects: Vec::new(),
            variable_header_rects: [None; 3],
            function_header_rects: [None; 3],
            quick_eval: QuickEval::new(),
            quick_eval_rect: None,
            precision_input: workbench_precision.to_string(),
            precision_error: None,
            precision_seen: workbench_precision,
            precision_confirm_rects: Vec::new(),
            precision_field_rect: None,
            precision_apply_rect: None,
            layout: None,
            // A11Y-001: the editor owns the keyboard at launch, as it always has.
            focused_pane: Pane::Editor,
            pane_focus: None,
            last_announced_run: None,
            run_button_id: None,
            plot_fit_id: None,
            workspace_tab_ids: [None; 2],
            workspace_filter_id: None,
            shortcuts_open: false,
            shortcuts_focus: false,
            control_matrix_open: false,
            filter_focus: false,
            // A11Y-014: `launch` re-reads this from the loaded settings; the constructor's
            // default is the settings default (ASCII, the engine's own notation).
            glyph_mode: GlyphMode::Ascii,
            glyph_coverage: std::collections::HashMap::new(),
            focus_requested: false,
            ime_chord_deferred: false,
            ime_composition: false,
            ime_composition_owner: None,
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
        // DOC-009: the notation is the active document's own preference, so selecting a tab
        // switches the notation with it instead of one window-wide value leaking across tabs.
        self.glyph_mode = GlyphMode::from_unicode(self.workbench.active_document().unicode_glyphs);
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
        // PLT-005/PLT-009: the plot pane is owned by the active document, and the key that decides
        // whether the pane reloads carries the document id and the run id as well as the bytes.
        // Selecting another tab therefore always replaces or clears the chart; two tabs whose
        // captures happen to be byte-identical still reload, so the pane can never keep painting
        // one tab's chart while another tab is active.
        let (document_id, document_title, run_id) = {
            let active = self.workbench.active_document();
            (
                active.id.0,
                active.title.clone(),
                active.run_id().map(|run| run.0),
            )
        };
        let current_svg = self.view.plot.as_ref().map(|plot| plot.svg.clone());
        let wanted = current_svg
            .as_ref()
            .map(|svg| (document_id, run_id, svg.clone()));
        if wanted != self.loaded_plot {
            match &wanted {
                Some((_, _, svg)) => {
                    self.plot.set_svg(&faulted_capture_svg(svg));
                    self.loaded_plot = wanted;
                }
                None => {
                    self.plot.clear();
                    self.loaded_plot = None;
                }
            }
        }
        // PLT-005: provenance is copied in the same pass that loads the capture, so the header,
        // the canvas's accessible description and the chart itself always name the same document
        // and run. `current_svg` is None for a tab with no capture, and the pane holds no
        // provenance either (it was cleared above).
        self.plot.provenance = current_svg.as_ref().map(|_| plot::PlotProvenance {
            document_id,
            document: document_title,
            run: run_id,
        });
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
        format!(
            "{session:?}|{recent:?}|{precision}|{:?}",
            self.settings.editor
        )
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

    /// LAY-004: copies the persisted workbench layout into the live fractions.
    fn apply_workbench_layout(&mut self) {
        let layout = self.settings.workbench_layout;
        self.editor_frac = layout.editor_permille as f32 / 1000.0;
        self.workspace_frac = layout.workspace_permille as f32 / 1000.0;
        self.logs_frac = layout.logs_permille as f32 / 1000.0;
        self.inspector_frac = layout.inspector_permille as f32 / 1000.0;
        self.compact_editor_frac = layout.compact_editor_permille as f32 / 1000.0;
        self.compact_pane = match layout.compact_pane {
            CompactPaneSetting::Hidden => None,
            CompactPaneSetting::Graph => Some(CompactPane::Graph),
            CompactPaneSetting::Workspace => Some(CompactPane::Workspace),
            CompactPaneSetting::Logs => Some(CompactPane::Logs),
            CompactPaneSetting::Inspector => Some(CompactPane::Inspector),
        };
        self.persisted_layout = layout;
    }

    /// LAY-004: the workbench layout value that represents the live pane sizes.
    fn layout_settings(&self) -> WorkbenchLayout {
        WorkbenchLayout {
            editor_permille: (self.editor_frac * 1000.0).round().clamp(0.0, 1000.0) as u32,
            workspace_permille: (self.workspace_frac * 1000.0).round().clamp(0.0, 1000.0) as u32,
            logs_permille: (self.logs_frac * 1000.0).round().clamp(0.0, 1000.0) as u32,
            inspector_permille: (self.inspector_frac * 1000.0).round().clamp(0.0, 1000.0) as u32,
            compact_editor_permille: (self.compact_editor_frac * 1000.0)
                .round()
                .clamp(0.0, 1000.0) as u32,
            compact_pane: match self.compact_pane {
                None => CompactPaneSetting::Hidden,
                Some(CompactPane::Graph) => CompactPaneSetting::Graph,
                Some(CompactPane::Workspace) => CompactPaneSetting::Workspace,
                Some(CompactPane::Logs) => CompactPaneSetting::Logs,
                Some(CompactPane::Inspector) => CompactPaneSetting::Inspector,
            },
        }
    }

    /// LAY-004: writes the layout to the settings file when a splitter, the reset command or the
    /// compact drawer changed it. Called after the change, not on every painted frame.
    fn persist_layout_if_changed(&mut self) {
        let layout = self.layout_settings();
        if layout == self.persisted_layout {
            return;
        }
        self.settings.workbench_layout = layout;
        self.persisted_layout = layout;
        self.persist_state(true);
    }

    /// LAY-003: restores the shipped pane sizes. Pane visibility is the user's separate choice
    /// and is left as it is; the reset command and a splitter double-click both land here.
    fn reset_pane_sizes(&mut self) {
        self.editor_frac = DEFAULT_EDITOR_FRACTION;
        self.workspace_frac = DEFAULT_WORKSPACE_FRACTION;
        self.logs_frac = DEFAULT_LOGS_FRACTION;
        self.inspector_frac = DEFAULT_INSPECTOR_FRACTION;
        self.compact_editor_frac = COMPACT_EDITOR_DEFAULT;
        self.persist_layout_if_changed();
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
    /// Ctrl+N / File > New: add an empty tab and make it the active document.
    ///
    /// DOC-002: the tab list changes under this pass's editors, so the editors are reconciled
    /// immediately (the same way Open and Close do). Without this the rest of this frame - the
    /// toolbar reads `self.editors[active]` for its line/byte count - indexes a document that has
    /// no editor yet and the click panics the process.
    fn new_document(&mut self, ctx: &egui::Context) {
        let index = self.workbench.new_document();
        self.sync_after_tab_change(index, ctx);
        self.pending_focus = Some(surface_id());
        ctx.request_repaint();
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
    /// A cancelled dialog changes nothing at all. DOC-006: a write the workbench refuses because
    /// the target changed on disk opens the conflict prompt instead; nothing is written.
    fn save_active(&mut self, ask_path: bool, ctx: &egui::Context) {
        let active = self.workbench.active();
        let _ = self.write_document_through_dialogs(active, ask_path, ctx);
    }

    /// Writes one document, asking the native Save As dialog when needed.
    ///
    /// Returns `None` when the user cancelled the dialog (nothing was written and the document is
    /// unchanged) or when the write was refused as a disk conflict (the prompt is now open),
    /// otherwise the workbench's outcome.
    fn write_document_through_dialogs(
        &mut self,
        index: usize,
        ask_path: bool,
        ctx: &egui::Context,
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
        let outcome = if !ask_path && has_path {
            self.workbench.save(index)
        } else {
            let path = self.dialogs.save_as(&suggested, initial_dir.as_deref())?;
            self.workbench.save_as(index, path)
        };
        match outcome {
            SaveOutcome::Conflict(report) => {
                self.open_conflict(index, report, ctx);
                None
            }
            other => Some(other),
        }
    }

    /// DOC-006: opens the prompt for a write the workbench refused.
    fn open_conflict(&mut self, index: usize, report: FileConflict, ctx: &egui::Context) {
        let Some(document) = self.workbench.documents().get(index).map(|doc| doc.id) else {
            return;
        };
        let focus_before = ctx.memory(|memory| memory.focused());
        self.conflict = Some(ConflictPrompt {
            document,
            target: report.path.clone(),
            report,
            focus_before,
            confirm_reload: false,
            error: None,
        });
        // The prompt is painted on the next pass; ask for it even when no input event arrived.
        ctx.request_repaint();
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
        // question of whether journaled text should be recovered. A DOC-006 file conflict is
        // answered first for the same reason: a close must not discard the buffer whose save the
        // conflict prompt is still deciding.
        if self.close.is_some() || self.recovery_prompt.is_some() || self.conflict.is_some() {
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
                match self.write_document_through_dialogs(index, false, ctx) {
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
                    Some(SaveOutcome::Conflict(_)) => {
                        // DOC-006: the file prompt is now the top question. The close prompt
                        // stays queued underneath it and continues once the conflict is settled;
                        // answering Cancel there leaves the tab open.
                        if let Some(prompt) = self.close.as_mut() {
                            prompt.error = Some(
                                "The file changed on disk; answer its prompt first.".to_string(),
                            );
                        }
                    }
                    None => {
                        // The Save As dialog was cancelled or the write was refused as a
                        // conflict. The prompt stays open so the user can still choose Discard
                        // or Cancel.
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
        // DOC-006: the file conflict is a save decision; closing now could discard the buffer.
        if self.conflict.is_some() {
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
                    .font(type_scale::Role::Title.font())
                    .color(theme::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Save changes to {title} before closing?"))
                    .color(theme::text())
                    .font(type_scale::Role::Body.font()),
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
                    let text = RichText::new(choice.label()).font(type_scale::Role::Body.font());
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
                    .font(type_scale::Role::Title.font())
                    .color(theme::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The last session did not close cleanly. These buffers were journaled before \
                     it ended:",
                )
                .color(theme::text())
                .font(type_scale::Role::Body.font()),
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
                            ui.label(
                                RichText::new(&record.title)
                                    .monospace()
                                    .color(theme::accent()),
                            );
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
                                    let text = RichText::new(choice.label())
                                        .font(type_scale::Role::Body.font());
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

    /// DOC-006: re-probes every located document's file and refreshes its tab marker.
    ///
    /// The probe runs when the window regains keyboard focus (the moment the user comes back from
    /// the other program that edited the file) and on a 750 ms poll while it is focused, so the
    /// conflict is visible in the tab strip before the user reaches for Save.
    fn poll_disk_changes(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        let regained = focused && !self.window_focused;
        self.window_focused = focused;
        let now = Instant::now();
        let due = now.duration_since(self.last_disk_poll) >= Duration::from_millis(750);
        if !(regained || due) {
            ctx.request_repaint_after(Duration::from_millis(750));
            return;
        }
        self.last_disk_poll = now;
        if !self.workbench.refresh_disk_status().is_empty() {
            ctx.request_repaint();
        }
        ctx.request_repaint_after(Duration::from_millis(750));
    }

    /// DOC-006: what the open conflict prompt is asking about, for the window tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn conflict_summary(&self) -> Option<ConflictSummary> {
        self.conflict.as_ref().map(|prompt| ConflictSummary {
            document: prompt.document,
            target: prompt.target.clone(),
            confirm_reload: prompt.confirm_reload,
            headline: prompt.report.headline().to_string(),
        })
    }

    /// DOC-006: paints the external-change prompt and applies the answer.
    ///
    /// Like the other prompts it is a foreground modal and the caller strips every key event, so
    /// no keystroke can edit the buffer or trigger a save while the question is open. The prompt is
    /// the only route to Keep mine; Cancel writes nothing and returns the focus that was taken.
    fn conflict_prompt_ui(&mut self, ctx: &egui::Context, escape: bool) {
        self.conflict_buttons.clear();
        self.reload_buttons.clear();
        let Some(prompt) = self.conflict.clone() else {
            return;
        };
        let Some(index) = self.workbench.document_index(prompt.document) else {
            // The document disappeared while the prompt was up; nothing can be saved.
            self.conflict = None;
            return;
        };
        let title = self.workbench.documents()[index].title.clone();
        let dirty = self.workbench.documents()[index].dirty();
        let report = prompt.report.clone();
        let mut answer: Option<ConflictAnswer> = None;
        let mut reload_answer: Option<ReloadAnswer> = None;
        let modal = egui::Modal::new(egui::Id::unique("file-conflict-prompt")).show(ctx, |ui| {
            ui.set_max_width(660.0);
            if prompt.confirm_reload {
                ui.label(
                    RichText::new("Reload from disk?")
                        .strong()
                        .font(type_scale::Role::Title.font())
                        .color(theme::text()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{title} has unsaved changes. Reloading replaces the buffer with the \
                         version on disk and discards those changes."
                    ))
                    .color(theme::text())
                    .font(type_scale::Role::Body.font()),
                );
                ui.label(
                    RichText::new(report.current_line())
                        .small()
                        .color(theme::dim()),
                );
                if let Some(error) = &prompt.error {
                    ui.add_space(2.0);
                    ui.label(RichText::new(error).small().color(theme::error()));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for choice in [ReloadAnswer::DiscardAndReload, ReloadAnswer::Back] {
                        let text =
                            RichText::new(choice.label()).font(type_scale::Role::Body.font());
                        let text = if choice == ReloadAnswer::DiscardAndReload {
                            text.strong()
                        } else {
                            text
                        };
                        let response = ui.add(egui::Button::new(text).min_size(vec2(150.0, 26.0)));
                        if response.clicked() {
                            reload_answer = Some(choice);
                        }
                        self.reload_buttons.push((choice, response.rect));
                    }
                });
            } else {
                ui.label(
                    RichText::new(report.headline())
                        .strong()
                        .font(type_scale::Role::Title.font())
                        .color(theme::text()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(prompt.target.display().to_string())
                        .monospace()
                        .color(theme::text()),
                );
                ui.label(
                    RichText::new(report.recorded_line())
                        .color(theme::text())
                        .font(type_scale::Role::Body.font()),
                );
                ui.label(
                    RichText::new(report.current_line())
                        .color(theme::warning())
                        .font(type_scale::Role::Body.font()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Your buffer: {} bytes{}. Nothing has been written.",
                        report.buffer_bytes,
                        if dirty { ", with unsaved changes" } else { "" }
                    ))
                    .small()
                    .color(theme::dim()),
                );
                if report.other_target {
                    ui.label(
                        RichText::new(
                            "This is a different file from the one this tab read; saving over it \
                             would replace a file you have not seen.",
                        )
                        .small()
                        .color(theme::dim()),
                    );
                }
                if matches!(report.current, crate::document_io::DiskState::Missing) {
                    ui.label(
                        RichText::new(
                            "Reload is unavailable because the file is gone. Keep mine would \
                             create it again deliberately; Cancel leaves the tab open.",
                        )
                        .small()
                        .color(theme::dim()),
                    );
                }
                if let Some(error) = &prompt.error {
                    ui.add_space(2.0);
                    ui.label(RichText::new(error).small().color(theme::error()));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for choice in [
                        ConflictAnswer::Reload,
                        ConflictAnswer::KeepMine,
                        ConflictAnswer::Cancel,
                    ] {
                        let enabled = match choice {
                            ConflictAnswer::Reload => {
                                matches!(report.current, crate::document_io::DiskState::Present(_))
                            }
                            _ => true,
                        };
                        let text =
                            RichText::new(choice.label()).font(type_scale::Role::Body.font());
                        let text = match choice {
                            ConflictAnswer::KeepMine => text.strong(),
                            ConflictAnswer::Reload => text,
                            ConflictAnswer::Cancel => text,
                        };
                        let response = ui.add_enabled(
                            enabled,
                            egui::Button::new(text).min_size(vec2(120.0, 26.0)),
                        );
                        if response.clicked() {
                            answer = Some(choice);
                        }
                        self.conflict_buttons.push((choice, response.rect));
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Escape cancels and writes nothing.")
                        .small()
                        .color(theme::dim()),
                );
            }
        });
        if prompt.confirm_reload {
            if let Some(choice) = reload_answer {
                self.answer_reload(choice, ctx);
            } else if escape || modal.should_close() {
                self.answer_reload(ReloadAnswer::Back, ctx);
            }
            return;
        }
        if answer.is_none() && (escape || modal.should_close()) {
            answer = Some(ConflictAnswer::Cancel);
        }
        if let Some(answer) = answer {
            self.answer_conflict(answer, ctx);
        }
    }

    /// DOC-006: applies one answer to the conflict prompt.
    fn answer_conflict(&mut self, answer: ConflictAnswer, ctx: &egui::Context) {
        let Some(prompt) = self.conflict.clone() else {
            return;
        };
        let Some(index) = self.workbench.document_index(prompt.document) else {
            self.conflict = None;
            return;
        };
        match answer {
            ConflictAnswer::Cancel => {
                // Cancel is deliberately the cheapest branch: nothing is written and no editor,
                // buffer, caret or selection is touched. The focus that was taken is given back.
                self.conflict = None;
                self.pending_focus = Some(prompt.focus_before.unwrap_or_else(surface_id));
                ctx.request_repaint();
            }
            ConflictAnswer::Reload => {
                if !matches!(
                    prompt.report.current,
                    crate::document_io::DiskState::Present(_)
                ) {
                    if let Some(open) = self.conflict.as_mut() {
                        open.error = Some(
                            "The file cannot be reloaded because it is no longer readable."
                                .to_string(),
                        );
                    }
                    ctx.request_repaint();
                    return;
                }
                if self.workbench.documents()[index].dirty() {
                    if let Some(open) = self.conflict.as_mut() {
                        open.confirm_reload = true;
                        open.error = None;
                    }
                } else {
                    self.perform_reload(&prompt, ctx);
                }
                ctx.request_repaint();
            }
            ConflictAnswer::KeepMine => {
                // The user has seen both versions: this is the deliberate overwrite.
                match self.workbench.keep_mine_to(index, prompt.target.clone()) {
                    SaveOutcome::Saved { .. } => {
                        self.conflict = None;
                        self.finish_conflict_flow(&prompt, ctx);
                    }
                    SaveOutcome::Failed(message) => {
                        if let Some(open) = self.conflict.as_mut() {
                            open.error = Some(message);
                        }
                    }
                    other => {
                        let message = format!("Save was refused: {other:?}");
                        if let Some(open) = self.conflict.as_mut() {
                            open.error = Some(message);
                        }
                    }
                }
                ctx.request_repaint();
            }
        }
    }

    /// DOC-006: answers the dirty-buffer Reload confirmation.
    fn answer_reload(&mut self, choice: ReloadAnswer, ctx: &egui::Context) {
        let Some(prompt) = self.conflict.clone() else {
            return;
        };
        match choice {
            ReloadAnswer::Back => {
                if let Some(open) = self.conflict.as_mut() {
                    open.confirm_reload = false;
                    open.error = None;
                }
            }
            ReloadAnswer::DiscardAndReload => self.perform_reload(&prompt, ctx),
        }
        ctx.request_repaint();
    }

    /// DOC-006: loads the disk version into the tab and closes the prompt on success.
    fn perform_reload(&mut self, prompt: &ConflictPrompt, ctx: &egui::Context) {
        let Some(index) = self.workbench.document_index(prompt.document) else {
            self.conflict = None;
            return;
        };
        match self.workbench.reload(index) {
            Ok(()) => {
                self.conflict = None;
                self.finish_conflict_flow(prompt, ctx);
            }
            Err(message) => {
                if let Some(open) = self.conflict.as_mut() {
                    open.error = Some(message);
                    open.confirm_reload = false;
                }
            }
        }
        ctx.request_repaint();
    }

    /// DOC-006: after Keep mine or Reload, continues a close flow that was waiting on the save.
    fn finish_conflict_flow(&mut self, prompt: &ConflictPrompt, ctx: &egui::Context) {
        // A close prompt's Save was the write that hit the conflict. Keep mine saved it and
        // Reload made the buffer match disk, so in both cases the tab may now be closed exactly
        // as the close flow intended. Cancel never reaches this function.
        if self
            .close
            .as_ref()
            .is_some_and(|close| close.current() == prompt.document)
        {
            self.advance_close(ctx);
        }
        self.pending_focus = Some(prompt.focus_before.unwrap_or_else(surface_id));
        ctx.request_repaint();
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
            FileCommand::New => self.new_document(ctx),
            FileCommand::Open => self.open_dialog(ctx),
            FileCommand::Save => self.save_active(false, ctx),
            FileCommand::SaveAs => self.save_active(true, ctx),
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
        // LAY-011: a keyboard pane jump must also reveal the pane it focuses; in the compact
        // layout Plot, Workspace and Logs live behind the drawer.
        if self.layout_mode == LayoutMode::Compact {
            match pane {
                Pane::Plot => self.compact_pane = Some(CompactPane::Graph),
                Pane::Workspace => self.compact_pane = Some(CompactPane::Workspace),
                Pane::Logs => self.compact_pane = Some(CompactPane::Logs),
                Pane::Toolbar | Pane::Editor => {}
            }
        }
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

    /// DOC-009 / A11Y-014: switches the active document between ASCII and Unicode notation.
    ///
    /// The preference belongs to the tab: the value is carried in the session, so a restart and a
    /// tab switch both restore this tab's own notation instead of cross-talking with another tab.
    /// `settings.unicode_glyphs` stays the default a newly created tab inherits, and it is
    /// persisted immediately.
    fn set_unicode_glyphs(&mut self, enabled: bool, ctx: &egui::Context) {
        if self.workbench.active_unicode_glyphs() == enabled {
            return;
        }
        self.workbench
            .set_unicode_glyphs(self.workbench.active(), enabled);
        self.settings.unicode_glyphs = enabled;
        self.glyph_mode = GlyphMode::from_unicode(enabled);
        self.workbench.log_active(
            LogKind::Info,
            format!("Mathematical glyphs: {} notation", self.glyph_mode.name()),
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
    fn dispatch_binding(
        &mut self,
        binding: &'static keyboard::Binding,
        ctx: &egui::Context,
    ) -> bool {
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
                if self.layout_mode == LayoutMode::Compact {
                    self.compact_pane = Some(CompactPane::Workspace);
                }
                self.request_pane_focus(Pane::Workspace, ctx);
                if let Some(id) = self.workspace_tab_ids[0] {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
                true
            }
            Action::WorkspaceInspectorView => {
                self.inspector_open = true;
                if self.layout_mode == LayoutMode::Compact {
                    self.compact_pane = Some(CompactPane::Workspace);
                }
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
            // LOG-005: the keyboard half of the Logs pane's copy and export commands.
            Action::CopyLogSelection => {
                self.copy_log(LogCopy::Selection, ctx);
                true
            }
            Action::CopyLogAll => {
                self.copy_log(LogCopy::All, ctx);
                true
            }
            Action::ClearLog => {
                self.workbench.clear_log_active();
                self.log_selection.clear();
                self.log_anchor = None;
                self.log_status = Some("log cleared".to_string());
                true
            }
            Action::SaveLog => {
                self.save_log();
                true
            }
            // LAY-003: the keyboard/command half of "double-click or command restores sensible
            // defaults"; the splitter double-click calls the same method.
            Action::ResetLayout => {
                self.reset_pane_sizes();
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
                let enabled = !self.workbench.active_unicode_glyphs();
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

    /// A11Y-008: withholds the keyboard from every accelerator while an IME composition holds it.
    ///
    /// Called once, before any pane or shortcut table reads this pass's events. While the
    /// composition is active every key, text, paste, copy and cut event is dropped, so Ctrl+Enter,
    /// Ctrl+Space, Ctrl+Shift+* and the F-keys reach the IME and no window/editor handler sees
    /// them. After the composition ends the drop is held until a pass carries neither a
    /// composition nor a command chord: a chord whose key events the platform split across passes
    /// cannot surface later, so the pending chord is never replayed. Plain typing is not withheld
    /// by the latch, so the first character typed after a commit is not lost.
    fn defer_ime_shortcuts(&mut self, ui: &Ui) {
        let ctx = ui.ctx().clone();
        let focus = ctx.memory(|memory| memory.focused());
        // A composition belongs to one widget; moving focus ends it even when the platform sends
        // no event for the widget that lost it.
        if self.ime_composition && self.ime_composition_owner != focus {
            self.ime_composition = false;
            self.ime_composition_owner = None;
        }
        // The pass's IME events are the window's only view of a composition in a widget other than
        // the editor (the quick-eval `TextEdit` keeps its pre-edit private). The last IME event of
        // the pass wins, which matches the platform sequence Preedit("") -> Commit.
        let update = ctx.input(|input| {
            let mut update = None;
            for event in &input.events {
                if let egui::Event::Ime(ime) = event {
                    match ime {
                        egui::ImeEvent::Preedit { text, .. } => update = Some(!text.is_empty()),
                        egui::ImeEvent::Commit(_) | egui::ImeEvent::DeleteSurrounding { .. } => {
                            update = Some(false);
                        }
                        _ => {}
                    }
                }
            }
            update
        });
        if update == Some(true) && focus.is_some() {
            self.ime_composition = true;
            self.ime_composition_owner = focus;
        } else if update == Some(false) {
            self.ime_composition = false;
            self.ime_composition_owner = None;
        }
        // The editor paints its own pre-edit from the same events; including it keeps the rule
        // correct for a caller that drives the editor widget without the window's event scan.
        let composing = self.ime_composition
            || self
                .editors
                .get(self.workbench.active())
                .is_some_and(|editor| editor.preedit.is_some());
        self.ime_chord_deferred |= composing;
        if !self.ime_chord_deferred {
            return;
        }
        // A held command modifier turns every key event into a potential accelerator, including a
        // bare Enter that the window's frame-modifier fallback would read as Ctrl+Enter.
        let command_held = ctx.input(|input| input.modifiers.command || input.modifiers.ctrl);
        // The window registers F1, F6 and F8, but a function key is never composition input:
        // defer every `F<n>` while the chord latch is held, not only the currently bound ones.
        let is_function_key = |key: Key| {
            let name = key.name();
            name.len() > 1
                && name.starts_with('F')
                && name[1..].bytes().all(|byte| byte.is_ascii_digit())
        };
        let mut deferred_chord = false;
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                let chord = match event {
                    egui::Event::Key { key, modifiers, .. } => {
                        command_held
                            || modifiers.command
                            || modifiers.ctrl
                            || modifiers.alt
                            || is_function_key(*key)
                    }
                    _ => false,
                };
                deferred_chord |= chord;
                let typing = composing
                    && matches!(
                        event,
                        egui::Event::Key { .. }
                            | egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Copy
                            | egui::Event::Cut
                    );
                !(chord || typing)
            });
        });
        // The latch survives the pass that ends the composition; a pass with neither a composition
        // nor a deferred chord releases it, so the next deliberate shortcut works normally.
        if !composing && !deferred_chord {
            self.ime_chord_deferred = false;
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

    /// ARC-007: submit the active document's run through the same path the Run button uses.
    pub(crate) fn measure_begin_run(&mut self) -> bool {
        self.workbench.run_active()
    }

    /// ARC-007: the active document's run state, read from the workbench.
    pub(crate) fn measure_run_state(&self) -> crate::run_state::RunState {
        self.workbench.active_document().state()
    }

    /// ARC-007: the active run id, so a capture can name the run that produced it.
    pub(crate) fn measure_run_id(&self) -> Option<crate::execution::RunId> {
        self.workbench.active_document().run_id()
    }

    /// ARC-007: the committed result shown by the panes, when one exists.
    pub(crate) fn measure_result(&self) -> Option<String> {
        self.workbench
            .active_document()
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.clone())
    }

    /// ARC-007: the keyboard focus group currently active, read after an injected F6.
    pub(crate) fn measure_focused_pane(&self) -> Pane {
        self.focused_pane
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
                            let label = format!("{}   {}:{}", hit.name, hit.row + 1, hit.col + 1);
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
                    .checkbox(
                        &mut preferences.auto_close,
                        "Auto-close brackets and quotes",
                    )
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
                let mut unicode = self.workbench.active_unicode_glyphs();
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
                    RichText::new(format!("Values render as {}", self.glyph_mode.label()))
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
                            settings::MIN_LINE_HEIGHT_PERCENT..=settings::MAX_LINE_HEIGHT_PERCENT,
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
                    .checkbox(&mut theme_follows_system, "Follow the OS light/dark theme")
                    .on_hover_text(
                        "On: an OS application-theme change repaints the window, keeping the \
                         document, the caret, the run and the focus. Off: the toolbar's Dark/Light \
                         choice is an explicit override.",
                    )
                    .changed();
                ui.label(
                    RichText::new(if theme_follows_system {
                        format!(
                            "Following the OS theme (currently {})",
                            self.theme_button_label()
                        )
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
        if self.layout_mode != LayoutMode::Split {
            return self.file_bar_compact(ui);
        }
        let ctx = ui.ctx().clone();
        self.file_buttons.clear();
        // A11Y-004: the file bar wraps instead of running under the window edge at a high scale.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("File").small().color(theme::dim()));
            if self.file_bar_button(ui, "New", "New document (Ctrl+N)") {
                self.new_document(&ctx);
            }
            if self.file_bar_button(ui, "Open...", "Open a Lovelace script from disk (Ctrl+O)") {
                self.open_dialog(&ctx);
            }
            if self.file_bar_button(
                ui,
                "Save",
                "Save the active document (Ctrl+S); asks for a path when it has none",
            ) {
                self.save_active(false, &ctx);
            }
            if self.file_bar_button(
                ui,
                "Save As...",
                "Save the active document under a new name (Ctrl+Shift+S)",
            ) {
                self.save_active(true, &ctx);
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

    /// LAY-011: the one-row file bar used when the compact layout is in force.
    ///
    /// The long path label is what makes the full bar wrap to two or three rows at 200%; here it
    /// is truncated to whatever width remains. Every action of the full bar is still one click
    /// away: Save stays visible and the rest live in the File menu (whose items are ordinary
    /// buttons once it is open, so UI Automation finds them by name).
    fn file_bar_compact(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.file_buttons.clear();
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                ui.set_min_width(220.0);
                if ui.button("New").clicked() {
                    self.apply_file_command(FileCommand::New, &ctx);
                    ui.close();
                }
                if ui.button("Open...").clicked() {
                    self.apply_file_command(FileCommand::Open, &ctx);
                    ui.close();
                }
                if ui.button("Save As...").clicked() {
                    self.apply_file_command(FileCommand::SaveAs, &ctx);
                    ui.close();
                }
                let recent = self.workbench.recent_files().to_vec();
                if recent.is_empty() {
                    let recent = ui.add_enabled(false, egui::Button::new("Recent"));
                    // VIS-006: no recent file yet; the disabled command shows its dashed/hatched
                    // not-actionable cue rather than a half-opaque label.
                    mark_disabled(ui, &recent);
                } else {
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
                    });
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    self.apply_file_command(FileCommand::CloseTab, &ctx);
                    ui.close();
                }
                if ui.button("Quit").clicked() {
                    self.request_close(CloseTarget::Window, &ctx);
                    ui.close();
                }
            });
            if self.file_bar_button(
                ui,
                "Save",
                "Save the active document (Ctrl+S); asks for a path when it has none",
            ) {
                self.save_active(false, &ctx);
            }
            let document = self.workbench.active_document();
            ui.label(
                RichText::new(document.encoding_label())
                    .monospace()
                    .small()
                    .color(theme::dim()),
            )
            .on_hover_text("The encoding and newline style this document saves with");
            let name = document.tab_label();
            ui.add(
                egui::Label::new(RichText::new(name).monospace().small().color(theme::dim()))
                    .truncate(),
            );
        });
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
        // VIS-004: the semibold face the strong roles paint with is registered before any galley
        // is laid out, for the same reason: a named family that is not registered has no glyphs.
        type_scale::ensure_installed(ui.ctx());
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
        // DOC-006: external changes are noticed when the window regains focus and on a short
        // poll, so the tab strip warns before the user tries to save over another program's edit.
        self.poll_disk_changes(&ctx);
        // While a prompt is open nothing in this frame may consume a keystroke: read the keys the
        // prompt itself uses, then strip every key and text event so no editor can be edited
        // behind the modal. The modal's buttons still receive pointer events, and the prompts are
        // drawn in egui's foreground layer even though they are built before the panes.
        if self.close.is_some() || self.recovery_prompt.is_some() || self.conflict.is_some() {
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
            // can offer to overwrite anything. The conflict prompt (DOC-006) is next: it is the
            // question the close prompt's Save produced. Only one is painted at a time.
            if self.recovery_prompt.is_some() {
                self.recovery_prompt_ui(&ctx, escape);
            } else if self.conflict.is_some() {
                self.conflict_prompt_ui(&ctx, escape);
            } else {
                self.close_prompt_ui(&ctx, escape, enter);
            }
        }

        // A11Y-008: an IME composition owns the keyboard. This runs before the first shortcut
        // table below, so Ctrl+Enter/Escape, the quick-eval and editor chords, the file and
        // navigation commands, the F-keys and the binding table all see a pass with those events
        // removed while the composition is active — and a chord that arrives just after the
        // platform ends the composition is still withheld instead of being applied late.
        self.defer_ime_shortcuts(ui);

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
            let editor_focused = ui.ctx().memory(|memory| memory.focused()) == Some(surface_id());
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

        // LAY-011: the layout is chosen from the client size in points before any chrome is
        // drawn, so the compact chrome (one row per bar) and the compact workbench are decided
        // together. 900x600 and 1280x800 at 100% keep the split; the same windows at a scale that
        // leaves fewer than 760x340 points use the compact layout instead of overflowing.
        self.layout_mode =
            LayoutMode::for_window(ui.max_rect().size(), ui.ctx().pixels_per_point());

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
        // PERF-011: the precision cost estimate, over the toolbar it belongs to. It is painted
        // after every pane, like the other windows, so a click on it cannot also reach a control
        // underneath.
        self.precision_estimate_window(ui.ctx());
        // VIS-006: the control state matrix, a real window over every pane.
        let mut matrix_open = self.control_matrix_open;
        control::matrix_window(ui.ctx(), &mut matrix_open);
        self.control_matrix_open = matrix_open;
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
        if self.layout_mode != LayoutMode::Split {
            return self.toolbar_compact(ui);
        }
        // A11Y-004: at 125-200% the toolbar's controls wrap onto extra rows instead of being
        // clipped by the window edge.
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Lovelace Desktop")
                    .strong()
                    .color(theme::text())
                    .font(type_scale::Role::Title.font()),
            );
            ui.label(RichText::new(self.workbench.engine_name()).color(theme::dim()));
            ui.separator();

            self.run_precision_controls(ui);
            ui.separator();

            if ui
                .button("Editor settings")
                .on_hover_text("Indent width, tabs vs spaces and bracket/quote auto-close (Ctrl+,)")
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
                // VIS-006: the in-window specimen of every control state, so the state table is
                // reviewable and capturable from the shipped product.
                if ui.button("Control states (VIS-006)").clicked() {
                    self.control_matrix_open = true;
                    ui.close();
                }
                ui.separator();
                // LAY-003: the visible command that restores the shipped pane sizes. The same
                // operation is a splitter double-click and the Ctrl+Shift+L binding.
                if ui
                    .button(format!(
                        "Reset pane sizes ({})",
                        keyboard::chord_of(Action::ResetLayout)
                    ))
                    .on_hover_text(
                        "Restore the default editor, graph, workspace, logs and inspector sizes",
                    )
                    .clicked()
                {
                    self.reset_pane_sizes();
                    ui.close();
                }
                ui.separator();
                let mut unicode = self.workbench.active_unicode_glyphs();
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
            let theme_response = ui.button(self.theme_button_label()).on_hover_text(
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

    /// LAY-011: the shared Run / Cancel / Precision group.
    ///
    /// Extracted so the normal and compact toolbars show the same controls, with the same ids,
    /// shortcuts and validation: the compact layout must never make Run, Cancel or the precision
    /// field harder to reach than the split layout does.
    fn run_precision_controls(&mut self, ui: &mut Ui) {
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
        // VIS-006: a disabled Run is not merely translucent; it carries the dashed outline and
        // hatch that make "not actionable" visible without relying on alpha.
        if !toolbar.run_enabled {
            mark_disabled(ui, &run);
        }
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
            if !toolbar.cancel_enabled {
                mark_disabled(ui, &cancel);
            }
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
        self.precision_field_rect = Some(field.rect);
        let apply = ui
            .add(egui::Button::new("Apply"))
            .on_hover_text("Apply the typed precision to the active document");
        self.precision_apply_rect = Some(apply.rect);
        let submitted = apply.clicked()
            || (field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        if submitted {
            // PERF-011: above the documented threshold the workbench returns the measured
            // estimate instead of applying the value. The dialog is opened by the pane's pass and
            // the change happens only through its Set precision control.
            match self
                .workbench
                .request_active_precision(&self.precision_input)
            {
                crate::workbench::PrecisionRequest::Applied(decimals) => {
                    self.precision_seen = decimals;
                    self.precision_input = decimals.to_string();
                    self.precision_error = None;
                }
                crate::workbench::PrecisionRequest::Invalid(guidance) => {
                    self.precision_error = Some(guidance);
                }
                crate::workbench::PrecisionRequest::Confirm(_) => {
                    self.precision_error = None;
                }
                crate::workbench::PrecisionRequest::Refused { reason, .. } => {
                    self.precision_error = Some(reason);
                }
            }
        }
        ui.label(RichText::new(format!("{} digits", self.view.precision)).color(theme::dim()))
            .on_hover_text("The active document's applied display precision");
        // PERF-011: the live estimate, painted before the change: a typed value above the
        // threshold shows what the measured curve says it will cost.
        if let Some(digits) = self.typed_precision_above_threshold() {
            if let Some(curve) = self.workbench.precision_curve() {
                let cost = curve.cost(digits);
                ui.label(
                    RichText::new(format!("estimate {}", cost.estimate_text()))
                        .small()
                        .color(theme::warning()),
                )
                .on_hover_text(format!(
                    "{}. {}",
                    cost.line(),
                    curve.measured_line()
                ));
            } else {
                ui.label(
                    RichText::new("measuring the engine's cost…")
                        .small()
                        .color(theme::dim()),
                )
                .on_hover_text(
                    "No estimate is offered until the engine's digits-to-time curve has been \
                     measured; the precision is not changed meanwhile.",
                );
            }
        }
        if let Some(guidance) = &self.precision_error {
            ui.label(RichText::new(guidance).color(theme::error()))
                .on_hover_text("The precision was not changed; the run uses the last valid value.");
        }
    }

    /// PERF-011: the typed precision when it is a valid value above the estimate threshold.
    fn typed_precision_above_threshold(&self) -> Option<usize> {
        let digits = crate::workbench::parse_precision(&self.precision_input).ok()?;
        (digits > crate::workbench::PRECISION_WARN_DIGITS).then_some(digits)
    }

    /// LAY-011: the one-row toolbar used when the compact layout is in force.
    ///
    /// Run, Cancel and the precision field stay visible; the navigation, Help and Theme controls
    /// move into the Actions menu, one click away and still named for UI Automation. The editor's
    /// own row/byte counters move to the status strip so they do not force a second row.
    fn toolbar_compact(&mut self, ui: &mut Ui) {
        // LAY-011: with Cancel on the row the compact toolbar is at its widest. At 200% in the
        // 900x600 minimum window the default 8 pt gap pushed the Actions menu 16 px past the
        // client edge while a run was in flight (round 61). Four points between controls keeps the
        // whole row - including Actions - inside the client without dropping any control.
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.horizontal(|ui| {
            self.run_precision_controls(ui);
            ui.separator();
            ui.menu_button("Actions", |ui| {
                ui.set_min_width(240.0);
                if ui.button("Editor settings").clicked() {
                    self.editor_settings_open = true;
                    ui.close();
                }
                if ui.button("Go to line").clicked() {
                    self.find_symbol = None;
                    self.goto_line = Some(GoToLinePrompt {
                        text: String::new(),
                        error: None,
                        focus: true,
                    });
                    ui.close();
                }
                if ui.button("Find symbol").clicked() {
                    self.goto_line = None;
                    self.find_symbol = Some(SymbolPrompt {
                        query: String::new(),
                        selected: 0,
                        focus: true,
                    });
                    ui.close();
                }
                if ui.button("Find...").clicked() {
                    let active = self.workbench.active();
                    self.editors[active].find_open();
                    self.find_focus = true;
                    ui.close();
                }
                ui.separator();
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
                if ui.button("Control states (VIS-006)").clicked() {
                    self.control_matrix_open = true;
                    ui.close();
                }
                // LAY-003: the compact layout's visible reset command, beside the shortcuts entry.
                if ui
                    .button(format!(
                        "Reset pane sizes ({})",
                        keyboard::chord_of(Action::ResetLayout)
                    ))
                    .clicked()
                {
                    self.reset_pane_sizes();
                    ui.close();
                }
                let mut unicode = self.workbench.active_unicode_glyphs();
                if ui
                    .checkbox(&mut unicode, "Unicode mathematical glyphs")
                    .changed()
                {
                    self.set_unicode_glyphs(unicode, ui.ctx());
                }
                ui.separator();
                let theme_label = self.theme_button_label();
                if ui.button(theme_label).clicked() {
                    let next = self.theme.toggled();
                    self.set_theme(next, ui.ctx());
                    ui.close();
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
            })
            .response
            .on_hover_text("Editor navigation, Help, the Unicode setting and the theme");
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
                response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Tab, true, name.clone()));
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
        if self.layout_mode != LayoutMode::Split {
            return self.status_bar_compact(ui);
        }
        let active = self.workbench.active();
        let (line, column) = self.editors[active].line_col_1based();
        let status = self.workbench.status_text(line, column);
        // A11Y-012 / VIS-007: the run outcome carries the semantic role's text marker, not only
        // a colour. `[ERR]` and `[OK]` are readable in greyscale, by a screen reader, and in the
        // UIA name below; the running states now carry `[RUN]` and a cancelled run
        // `[CANCELLED]`, so no outcome is distinguished by colour alone.
        let outcome = semantic::SemanticRole::for_run_state(self.view.run_state);
        let marker = outcome.map(|role| role.marker());
        let diagnostic = self.view.diagnostic.clone();
        let state = self.view.run_state;
        // TST-011 / J-19: a run-state transition is the one status change a screen reader must
        // hear. `last_announced_run` makes the status leaf a live region on exactly that pass.
        let announce_status = self.last_announced_run != Some(state);
        self.last_announced_run = Some(state);
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
        let state_color = status_state_color(state);
        let total = self.view.total.max(1);
        let committed = self.view.committed_steps.min(total);
        let last_run_ms = self
            .workbench
            .documents()
            .get(active)
            .map(|document| document.elapsed_ms);
        let running = self.workbench.active_is_running();
        let focus = self.focused_pane.label();
        // A11Y-004: the strip is exactly one row high at every supported client size. The long
        // document summary is the only variable-length piece, so it is painted in the shrinking
        // side and elides to the room that is left; the fixed readouts (progress, last-run time,
        // focus group) keep their place at the right. `egui::Panel` measures a bottom panel's
        // height one pass after painting it, so a row that wrapped *down* would let that pass's
        // controls hang below the client edge: the wrapped row is what this layout removes, not
        // the facts (the full text is still the `Status` node's name and the label's tooltip).
        egui::Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                if let Some(marker) = marker {
                    ui.label(
                        RichText::new(marker)
                            .font(type_scale::Role::Badge.font())
                            .color(state_color),
                    )
                    .on_hover_text(if diagnostic.is_some() {
                        "The last run failed; the log names its line and column"
                    } else {
                        "The last run's outcome (the marker is text, not only colour)"
                    });
                }
                // A11Y-012: a failed run's status names the error's location in text. The
                // colour is redundant with the marker and the words. It is placed before the
                // summary because it is short and must never be the part that elides.
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
                // VIS-002: the run state's semantic colour is a theme token, not a fixed
                // accent. This label is last in the shrinking side, so it is the one that
                // truncates when the client is narrower than the facts.
                let mut visible = status.clone();
                if running {
                    visible.push_str("  |  Cancel is available in the toolbar.");
                }
                let status_line = ui
                    .add(
                        egui::Label::new(RichText::new(visible).monospace().color(state_color))
                            .truncate()
                            .show_tooltip_when_elided(false),
                    )
                    .on_hover_text(&label);
                // TST-011 / J-19: the run-state transition is the status strip's announcement.
                // The live region is this leaf, not the strip's container: AccessKit inherits
                // `live` from the nearest ancestor (accesskit_consumer `Node::live`), so a
                // live container would also announce the marker, the last-run time and the
                // focus group. A leaf announces one status line per transition, and only on
                // the transition itself - a permanently live leaf would re-speak the caret.
                if announce_status {
                    let id = status_line.id;
                    ui.ctx().accesskit_node_builder(id, |node| {
                        node.set_live(egui::accesskit::Live::Polite);
                    });
                }
            },
            |ui| {
                // A11Y-001: the status strip names the focus group, so a keyboard-only user
                // (and a UIA capture) can tell which pane F6 is in without reading the
                // ring's position. Right-to-left: the first item added is the rightmost.
                // A right-to-left Ui lays a galley out right-aligned, which anchors
                // `TextShape::pos` at the rect's right edge; a left-aligned galley in a rect
                // that is exactly the text's width paints the same picture and keeps the
                // shape's origin at its left edge. The painted shapes a11y_004 measures are
                // read from that origin, so keep the labels on the convention they measure.
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("focus: {focus}"))
                            .small()
                            .color(theme::dim()),
                    )
                    .halign(egui::Align::LEFT),
                )
                .on_hover_text(
                    "The pane group the keyboard is in (F6 moves it; Ctrl+1..5 jump directly)",
                );
                if let Some(last_run_ms) = last_run_ms {
                    ui.separator();
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{last_run_ms} ms last run")).color(theme::dim()),
                        )
                        .halign(egui::Align::LEFT),
                    );
                }
                // A11Y-002: the committed progress is a real `ProgressIndicator` with a
                // numeric value, not text a reader has to parse out of the status string.
                // egui fills the node's value with a percentage; the explicit range below
                // states the numbers a screen reader can act on (committed statements out of
                // the run's total).
                ui.separator();
                let progress = ui.add(
                    egui::ProgressBar::new(committed as f32 / total as f32)
                        .desired_width(76.0)
                        .text(format!("{committed}/{total}")),
                );
                ui.ctx().accesskit_node_builder(progress.id, |node| {
                    node.set_min_numeric_value(0.0);
                    node.set_max_numeric_value(total as f64);
                    node.set_numeric_value(committed as f64);
                    // TST-011 / J-19: the committed count is progress a screen reader hears.
                    // The node's name only changes when a statement commits (or a new run's
                    // total appears), so a polite live region here cannot echo the caret.
                    node.set_live(egui::accesskit::Live::Polite);
                });
                progress.on_hover_text("Committed statements of the last run's snapshot");
                // RUN-005: the in-flight statement's kernel sub-progress, painted as its OWN
                // readout beside the statement count. The count is statements committed out of
                // the run's total; this is the engine's phase and the kernel's own percentage
                // of its own work, so a single long `sqrt(2)`/`pi(n)`/`n!` statement no longer
                // looks hung at `1/1`. Absent whenever the engine published none (a short
                // statement, or an evaluator with no kernel progress seam).
                if let Some(kernel) = self.workbench.active_document().kernel_progress().cloned() {
                    ui.separator();
                    let kernel_bar = ui.add(
                        egui::ProgressBar::new(kernel.fraction.clamp(0.0, 1.0))
                            .desired_width(112.0)
                            .text(format!("kernel {}", kernel.label())),
                    );
                    ui.ctx().accesskit_node_builder(kernel_bar.id, |node| {
                        node.set_min_numeric_value(0.0);
                        node.set_max_numeric_value(1.0);
                        node.set_numeric_value(kernel.fraction as f64);
                    });
                    kernel_bar.on_hover_text(
                        "Kernel sub-progress reported by the running statement's engine \
                             (RUN-005): the phase the kernel is in and its own percentage of its \
                             own work",
                    );
                }
            },
        );
    }

    /// LAY-011: the one-row status strip used when the compact layout is in force.
    ///
    /// The accessible status name still carries the whole state (marker, file, revision,
    /// precision, caret, run state and the committed count); the visible row states the same
    /// facts compactly enough to stay on one line at the minimum window.
    fn status_bar_compact(&mut self, ui: &mut Ui) {
        let active = self.workbench.active();
        let (line, column) = self.editors[active].line_col_1based();
        let status = self.workbench.status_text(line, column);
        // VIS-007: the compact strip carries the same semantic marker and colour as the full one.
        let marker =
            semantic::SemanticRole::for_run_state(self.view.run_state).map(|role| role.marker());
        let diagnostic = self.view.diagnostic.clone();
        // TST-011 / J-19: the compact layout announces the run-state transition through its own
        // status leaf; the split layout's strip is not painted in this mode.
        let announce_status = self.last_announced_run != Some(self.view.run_state);
        self.last_announced_run = Some(self.view.run_state);
        let mut accessible = match marker {
            Some(marker) => format!("{marker} {status}"),
            None => status.clone(),
        };
        if let Some(diagnostic) = &diagnostic {
            accessible.push_str(&format!(
                " | error at Ln {}, Col {}: {}",
                diagnostic.line, diagnostic.column, diagnostic.message
            ));
        }
        label_ui_node_as(ui, egui::Role::Status, &accessible);
        let title = self.workbench.active_document().tab_label();
        let state = self.view.run_state.label();
        let mut short = match marker {
            Some(marker) => format!("{marker} {title} · Ln {line}, Col {column} · {state}"),
            None => format!("{title} · Ln {line}, Col {column} · {state}"),
        };
        if self.view.total > 0 {
            short.push_str(&format!(
                " · {}/{}",
                self.view.committed_steps.min(self.view.total),
                self.view.total
            ));
        }
        // RUN-005: the compact strip carries the same kernel segment the full strip paints, so a
        // narrow window still shows the running kernel's phase beside the statement count.
        if let Some(kernel) = self.workbench.active_document().kernel_progress().cloned() {
            short.push_str(&format!(" · kernel {}", kernel.label()));
        }
        ui.horizontal(|ui| {
            let compact_line = ui.label(
                RichText::new(short)
                    .monospace()
                    .small()
                    .color(status_state_color(self.view.run_state)),
            );
            if announce_status {
                let id = compact_line.id;
                ui.ctx().accesskit_node_builder(id, |node| {
                    node.set_live(egui::accesskit::Live::Polite);
                });
            }
            if let Some(diagnostic) = &diagnostic {
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "error at Ln {}, Col {}",
                        diagnostic.line, diagnostic.column
                    ))
                    .monospace()
                    .small()
                    .color(theme::error()),
                );
            }
        });
    }

    fn panes(&mut self, ui: &mut Ui, full: Rect) {
        ui.painter().rect_filled(full, 0.0, theme::app_bg());
        // RUN-003: one view for the whole frame. The clone is what a pane closure can borrow
        // while another pane mutates `self`; no pane reads pane data from the workbench.
        let view = self.view.clone();
        match self.layout_mode {
            LayoutMode::Split | LayoutMode::DenseSplit => {
                let dense = self.layout_mode == LayoutMode::DenseSplit;
                let layout = compute_layout(
                    full,
                    self.editor_frac,
                    self.workspace_frac,
                    self.logs_frac,
                    self.inspector_frac,
                    dense,
                );
                self.layout = Some(layout);
                self.compact_tabs.clear();
                self.compact_hide = None;
                self.compact_inspector = None;
                self.editor_pane(ui, layout.editor, &view);
                self.plot_pane(ui, layout.plot, &view);
                self.workspace_pane(ui, layout.workspace, &view);
                self.logs_pane(ui, layout.logs, &view);
                self.symbolic_inspector_pane(ui, layout.inspector, &view);

                let drag_a = splitter(
                    ui,
                    layout.split_a,
                    "split_editor",
                    true,
                    "Editor / graph splitter",
                );
                let drag_b = splitter(
                    ui,
                    layout.split_b,
                    "split_workspace",
                    true,
                    "Graph / workspace splitter",
                );
                let drag_c = splitter(
                    ui,
                    layout.split_c,
                    "split_logs",
                    false,
                    "Top row / bottom strip splitter",
                );
                let drag_d = splitter(
                    ui,
                    layout.split_d,
                    "split_inspector",
                    true,
                    "Logs / Symbolic Inspector splitter",
                );

                if drag_a.delta != 0.0 {
                    self.editor_frac =
                        (self.editor_frac + drag_a.delta / full.width().max(1.0)).clamp(0.02, 0.95);
                }
                if drag_b.delta != 0.0 {
                    self.workspace_frac = (self.workspace_frac
                        - drag_b.delta / full.width().max(1.0))
                    .clamp(0.02, 0.95);
                }
                if drag_c.delta != 0.0 {
                    self.logs_frac =
                        (self.logs_frac - drag_c.delta / full.height().max(1.0)).clamp(0.02, 0.9);
                }
                if drag_d.delta != 0.0 {
                    // The inspector sits to the right of its divider, so a rightward drag must
                    // shrink its fraction: the divider follows the pointer.
                    self.inspector_frac = (self.inspector_frac
                        - drag_d.delta / full.width().max(1.0))
                    .clamp(0.05, 0.95);
                }
                // LAY-003: a double-click restores the shipped pane sizes; a completed drag or a
                // keyboard resize writes the new sizes. LAY-004 persists exactly here, once per
                // gesture rather than once per painted frame.
                let outcomes = [drag_a, drag_b, drag_c, drag_d];
                if outcomes.iter().any(|outcome| outcome.reset) {
                    self.reset_pane_sizes();
                } else if outcomes
                    .iter()
                    .any(|outcome| outcome.drag_stopped || outcome.keyboard)
                {
                    self.persist_layout_if_changed();
                }
            }
            LayoutMode::Compact => self.compact_panes(ui, full, &view),
        }
    }

    /// LAY-011: the minimum-window layout.
    ///
    /// The editor stays on the left and keeps at least [`COMPACT_EDITOR_MIN`] of the width whether
    /// the drawer is open or closed. The ancillary panes live in one drawer selected from the tab
    /// row, so nothing is squeezed into a column too narrow to read and no pane can be pushed out
    /// of the window. Every pane's rectangle is inside `full`, and the drawer is one click away.
    fn compact_panes(&mut self, ui: &mut Ui, full: Rect, view: &DocumentView) {
        let layout = compute_compact_layout(full, self.compact_pane, self.compact_editor_frac);
        self.layout = Some(PaneLayout {
            full,
            mode: LayoutMode::Compact,
            editor: layout.editor,
            plot: match layout.pane {
                Some(CompactPane::Graph) => layout.drawer.unwrap_or(Rect::NOTHING),
                _ => Rect::NOTHING,
            },
            workspace: match layout.pane {
                Some(CompactPane::Workspace) => layout.drawer.unwrap_or(Rect::NOTHING),
                _ => Rect::NOTHING,
            },
            logs: match layout.pane {
                Some(CompactPane::Logs) => layout.drawer.unwrap_or(Rect::NOTHING),
                _ => Rect::NOTHING,
            },
            inspector: match layout.pane {
                Some(CompactPane::Inspector) => layout.drawer.unwrap_or(Rect::NOTHING),
                _ => Rect::NOTHING,
            },
            split_a: Rect::NOTHING,
            split_b: Rect::NOTHING,
            split_c: Rect::NOTHING,
            split_d: Rect::NOTHING,
            compact: Some(layout),
        });
        self.compact_tabs.clear();
        self.compact_hide = None;
        self.compact_inspector = None;
        self.compact_pane_tabs(ui, layout.tab_bar);
        self.editor_pane(ui, layout.editor, view);
        match (layout.pane, layout.drawer) {
            (Some(CompactPane::Graph), Some(drawer)) => self.plot_pane(ui, drawer, view),
            (Some(CompactPane::Workspace), Some(drawer)) => self.workspace_pane(ui, drawer, view),
            (Some(CompactPane::Logs), Some(drawer)) => self.logs_pane_compact(ui, drawer, view),
            (Some(CompactPane::Inspector), Some(drawer)) => {
                self.symbolic_inspector_pane(ui, drawer, view)
            }
            _ => {}
        }
        if let (Some(splitter_rect), Some(drawer)) = (layout.splitter, layout.drawer) {
            let outcome = splitter(
                ui,
                splitter_rect,
                "compact_split_editor",
                true,
                "Editor / pane drawer splitter",
            );
            if outcome.delta != 0.0 {
                let usable = (layout.editor.width() + drawer.width()).max(1.0);
                self.compact_editor_frac = (self.compact_editor_frac + outcome.delta / usable)
                    .clamp(COMPACT_EDITOR_MIN, 0.82);
            }
            if outcome.reset {
                self.reset_pane_sizes();
            } else if outcome.drag_stopped || outcome.keyboard {
                self.persist_layout_if_changed();
            }
        }
    }

    /// LAY-011 / LAY-002: the compact pane switcher. Every collapsed pane is one pointer action
    /// away and every button is named for UI Automation; the Inspector tab opens the dedicated
    /// Symbolic Inspector drawer, so that pane is one action away too.
    fn compact_pane_tabs(&mut self, ui: &mut Ui, rect: Rect) {
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center))
                .accessibility_role(egui::Role::TabList)
                .accessibility_label("Collapsed panes"),
            |ui| {
                ui.add_space(4.0);
                for pane in [
                    CompactPane::Graph,
                    CompactPane::Workspace,
                    CompactPane::Logs,
                    CompactPane::Inspector,
                ] {
                    let selected = self.compact_pane == Some(pane);
                    let response = ui.selectable_label(selected, pane.label());
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Tab, true, pane.label().to_string())
                    });
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_selected(selected);
                    });
                    self.compact_tabs.push((pane, response.rect));
                    if pane == CompactPane::Inspector {
                        self.compact_inspector = Some(response.rect);
                    }
                    if response.clicked() {
                        self.compact_pane = Some(pane);
                        // LAY-004: which pane the drawer shows is the window's visible-pane
                        // state, so it is written as soon as the user chooses it.
                        self.persist_layout_if_changed();
                    }
                }
                if self.compact_pane.is_some() {
                    let hide = ui.button("Hide panes");
                    self.compact_hide = Some(hide.rect);
                    if hide.clicked() {
                        self.compact_pane = None;
                        self.persist_layout_if_changed();
                    }
                }
            },
        );
    }

    /// LAY-011: the logs drawer. The compact drawer is narrow, so its log lines wrap instead of
    /// being clipped at the drawer edge and the quick-eval field shrinks to the space it has.
    fn logs_pane_compact(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        if self.pane_focus == Some(Pane::Logs) {
            self.pane_focus = None;
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(quick_eval_id()));
        }
        let active = self.workbench.active();
        let legend = log_kind_legend();
        let header_line = {
            let (line, column) = self.editors[active].line_col_1based();
            format!("run log · Ln {line}, Col {column}")
        };
        let header_hint = header_text_width(&header_line) + header_text_width("Logs") + 8.0 + 80.0;
        with_pane(ui, rect, "Logs", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                ui.label(
                    RichText::new("Logs")
                        .font(type_scale::Role::PaneTitle.font())
                        .color(theme::text()),
                );
                ui.add(
                    egui::Label::new(RichText::new(&header_line).color(theme::dim())).truncate(),
                )
                .on_hover_text(format!("Log kinds: {legend}"));
                let compact_clear = ui
                    .add_enabled(
                        !view.logs.is_empty(),
                        egui::Button::new(RichText::new("Clear log").small()),
                    )
                    .on_hover_text("Empty this document's log; future runs append to it again");
                if view.logs.is_empty() {
                    mark_disabled(ui, &compact_clear);
                }
                if compact_clear.clicked() {
                    self.workbench.clear_log_active();
                }
            });
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(body.shrink2(vec2(6.0, 2.0)))
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    self.quick_eval_row_compact(ui);
                    let logs = &view.logs;
                    if logs.is_empty() {
                        // VIS-008: the compact log strip keeps the same empty state (reason and
                        // next step) and offers Run; a short strip collapses the headline.
                        let state = empty::logs();
                        let rect = ui.available_rect_before_wrap();
                        if empty::paint(ui, rect, &state) == Some(empty::EmptyAction::Run) {
                            self.workbench.run_active();
                        }
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("logs_scroll_compact")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for entry in logs {
                                let color =
                                    semantic::SemanticRole::for_log_kind(entry.kind).color();
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(entry.render()).monospace().color(color),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                },
            );
        });
    }

    /// LAY-011: the quick-eval row in the narrow drawer: the field takes what is left and log
    /// wrapping keeps every line inside the pane.
    fn quick_eval_row_compact(&mut self, ui: &mut Ui) {
        let mut submit = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Quick eval").strong().color(theme::text()));
            let id = quick_eval_id();
            let width = (ui.available_width() - 24.0).max(60.0);
            let response = ui.add(
                egui::TextEdit::singleline(self.quick_eval.text_mut())
                    .id(id)
                    .hint_text("quick eval")
                    .desired_width(width),
            );
            label_input(ui, &response, "Quick eval expression");
            self.quick_eval_rect = Some(response.rect);
            if response.has_focus() {
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
                    response.surrender_focus();
                }
            }
            if response.lost_focus() && key_pressed_without_modifiers(ui, Key::Enter) {
                submit = true;
            }
        });
        if submit {
            self.submit_quick_eval();
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(quick_eval_id()));
        }
        ui.add_space(2.0);
    }

    fn editor_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        // A11Y-001: F6 / Ctrl+2 focus the editor surface. Requesting it before the editor paints
        // means the ring and the caret are already in place when the keys are read.
        if self.pane_focus == Some(Pane::Editor) {
            self.pane_focus = None;
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(surface_id()));
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
        // LAY-011: at the minimum window the compact editor header keeps only the facts that are
        // needed to identify the buffer; the typography summary and the fixed editing-state
        // allowance would otherwise take half of the pane's height away from the text.
        let compact = self.layout_mode.compact_headers();
        // A11Y-005: the header states the resolved typography, so a UIA capture proves which
        // font, size, line height and ligature choice the frame is painted with. At a narrow
        // pane it is the first label dropped: it is the least important header fact and keeping
        // it would force a fourth header row at 200% interface scale.
        let typography_line = (!compact && rect.width() >= 420.0)
            .then(|| typography::Typography::new(&self.settings.editor.typography).summary());
        let diagnostic_line = diagnostic
            .as_ref()
            .map(|diagnostic| format!("[ERR] {}", diagnostic_summary(diagnostic)));
        let completion_line = self.editors[active].completion_summary();
        let bracket_line = self.editors[active].bracket_status();
        let signature_line = self.editors[active].signature_summary();
        let find_line = self.editors[active].find_summary();
        let mut header_hint = header_text_width("Editor")
            + header_text_width(&title)
            + header_text_width(&header_lines)
            + header_text_width(&encoding);
        if let Some(line) = &typography_line {
            header_hint += header_text_width(line);
        }
        // The editing-state summaries (completion, bracket, signature, find) come and go with
        // every pass; their widths are deliberately not measured, because a hint that changed
        // when a status appeared would resize the header and shift the whole pane between two
        // frames (which also moves a pointer target out from under a click). A fixed allowance
        // covers all four at once; the compact layout keeps only a token allowance so the text
        // body, not the header, owns the pane.
        header_hint += if compact { 90.0 } else { 280.0 };
        if let Some(extra) = diagnostic_line.as_deref() {
            header_hint += 8.0 + header_text_width(extra);
        }
        with_pane(ui, rect, &pane_title, header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                // LAY-002: a visible pane title. The same string is the pane's accessible name.
                ui.label(
                    RichText::new("Editor")
                        .font(type_scale::Role::PaneTitle.font())
                        .color(theme::text()),
                );
                ui.label(RichText::new(&title).color(theme::accent()).monospace());
                ui.label(RichText::new(&header_lines).color(theme::dim()));
                ui.label(
                    RichText::new(&encoding)
                        .monospace()
                        .small()
                        .color(theme::dim()),
                )
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
                let bar = Rect::from_min_size(body.min, vec2(body.width(), 86.0));
                self.find_bar(ui, bar, active);
                let top = bar.bottom() + 2.0;
                body = Rect::from_min_max(pos2(body.left(), top), body.max);
            }
            let before = self.editors[active].edits;
            let _ = self.editors[active].ui(ui, body, &catalogue, &engine_state);
            if self.editors[active].edits != before {
                // An edit makes the old position describe text that is gone. Drop it now, not
                // only on the next frame, so the paint pass cannot show a stale underline.
                self.editors[active].diagnostic = None;
                let source = self.editors[active].source();
                self.workbench.set_source(active, source);
                self.loaded_revisions[active] = self.workbench.documents()[active].revision;
            }
            if self.editors[active].bytes() == 0 {
                // VIS-008: the editor pane's empty state explains what the editor is for, why it
                // is empty and what to do next, and offers the first-step command. An empty
                // buffer is one blank line (laid out rows = 1), so the empty condition is the
                // byte count, not the row count.
                let state = empty::editor();
                let action = empty::paint(ui, body.shrink(8.0), &state);
                match action {
                    Some(empty::EmptyAction::FocusEditor) => self.focus_editor(ui.ctx()),
                    _ => {}
                }
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
                                .desired_width(150.0)
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
                });
                // VIS-004: the count/error and the navigation controls are a second row, so a
                // wider 11 pt caption or a longer error cannot push Next/Close past the editor's
                // edge at the minimum window.
                // The count/error/guidance is on a fixed one-line row: the truncating label
                // absorbs the width that is left instead of wrapping the navigation row to a
                // second line (which would move the replace row under a stale pointer target).
                ui.horizontal(|ui| {
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
                    ui.separator();
                    let find = &self.editors[active].find;
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
                            if text == "No matches" {
                                // VIS-008: the search's empty state. The reason names the query
                                // and the next step names the toggles that might be hiding a
                                // match, so "No matches" is guidance rather than a dead end.
                                let state = empty::search(&find.query);
                                let guidance = format!("{} {}", state.reason, state.next);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&guidance)
                                            .monospace()
                                            .small()
                                            .color(theme::dim()),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&guidance);
                            } else {
                                ui.label(
                                    RichText::new(text).monospace().small().color(theme::dim()),
                                );
                            }
                        }
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
                    if replacement.has_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
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
                    self.find_buttons
                        .push(("Replace all", replace_all_button.rect));
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
        // PLT-005: the document and run the chart belongs to, painted in the header beside the
        // engine's title and repeated in the canvas's accessible description by `Plot::ui`.
        let provenance_label = self
            .plot
            .provenance
            .as_ref()
            .map(|provenance| provenance.label())
            .unwrap_or_else(|| "no document".to_string());
        // LAY-011: in the compact drawer the plot header carries the six plot actions and nothing
        // else; the zoom readout, the engine title and the raster diagnostics would each cost a
        // wrapped row and leave less canvas than the actions themselves.
        let compact = self.layout_mode.compact_headers();
        let mut header_hint = if compact {
            36.0 + 48.0 + 78.0 + 72.0 + 100.0 + 105.0 + 16.0
        } else {
            36.0 + 48.0
                + 78.0
                + 72.0
                + 8.0
                + header_text_width(&zoom_label)
                + 8.0
                + header_text_width(plot_title.as_deref().unwrap_or("no engine plot"))
                + 8.0
                + header_text_width(&provenance_label)
                + 8.0
                + 100.0
                + 105.0
        };
        // LAY-002: the visible pane title, on every layout (the compact form too).
        header_hint += header_text_width("Graph");
        if let Some((status, _)) = &self.plot_export_status {
            header_hint += header_text_width(status);
        }
        with_pane(ui, rect, "Graph", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                // LAY-002: a visible pane title; "Graph" is the pane's accessible name too.
                ui.label(
                    RichText::new("Graph")
                        .font(type_scale::Role::PaneTitle.font())
                        .color(theme::text()),
                );
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
                if !compact {
                    ui.label(RichText::new(&zoom_label).monospace().color(theme::dim()))
                        .on_hover_text("Zoom as a percentage of natural size");
                    ui.separator();
                    match &plot_title {
                        Some(title) => {
                            ui.add(
                                egui::Label::new(RichText::new(title).color(theme::accent()))
                                    .truncate(),
                            );
                        }
                        None => {
                            ui.label(RichText::new("no engine plot").color(theme::dim()));
                        }
                    }
                    ui.separator();
                    // PLT-005: the chart never appears without the document and run that produced
                    // it. The label is a real text node, so it is in the accessibility tree and in
                    // a UIA capture rather than only in the painted pixels.
                    ui.add(
                        egui::Label::new(
                            RichText::new(&provenance_label).small().color(theme::dim()),
                        )
                        .truncate(),
                    )
                    .on_hover_text("The document and run this chart belongs to");
                    ui.separator();
                }
                // PLT-006: both exports go through the native save dialog. SVG copies the
                // engine's captured bytes; PNG opens an in-window options step first.
                let export_svg = ui
                    .add_enabled(!empty, egui::Button::new("Export SVG..."))
                    .on_hover_text("Save the engine's original SVG bytes unchanged");
                if !empty && export_svg.clicked() {
                    self.export_plot_svg();
                }
                // VIS-006: a disabled export is not merely translucent; it carries the dashed
                // outline and diagonal hatch that say "not actionable" without colour or alpha.
                if empty {
                    mark_disabled(ui, &export_svg);
                }
                let export_png = ui
                    .add_enabled(!empty, egui::Button::new("Export PNG..."))
                    .on_hover_text("Rasterise at a chosen scale and background");
                if !empty && export_png.clicked() {
                    self.plot_export = Some(PngExport::default());
                }
                if empty {
                    mark_disabled(ui, &export_png);
                }
                if !compact {
                    if let Some((status, error)) = &self.plot_export_status {
                        ui.add(
                            egui::Label::new(RichText::new(status).small().color(if *error {
                                theme::error()
                            } else {
                                theme::accent()
                            }))
                            .truncate(),
                        );
                    }
                }
            });
            if empty {
                // VIS-008: the graph's empty state names what the pane draws, why there is no
                // plot and the call that produces one; it offers Run itself.
                let state = empty::plot();
                if empty::paint(ui, body.shrink(8.0), &state) == Some(empty::EmptyAction::Run) {
                    self.workbench.run_active();
                }
                return;
            }
            self.plot.title = plot_title.clone();
            if compact {
                // LAY-011: the raster diagnostics are a development readout; in the narrow drawer
                // the whole body is the canvas.
                self.plot.ui(ui, body);
                return;
            }
            let info = format!(
                "raster {}x{} @ {:.2}x · {:.2} ms parse · {:.2} ms raster · {:.2} ms upload · {} frames painted meanwhile",
                self.plot.raster_px[0],
                self.plot.raster_px[1],
                self.plot.raster_scale,
                self.plot.parse_ms,
                self.plot.last_raster_ms,
                self.plot.last_upload_ms,
                self.plot.frames_during_raster
            );
            let legend = self.plot.legend_entries();
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(Rect::from_min_size(body.min, vec2(body.width(), 18.0)))
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    // VIS-007: the plot legend pairs every series' stroke colour with its number,
                    // the same number the canvas badge draws at the series' first point. The
                    // number labels are real text nodes, so a UIA dump names them.
                    for (number, color) in &legend {
                        let (swatch, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter().rect_filled(swatch, 1.0, *color);
                        ui.label(
                            RichText::new(format!("series {number}"))
                                .small()
                                .color(theme::dim()),
                        );
                    }
                    if !legend.is_empty() {
                        ui.separator();
                    }
                    // VIS-004/VIS-005: the diagnostics caption truncates instead of crossing the
                    // pane border when the pane is narrow (or the interface scale is 200%).
                    ui.add(
                        egui::Label::new(RichText::new(&info).small().color(theme::dim()))
                            .truncate(),
                    )
                    .on_hover_text(&info);
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

    /// SYM-007: the folder an inspector export is written to: `exports/` beside the settings file.
    ///
    /// A test or headless window whose storage has no settings path returns `None`, and the
    /// inspector's export control says so instead of writing into the process directory. The
    /// directory is created on the first export, so a fresh profile just works.
    fn inspector_export_dir(&self) -> Option<PathBuf> {
        self.storage
            .settings_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(|root| root.join("exports"))
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
        self.plot_export_status = Some(
            match self.plot.export_png_to(&path, 2.0, PngBackground::Opaque) {
                Ok([width, height]) => (
                    format!(
                        "PNG: {width}x{height} px {} -> {}",
                        PngBackground::Opaque.label(),
                        name
                    ),
                    false,
                ),
                Err(error) => (error, true),
            },
        );
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

    /// PERF-011: the cost estimate an extreme precision request must be confirmed against.
    ///
    /// The estimate is the measured curve's own number, painted with the points it was fitted to
    /// and the budget that would refuse it; nothing here re-derives a time. `Set precision`
    /// applies the change and `Cancel` leaves the document exactly as it was — both are ordinary
    /// buttons with stable accessible names, so the window capture can invoke them through UI
    /// Automation and a keyboard user reaches them with Tab.
    fn precision_estimate_window(&mut self, ctx: &egui::Context) {
        self.precision_confirm_rects.clear();
        let Some(cost) = self.workbench.pending_precision().cloned() else {
            return;
        };
        let curve_line = self
            .workbench
            .precision_curve()
            .map(|curve| curve.measured_line())
            .unwrap_or_default();
        let mut decision: Option<bool> = None;
        egui::Window::new("Precision cost estimate")
            .id(Id::unique("perf_precision_estimate"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} digits is an extreme precision request.",
                        cost.digits
                    ))
                    .strong()
                    .color(theme::text()),
                );
                ui.label(
                    RichText::new(format!(
                        "Estimated {} for one {} evaluation.",
                        cost.estimate_text(),
                        crate::workbench::PRECISION_WORKLOAD
                    ))
                    .color(theme::accent()),
                );
                ui.label(
                    RichText::new(format!(
                        "The estimate comes from the measured curve: {curve_line}. The product \
                         budget is {} s for one precision change.",
                        crate::workbench::SESSION_BUDGET_SECONDS
                    ))
                    .small()
                    .color(theme::dim()),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    let set = ui.add(egui::Button::new("Set precision")).on_hover_text(
                        "Apply the precision. Every later evaluation runs at this precision.",
                    );
                    let cancel = ui
                        .add(egui::Button::new("Cancel"))
                        .on_hover_text("Leave the precision unchanged");
                    self.precision_confirm_rects
                        .push(("Set precision".to_string(), set.rect));
                    self.precision_confirm_rects
                        .push(("Cancel".to_string(), cancel.rect));
                    if set.clicked() {
                        decision = Some(true);
                    }
                    if cancel.clicked() {
                        decision = Some(false);
                    }
                });
            });
        match decision {
            Some(true) => {
                if let Some(digits) = self.workbench.confirm_precision() {
                    self.precision_seen = digits;
                    self.precision_input = digits.to_string();
                    self.precision_error = None;
                }
            }
            Some(false) => {
                self.workbench.cancel_precision();
            }
            None => {}
        }
    }

    fn workspace_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        let has_snapshot = view.has_snapshot;
        // DOC-010: a restored tab's engine state is explicitly reported as reset until its next
        // run, so the empty Workspace pane is not indistinguishable from a run that produced
        // nothing. The note belongs to the document, not to the frame.
        let reset_note = self.workbench.active_document().engine_state_note.clone();
        // SYM-007: where an inspector copy or export goes, resolved before the pane borrows the
        // tree's interaction state.
        let export_dir = self.inspector_export_dir();
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
                "rev {} · no committed statement{}{}",
                view.revision.0,
                if stale { " · STALE" } else { "" },
                if reset_note.is_some() {
                    " · ENGINE STATE RESET"
                } else {
                    ""
                }
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
        let mut variable_action_rects: Vec<(String, Rect)> = Vec::new();
        // WKS-006: the variable whose full value the pane is showing, if any. The state lives on
        // the window (not this pass), so a repaint cannot close the detail the user opened.
        let mut variable_detail = self.variable_detail.clone();
        // RES-004: the RESULT tree's collapse state and the headers this pass paints. The state
        // lives on the window, not in the paint pass, so a repaint or an unrelated update (a
        // quick eval, an edit) cannot reset it; `paint_result_section` clears it only when the
        // committed result changes.
        let expansion = &mut self.expansion;
        let inspector_expansion = &mut self.inspector_expansion;
        let mut inspector_open = self.inspector_open;
        let mut inspector_toggle_rects: Vec<TreeRowSeam> = Vec::new();
        let mut inspector_control_rects: Vec<(String, Rect)> = Vec::new();
        let mut workspace_view_tabs: [Option<Rect>; 2] = [None; 2];
        let mut workspace_view_tab_ids: [Option<Id>; 2] = [None; 2];
        let mut workspace_filter_id: Option<Id> = None;
        let mut result_toggle_rects: Vec<(String, Rect)> = Vec::new();
        let pane_focus = self.pane_focus;
        let header_hint = 40.0
            + 140.0
            + header_text_width("Workspace")
            + header_text_width(&format!("{variable_count} vars · {function_count} funcs"));
        with_pane(ui, rect, "Workspace", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                // LAY-002: a visible pane title; the same string is the accessible name.
                ui.label(
                    RichText::new("Workspace")
                        .font(type_scale::Role::PaneTitle.font())
                        .color(theme::text()),
                );
                let label = ui.label("Filter");
                // LAY-011: the field never asks for more width than the header has left, so the
                // dense split's narrower Workspace column still paints the whole control.
                let field_width = 130.0_f32.min((ui.available_width() - 12.0).max(70.0));
                let filter_field = ui
                    .add(
                        egui::TextEdit::singleline(&mut filter)
                            .hint_text("variable or function")
                            .desired_width(field_width),
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
                    // wrapping at the full pane width would still end under the border. The extra
                    // 16 points cover the scroll bar the vertical ScrollArea reserves, so a
                    // wrapped line cannot be clipped horizontally at the viewport edge.
                    let result_width = (body.width() - 112.0).max(100.0);
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
                                let inspector_action = paint_inspector(
                                    ui,
                                    view,
                                    result_width,
                                    inspector_expansion,
                                    &mut inspector_toggle_rects,
                                    &mut inspector_control_rects,
                                    export_dir.as_deref(),
                                );
                                if inspector_action == Some(empty::EmptyAction::Run) {
                                    self.workbench.run_active();
                                }
                            });
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("workspace_scroll")
                            .show(ui, |ui| {
                                ui.add_space(4.0);

                                // RES-001: the last committed result's typed tree, built only from
                                // the engine's structured projection. The display text is a caption;
                                // it is never parsed for structure.
                                // VIS-001 review: the result tree reserves 112 pt for its nested
                                // indentation and scrollbar. That reserve must not leak to the
                                // variables/functions tables below it, or both tables render in a
                                // two-thirds-width strip and truncate their Kind/Value columns.
                                let result_action = ui
                                    .scope(|ui| {
                                        paint_result_section(
                                            ui,
                                            view,
                                            result_width,
                                            expansion,
                                            &mut result_toggle_rects,
                                        )
                                    })
                                    .inner;
                                if result_action == Some(empty::EmptyAction::Run) {
                                    self.workbench.run_active();
                                }

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
                                // WKS-003: the workspace's own revision. A delete or a clear
                                // changes the state without changing the source, so the pane
                                // states the counter that did move.
                                ui.label(
                                    RichText::new(format!("state rev {}", view.workspace_revision))
                                        .small()
                                        .color(theme::dim()),
                                )
                                .on_hover_text(
                                    "Workspace revision: moved by every workspace edit (delete a \
                                     variable, clear the variables); the source revision is shown \
                                     on the line above",
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let can_clear =
                                        has_snapshot && !self.workbench.active_is_running();
                                    let clear = ui
                                        .add_enabled(
                                            can_clear,
                                            egui::Button::new(
                                                RichText::new("Clear Variables").small(),
                                            ),
                                        )
                                        .on_hover_text(
                                            "Remove every variable; function definitions are retained",
                                        );
                                    // VIS-006: a disabled Clear Variables carries the dashed
                                    // outline and hatch, not only reduced opacity.
                                    if !can_clear {
                                        mark_disabled(ui, &clear);
                                    }
                                    if clear.clicked() {
                                        self.workbench.clear_variables_active();
                                    }
                                });
                            });
                            // WKS-003: the trailing control column of the variables table.
                            let row_actions_width = ROW_ACTION_WIDTH * 2.0;
                            let widths = variable_column_widths(
                                (ui.available_width() - row_actions_width).max(120.0),
                            );

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
                            let mut delete_requested: Option<String> = None;
                            let mut detail_requested: Option<String> = None;
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
                                            row.preview.text
                                        );
                                        let detail_open = variable_detail.as_deref()
                                            == Some(row.name.as_str());
                                        table_row(ui, label, |ui| {
                                            // WKS-006: the name cell opens the row's detail view,
                                            // which paints the full value the cell shortened.
                                            let name = ui.add_sized(
                                                [widths[0], 16.0],
                                                egui::Button::new(
                                                    RichText::new(cell_text(&row.name))
                                                        .monospace()
                                                        .color(theme::accent()),
                                                )
                                                .frame(false),
                                            );
                                            let name_accessible = format!(
                                                "{}{}",
                                                if detail_open { "Hide" } else { "Show" },
                                                format!(" details of {}", row.name)
                                            );
                                            name.widget_info(|| {
                                                egui::WidgetInfo::labeled(
                                                    egui::Role::Button,
                                                    true,
                                                    name_accessible.clone(),
                                                )
                                            });
                                            variable_action_rects
                                                .push((format!("details {}", row.name), name.rect));
                                            if name.clicked() {
                                                detail_requested = Some(row.name.clone());
                                            }
                                            name.on_hover_text(format!(
                                                "{} — open the full value ({} chars, {})",
                                                row.name,
                                                row.display.chars().count(),
                                                row.preview.type_label
                                            ));
                                            table_cell(
                                                ui,
                                                widths[1],
                                                RichText::new(cell_text(&row.kind))
                                                    .monospace()
                                                    .color(theme::dim()),
                                                &row.kind,
                                            );
                                            // WKS-006: the value cell is the type-aware preview
                                            // (matrix rows, record fields, complex pair, pretty
                                            // symbolic text); the full untyped display stays the
                                            // hover text and the printable detail.
                                            table_cell(
                                                ui,
                                                widths[2],
                                                RichText::new(row.preview_cell())
                                                    .monospace()
                                                    .color(theme::text()),
                                                &format!(
                                                    "{}: {}\nfull display: {}",
                                                    row.preview.type_label,
                                                    row.preview.text,
                                                    row.display
                                                ),
                                            );
                                            // WKS-003: delete exactly this variable. Disabled
                                            // while a run is in flight or before a run published a
                                            // workspace, like Clear Variables.
                                            let can_delete = has_snapshot
                                                && !self.workbench.active_is_running();
                                            let delete = ui.add_enabled(
                                                can_delete,
                                                egui::Button::new(
                                                    RichText::new("del")
                                                        .small()
                                                        .monospace()
                                                        .color(theme::error()),
                                                )
                                                .frame(false)
                                                .min_size(vec2(ROW_ACTION_WIDTH, 16.0)),
                                            );
                                            let delete_name =
                                                format!("Delete variable {}", row.name);
                                            delete.widget_info(|| {
                                                egui::WidgetInfo::labeled(
                                                    egui::Role::Button,
                                                    true,
                                                    delete_name.clone(),
                                                )
                                            });
                                            if !can_delete {
                                                mark_disabled(ui, &delete);
                                            }
                                            let delete = delete.on_hover_text(format!(
                                                "Delete {} only; functions and every other \
                                                 variable are unchanged",
                                                row.name
                                            ));
                                            variable_action_rects
                                                .push((format!("delete {}", row.name), delete.rect));
                                            if delete.clicked() {
                                                delete_requested = Some(row.name.clone());
                                            }
                                        });
                                    }
                                },
                            );
                            if let Some(name) = delete_requested {
                                self.workbench.delete_variable_active(&name);
                            }
                            if let Some(name) = detail_requested {
                                variable_detail = if variable_detail.as_deref() == Some(&name) {
                                    None
                                } else {
                                    Some(name)
                                };
                            }
                            // WKS-006: the detail view keeps the whole value: the cell may cut a
                            // preview, but the memory and this view hold the full display, the
                            // engine's structured projection of it and every published field.
                            if let Some(open) = variable_detail.clone() {
                                if let Some(row) =
                                    variable_rows.iter().find(|row| row.name == open)
                                {
                                    paint_variable_detail(ui, row);
                                } else {
                                    variable_detail = None;
                                }
                            }
                            if shown == 0 {
                                // VIS-008: three distinct reasons, three distinct next steps;
                                // each keeps the pane's purpose, and each offers the command that
                                // resolves it (Run, or Clear the filter).
                                let state = if !has_snapshot {
                                    match &reset_note {
                                        Some(note) => empty::EmptyState::new(
                                            "workspace",
                                            "Engine state was reset",
                                            "The Workspace table lists the variables the last \
                                             run committed.",
                                            note.clone(),
                                            "Run this document (Ctrl+Enter) to recompute its \
                                             variables and functions.",
                                        )
                                        .with_action(empty::EmptyAction::Run),
                                        None => empty::workspace(),
                                    }
                                } else if variable_rows.is_empty() {
                                    empty::EmptyState::new(
                                        "workspace",
                                        "No variables committed",
                                        "The Workspace table lists the variables the last run \
                                         committed.",
                                        "The last run published no variables.",
                                        "Assign one (for example x = 1) and run the document again.",
                                    )
                                    .with_action(empty::EmptyAction::Run)
                                } else {
                                    empty::filtered(
                                        "workspace",
                                        "The Workspace table lists the variables the last run \
                                         committed.",
                                        "No variable name matches the filter.",
                                    )
                                };
                                let available = ui.available_rect_before_wrap();
                                let rect = Rect::from_min_size(
                                    available.min,
                                    vec2(available.width(), available.height().min(96.0)),
                                );
                                match empty::paint(ui, rect, &state) {
                                    Some(empty::EmptyAction::Run) => {
                                        self.workbench.run_active();
                                    }
                                    Some(empty::EmptyAction::ClearFilter) => filter.clear(),
                                    _ => {}
                                }
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.label(
                                RichText::new(format!("FUNCTIONS · {function_count}"))
                                    .small()
                                    .color(theme::dim()),
                            );
                            let widths = function_column_widths(ui.available_width());

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
                                            let origin_hover = if row.origin == FunctionOrigin::Builtin {
                                                format!(
                                                    "{} · module: {}",
                                                    row.origin_label(),
                                                    row.module
                                                )
                                            } else {
                                                row.origin_label().to_string()
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
                                let state = if !has_snapshot {
                                    match &reset_note {
                                        Some(note) => empty::EmptyState::new(
                                            "functions",
                                            "Engine state was reset",
                                            "The functions table lists the engine builtins, \
                                             built-in families and your own func definitions.",
                                            note.clone(),
                                            "Run this document (Ctrl+Enter) to recompute its \
                                             variables and functions.",
                                        )
                                        .with_action(empty::EmptyAction::Run),
                                        None => empty::functions(),
                                    }
                                } else if function_rows.is_empty() {
                                    empty::EmptyState::new(
                                        "functions",
                                        "No functions committed",
                                        "The functions table lists the engine builtins, built-in families \
                                         and your own func definitions.",
                                        "The last run published no functions.",
                                        "Define one (for example func f(x) = x) and run again.",
                                    )
                                    .with_action(empty::EmptyAction::Run)
                                } else {
                                    empty::filtered(
                                        "functions",
                                        "The functions table lists the engine builtins, built-in families \
                                         and your own func definitions.",
                                        "No function name matches the filter.",
                                    )
                                };
                                let available = ui.available_rect_before_wrap();
                                let rect = Rect::from_min_size(
                                    available.min,
                                    vec2(available.width(), available.height().min(96.0)),
                                );
                                match empty::paint(ui, rect, &state) {
                                    Some(empty::EmptyAction::Run) => {
                                        self.workbench.run_active();
                                    }
                                    Some(empty::EmptyAction::ClearFilter) => filter.clear(),
                                    _ => {}
                                }
                            }
                        });
                    }
                },
            );
        });
        self.filter = filter;
        self.variables_table.sort = variable_sort;
        self.functions_table.sort = function_sort;
        self.variable_detail = variable_detail;
        self.variable_action_rects = variable_action_rects;
        self.variable_header_rects = variable_header_rects;
        self.function_header_rects = function_header_rects;
        self.result_toggle_rects = result_toggle_rects;
        self.inspector_open = inspector_open;
        self.inspector_toggle_rects = inspector_toggle_rects;
        self.inspector_control_rects = inspector_control_rects;
        self.workspace_view_tabs = workspace_view_tabs;
        // A11Y-001: the workspace group's first control is the Result tab; Ctrl+Alt+F targets the
        // filter. Both requests are satisfied on the pass that painted the widget.
        self.workspace_tab_ids = workspace_view_tab_ids;
        self.workspace_filter_id = workspace_filter_id;
        if pane_focus == Some(Pane::Workspace) {
            if let Some(id) = self.workspace_tab_ids[0] {
                self.pane_focus = None;
                ui.ctx().memory_mut(|memory| memory.request_focus(id));
            }
        }
        if self.filter_focus {
            if let Some(id) = self.workspace_filter_id {
                self.filter_focus = false;
                ui.ctx().memory_mut(|memory| memory.request_focus(id));
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
        // LOG-005: a selection names rows of the log that was painted when it was made. A
        // document switch, a clear or a cap eviction makes the same index name a different line,
        // so the selection is dropped rather than silently repointed.
        let seen = (view.logs.len(), view.logs_dropped);
        if seen != self.log_seen {
            self.log_selection.clear();
            self.log_anchor = None;
            self.log_seen = seen;
        }
        let selection_count = self.log_selection.len();
        let status = self.log_status.clone();
        let header_line = {
            let (line, column) = self.editors[active].line_col_1based();
            format!(
                "run log · caret Ln {line}, Col {column} · engine {}",
                self.workbench.engine_name()
            )
        };
        let mut header_hint = header_text_width(&header_line)
            + header_text_width("Logs")
            + 8.0
            + 60.0
            + 80.0
            + 3.0 * 96.0;
        if let Some(status) = &status {
            header_hint += 8.0 + header_text_width(status);
        }
        // LOG-005 / LOG-006: every request the header, the rows or a context menu makes this
        // pass. They are applied after the pane stops borrowing the tree's interaction state.
        let mut request: Option<LogRequest> = None;
        let mut log_row_rects: Vec<(usize, Rect)> = Vec::new();
        let mut log_pinned_rects: Vec<(usize, Rect)> = Vec::new();
        let mut click: Option<(usize, bool, bool)> = None;
        let mut jump: Option<(usize, usize)> = None;
        with_pane(ui, rect, "Logs", header_hint, |ui, header, body| {
            header_row(ui, header, |ui| {
                // LAY-002: a visible pane title; the same string is the accessible name.
                ui.label(
                    RichText::new("Logs")
                        .font(type_scale::Role::PaneTitle.font())
                        .color(theme::text()),
                );
                ui.label(RichText::new(&header_line).color(theme::dim()))
                    .on_hover_text(format!("Log kinds: {legend}"));
                // LOG-010: the list's own state, in the pane's words: whether it follows the
                // newest record, or how far the log has moved on while the user reads history.
                // `log_follow` is this frame's previous decision, which is what the list below
                // shows (the control that changes it is painted over the list itself).
                let (state_text, state_color, state_hint) = if self.log_follow {
                    (
                        "at newest record".to_string(),
                        theme::dim(),
                        "The list is at the newest record; new records keep it there while it \
                         stays at the bottom.",
                    )
                } else {
                    (
                        format!("scrolled back · {} new", self.log_unseen),
                        theme::warning(),
                        "Records that arrive now are appended while the view stays where it was; \
                         the Jump to newest control over the list returns to the tail.",
                    )
                };
                ui.label(RichText::new(state_text).small().color(state_color))
                    .on_hover_text(state_hint);
                if let Some(status) = &status {
                    ui.label(RichText::new(status).small().color(theme::accent()));
                }
            });
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(body.shrink2(vec2(10.0, 4.0)))
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    // VIS-008: an empty log paints its own state with the whole body, so the
                    // state's headline is not squeezed into the compact form (the card is only
                    // compact below 72 points, which the empty pane would otherwise hit).
                    if view.logs.is_empty() {
                        let state = empty::logs();
                        let rect = ui.available_rect_before_wrap();
                        if empty::paint(ui, rect, &state) == Some(empty::EmptyAction::Run) {
                            self.workbench.run_active();
                        }
                        ui.label(RichText::new(legend.clone()).small().color(theme::dim()));
                        return;
                    }
                    // LAY-011 / A11Y-004: a control only paints when the body has room for it, so
                    // a 200% interface scale squeezes the log — never a clipped control.
                    let room_for_rows = ui.available_height() > 28.0;
                    if room_for_rows {
                        // RUN-002: the quick-eval one-liner lives above the log list, so it stays
                        // visible with an empty log and cannot be confused with a history row.
                        self.quick_eval_row(ui);
                    }
                    // LOG-005: the copy/export/clear actions live in the body, not the header, so
                    // a 200% interface scale cannot push the pane's first control past the pane
                    // edge with a taller header row. They are painted only when the pane has room
                    // for them; the same actions are always on each line's context menu and on
                    // their keyboard chords.
                    let menu_room = room_for_rows && ui.available_height() > 24.0;
                    if menu_room {
                        ui.horizontal_wrapped(|ui| {
                            let can_clear = !view.logs.is_empty();
                            let menu = ui.menu_button(RichText::new("Log actions").small(), |ui| {
                                if ui
                                    .add_enabled(can_clear, egui::Button::new("Clear log"))
                                    .clicked()
                                {
                                    request = Some(LogRequest::Clear);
                                    ui.close();
                                }
                                if ui.add(egui::Button::new("Copy all")).clicked() {
                                    request = Some(LogRequest::CopyAll);
                                    ui.close();
                                }
                                // LOG-010: the same "return to the tail" action as the footer's
                                // button, reachable from the menu (and therefore from a narrow
                                // pane whose footer is suppressed).
                                if ui
                                    .add_enabled(
                                        !self.log_follow,
                                        egui::Button::new("Jump to newest"),
                                    )
                                    .on_hover_text(
                                        "Scroll to the newest record; the view follows new \
                                         records again once it is at the bottom",
                                    )
                                    .clicked()
                                {
                                    request = Some(LogRequest::Jump);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        selection_count > 0,
                                        egui::Button::new("Copy selected"),
                                    )
                                    .clicked()
                                {
                                    request = Some(LogRequest::CopySelection);
                                    ui.close();
                                }
                                if ui.add(egui::Button::new("Save log…")).clicked() {
                                    request = Some(LogRequest::Save);
                                    ui.close();
                                }
                            });
                            menu.response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    true,
                                    "Log actions: copy selected, copy all, save log, clear log",
                                )
                            });
                            menu.response.on_hover_text(
                                "Copy selected (Ctrl+Shift+C), copy all (Ctrl+Shift+A), save \
                                 log (Ctrl+Alt+L), clear log (Ctrl+Shift+K); the same actions are \
                                 on each line's context menu",
                            );
                        });
                    }
                    // LOG-006: the log cap is not silent. When records were discarded the pane
                    // says how many, where they went and what the export contains.
                    if view.logs_dropped > 0 {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "truncated history: the first {} record(s) were discarded at \
                                     the {} record cap; the {} retained record(s) below are the \
                                     exportable full run log",
                                    view.logs_dropped,
                                    crate::workbench::LOG_CAP,
                                    view.logs.len()
                                ))
                                .small()
                                .color(theme::warning()),
                            )
                            .on_hover_text(
                                "The cap bounds memory. Newer records are always kept, including \
                                 the current fatal error; only the oldest are discarded.",
                            );
                            if ui
                                .add(egui::Button::new(RichText::new("Export…").small()))
                                .clicked()
                            {
                                request = Some(LogRequest::Save);
                            }
                        });
                    }
                    let logs = &view.logs;
                    let selected = self.log_selection.clone();
                    let anchor = self.log_anchor;
                    // LOG-004 / LOG-006 / DOC-009: the two lines whose content must never be
                    // hidden by a scroll position are pinned above the list — the current
                    // diagnostic (clickable: it opens and highlights the source) and the last
                    // result (the value the notation setting localizes). They stay one row each,
                    // so the pin costs a bounded two widgets however long the log is.
                    let pinned: [(usize, &str); 2] = {
                        let diagnostic = logs
                            .iter()
                            .rposition(|line| {
                                line.kind == crate::log_kind::LogKind::Diagnostic
                                    && line.location.is_some()
                            });
                        let result = logs
                            .iter()
                            .rposition(|line| line.kind == crate::log_kind::LogKind::Result);
                        match (diagnostic, result) {
                            (Some(diagnostic), Some(result)) if diagnostic == result => {
                                [(diagnostic, "pinned diagnostic"), (usize::MAX, "")]
                            }
                            (Some(diagnostic), Some(result)) => {
                                [(diagnostic, "pinned diagnostic"), (result, "pinned last result")]
                            }
                            (Some(diagnostic), None) => {
                                [(diagnostic, "pinned diagnostic"), (usize::MAX, "")]
                            }
                            (None, Some(result)) => [(result, "pinned last result"), (usize::MAX, "")],
                            (None, None) => [(usize::MAX, ""), (usize::MAX, "")],
                        }
                    };
                    // The pin is painted only when the pane has room for it: in a short pane the
                    // virtualised list (which follows the newest record) carries the same lines,
                    // and a pin painted past the pane's bottom edge would be clipped content.
                    for (index, caption) in pinned {
                        if index == usize::MAX || index >= logs.len() {
                            continue;
                        }
                        if ui.available_height() < 20.0 {
                            break;
                        }
                        let entry = &logs[index];
                        // The pin is ONE line: a bounded prefix, truncated by the label, so a
                        // 90 000-cell result cannot wrap the pin into hundreds of lines and push
                        // the list (and the pane) out of the window.
                        let rendered = entry.render();
                        let head: String = rendered.chars().take(160).collect();
                        let text = if rendered.chars().count() > 160 {
                            format!("{head}…")
                        } else {
                            head
                        };
                        let color = semantic::SemanticRole::for_log_kind(entry.kind).color();
                        let label = ui.add(
                            egui::Label::new(RichText::new(text).monospace().small().color(color))
                                .sense(egui::Sense::click())
                                .truncate(),
                        );
                        let accessible = match entry.location {
                            Some((line, column)) => format!(
                                "{} (pinned; click to open source at line {}, column {})",
                                entry.render(),
                                line,
                                column
                            ),
                            None => format!("{} (pinned {caption})", entry.render()),
                        };
                        label.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Label, true, accessible.clone())
                        });
                        log_pinned_rects.push((index, label.rect));
                        if label.clicked() {
                            self.log_selection.clear();
                            self.log_selection.insert(index);
                            self.log_anchor = Some(index);
                            if let Some((line, column)) = entry.location {
                                jump = Some((line, column));
                            }
                        }
                        label.on_hover_text(match entry.location {
                            Some((line, column)) => format!(
                                "{caption} · click to open the source at line {line}, column {column}"
                            ),
                            None => format!("{caption} · also in the log list below"),
                        });
                    }
                    // LOG-010: the log list is virtualised. Each row is one fixed-height line,
                    // so egui's scroll area lays out only the rows inside its viewport: a
                    // 100 000-record log costs one viewport of widgets, not 100 000, and a live
                    // update adds rows without rebuilding the widget tree.
                    //
                    // The view follows the newest record by default and stays there only while it
                    // is at the bottom. Growth is measured on the *total* record count, because
                    // once the storage cap starts evicting, `logs.len()` stays fixed while records
                    // keep arriving. A pass that appended records while the user was reading
                    // history counts them for the footer instead of moving the viewport: a user
                    // is never yanked away from the line they are reading.
                    // LOG-010 acceptance seam: a start-scrolled view is held scrolled back until
                    // the user presses the jump control, so the capture of the affordance does not
                    // race a first-layout pass. Off unless the launch switch set it.
                    if self.log_force_scrolled {
                        self.log_follow = false;
                    }
                    let total = logs.len() + view.logs_dropped;
                    let added = total.saturating_sub(self.log_follow_total);
                    self.log_follow_total = total;
                    let grew = added > 0;
                    if grew && !self.log_follow {
                        self.log_unseen = self.log_unseen.saturating_add(added);
                    }
                    let follow_was = self.log_follow;
                    // A resize is detected from the body height the list is about to get. It
                    // changes the viewport without the user touching the scroll position, so the
                    // tail has to be re-requested instead of being read as "the user scrolled".
                    let body_height = ui.available_height();
                    let resized = (body_height - self.log_follow_body).abs() > 0.5;
                    self.log_follow_body = body_height;
                    // LOG-010: the state is painted in the pane header, and the way back to the
                    // tail is an overlay control laid over the bottom-right of the list when the
                    // user is scrolled back. It is an overlay rather than a footer on purpose: a
                    // permanent footer would take a row of log space from every pane, and the
                    // window's Logs strip is already the narrowest one.
                    self.log_jump_rect = None;
                    // LOG-010: the requested offset is set only on the pass that needs it (a
                    // launch, a jump, a growth while following, a resize). Re-asserting it on
                    // every pass would fight a wheel gesture whose delta egui applies later in
                    // the same pass, which is what made an earlier version snap back to the tail.
                    let request_offset = self.log_follow && (self.log_force_follow || grew || resized);
                    self.log_force_follow = false;
                    let mut area = egui::ScrollArea::vertical()
                        .id_salt("logs_scroll")
                        .auto_shrink([false, false])
                        // Belt and braces: egui keeps the view at the end while it is already
                        // there, and never moves a view the user scrolled away from. The
                        // start-scrolled acceptance seam disables it and pins the view to the top.
                        .stick_to_bottom(!self.log_force_scrolled);
                    if self.log_force_scrolled {
                        area = area.vertical_scroll_offset(0.0).animated(false);
                    }
                    if request_offset {
                        // The exact content height of `show_rows` (rows plus their spacing), so
                        // the offset clamps to the end instead of overflowing egui's row math.
                        let advance = LOG_ROW_HEIGHT + ui.spacing().item_spacing.y;
                        area = area
                            .vertical_scroll_offset(logs.len() as f32 * advance)
                            .animated(false);
                    }
                    let area_output = area.show_rows(ui, LOG_ROW_HEIGHT, logs.len(), |ui, range| {
                            for index in range {
                                let entry = &logs[index];
                                // LOG-010: a printed megabyte is one line in the model and a
                                // bounded painted prefix; the full line stays in the log and in
                                // the copy/export text.
                                let (full, hidden) = bounded_value_text(&entry.render());
                                let text = if hidden > 0 {
                                    format!("{full} … (+{hidden} chars)")
                                } else {
                                    full
                                };
                                let color =
                                    semantic::SemanticRole::for_log_kind(entry.kind).color();
                                let is_selected = selected.contains(&index);
                                let response = ui.add_sized(
                                    [ui.available_width(), LOG_ROW_HEIGHT],
                                    egui::Button::new(
                                        RichText::new(text).monospace().small().color(color),
                                    )
                                    .frame(is_selected)
                                    .fill(if is_selected {
                                        theme::selection_bg()
                                    } else {
                                        theme::panel_bg()
                                    }),
                                );
                                log_row_rects.push((index, response.rect));
                                let accessible = if entry.location.is_some() {
                                    format!(
                                        "{} (click to open source at line {}, column {})",
                                        entry.render(),
                                        entry.location.unwrap().0,
                                        entry.location.unwrap().1
                                    )
                                } else {
                                    entry.render()
                                };
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::Button,
                                        true,
                                        accessible.clone(),
                                    )
                                });
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_selected(is_selected);
                                    // TST-011 / J-19: the newest record is the log's live region.
                                    // A row appended (or rewritten at the tail) raises UIA
                                    // LiveRegionChanged with its full text, which is how a screen
                                    // reader hears progress/result/diagnostic without polling.
                                    // Older rows stay silent unless the user navigates to them.
                                    if index + 1 == logs.len() {
                                        node.set_live(egui::accesskit::Live::Polite);
                                    }
                                });
                                if response.clicked() {
                                    let (shift, command) = ui.input(|input| {
                                        (input.modifiers.shift, input.modifiers.command)
                                    });
                                    click = Some((index, shift, command));
                                    // LOG-004: a diagnostic line opens and focuses its source.
                                    if let Some((line, column)) = entry.location {
                                        jump = Some((line, column));
                                    }
                                }
                                let hover = match entry.location {
                                    Some((line, column)) => format!(
                                        "{} — click to open {line}:{column}; right-click for the \
                                         log actions",
                                        entry.render()
                                    ),
                                    None => format!(
                                        "{} — click to select; right-click for the log actions",
                                        entry.render()
                                    ),
                                };
                                let response = response.on_hover_text(hover);
                                response.context_menu(|ui| {
                                    if ui
                                        .add(egui::Button::new("Copy selected"))
                                        .clicked()
                                    {
                                        request = Some(LogRequest::CopySelection);
                                        ui.close();
                                    }
                                    if ui.add(egui::Button::new("Copy all")).clicked() {
                                        request = Some(LogRequest::CopyAll);
                                        ui.close();
                                    }
                                    if entry.location.is_some()
                                        && ui
                                            .add(egui::Button::new("Copy details"))
                                            .clicked()
                                    {
                                        request = Some(LogRequest::CopyDetails(index));
                                        ui.close();
                                    }
                                    if entry.location.is_some()
                                        && ui
                                            .add(egui::Button::new("Open source position"))
                                            .clicked()
                                    {
                                        if let Some((line, column)) = entry.location {
                                            jump = Some((line, column));
                                        }
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui.add(egui::Button::new("Clear log")).clicked() {
                                        request = Some(LogRequest::Clear);
                                        ui.close();
                                    }
                                    if ui.add(egui::Button::new("Save log…")).clicked() {
                                        request = Some(LogRequest::Save);
                                        ui.close();
                                    }
                                });
                                // The anchor of a Shift+click range is recorded with the click.
                                let _ = anchor;
                            }
                        });
                    // LOG-010: the jump-to-newest affordance. It is painted over the bottom-right
                    // of the list — the corner the newest rows occupy only when the view is scrolled
                    // back anyway — so it costs no log rows and is reachable whenever a row is.
                    // It is painted here, before the follow decision, so a click this pass is
                    // decided with the same information the user had when pressing it.
                    let mut jump_clicked = false;
                    if !follow_was {
                        let list = area_output.inner_rect;
                        let size = vec2(178.0, 20.0);
                        let rect = Rect::from_min_size(
                            pos2(list.right() - size.x - 12.0, list.bottom() - size.y - 6.0),
                            size,
                        );
                        let label = if self.log_unseen > 0 {
                            format!("Jump to newest · {} new", self.log_unseen)
                        } else {
                            "Jump to newest".to_string()
                        };
                        let jump = ui
                            .put(rect, egui::Button::new(RichText::new(label).small()))
                            .on_hover_text(
                                "Scroll the log to the newest record; it follows new records \
                                 again once it is at the bottom",
                            );
                        let accessible = format!(
                            "Jump to newest log record; {} record(s) arrived while scrolled back",
                            self.log_unseen
                        );
                        jump.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, accessible.clone())
                        });
                        self.log_jump_rect = Some(jump.rect);
                        // The control this pass painted has just been used: the next pass
                        // follows the tail and offers no jump, so the seam must not keep the
                        // rectangle the list no longer shows.
                        jump_clicked = jump.clicked();
                    }
                    // LOG-010: follow mode is decided from where the viewport actually ended this
                    // pass, not from whether the log grew. egui has already applied any wheel or
                    // drag delta and clamped the offset to the content, so "at the bottom" is
                    // `offset == max_offset`.
                    //
                    // The non-bottom cases are kept apart deliberately: a jump the user just
                    // pressed wins; a pass that itself asked for the tail and could not reach it
                    // (a first-paint measurement difference) retries — bounded by
                    // `LOG_FOLLOW_RETRIES`; a pass whose only movement came from the user ends
                    // follow mode and keeps it ended, so new records only raise the unseen count.
                    let max_offset = (area_output.content_size.y - area_output.inner_rect.height())
                        .max(0.0);
                    let at_bottom = (area_output.state.offset().y - max_offset).abs() <= 1.0;
                    if jump_clicked {
                        self.log_force_scrolled = false;
                        self.log_follow = true;
                        self.log_unseen = 0;
                        self.log_force_follow = true;
                        self.log_follow_retries = 0;
                        self.log_jump_rect = None;
                        ui.ctx().request_repaint();
                    } else if at_bottom {
                        self.log_follow = true;
                        self.log_unseen = 0;
                        self.log_follow_retries = 0;
                    } else if follow_was && (request_offset || resized) {
                        let retry = self.log_follow_retries < LOG_FOLLOW_RETRIES;
                        self.log_follow = retry;
                        self.log_force_follow = retry;
                        self.log_follow_retries = self.log_follow_retries.saturating_add(1);
                    } else {
                        self.log_follow = false;
                    }
                    // LOG-010 acceptance seam: the start-scrolled view stays scrolled back until
                    // the jump control is pressed. Off unless the launch switch set it.
                    if self.log_force_scrolled && !jump_clicked {
                        self.log_follow = false;
                        self.log_force_follow = false;
                        // Every retained record is newer than the first paint the user saw.
                        self.log_unseen = total;
                    }
                },
            );
        });
        self.log_row_rects = log_row_rects;
        self.log_pinned_rects = log_pinned_rects;
        // Apply this pass's selection change and requested action.
        if let Some((index, shift, command)) = click {
            if shift {
                let anchor = self.log_anchor.unwrap_or(index);
                let (low, high) = if anchor <= index {
                    (anchor, index)
                } else {
                    (index, anchor)
                };
                for row in low..=high {
                    self.log_selection.insert(row);
                }
            } else if command {
                if !self.log_selection.remove(&index) {
                    self.log_selection.insert(index);
                }
                self.log_anchor = Some(index);
            } else {
                self.log_selection.clear();
                self.log_selection.insert(index);
                self.log_anchor = Some(index);
            }
        }
        if let Some((line, column)) = jump {
            self.jump_to_source_position(line, column, ui.ctx());
        }
        match request {
            Some(LogRequest::Clear) => {
                self.workbench.clear_log_active();
                self.log_selection.clear();
                self.log_anchor = None;
                self.log_status = Some("log cleared".to_string());
            }
            Some(LogRequest::CopySelection) => {
                self.copy_log(LogCopy::Selection, ui.ctx());
            }
            Some(LogRequest::CopyAll) => {
                self.copy_log(LogCopy::All, ui.ctx());
            }
            Some(LogRequest::CopyDetails(index)) => {
                let line = self.view.logs.get(index).cloned();
                if let Some(line) = line {
                    ui.ctx().copy_text(format!("{}\r\n", line.text.trim_end()));
                    self.log_status = Some(format!(
                        "copied diagnostic details ({} chars)",
                        line.text.len()
                    ));
                }
            }
            Some(LogRequest::Save) => {
                self.save_log();
            }
            Some(LogRequest::Jump) => {
                // LOG-010: the requested jump is applied to the next pass's scroll position; the
                // repaint request is what makes that pass happen without another input.
                self.log_follow = true;
                self.log_unseen = 0;
                ui.ctx().request_repaint();
            }
            None => {}
        }
    }

    /// LOG-005: copies the selected lines or the whole retained log, in painted log order.
    fn copy_log(&mut self, what: LogCopy, ctx: &egui::Context) {
        let lines: Vec<String> = match what {
            LogCopy::Selection => self
                .log_selection
                .iter()
                .filter_map(|index| self.view.logs.get(*index))
                .map(|line| line.render())
                .collect(),
            LogCopy::All => self.view.logs.iter().map(|line| line.render()).collect(),
        };
        let count = lines.len();
        if count == 0 {
            self.log_status = Some("nothing to copy: no log line is selected".to_string());
            return;
        }
        let mut text = lines.join("\r\n");
        text.push_str("\r\n");
        let bytes = text.len();
        ctx.copy_text(text);
        self.log_status = Some(format!("copied {count} log line(s), {bytes} bytes"));
    }

    /// LOG-005 / LOG-006: writes the retained log to a file the user chooses.
    ///
    /// When the cap discarded earlier records the file starts with that fact, so an exported log
    /// can never claim to be the complete run while a prefix is missing.
    fn save_log(&mut self) {
        let (title, parent) = {
            let document = self.workbench.active_document();
            (
                document.title.clone(),
                document
                    .path
                    .as_ref()
                    .and_then(|path| path.parent().map(|parent| parent.to_path_buf())),
            )
        };
        let suggested = format!(
            "{}.log",
            title
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(title.as_str())
        );
        let Some(path) = self.dialogs.save_log(&suggested, parent.as_deref()) else {
            self.log_status = Some("save log cancelled".to_string());
            return;
        };
        let dropped = self.view.logs_dropped;
        let mut text = String::new();
        if dropped > 0 {
            text.push_str(&format!(
                "# truncated history: {dropped} earlier record(s) were discarded at the {} \
                 record cap\r\n",
                crate::workbench::LOG_CAP
            ));
        }
        for line in &self.view.logs {
            text.push_str(&line.render());
            text.push_str("\r\n");
        }
        let count = self.view.logs.len();
        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                self.log_status = Some(format!("saved {count} log line(s) to {}", path.display()));
            }
            Err(error) => {
                self.log_status = Some(format!("save log failed: {error}"));
            }
        }
    }

    /// LOG-004: opens the editor at a diagnostic's source position and highlights its span.
    ///
    /// The editor owns the caret, the selection and the scroll, so the jump and the highlight are
    /// one operation there; the pane focus follows so the keyboard is in the source the click
    /// named.
    fn jump_to_source_position(&mut self, line: usize, column: usize, ctx: &egui::Context) {
        let active = self.workbench.active();
        if self.editors[active]
            .jump_to_position(line, column)
            .is_some()
        {
            self.request_pane_focus(Pane::Editor, ctx);
            self.pending_focus = Some(surface_id());
            self.log_status = Some(format!("opened source at {line}:{column}"));
        } else {
            self.log_status = Some(format!("no source position {line}:{column} in this buffer"));
        }
    }

    /// LAY-002: the dedicated Symbolic Inspector pane.
    ///
    /// It shows the same committed inspection payload as the Workspace pane's Inspector view,
    /// but as its own titled pane beside the Logs pane, so the five Studio panes (editor,
    /// workspace, graph, logs, symbolic inspector) are each visible at once and can be resized
    /// independently. Its empty state and every "section missing" error come from
    /// [`paint_inspector`]; the pane reads only the committed snapshot, never the editor buffer.
    fn symbolic_inspector_pane(&mut self, ui: &mut Ui, rect: Rect, view: &DocumentView) {
        let state_line = if !view.has_snapshot {
            "empty".to_string()
        } else if view.result_inspection.is_some() {
            format!("rev {}", view.revision.0)
        } else {
            format!("rev {} · no payload", view.revision.0)
        };
        let header_hint =
            header_text_width("Symbolic Inspector") + header_text_width(&state_line) + 16.0;
        // SYM-007: computed before the pane borrows the tree's interaction state, so a copy or
        // export performed inside the paint has a destination to name.
        let export_dir = self.inspector_export_dir();
        let expansion = &mut self.inspector_expansion;
        let mut toggles: Vec<TreeRowSeam> = Vec::new();
        let mut controls: Vec<(String, Rect)> = Vec::new();
        with_pane(
            ui,
            rect,
            "Symbolic Inspector",
            header_hint,
            |ui, header, body| {
                header_row(ui, header, |ui| {
                    ui.label(
                        RichText::new("Symbolic Inspector")
                            .font(type_scale::Role::PaneTitle.font())
                            .color(theme::text()),
                    );
                    ui.label(RichText::new(&state_line).small().color(theme::dim()))
                        .on_hover_text(
                            "The six symbolic sections of the last committed result; the editor \
                         buffer is never inspected",
                        );
                });
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(body.shrink2(vec2(8.0, 4.0)))
                        .layout(Layout::top_down(Align::Min)),
                    |ui| {
                        let width = (body.width() - 112.0).max(80.0);
                        // LAY-011: egui's default minimum scrolled size is 64 points, which is taller
                        // than the bottom strip at the minimum window; the viewport would then report
                        // rows below the pane even though the paint clip is correct. The body is the
                        // authority for this pane's viewport, so the minimum is removed.
                        egui::ScrollArea::vertical()
                            .id_salt("symbolic_inspector_scroll")
                            .min_scrolled_height(0.0)
                            .min_scrolled_width(0.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let inspector_action = paint_inspector(
                                    ui,
                                    view,
                                    width,
                                    expansion,
                                    &mut toggles,
                                    &mut controls,
                                    export_dir.as_deref(),
                                );
                                if inspector_action == Some(empty::EmptyAction::Run) {
                                    self.workbench.run_active();
                                }
                            });
                    },
                );
            },
        );
        self.symbolic_inspector_toggle_rects = toggles;
        self.symbolic_inspector_control_rects = controls;
    }

    /// The one-line quick-eval field and its keyboard contract (RUN-002 / LOG-003).
    ///
    /// The label is visible and the hint text is the reference placeholder. Enter submits; Esc
    /// leaves the field without submitting and without editing the document; Up/Down restore
    /// submitted expressions newest first. Only an accepted submission clears the field.
    fn quick_eval_row(&mut self, ui: &mut Ui) {
        let mut submit = false;
        let mut navigate: i8 = 0;
        // LAY-002: the Logs pane is narrower now that the Symbolic Inspector shares the bottom
        // strip, so the row wraps its trailing hints and sizes the field to the space left
        // instead of laying a fixed 340-point field past the pane's right edge.
        ui.horizontal_wrapped(|ui| {
            let label = ui.label(RichText::new("Quick eval").strong().color(theme::text()));
            let id = quick_eval_id();
            let width = 340.0_f32.min((ui.available_width() - 150.0).max(90.0));
            let response = ui
                .add(
                    egui::TextEdit::singleline(self.quick_eval.text_mut())
                        .id(id)
                        .hint_text("quick eval — one-liner, press Enter")
                        .desired_width(width),
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

/// LOG-010: the fixed height of one painted log row.
///
/// Virtualisation requires uniform rows; a log line is one monospace line, so a fixed height is
/// its exact geometry rather than an approximation.
const LOG_ROW_HEIGHT: f32 = 15.0;

/// LOG-010: how many consecutive passes may ask for the tail without reaching it before follow
/// mode gives up (a scroll-area measurement difference, never a user scroll: that path is
/// detected separately and ends follow mode immediately).
const LOG_FOLLOW_RETRIES: u8 = 3;

/// LOG-005: one copy/clear/save request the Logs pane raises in a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogRequest {
    /// Empty the active document's log.
    Clear,
    /// Copy the selected rows, in log order.
    CopySelection,
    /// Copy every retained row.
    CopyAll,
    /// Copy one diagnostic line's full details.
    CopyDetails(usize),
    /// Write the retained log to a file.
    Save,
    /// LOG-010: return the viewport to the newest record and follow it again.
    Jump,
}

/// LOG-005: which log lines a copy command covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogCopy {
    /// The selected rows only.
    Selection,
    /// Every retained row.
    All,
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
///
/// `export_dir` is where a MathIR export is written (SYM-007); `None` disables the export control
/// instead of writing somewhere the user cannot find. `controls` receives every copy/export
/// control's key and rectangle, so the pointer tests address the same controls a user clicks.
fn paint_inspector(
    ui: &mut Ui,
    view: &DocumentView,
    width: f32,
    expansion: &mut InspectorExpansion,
    toggles: &mut Vec<TreeRowSeam>,
    controls: &mut Vec<(String, Rect)>,
    export_dir: Option<&Path>,
) -> Option<empty::EmptyAction> {
    // LAY-011: the caller already reserves the indent and scrollbar; do not floor this back up to
    // a width the narrow pane cannot hold.
    ui.set_max_width(width.max(80.0));
    ui.add(
        egui::Label::new(
            RichText::new("INSPECTOR · last committed result")
                .small()
                .color(theme::dim()),
        )
        .wrap(),
    );
    if !view.has_snapshot {
        // VIS-008: the inspector's empty state names the six sections it can show, why there is
        // nothing yet and the command that inspects a result; the caller runs a requested action.
        let state = empty::inspector();
        let available = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(
            available.min,
            vec2(available.width(), available.height().min(140.0)),
        );
        return empty::paint(ui, rect, &state);
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
        return None;
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
        return None;
    };

    let subject = &inspection.subject;
    let detail = subject
        .detail
        .as_deref()
        .map(|detail| format!(" · {detail}"))
        .unwrap_or_default();
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "subject  {} · {}{detail}",
                subject.kind, subject.path
            ))
            .monospace()
            .color(theme::accent()),
        )
        .wrap(),
    );
    // SYM-006: the engine's own status for this result, right where the result is inspected.
    paint_result_status(ui, view);

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
            paint_inspector_tree(ui, tree, expansion, toggles, controls);
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
                // SYM-005: whether this rewrite's preconditions were met, from the engine's own
                // published applicability. `Applicable` states the rule held unconditionally;
                // `ApplicableWithConditions` states it held only under the conditions listed
                // below; the empty value is the explicit "this engine published none" state, so a
                // step is never silently implied to be unconditional.
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("applicability  {}", step.applicability_label()))
                            .small()
                            .color(if step.applicability == "Applicable" {
                                theme::dim()
                            } else if step.applicability.is_empty() {
                                theme::warning()
                            } else {
                                theme::accent()
                            }),
                    )
                    .wrap(),
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

    // SYM-005: the rules the engine TRIED and rejected, each with its applicability decision and
    // the engine's reason. This is the half of the row a trace alone could never show: a rule that
    // did not fire has no step, so without this section the pane could only say "not applied".
    match &inspection.applicability {
        Section::Supplied(attempts) => {
            paint_inspector_heading(
                ui,
                "Applicability",
                &count_label(attempts.len(), "rejected rule", "rejected rules"),
            );
            if attempts.is_empty() {
                ui.label(
                    RichText::new(
                        "Every rule the engine tried fired or was not applicable this time; no \
                         rule was rejected for this result.",
                    )
                    .small()
                    .color(theme::dim()),
                );
            }
            for attempt in attempts {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("{}  {}", attempt.rule_id, attempt.status_label()))
                        .monospace()
                        .color(theme::warning()),
                );
                ui.label(
                    RichText::new(format!("precondition  {}", attempt.precondition))
                        .small()
                        .color(theme::dim()),
                );
                ui.label(
                    RichText::new(format!(
                        "pattern  {}",
                        if attempt.pattern_matched {
                            "matched this node"
                        } else {
                            "did not match this node"
                        }
                    ))
                    .small()
                    .color(theme::dim()),
                );
                if !attempt.reason.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("reason  {}", attempt.reason))
                                .small()
                                .color(theme::text()),
                        )
                        .wrap(),
                    );
                }
            }
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "Applicability", "");
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
            paint_mathir_body(ui, view, mathir, controls, export_dir);
        }
        Section::Missing(reason) => {
            paint_inspector_heading(ui, "MathIR", "");
            paint_inspector_reason(ui, reason);
        }
    }
    None
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
        ui.label(
            RichText::new(assumption.type_label())
                .small()
                .color(theme::accent()),
        );
        ui.label(
            RichText::new(&assumption.label)
                .monospace()
                .color(theme::text()),
        );
    });
    if let Some(canonical) = &assumption.canonical {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("canonical").small().color(theme::dim()));
            ui.label(
                RichText::new(canonical)
                    .small()
                    .monospace()
                    .color(theme::dim()),
            );
        });
    }
}

// ---------------------------------------------------------------------------------------------
// SYM-006: the status badge painted beside the result.
// ---------------------------------------------------------------------------------------------

/// The engine-published status badges for this committed result.
fn result_status_badges(view: &DocumentView) -> Vec<StatusBadge> {
    symbolic_status::badges(
        view.result_structured.as_ref(),
        view.result_inspection.as_ref(),
    )
}

/// Paints the status badges from the engine's own fields (SYM-006).
///
/// The badge is the engine's claim, not the pane's: a `Partial`, `Unevaluated` or conditional
/// result can never reach the completion colours, because `StatusBadge::claims_completion` is what
/// selects them.
fn paint_result_status(ui: &mut Ui, view: &DocumentView) {
    let badges = result_status_badges(view);
    if badges.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for badge in &badges {
            let color = if badge.claims_completion() {
                theme::positive()
            } else {
                match badge.tone {
                    Tone::Complete => theme::positive(),
                    Tone::Conditional => theme::warning(),
                    Tone::Incomplete => theme::warning(),
                }
            };
            ui.add(
                egui::Label::new(
                    RichText::new(&badge.word)
                        .small()
                        .strong()
                        .monospace()
                        .color(color),
                )
                .wrap(),
            )
            .on_hover_text(format!("{}: {}", badge.tone.label(), badge.detail));
            ui.add(
                egui::Label::new(RichText::new(&badge.detail).small().color(theme::dim())).wrap(),
            );
        }
    });
}

// ---------------------------------------------------------------------------------------------
// SYM-003 / SYM-011: the expression tree.
// ---------------------------------------------------------------------------------------------

/// SYM-011: the tree paints inside its own bounded, scrolled body, so a deep expression cannot
/// push the assumptions, trace and MathIR sections below the pane.
///
/// ROUND-67: the bound is a maximum, not a reservation. A shallow tree (the common case: the
/// inspector shows the RESULT expression, often one node) shrinks to its rows instead of spending
/// 260 points of empty space that would push the Applicability section out of a 1600x1000 pane.
const TREE_VIEWPORT_HEIGHT: f32 = 260.0;
/// SYM-003: the indentation one tree level adds, in points.
const TREE_INDENT: f32 = 14.0;

/// The widget id of one row's header. It is derived from the node path, so a keyboard move can
/// name the row it wants focused without the row having to be painted first.
fn tree_row_id(path: &str) -> Id {
    Id::unique(("inspector_tree_row", path))
}

/// The parent path of a tree row, or `None` for the root.
fn parent_tree_path(path: &str) -> Option<&str> {
    let (parent, _) = path.rsplit_once('.')?;
    Some(parent)
}

/// Paints the expression tree: the rows the current level bound paints, inside a scrolled body
/// that lays out only the rows in view (SYM-011).
///
/// Each row is one focusable control whose accessible name carries the node kind, the engine
/// rendering, the node path and the expand/collapse state, plus a `copy` control that puts that
/// node's engine canonical form on the clipboard (SYM-003). Arrow Up/Down move between rows,
/// Left/Right collapse and expand, Home/End jump to the ends.
fn paint_inspector_tree(
    ui: &mut Ui,
    root: &TreeNode,
    expansion: &mut InspectorExpansion,
    toggles: &mut Vec<TreeRowSeam>,
    controls: &mut Vec<(String, Rect)>,
) {
    let rows = tree_rows(root, expansion);
    let deepest = root.depth();
    let row_height = ui.text_style_height(&TextStyle::Monospace) + 8.0;
    ui.label(
        RichText::new(format!(
            "{} rows · levels {}/{} · {} nodes",
            rows.len(),
            expansion.levels().min(deepest),
            deepest,
            root.count()
        ))
        .small()
        .color(theme::dim()),
    );
    let pending = expansion.pending_focus().map(str::to_string);
    let mut scroll_to = pending
        .as_deref()
        .and_then(|path| rows.iter().position(|row| row.path == path))
        .map(|index| index as f32 * row_height);
    let mut focused: Option<usize> = None;
    let mut painted: Vec<(usize, Id)> = Vec::new();
    let mut copy_request: Option<(String, String)> = None;
    let mut toggle_request: Option<String> = None;
    let mut area = egui::ScrollArea::vertical()
        .id_salt("inspector_tree")
        .max_height(TREE_VIEWPORT_HEIGHT)
        .auto_shrink([false, true]);
    if let Some(offset) = scroll_to.take() {
        area = area.vertical_scroll_offset(offset);
    }
    area.show_rows(ui, row_height, rows.len(), |ui, range| {
        for index in range {
            let row = &rows[index];
            let width = ui.available_width().max(60.0);
            ui.allocate_ui_with_layout(
                vec2(width, row_height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add_space(2.0 + row.level as f32 * TREE_INDENT);
                    let glyph = row.glyph();
                    let visible = format!("{glyph} {}", row.header());
                    let color = if row.has_children {
                        theme::accent()
                    } else {
                        theme::text()
                    };
                    let response = ui
                        .push_id(&row.path, |ui| {
                            ui.add(
                                egui::Button::new(RichText::new(visible).monospace().color(color))
                                    .frame(false),
                            )
                        })
                        .inner;
                    let accessible = row.accessible_name();
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, accessible.clone())
                    });
                    painted.push((index, response.id));
                    if response.has_focus() {
                        focused = Some(index);
                        // SYM-003: the tree owns arrow-key traversal, so a focused row keeps
                        // both arrow axes instead of letting egui move the focus spatially.
                        ui.memory_mut(|memory| {
                            memory.set_focus_lock_filter(
                                response.id,
                                egui::EventFilter {
                                    tab: true,
                                    horizontal_arrows: true,
                                    vertical_arrows: true,
                                    escape: false,
                                },
                            )
                        });
                    }
                    if response.clicked() {
                        if row.has_children {
                            toggle_request = Some(row.path.clone());
                        }
                        ui.memory_mut(|memory| memory.request_focus(response.id));
                        ui.ctx().request_repaint();
                    }
                    let state = row.state_word();
                    response.clone().on_hover_text(format!(
                        "{} · {state} — click or press Space/Enter to {}",
                        row.path,
                        if row.collapsed { "expand" } else { "collapse" }
                    ));
                    toggles.push(TreeRowSeam {
                        path: row.path.clone(),
                        rect: response.rect,
                        id: response.id,
                    });

                    // SYM-003: copy this node's engine canonical form.
                    let enabled = !row.canonical.is_empty();
                    let copy = ui.add_enabled(
                        enabled,
                        egui::Button::new(
                            RichText::new("copy")
                                .small()
                                .monospace()
                                .color(theme::dim()),
                        )
                        .frame(false),
                    );
                    let copy_name = format!("Copy canonical of {}", row.header());
                    copy.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, copy_name.clone())
                    });
                    let hint = if enabled {
                        format!("Copy {} -> {}", row.path, row.canonical)
                    } else {
                        format!("{} carries no canonical form", row.path)
                    };
                    let copy = copy.on_hover_text(hint);
                    controls.push((format!("copy {}", row.path), copy.rect));
                    if copy.clicked() {
                        copy_request = Some((row.accessible_name(), row.canonical.clone()));
                    }
                },
            );
        }
    });
    if let Some((name, canonical)) = copy_request {
        copy_to_clipboard(ui, &name, &canonical);
    }
    if let Some(path) = toggle_request {
        expansion.toggle(&path);
        ui.memory_mut(|memory| memory.request_focus(tree_row_id(&path)));
        ui.ctx().request_repaint();
    }
    if let Some(index) = pending
        .as_deref()
        .and_then(|path| rows.iter().position(|row| row.path == path))
    {
        if let Some((_, id)) = painted.iter().find(|(row, _)| *row == index) {
            ui.memory_mut(|memory| memory.request_focus(*id));
        }
        expansion.clear_pending_focus();
    }

    // SYM-003: keyboard traversal of the rows.
    if let Some(index) = focused {
        let row = &rows[index];
        let (up, down, left, right, home, end) = ui.input(|input| {
            (
                input.key_pressed(Key::ArrowUp),
                input.key_pressed(Key::ArrowDown),
                input.key_pressed(Key::ArrowLeft),
                input.key_pressed(Key::ArrowRight),
                input.key_pressed(Key::Home),
                input.key_pressed(Key::End),
            )
        });
        let mut target: Option<usize> = None;
        if down && index + 1 < rows.len() {
            target = Some(index + 1);
        }
        if up && index > 0 {
            target = Some(index - 1);
        }
        if home {
            target = Some(0);
        }
        if end {
            target = Some(rows.len().saturating_sub(1));
        }
        if right && row.has_children && row.collapsed {
            expansion.set_collapsed(&row.path, false);
            ui.ctx().request_repaint();
        } else if right && row.has_children {
            target = Some(index + 1);
        }
        if left && row.has_children && !row.collapsed {
            expansion.set_collapsed(&row.path, true);
            ui.ctx().request_repaint();
        } else if left {
            if let Some(parent) = parent_tree_path(&row.path) {
                target = rows.iter().position(|candidate| candidate.path == parent);
            }
        }
        if let Some(target) = target {
            match painted.iter().find(|(row, _)| *row == target) {
                Some((_, id)) => {
                    ui.memory_mut(|memory| memory.request_focus(*id));
                    ui.ctx().request_repaint();
                }
                // The row is outside the painted range: ask the next pass to scroll to it.
                None => {
                    expansion.set_pending_focus(&rows[target].path);
                    ui.ctx().request_repaint();
                }
            }
        }
    }

    // SYM-011: the level bound, with what it is hiding.
    let hidden: usize = rows.iter().map(|row| row.deeper_nodes).sum();
    if expansion.has_deeper(deepest) && hidden > 0 {
        let label = format!("Load {TREE_LEVELS_STEP} more levels ({hidden} nodes hidden)");
        let button = ui.add(egui::Button::new(RichText::new(&label).small()));
        let name = label.clone();
        button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
        if button.clicked() {
            expansion.load_deeper(deepest);
            ui.ctx().request_repaint();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// SYM-007: the MathIR body.
// ---------------------------------------------------------------------------------------------

/// The transient status line an inspector copy/export leaves under its controls.
fn inspector_status_id() -> Id {
    Id::unique("inspector_transient_status")
}

/// Puts `text` on the platform clipboard through egui, and reports it in the pane.
///
/// `ui.ctx().copy_text` is the window's one clipboard path: eframe forwards it to the platform
/// clipboard, so the bytes a user pastes are the bytes the pane reports here.
fn copy_to_clipboard(ui: &mut Ui, what: &str, text: &str) {
    ui.ctx().copy_text(text.to_string());
    let message = format!("Copied {what} ({} bytes)", text.len());
    ui.data_mut(|data| data.insert_temp(inspector_status_id(), (message, false)));
    ui.ctx().request_repaint();
}

/// SYM-007: writes the engine's IR text, byte for byte, and reports the file in the pane.
fn export_mathir(ui: &mut Ui, view: &DocumentView, text: &str, dir: Option<&Path>) {
    let Some(dir) = dir else {
        ui.data_mut(|data| {
            data.insert_temp(
                inspector_status_id(),
                (
                    "Export MathIR: this window has no writable application directory".to_string(),
                    true,
                ),
            )
        });
        return;
    };
    let path = dir.join(format!("mathir-rev-{}.ir", view.revision.0));
    let status = match write_mathir_export(&path, text) {
        Ok(bytes) => {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            (format!("Exported MathIR: {bytes} bytes -> {name}"), false)
        }
        Err(error) => (format!("Export MathIR failed: {error}"), true),
    };
    ui.data_mut(|data| data.insert_temp(inspector_status_id(), status));
    ui.ctx().request_repaint();
}

/// SYM-007: writes the IR text unchanged to `path`, creating its directory.
fn write_mathir_export(path: &Path, text: &str) -> std::io::Result<usize> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text.as_bytes())?;
    Ok(text.len())
}

/// One class of MathIR text, so the IR is highlighted like an assembly listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IrClass {
    /// The `#!mathir <version>` format line.
    Directive,
    /// The `param` / `const` directives.
    Keyword,
    /// An instruction opcode (`PowInt`, `Mul`, ...).
    Opcode,
    /// An operand index or the constant-name text.
    Operand,
    /// A separator.
    Delimiter,
    /// Anything the format does not classify.
    Plain,
}

/// One highlighted run of one MathIR line.
#[derive(Clone, Debug, PartialEq, Eq)]
struct IrToken {
    text: String,
    class: IrClass,
}

/// Splits one line of the engine's IR text into its syntax classes.
///
/// The grammar is the engine's published line format (`mathir` `IrProgram::serialize`): a
/// `#!mathir <version>` header, `param <name>` and `const <name>` directives, then one
/// `<Opcode> <aux> <operands>` instruction per line. Nothing is added, renamed or reordered.
fn ir_line_tokens(line: &str) -> Vec<IrToken> {
    let plain = |text: &str| IrToken {
        text: text.to_string(),
        class: IrClass::Plain,
    };
    if line.is_empty() {
        return vec![plain(" ")];
    }
    if line.starts_with('#') {
        return vec![IrToken {
            text: line.to_string(),
            class: IrClass::Directive,
        }];
    }
    let (head, rest) = match line.split_once(' ') {
        Some((head, rest)) => (head, rest),
        None => (line, ""),
    };
    if head == "param" || head == "const" {
        let mut tokens = vec![IrToken {
            text: head.to_string(),
            class: IrClass::Keyword,
        }];
        if !rest.is_empty() {
            tokens.push(plain(" "));
            tokens.push(IrToken {
                text: rest.to_string(),
                class: IrClass::Operand,
            });
        }
        return tokens;
    }
    let mut tokens = vec![IrToken {
        text: head.to_string(),
        class: IrClass::Opcode,
    }];
    if rest.is_empty() {
        return tokens;
    }
    tokens.push(plain(" "));
    let (aux, operands) = match rest.split_once(' ') {
        Some((aux, operands)) => (aux, operands),
        None => (rest, ""),
    };
    tokens.push(IrToken {
        text: aux.to_string(),
        class: IrClass::Operand,
    });
    if !operands.is_empty() {
        tokens.push(plain(" "));
        for (index, operand) in operands.split(',').enumerate() {
            if index > 0 {
                tokens.push(IrToken {
                    text: ",".to_string(),
                    class: IrClass::Delimiter,
                });
            }
            tokens.push(IrToken {
                text: operand.to_string(),
                class: IrClass::Operand,
            });
        }
    }
    tokens
}

/// The layout job one MathIR line paints with, per-token colours from the syntax palette.
fn ir_layout_job(text: &str, font: FontId) -> LayoutJob {
    let palette = theme::syntax();
    let mut job = LayoutJob::default();
    // The IR is a listing: it scrolls horizontally instead of wrapping a line mid-instruction.
    job.wrap.max_width = f32::INFINITY;
    let mut first = true;
    for line in text.split('\n') {
        if !first {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: palette.plain,
                    ..Default::default()
                },
            );
        }
        first = false;
        for token in ir_line_tokens(line) {
            let color = match token.class {
                IrClass::Directive | IrClass::Keyword => palette.keyword,
                IrClass::Opcode => palette.builtin,
                IrClass::Operand => palette.number,
                IrClass::Delimiter => palette.delimiter,
                IrClass::Plain => palette.plain,
            };
            job.append(
                &token.text,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        }
    }
    job
}

/// Paints the MathIR section's body: the highlighted, monospaced, selectable, scrollable IR text,
/// with the copy and export controls beside it (SYM-007).
fn paint_mathir_body(
    ui: &mut Ui,
    view: &DocumentView,
    mathir: &crate::inspector::MathIr,
    controls: &mut Vec<(String, Rect)>,
    export_dir: Option<&Path>,
) {
    let font = TextStyle::Monospace.resolve(ui.style());
    let mut copied = false;
    let mut exported = false;
    ui.horizontal_wrapped(|ui| {
        let copy = ui.add(egui::Button::new(RichText::new("Copy MathIR").small()));
        let copy_name = "Copy MathIR".to_string();
        copy.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, copy_name.clone()));
        controls.push(("copy mathir".to_string(), copy.rect));
        if copy.clicked() {
            copied = true;
        }
        let export = ui.add_enabled(
            export_dir.is_some(),
            egui::Button::new(RichText::new("Export MathIR").small()),
        );
        let export_name = "Export MathIR".to_string();
        export.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, export_name.clone())
        });
        let export = export.on_hover_text(match export_dir {
            Some(dir) => format!("Write the IR unchanged to {}", dir.display()),
            None => "This window has no writable application directory".to_string(),
        });
        controls.push(("export mathir".to_string(), export.rect));
        if export.clicked() {
            exported = true;
        }
        ui.label(
            RichText::new(format!(
                "{} bytes · {} lines",
                mathir.text.len(),
                mathir.text.lines().count()
            ))
            .small()
            .color(theme::dim()),
        );
    });
    if copied {
        copy_to_clipboard(ui, "MathIR", &mathir.text);
    }
    if exported {
        export_mathir(ui, view, &mathir.text, export_dir);
    }
    if let Some((message, error)) =
        ui.data(|data| data.get_temp::<(String, bool)>(inspector_status_id()))
    {
        ui.add(
            egui::Label::new(RichText::new(message).small().color(if error {
                theme::warning()
            } else {
                theme::positive()
            }))
            .wrap(),
        );
    }
    // SYM-007 groundwork: monospaced, selectable, and scrollable both ways. The outer pane
    // scrolls vertically; this inner area bounds the IR's own height and width.
    egui::ScrollArea::both()
        .id_salt("inspector_mathir")
        .max_height(240.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(ir_layout_job(&mathir.text, font))
                    .selectable(true)
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        });
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
) -> Option<empty::EmptyAction> {
    // LAY-011: the caller already reserves the indent and scrollbar; do not floor this back up to
    // a width the narrow pane cannot hold.
    ui.set_max_width(width.max(80.0));
    if !view.has_snapshot {
        // VIS-008: no committed result yet, with the pane's purpose, the reason and the command
        // that produces one. The caller runs the returned action.
        let state = empty::results();
        let available = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(
            available.min,
            vec2(available.width(), available.height().min(120.0)),
        );
        let action = empty::paint(ui, rect, &state);
        return action;
    }
    ui.label(
        RichText::new("RESULT · structured projection")
            .small()
            .color(theme::dim()),
    );
    // SYM-006: the engine's own status for this result, beside the result it describes.
    paint_result_status(ui, view);
    let projection = ResultProjection::new(view.result.clone(), view.result_structured.clone());
    // RES-002: exactness and provenance, from the projection and the committed precision. A
    // value whose projection states no exactness says so instead of implying it is exact.
    if let Some(provenance) = crate::results_tree::Provenance::of(&projection, view.precision) {
        let line = provenance.line();
        ui.label(RichText::new(&line).small().monospace().color(
            if provenance.exactness == crate::results_tree::Exactness::Approximate {
                theme::warning()
            } else {
                theme::dim()
            },
        ))
        .on_hover_text(
            "The engine's own kind, exactness and precision for this value, and any print budget \
             that shortened its rendering",
        );
    }
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
    None
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
            ui.add(
                egui::Button::new(RichText::new(visible).monospace().color(theme::accent()))
                    .frame(false),
            )
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

/// RES-002 / RES-010: one scalar row: the projection kind, the value (bounded for paint), then the
/// remaining published fields in readable words.
///
/// The exactness the projection published is painted as `exact` / `approximate` rather than the
/// raw `exact=true`, and a shortened value names the reason and budget the engine published. A
/// value with no `exact` field paints no exactness word at all.
fn paint_scalar(ui: &mut Ui, scalar: &crate::results_tree::ScalarNode) {
    let (value, hidden) = bounded_value_text(&scalar.value);
    // RES-010: the paint bound is stated before the value, not after it: a value longer than the
    // bound pushes anything below it out of the pane, and the note explaining the shortening must
    // stay visible exactly when it matters.
    if hidden > 0 {
        ui.label(
            RichText::new(bounded_value_note(hidden))
                .small()
                .color(theme::warning()),
        )
        .on_hover_text("The whole value is kept in memory and copied in full");
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&scalar.kind).monospace().color(theme::dim()));
        ui.label(RichText::new(&value).monospace().color(theme::text()));
        for (key, raw) in &scalar.details {
            let text = describe_scalar_detail(key, raw);
            let color = match key.as_str() {
                "truncationReason" => theme::warning(),
                "exact" if raw == "false" => theme::warning(),
                _ => theme::dim(),
            };
            ui.label(RichText::new(text).small().monospace().color(color));
        }
    });
}

/// RES-002: one projection detail field in the pane's words instead of `key=raw`.
fn describe_scalar_detail(key: &str, raw: &str) -> String {
    match (key, raw) {
        ("exact", "true") => "exact".to_string(),
        ("exact", "false") => "approximate".to_string(),
        ("truncated", "true") => "truncated".to_string(),
        ("truncationReason", reason) => format!("truncated: {reason}"),
        ("budget", budget) => format!("budget {budget}"),
        _ => format!("{key}={raw}"),
    }
}

/// The absent value, painted where a node would be.
fn paint_absent(ui: &mut Ui) {
    ui.label(
        RichText::new("Null - no value")
            .monospace()
            .color(theme::dim()),
    );
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
        ui.label(
            RichText::new(format!("{key}={value}"))
                .small()
                .color(theme::dim()),
        );
    }
}

/// RES-003 / RES-010: above this many cells, an array is painted as a virtualised window.
///
/// Small arrays keep the collapsible per-element view (a 2-D matrix shows rows of cells, and a
/// symbolic cell is itself a node). A large array must not allocate a widget per element: the
/// pane paints only the rows inside its scroll viewport, so a 100 000-cell matrix costs the same
/// paint as a 100-cell one.
const VIRTUAL_ARRAY_CELLS: usize = 400;

/// The fixed height of one painted array row (virtualisation needs uniform rows).
const ARRAY_ROW_HEIGHT: f32 = 16.0;

/// The greatest height the virtualised array viewport takes inside the pane.
const ARRAY_VIEWPORT_HEIGHT: f32 = 200.0;

/// The most matrix columns one virtualised row paints; wider rows name the hidden count.
const MAX_MATRIX_COLUMNS: usize = 12;

/// Paints an array's body. A 2-D array's rows hold the cells laid out as columns; a 1-D array's
/// rows hold one element each. A higher-rank array names its shape and lists its row-major
/// elements exactly like a flat array.
///
/// RES-003 / RES-010: an array with more than [`VIRTUAL_ARRAY_CELLS`] cells is painted through a
/// virtualised scroll area instead: only the rows the viewport shows are laid out, each row is one
/// fixed-height line (so the scroll geometry is exact), and the element kinds stay visible beside
/// each value. The whole value is still in the model; nothing is dropped from the tree.
fn paint_array_body(
    ui: &mut Ui,
    array: &ArrayNode,
    path: &str,
    expansion: &mut ExpansionState,
    toggles: &mut Vec<(String, Rect)>,
) {
    if array.elements.len() > VIRTUAL_ARRAY_CELLS {
        paint_virtual_array(ui, array, path);
        return;
    }
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

/// RES-003 / RES-010: paints a large array one viewport at a time.
///
/// The scroll area owns the row geometry, so egui lays out exactly the rows it shows plus a
/// small margin: a 100 000-cell matrix paints the visible rows (each at most
/// [`MAX_MATRIX_COLUMNS`] columns) instead of one widget per element. Each row names the element
/// kinds it shows and how many columns it did not paint, so the shape and kinds stay visible.
fn paint_virtual_array(ui: &mut Ui, array: &ArrayNode, path: &str) {
    let rows = array.rows();
    let (row_count, columns) = if array.is_matrix() && !rows.is_empty() {
        (rows.len(), array.shape[1].max(1))
    } else {
        (array.elements.len(), 1)
    };
    let painted_columns = columns.min(MAX_MATRIX_COLUMNS);
    ui.label(
        RichText::new(format!(
            "virtualised: {row_count} row(s) of {columns} cell(s); painting the rows in view \
             ({} column(s) per row, {} element(s) in the value)",
            painted_columns,
            array.elements.len()
        ))
        .small()
        .color(theme::dim()),
    );
    if columns > painted_columns {
        ui.label(
            RichText::new(format!(
                "each row names its first {painted_columns} of {columns} columns; the rest stay in \
                 the value"
            ))
            .small()
            .color(theme::warning()),
        );
    }
    let row_height = ARRAY_ROW_HEIGHT;
    egui::ScrollArea::vertical()
        .id_salt(format!("{path}.virtual_rows"))
        .max_height(ARRAY_VIEWPORT_HEIGHT)
        .auto_shrink([false, true])
        .show_rows(ui, row_height, row_count, |ui, range| {
            for row_index in range {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [56.0, row_height],
                        egui::Label::new(
                            RichText::new(format!("[{row_index}]"))
                                .small()
                                .monospace()
                                .color(theme::accent()),
                        )
                        .truncate(),
                    );
                    for column_index in 0..painted_columns {
                        let node = if array.is_matrix() && !rows.is_empty() {
                            rows.get(row_index).and_then(|row| row.get(column_index))
                        } else {
                            array.elements.get(row_index)
                        };
                        let text = match node {
                            Some(StructuredNode::Scalar(scalar)) => {
                                let (value, _) = bounded_value_text(&scalar.value);
                                format!("{} {value}", scalar.kind)
                            }
                            Some(StructuredNode::Null) | None => "Null".to_string(),
                            Some(other) => other.headline(),
                        };
                        ui.add_sized(
                            [96.0, row_height],
                            egui::Label::new(
                                RichText::new(text).small().monospace().color(theme::text()),
                            )
                            .truncate(),
                        );
                    }
                    if columns > painted_columns {
                        ui.label(
                            RichText::new(format!("… {} more", columns - painted_columns))
                                .small()
                                .color(theme::dim()),
                        );
                    }
                });
            }
        });
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
///
/// VIS-001 review: the kind column must fit the engine's own kind names (`Natural`, `Integer`,
/// `Symbolic`, `Record`, `Complex`) at 13 pt monospace, not abbreviate every row to `Nat…`.
/// The name column gives up a little width for that; a long name still has its hover text and
/// the detail view.
fn variable_column_widths(available: f32) -> [f32; 3] {
    let usable = (available - 16.0).max(96.0);
    let name = (usable * 0.28).max(32.0);
    let kind = (usable * 0.36).max(46.0);
    let display = (usable - name - kind).max(16.0);
    [name, kind, display]
}

/// The width of one trailing row action (`×` delete, and the detail view's own controls).
const ROW_ACTION_WIDTH: f32 = 26.0;

/// RES-010: the most characters one value label paints.
///
/// A million-digit value must not become one million-glyph galley: the pane paints a bounded
/// prefix, states how much it hid and keeps the full text available to the copy command and to
/// the model. The bound is a paint bound only — nothing truncates the value in memory.
pub const MAX_PAINTED_VALUE_CHARS: usize = 4096;

/// RES-010: bounds one value string for painting, returning the text and how many characters were
/// withheld (`0` when the whole value is painted).
fn bounded_value_text(text: &str) -> (String, usize) {
    let total = text.chars().count();
    if total <= MAX_PAINTED_VALUE_CHARS {
        return (text.to_string(), 0);
    }
    let head: String = text.chars().take(MAX_PAINTED_VALUE_CHARS).collect();
    (head, total - MAX_PAINTED_VALUE_CHARS)
}

/// RES-010 / WKS-006: the one-line note a bounded value paints under itself.
fn bounded_value_note(hidden: usize) -> String {
    format!(
        "… {hidden} more character(s) not painted (the full value is kept and copied); \
         the pane paints at most {MAX_PAINTED_VALUE_CHARS}"
    )
}

/// WKS-006: the detail view of one variable, painted under the variables table.
///
/// The table cell is a bounded preview; this view is the whole value the committed snapshot
/// holds — the full display text, and the engine's published structure behind it (shape and
/// element kinds for an array, type and fields for a record, pretty/canonical/domain/free symbols
/// for a symbolic value, the real and imaginary parts for a complex number). The text is kept
/// untruncated in memory; only the paint is bounded, and the copy command carries every character.
fn paint_variable_detail(ui: &mut Ui, row: &crate::workspace_table::VariableRow) {
    ui.add_space(4.0);
    ui.scope_builder(
        UiBuilder::new()
            .accessibility_role(egui::Role::Group)
            .accessibility_label(format!("Variable details · {}", row.name)),
        |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("Details · {}", row.name))
                        .small()
                        .strong()
                        .color(theme::accent()),
                );
                ui.label(
                    RichText::new(&row.preview.type_label)
                        .small()
                        .monospace()
                        .color(theme::dim()),
                );
                if row.preview.shortened {
                    ui.label(
                        RichText::new("preview shortened in the cell")
                            .small()
                            .color(theme::warning()),
                    );
                }
            });
            let (shown, hidden) = bounded_value_text(&row.display);
            ui.add(
                egui::Label::new(
                    RichText::new(&shown)
                        .monospace()
                        .small()
                        .color(theme::text()),
                )
                .wrap(),
            );
            if hidden > 0 {
                ui.label(
                    RichText::new(bounded_value_note(hidden))
                        .small()
                        .color(theme::warning()),
                );
            }
            if let Some(structured) = &row.structured {
                paint_value_structure(ui, structured);
            } else {
                ui.label(
                    RichText::new(
                        "The engine published no structured projection for this value; only the \
                         display text is available.",
                    )
                    .small()
                    .color(theme::dim()),
                );
            }
            ui.horizontal(|ui| {
                let copy = ui.add(egui::Button::new(RichText::new("Copy value").small()));
                copy.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("Copy value of {}", row.name),
                    )
                });
                if copy.clicked() {
                    copy_to_clipboard(ui, &format!("value of {}", row.name), &row.display);
                }
                ui.label(
                    RichText::new(format!("{} characters", row.display.chars().count()))
                        .small()
                        .color(theme::dim()),
                )
                .on_hover_text("The full value, not the preview and not the painted prefix");
            });
        },
    );
    ui.add_space(4.0);
}

/// WKS-006 / RES-002: the published structure of one value, as readable rows.
///
/// Every field named here is one the engine's projection actually publishes; a field the
/// projection omitted is not invented.
fn paint_value_structure(ui: &mut Ui, structured: &lovelace_json::Json) {
    let text = |key: &str| -> Option<String> {
        match structured.get(key) {
            Some(lovelace_json::Json::Str(value)) => Some(value.clone()),
            Some(lovelace_json::Json::Num(value)) => Some(value.clone()),
            Some(lovelace_json::Json::Bool(value)) => Some(value.to_string()),
            _ => None,
        }
    };
    let kind = text("kind").unwrap_or_else(|| "value".to_string());
    ui.label(
        RichText::new(format!("structured · {kind}"))
            .small()
            .color(theme::dim()),
    );
    if let Some(type_name) = text("type") {
        ui.label(
            RichText::new(format!("type       {type_name}"))
                .small()
                .monospace()
                .color(theme::text()),
        );
    }
    if let Some(shape) = structured.get("shape").and_then(|shape| shape.as_arr()) {
        let shape: Vec<String> = shape
            .iter()
            .map(|item| match item {
                lovelace_json::Json::Num(number) => number.clone(),
                lovelace_json::Json::Str(value) => value.clone(),
                _ => "?".to_string(),
            })
            .collect();
        ui.label(
            RichText::new(format!("shape      {}", shape.join(" x ")))
                .small()
                .monospace()
                .color(theme::text()),
        );
    }
    let mut scalar_rows: Vec<(String, String)> = Vec::new();
    for key in [
        "value",
        "re",
        "im",
        "pretty",
        "canonical",
        "domain",
        "nodeCount",
        "exact",
        "truncated",
        "truncationReason",
        "budget",
    ] {
        if let Some(value) = text(key) {
            scalar_rows.push((key.to_string(), value));
        }
    }
    if let Some(symbols) = structured.get("freeSymbols").and_then(|s| s.as_arr()) {
        let symbols: Vec<String> = symbols
            .iter()
            .filter_map(|item| match item {
                lovelace_json::Json::Str(value) => Some(value.clone()),
                _ => None,
            })
            .collect();
        if !symbols.is_empty() {
            scalar_rows.push(("freeSymbols".to_string(), symbols.join(", ")));
        }
    }
    for (key, value) in scalar_rows {
        let (shown, hidden) = bounded_value_text(&value);
        let line = format!("{key:<10} {shown}");
        ui.add(
            egui::Label::new(RichText::new(line).small().monospace().color(theme::text())).wrap(),
        );
        if hidden > 0 {
            ui.label(
                RichText::new(bounded_value_note(hidden))
                    .small()
                    .color(theme::warning()),
            );
        }
    }
    // Arrays and records: the published children, with element kinds, in publication order.
    let children: Vec<lovelace_json::Json> = structured
        .get("elements")
        .and_then(|elements| elements.as_arr())
        .map(|elements| elements.to_vec())
        .unwrap_or_else(|| {
            structured
                .get("fields")
                .and_then(|fields| fields.as_arr())
                .map(|fields| fields.to_vec())
                .unwrap_or_default()
        });
    for (index, child) in children.iter().take(MAX_STRUCTURE_CHILDREN).enumerate() {
        let child_kind = child
            .get("kind")
            .and_then(|kind| kind.as_str())
            .unwrap_or("value");
        let child_value = match child {
            lovelace_json::Json::Str(value) => value.clone(),
            _ => child
                .get("value")
                .and_then(|value| match value {
                    lovelace_json::Json::Str(text) => Some(text.clone()),
                    lovelace_json::Json::Num(text) => Some(text.clone()),
                    _ => None,
                })
                .or_else(|| {
                    child
                        .get("pretty")
                        .and_then(|pretty| pretty.as_str().map(str::to_string))
                })
                .unwrap_or_default(),
        };
        let name = child
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("[{index}]"));
        let (shown, _) = bounded_value_text(&child_value);
        ui.label(
            RichText::new(format!("  {name}  {child_kind}  {shown}"))
                .small()
                .monospace()
                .color(theme::dim()),
        );
    }
    if children.len() > MAX_STRUCTURE_CHILDREN {
        ui.label(
            RichText::new(format!(
                "  … {} more element(s) in the value; the detail view paints the first {}",
                children.len() - MAX_STRUCTURE_CHILDREN,
                MAX_STRUCTURE_CHILDREN
            ))
            .small()
            .color(theme::warning()),
        );
    }
}

/// WKS-006 / RES-010: the most structured children the detail view paints at once.
pub const MAX_STRUCTURE_CHILDREN: usize = 64;

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

/// The visible origin colour: builtins are quiet and user definitions take the accent so the two
/// rows are distinguishable at a glance.
fn origin_color(origin: FunctionOrigin) -> Color32 {
    match origin {
        FunctionOrigin::Builtin => theme::dim(),
        FunctionOrigin::User => theme::accent(),
    }
}

/// VIS-002 / VIS-007: the semantic colour of the status strip's run-state text.
///
/// Every branch is the semantic role table's token, so the strip cannot paint a semantic colour
/// the role table does not also document with a marker and a shape.
fn status_state_color(state: crate::run_state::RunState) -> Color32 {
    semantic::SemanticRole::for_run_state(state)
        .map(|role| role.color())
        .unwrap_or_else(theme::accent)
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

/// VIS-006: paints the disabled cue when the widget's own response resolves to Disabled.
///
/// The state is not inferred from the caller's `bool`: [`control::ControlFlags::from_response`]
/// reads `Response::enabled()` (the flag egui itself uses) and [`control::ControlFlags::resolve`]
/// chooses the state with the documented precedence. The cue is a dashed outline plus a hatch, so
/// a disabled control is different in shape, not only in opacity.
fn mark_disabled(ui: &Ui, response: &Response) {
    let flags = control::ControlFlags::from_response(response, false, false);
    if flags.resolve() == control::ControlState::Disabled {
        control::paint_disabled_cue(ui, response.rect);
    }
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
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColumnHeader, true, name.clone()));
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
    let rows = (1.35 * hint / available).ceil().clamp(1.0, 6.0);
    // LAY-011: 24 points per painted row, not 20: a 20 pt button plus egui's item spacing and the
    // header's own padding occupy a little more than the control's height, and a dense split at
    // 200% wrapped the plot header onto four rows, whose last row overlapped the canvas when the
    // allowance was only the button height.
    4.0 + rows * 24.0
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

/// PLT-008 acceptance seam: the capture the window loads, optionally replaced by an invalid one.
///
/// The shipped window can only receive a capture the engine produced, and the engine never
/// produces a malformed or oversized document. To observe the pane's bounded failure in the real
/// window, `LOVELACE_PLOT_FAULT=malformed` or `=oversized` replaces the capture bytes at the same
/// point the window would load them; the fault is off unless the variable is set, no script can
/// reach it, and it changes nothing about how the pane handles the document. This mirrors the
/// `LOVELACE_ENGINE_FAULT` seam the RUN-017 recovery capture already uses.
fn faulted_capture_svg(svg: &str) -> String {
    match std::env::var("LOVELACE_PLOT_FAULT").ok().as_deref() {
        Some("malformed") => {
            // Truncated mid-path: the exact defect class a damaged capture would show.
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"600\"><path d=\"M 0 0 L"
                .to_string()
        }
        Some("oversized") => {
            // One byte over the pane's cap, so the refusal is the size rule and not a parse error.
            let padding = "x".repeat(crate::ui::plot::MAX_SVG_BYTES + 1);
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"600\"><desc>{padding}</desc></svg>"
            )
        }
        Some("heavy") => {
            // PLT-012: a document whose rasterisation is heavy but whose parse is not, so the
            // window capture can show how many frames the UI painted while the canvas worker
            // rasterised it. Four series of 30 000 points each: 120 000 points inside ~1.6 MiB,
            // drawn as four dense bands rather than one solid block.
            let mut svg = String::from(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"600\">\
                 <rect width=\"800\" height=\"600\" fill=\"#ffffff\"/>",
            );
            for (series, (base, stroke)) in [
                (110.0f32, "#e6194b"),
                (240.0, "#3cb44b"),
                (370.0, "#4363d8"),
                (500.0, "#f58231"),
            ]
            .iter()
            .enumerate()
            {
                svg.push_str(&format!(
                    "<polyline fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" points=\""
                ));
                for point in 0..30_000 {
                    let x = 8.0 + point as f32 * (784.0 / 29_999.0);
                    let y = base
                        + 42.0 * ((point as f32 * 0.19) + series as f32 * 1.7).sin()
                        + 10.0 * ((point as f32 * 0.031) + series as f32).cos();
                    svg.push_str(&format!("{x:.1},{y:.1} "));
                }
                svg.push_str("\"/>");
            }
            svg.push_str("</svg>");
            svg
        }
        _ => svg.to_string(),
    }
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
    // LAY-011: a header never takes more than the pane has, so a compressed pane cannot push its
    // body (and the controls painted in it) below the client. The clip keeps every painted shape
    // inside the pane as a second line of defence.
    let header_h = header_height(rect.width(), header_hint).min((rect.height() - 4.0).max(0.0));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .accessibility_role(egui::Role::Pane)
            .accessibility_label(title),
        |ui| {
            ui.shrink_clip_rect(rect);
            ui.painter().rect_filled(rect, 4.0, theme::panel_bg());
            ui.painter().rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0, theme::border()),
                StrokeKind::Inside,
            );
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), header_h));
            ui.painter().rect_filled(header, 4.0, theme::header_bg());
            let body_top = (header.bottom() + 1.0).min(rect.bottom() - 1.0);
            let body = Rect::from_min_max(
                pos2(rect.left() + 1.0, body_top),
                pos2(rect.right() - 1.0, rect.bottom() - 1.0),
            );
            add(ui, header, body)
        },
    )
    .inner
}

/// LAY-003: what one painted splitter did in this pass.
#[derive(Clone, Copy, Debug, Default)]
struct SplitterOutcome {
    /// Movement of the divider along its own axis, in points.
    delta: f32,
    /// True on the pass the pointer released a drag: the moment to persist the new sizes.
    drag_stopped: bool,
    /// LAY-003: a double-click asks for the shipped pane sizes.
    reset: bool,
    /// True when an arrow/Home/End key moved the divider: persist at once (no release follows).
    keyboard: bool,
}

fn splitter(
    ui: &mut Ui,
    rect: Rect,
    salt: &str,
    horizontal_axis: bool,
    label: &str,
) -> SplitterOutcome {
    let id = Id::unique(salt);
    let response = ui.interact(rect, id, Sense::click_and_drag());
    if response.clicked() || response.drag_started() || response.is_pointer_button_down_on() {
        response.request_focus();
    }
    let focused = response.has_focus();
    let hot = response.hovered() || response.dragged() || focused;
    ui.painter().rect_filled(
        rect,
        0.0,
        if hot {
            theme::splitter_hot()
        } else {
            theme::splitter_bg()
        },
    );
    let grip = if horizontal_axis {
        Rect::from_center_size(rect.center(), vec2(2.0, rect.height()))
    } else {
        Rect::from_center_size(rect.center(), vec2(rect.width(), 2.0))
    };
    ui.painter().rect_filled(
        grip,
        1.0,
        if hot {
            theme::accent()
        } else {
            theme::control_border()
        },
    );

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

    // LAY-003: a focused splitter resizes from the keyboard. Arrow keys step 8 points (32 with
    // Shift); Home/End move the divider to its extreme, where the layout's minimum-size clamp
    // stops it. `keyboard` is reported so the caller persists the new size immediately.
    let mut keyboard = false;
    if focused {
        let step = if ui.input(|input| input.modifiers.shift) {
            32.0
        } else {
            8.0
        };
        if horizontal_axis {
            if ui.input(|input| input.key_pressed(Key::ArrowRight)) {
                delta += step;
                keyboard = true;
            }
            if ui.input(|input| input.key_pressed(Key::ArrowLeft)) {
                delta -= step;
                keyboard = true;
            }
        } else {
            if ui.input(|input| input.key_pressed(Key::ArrowDown)) {
                delta += step;
                keyboard = true;
            }
            if ui.input(|input| input.key_pressed(Key::ArrowUp)) {
                delta -= step;
                keyboard = true;
            }
        }
        if ui.input(|input| input.key_pressed(Key::Home)) {
            delta -= 1000.0;
            keyboard = true;
        }
        if ui.input(|input| input.key_pressed(Key::End)) {
            delta += 1000.0;
            keyboard = true;
        }
    }

    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, label));
    SplitterOutcome {
        delta,
        drag_stopped: response.drag_stopped(),
        reset: response.double_clicked(),
        keyboard,
    }
}

/// The application icon: the complete heart-and-infinity emblem, with no wordmark, on the
/// banner's near-black ground (`assets/rustlace-icon.png`, 512x512).
///
/// The PNG is embedded so the window icon is identical on both platforms; eframe decodes it at
/// startup. A missing or unreadable asset costs the icon, not the window, so the fallback is an
/// empty icon.
pub(crate) fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../assets/rustlace-icon.png"))
        .unwrap_or_default()
}

/// The native window options shared by the workbench and the EDT-016 measurement mode.
pub(crate) fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Lovelace Desktop")
            .with_icon(window_icon()),
        // Renderer selection is deliberate and platform-conditional. Windows keeps eframe's
        // default (wgpu), which is the backend the shipped product has always used there, so
        // nothing about the Windows window changes. Linux selects glow (OpenGL): eframe's
        // default feature set is wgpu-only, and wgpu cannot acquire a device under WSLg (no
        // /dev/dri; the XWayland/EGL path reports "Parent device is lost") while GLX through
        // mesa llvmpipe renders. See Cargo.toml for the matching glow feature.
        #[cfg(not(windows))]
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    }
}

/// Opens the native workbench window and runs it until the user closes it.
pub fn run(initial_path: Option<PathBuf>) -> i32 {
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

    /// The icon is the complete infinity emblem: a transparent rounded corner, the banner's
    /// near-black ground, the cream stroke on both right-hand branches (upper lobe and the lower
    /// branch that completes it), the orange heart, and the same 512x512 image reaches the native
    /// window options.
    #[test]
    fn window_icon_is_the_complete_infinity_emblem() {
        let icon = window_icon();
        assert_eq!((icon.width, icon.height), (512, 512));
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            let offset = (y * icon.width as usize + x) * 4;
            [
                icon.rgba[offset],
                icon.rgba[offset + 1],
                icon.rgba[offset + 2],
                icon.rgba[offset + 3],
            ]
        };
        // Outside the rounded tile, the banner's ground, and the infinity's cream stroke in the
        // upper lobe, then in the lower branch that closes the right-hand side.
        assert_eq!(pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(256, 20), [3, 3, 3, 255]);
        assert!(pixel(452, 232)[..3].iter().all(|channel| *channel >= 230));
        assert!(pixel(439, 329)[..3].iter().all(|channel| *channel >= 230));
        // The heart's orange, with no transparent pixel in the mark.
        let heart = pixel(169, 213);
        assert!(heart[0] >= 240 && (90..=160).contains(&heart[1]) && heart[2] <= 40);
        assert_eq!(heart[3], 255);

        let wired = native_options()
            .viewport
            .icon
            .expect("native options carry the window icon");
        assert_eq!((wired.width, wired.height), (512, 512));
    }

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

    /// RUN-005: a test engine that reports kernel sub-progress through the REAL seam (the sink the
    /// execution core installs) and then holds the statement in flight until the test releases it,
    /// so a frame can be painted in the middle of one long kernel.
    struct KernelReportingEngine {
        gate: Arc<Gate>,
        sink: Option<crate::engine::KernelProgressSink>,
    }

    impl Evaluator for KernelReportingEngine {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            if let Some(sink) = &self.sink {
                sink("sqrt:integer-square-root", 0.10);
            }
            self.gate.wait();
            Ok("1.4142".to_string())
        }

        fn set_kernel_progress_sink(&mut self, sink: Option<crate::engine::KernelProgressSink>) {
            self.sink = sink;
        }
    }

    /// The window paints the kernel segment beside the statement count while the statement that
    /// owns it is still running, and stops painting it once that statement has committed.
    #[test]
    fn the_status_strip_paints_the_kernel_sub_progress_beside_the_statement_count() {
        let gate = Arc::new(Gate::default());
        let factory_gate = Arc::clone(&gate);
        let workbench = Workbench::with_factory(Box::new(move || {
            Box::new(KernelReportingEngine {
                gate: Arc::clone(&factory_gate),
                sink: None,
            })
        }));
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        app.workbench.set_source(0, "sqrt(2)\n".to_string());
        assert!(
            app.workbench.run_active(),
            "the run must be accepted before a frame can paint it"
        );
        // Paint passes until the strip shows the kernel's own phase. The engine is blocked in its
        // own evaluation, so this cannot race the commit: the statement stays in flight until the
        // gate is released below.
        let mut texts = Vec::new();
        let mut observed = false;
        for _ in 0..40 {
            frame(&ctx, &mut app, Vec::new());
            texts = pump_text(&ctx, &mut app, 1);
            if texts.iter().any(|text| text.contains("kernel sqrt:")) {
                observed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            observed,
            "the status strip never painted the kernel sub-progress: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("kernel sqrt:integer-square-root 10%")),
            "the painted strip must name the engine's phase and fraction: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("0/1")),
            "the statement count must be painted beside it: {texts:?}"
        );
        gate.release();
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(30)),
            "the run must finish once the gate opens"
        );
        let after = pump_text(&ctx, &mut app, 1);
        assert!(
            !after.iter().any(|text| text.contains("kernel sqrt:")),
            "a committed statement's kernel segment must not linger: {after:?}"
        );
        assert!(
            after.iter().any(|text| text.contains("1/1")),
            "the committed statement count stays painted: {after:?}"
        );
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

    /// DOC-006: the center of the conflict prompt button the test wants to click.
    fn conflict_button(app: &WorkbenchApp, choice: ConflictAnswer) -> egui::Pos2 {
        app.conflict_buttons
            .iter()
            .find(|(painted, _)| *painted == choice)
            .map(|(_, rect)| rect.center())
            .unwrap_or_else(|| panic!("the {choice:?} button was not painted"))
    }

    /// DOC-006: the buttons the open conflict prompt painted, in their on-screen order.
    fn conflict_choices(app: &WorkbenchApp) -> Vec<&'static str> {
        app.conflict_buttons
            .iter()
            .map(|(choice, _)| choice.label())
            .collect()
    }

    /// DOC-006: the center of the dirty-buffer Reload confirmation button.
    fn reload_button(app: &WorkbenchApp, choice: ReloadAnswer) -> egui::Pos2 {
        app.reload_buttons
            .iter()
            .find(|(painted, _)| *painted == choice)
            .map(|(_, rect)| rect.center())
            .unwrap_or_else(|| panic!("the {choice:?} button was not painted"))
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
        assert_eq!(
            app.theme,
            theme::ThemeId::Dark,
            "dark is the shipped default"
        );
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
        assert_eq!(
            app.settings.theme,
            theme::ThemeId::Light,
            "the choice persists"
        );
        assert_eq!(
            app.workbench.documents()[0].source,
            before_source,
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
            app.workbench.documents()[0].revision,
            before_revision,
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
        assert_eq!(
            ctx.global_style().visuals.widgets.inactive.bg_fill,
            light.button_bg
        );
        assert!(
            !ctx.global_style().visuals.dark_mode,
            "light text transfer function"
        );

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
        frame(
            &egui::Context::default(),
            &mut app_with_source("1", 0, 0),
            Vec::new(),
        );
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
        assert!(
            app.goto_line.is_none(),
            "a successful jump closes the prompt"
        );
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
            app.find_buttons
                .iter()
                .any(|(label, _)| *label == "Replace all"),
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
            app.workbench.documents()[0].source,
            "1 two 1 two 1",
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
            app.editors[0]
                .find
                .error()
                .map(|error| { error.contains("Invalid regular expression") }),
            Some(true),
            "the invalid pattern is a visible error state"
        );
        assert!(
            app.editors[0]
                .find_summary()
                .expect("open")
                .starts_with("Find: invalid regex")
        );
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

        let mut compile = completion_app("", 0, 0, vec![builtin("compile", &["f", "params"])]);
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

    /// EDT-017: the focused editor asks the integration for IME every pass, at the caret.
    ///
    /// This is the request whose absence made egui-winit leave `set_ime_allowed(false)` and winit
    /// drop every `WM_IME_*` message: the platform composition could never reach the editor.
    #[test]
    fn the_focused_editor_requests_ime_at_its_caret_and_updates_it_each_pass() {
        let mut app = WorkbenchApp::with_workbench(Workbench::new());
        let ctx = egui::Context::default();
        // The first pass requests focus for the editor; the second paints it focused.
        frame(&ctx, &mut app, Vec::new());
        let output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            Vec::new(),
            Default::default(),
        );
        let first = output.platform_output.ime.expect(
            "a focused editor must set PlatformOutput::ime; without it the platform drops IME",
        );
        assert_eq!(first.purpose, egui::IMEPurpose::Normal);
        assert!(
            first.rect.is_positive(),
            "the request carries the editor's on-screen text rect: {:?}",
            first.rect
        );
        assert!(
            first.rect.contains_rect(first.cursor_rect),
            "the caret rect {:?} lies inside the text surface {:?}",
            first.cursor_rect,
            first.rect
        );
        assert!(
            (first.cursor_rect.width() - 1.0).abs() < 0.01 && first.cursor_rect.height() > 1.0,
            "the cursor rect is a thin caret: {:?}",
            first.cursor_rect
        );

        // A moved caret must move the request in the same pass, or the candidate window would
        // stay at the old position.
        app.editors[0].insert_str("ab");
        app.editors[0].set_caret(0, 2);
        let output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            Vec::new(),
            Default::default(),
        );
        let moved = output
            .platform_output
            .ime
            .expect("the still-focused editor keeps requesting IME");
        assert!(
            moved.cursor_rect.min.x > first.cursor_rect.min.x,
            "the caret request followed the caret: {:?} -> {:?}",
            first.cursor_rect,
            moved.cursor_rect
        );
    }

    /// A11Y-008: a chord that arrives during a composition is withheld, and it is not replayed
    /// when the composition ends. A later deliberate chord must still work (the latch releases).
    #[test]
    fn a_chord_during_an_ime_composition_is_deferred_and_never_replayed() {
        let mut app = WorkbenchApp::with_workbench(Workbench::new());
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new()); // focus the editor
        frame(&ctx, &mut app, Vec::new()); // paint it focused
        let initial = app.editors[0].source();

        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        // The IME starts a composition; the editor paints the pre-edit and does not edit.
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "に".to_string(),
                active_range_chars: None,
            })],
        );
        assert_eq!(
            app.editors[0].preedit.as_deref(),
            Some("に"),
            "the composition is on screen before the chord"
        );
        assert_eq!(app.workbench.documents()[0].state(), RunState::Idle);

        // Ctrl+Enter arrives while composing. A real IME also clears the composition on the
        // modifier key, which is a platform `Preedit("")`: the app must not run the document.
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(ctrl),
                key(Key::Enter, ctrl),
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: String::new(),
                    active_range_chars: None,
                }),
            ],
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Idle,
            "Ctrl+Enter must not start a run while an IME composition owns the keyboard"
        );
        assert_eq!(
            app.editors[0].source(),
            initial,
            "the chord must not edit the buffer"
        );

        // The platform can split a chord across passes: the composition has ended, but the Enter
        // key event arrives on its own. It must be dropped, not applied late.
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Idle,
            "the deferred chord must not be applied retroactively after the composition ends"
        );

        // A pass with no composition and no chord releases the deferral; a deliberate Ctrl+Enter
        // afterwards must run normally.
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );
        assert!(
            app.workbench.active_is_running(),
            "a deliberate Ctrl+Enter after the composition must still run; the deferral is not sticky"
        );
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(30)),
            "the accepted run must finish"
        );
    }

    /// A11Y-008: the deferral covers every focused text widget, not only the editor. The
    /// quick-eval field is an `egui::TextEdit` whose pre-edit the window cannot inspect, so the
    /// rule is driven by the window's IME events.
    #[test]
    fn a_chord_during_a_quick_eval_composition_is_deferred_too() {
        let mut app = WorkbenchApp::with_workbench(Workbench::new());
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        app.ime_probe_focus_quick_eval(&ctx);
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            app.ime_probe_snapshot(&ctx).3,
            "quick-eval",
            "the quick-eval field owns the keyboard"
        );
        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "か".to_string(),
                active_range_chars: None,
            })],
        );
        assert!(
            app.ime_composition,
            "the window's IME events mark the field's composition active"
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Idle,
            "Ctrl+Enter must not run while the quick-eval field composes"
        );

        // End the composition: the empty pre-edit releases the latch in that same pass, and the
        // next deliberate chord runs normally.
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            })],
        );
        assert!(
            !app.ime_composition,
            "the empty pre-edit ended the composition"
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Enter, ctrl)],
        );
        assert!(
            app.workbench.active_is_running(),
            "a deliberate Ctrl+Enter after the field's composition must still run"
        );
        assert!(
            app.workbench.wait_until_idle(Duration::from_secs(30)),
            "the accepted run must finish"
        );
    }

    /// A11Y-008: the deferral withholds accelerators, not ordinary typing: the first character
    /// after a commit lands in the buffer even on the pass where the latch is still held.
    #[test]
    fn typing_right_after_an_ime_commit_is_not_withheld_by_the_shortcut_deferral() {
        let mut app = WorkbenchApp::with_workbench(Workbench::new());
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "か".to_string(),
                active_range_chars: None,
            })],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("か".to_string()))],
        );
        assert!(
            app.editors[0].source().starts_with("か"),
            "the commit inserted the composed text at the caret: {:?}",
            app.editors[0].source()
        );
        // The previous pass still had a composition, so the latch is held on this one.
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::Text("x".to_string()),
                key(Key::X, egui::Modifiers::default()),
            ],
        );
        assert!(
            app.editors[0].source().starts_with("かx"),
            "plain typing after a commit must not be swallowed: {:?}",
            app.editors[0].source()
        );
        assert_eq!(
            app.workbench.documents()[0].state(),
            RunState::Idle,
            "typing must not have started a run"
        );
    }

    fn builtin(name: &str, parameters: &[&str]) -> EngineFunction {
        EngineFunction {
            name: name.to_string(),
            parameters: parameters.iter().map(|p| p.to_string()).collect(),
            is_builtin: true,
            module: "Core".to_string(),
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
            logs_dropped: 0,
            workspace_revision: 1,
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
                let _ = paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
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
                let _ = paint_result_section(ui, view, 420.0, expansion, toggles);
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
            let canonical = if kind == "Sym" {
                format!("(sym {label})")
            } else {
                format!("(rat {label} 1)")
            };
            Json::obj([
                ("kind", Json::Str(kind.to_string())),
                ("label", Json::Str(label.to_string())),
                // SYM-003: the engine publishes one canonical form per node.
                ("canonical", Json::Str(canonical)),
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
                                        "canonical",
                                        Json::Str(
                                            "(mul (rat 3 1) (pow (sym x) (rat 2 1)))".to_string(),
                                        ),
                                    ),
                                    (
                                        "children",
                                        Json::Arr(vec![
                                            leaf("Num", "3"),
                                            Json::obj([
                                                ("kind", Json::Str("Pow".to_string())),
                                                ("label", Json::Str("x^2".to_string())),
                                                (
                                                    "canonical",
                                                    Json::Str(
                                                        "(pow (sym x) (rat 2 1))".to_string(),
                                                    ),
                                                ),
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
                                // SYM-005: the fired step's own applicability, as the engine
                                // publishes it.
                                (
                                    "applicability",
                                    Json::Str("ApplicableWithConditions".to_string()),
                                ),
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
                    // SYM-005: the rejected rule with the engine's applicability and reason.
                    (
                        "applicability",
                        Json::obj([
                            (
                                "applied",
                                Json::Arr(vec![Json::obj([
                                    ("ruleId", Json::Str("logexp.exp-log".to_string())),
                                    (
                                        "applicability",
                                        Json::Str("ApplicableWithConditions".to_string()),
                                    ),
                                ])]),
                            ),
                            (
                                "attempts",
                                Json::Arr(vec![Json::obj([
                                    (
                                        "ruleId",
                                        Json::Str("pow.sqrt-square-nonnegative".to_string()),
                                    ),
                                    ("patternMatched", Json::Bool(true)),
                                    ("precondition", Json::Str("NotApplicable".to_string())),
                                    (
                                        "reason",
                                        Json::Str(
                                            "precondition false: the radicand is not known to be \
                                             nonnegative in this session"
                                                .to_string(),
                                        ),
                                    ),
                                ])]),
                            ),
                            (
                                "counts",
                                Json::obj([
                                    ("applied", Json::Num("1".to_string())),
                                    ("rejected", Json::Num("1".to_string())),
                                    ("conditioned", Json::Num("0".to_string())),
                                ]),
                            ),
                        ]),
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
        toggles: &mut Vec<TreeRowSeam>,
        events: Vec<egui::Event>,
    ) -> Vec<String> {
        paint_inspector_pass(ctx, view, expansion, toggles, events, None).0
    }

    /// One inspector pass with its text and the platform output (the clipboard commands a copy
    /// leaves behind), so a test can assert what the window would put on the clipboard.
    fn paint_inspector_pass(
        ctx: &egui::Context,
        view: &DocumentView,
        expansion: &mut InspectorExpansion,
        toggles: &mut Vec<TreeRowSeam>,
        events: Vec<egui::Event>,
        export_dir: Option<&std::path::Path>,
    ) -> (Vec<String>, egui::PlatformOutput, Vec<(String, Rect)>) {
        toggles.clear();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 1200.0))),
            events,
            ..Default::default()
        };
        let mut controls: Vec<(String, Rect)> = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let _ = paint_inspector(
                    ui,
                    view,
                    420.0,
                    expansion,
                    toggles,
                    &mut controls,
                    export_dir,
                );
            });
        });
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        (texts, output.platform_output, controls)
    }

    /// SYM-001 at the paint layer: all seven sections come from the payload, not from the display.
    /// SYM-005 adds the seventh: the applicability value on each step and the rejected rules with
    /// the engine's reason.
    #[test]
    fn the_inspector_paints_all_seven_sections_from_the_engine_payload() {
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
            texts.iter().any(|text| text.ends_with("Num · 3")),
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
        // SYM-005: the step states its applicability instead of claiming the engine publishes
        // none, and the wording distinguishes "conditioned" from "unconditional".
        assert!(
            texts.iter().any(
                |text| text == "applicability  conditioned (met under the required conditions)"
            ),
            "the step's applicability is painted: {texts:?}"
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
        // SYM-005: the rejected rule and the engine's own reason are painted, so an unmet
        // precondition is a reason rather than a silent absence.
        assert!(
            texts.iter().any(|text| text == "Applicability"),
            "the applicability section is painted: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "pow.sqrt-square-nonnegative  not applicable"),
            "the rejected rule with its status: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "precondition  NotApplicable"),
            "the engine's precondition value: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains(
                "reason  precondition false: the radicand is not known to be nonnegative"
            )),
            "the unmet precondition's reason is painted: {texts:?}"
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

    /// The widget id of the painted header for one tree path (the seam a click or an arrow key
    /// focuses).
    fn row_id(rows: &[TreeRowSeam], path: &str) -> Id {
        rows.iter()
            .find(|row| row.path == path)
            .map(|row| row.id)
            .unwrap_or_else(|| panic!("no painted row for {path}: {rows:?}"))
    }

    /// The rectangle of one painted inspector control (the pointer-test seam).
    fn control_rect(controls: &[(String, Rect)], key: &str) -> Rect {
        controls
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("no painted inspector control {key:?}: {controls:?}"))
    }

    /// Every text this pass would put on the platform clipboard.
    fn copied_texts(output: &egui::PlatformOutput) -> Vec<String> {
        output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// One key press as the window receives it.
    fn key_press(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// Clicks one painted control with real pointer events and returns the pass that released the
    /// button, whose output carries the click's effect (the clipboard command or the export).
    fn click_control(
        ctx: &egui::Context,
        view: &DocumentView,
        expansion: &mut InspectorExpansion,
        toggles: &mut Vec<TreeRowSeam>,
        controls: &mut Vec<(String, Rect)>,
        key: &str,
        export_dir: Option<&std::path::Path>,
    ) -> (Vec<String>, egui::PlatformOutput) {
        let pos = control_rect(controls, key).center();
        let _ = paint_inspector_pass(
            ctx,
            view,
            expansion,
            toggles,
            vec![egui::Event::PointerMoved(pos)],
            export_dir,
        );
        let _ = paint_inspector_pass(
            ctx,
            view,
            expansion,
            toggles,
            vec![pointer_button(pos, true)],
            export_dir,
        );
        let (texts, output, painted) = paint_inspector_pass(
            ctx,
            view,
            expansion,
            toggles,
            vec![pointer_button(pos, false)],
            export_dir,
        );
        *controls = painted;
        (texts, output)
    }

    /// Every painted galley and the colour of each of its sections, in paint order.
    fn collect_shape_colors(shape: &egui::Shape, out: &mut Vec<(String, Vec<Color32>)>) {
        match shape {
            egui::Shape::Text(text) => {
                let colors = text
                    .galley
                    .job
                    .sections
                    .iter()
                    .map(|section| section.format.color)
                    .collect();
                out.push((text.galley.text().to_string(), colors));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_shape_colors(shape, out);
                }
            }
            _ => {}
        }
    }

    /// The exact IR text the engine's compile path emits for `3*x^2`.
    fn supplied_ir_text() -> String {
        "#!mathir 2\nparam x\nParameter 0 0\nPowInt 0 0,1\nMul 0 0,2".to_string()
    }

    /// An inspection payload whose tree is one root with `children` leaves: the wide expression
    /// that must not be painted eagerly.
    fn wide_tree_view(children: usize) -> DocumentView {
        use lovelace_json::Json;
        let leaf = |index: usize| {
            Json::obj([
                ("kind", Json::Str("Sym".to_string())),
                ("label", Json::Str(format!("x{index}"))),
                ("canonical", Json::Str(format!("(sym x{index})"))),
                ("children", Json::Arr(Vec::new())),
            ])
        };
        let root = Json::obj([
            ("kind", Json::Str("Add".to_string())),
            ("label", Json::Str("sum".to_string())),
            ("canonical", Json::Str("(add ...)".to_string())),
            ("children", Json::Arr((0..children).map(leaf).collect())),
        ]);
        let mut view = result_view("Symbolic sum");
        view.result_inspection = Some(Json::obj([
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
                        Json::obj([("text", Json::Str("sum".to_string()))]),
                    ),
                    (
                        "tree",
                        Json::obj([
                            ("nodes", Json::Num((children + 1).to_string())),
                            ("root", root),
                        ]),
                    ),
                ]),
            ),
        ]));
        view
    }

    /// An inspection payload whose tree is a balanced binary expression of `2^(level+1) - 1`
    /// nodes, so the level bound has something deep to cut.
    fn deep_tree_view(level: usize) -> DocumentView {
        use lovelace_json::Json;
        fn node(level: usize) -> lovelace_json::Json {
            use lovelace_json::Json;
            if level == 0 {
                return Json::obj([
                    ("kind", Json::Str("Sym".to_string())),
                    ("label", Json::Str("x".to_string())),
                    ("canonical", Json::Str("(sym x)".to_string())),
                    ("children", Json::Arr(Vec::new())),
                ]);
            }
            Json::obj([
                ("kind", Json::Str("Add".to_string())),
                ("label", Json::Str(format!("d{level}"))),
                ("canonical", Json::Str(format!("(add d{level})"))),
                (
                    "children",
                    Json::Arr(vec![node(level - 1), node(level - 1)]),
                ),
            ])
        }
        let mut view = result_view("Symbolic d");
        view.result_inspection = Some(Json::obj([
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
                    ("pretty", Json::obj([("text", Json::Str("d".to_string()))])),
                    (
                        "tree",
                        Json::obj([
                            ("nodes", Json::Num(((1u64 << (level + 1)) - 1).to_string())),
                            ("root", node(level)),
                        ]),
                    ),
                ]),
            ),
        ]));
        view
    }

    /// SYM-003: every painted row has a control that copies THAT node's engine canonical form,
    /// and the clipboard receives exactly that string.
    #[test]
    fn sym_003_a_tree_row_copies_its_engine_canonical_form_to_the_clipboard() {
        let ctx = egui::Context::default();
        let view = inspector_view(supplied_mathir());
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let (_texts, _output, controls) =
            paint_inspector_pass(&ctx, &view, &mut expansion, &mut toggles, Vec::new(), None);
        for path in [
            "inspect.tree",
            "inspect.tree.0",
            "inspect.tree.1",
            "inspect.tree.1.0",
            "inspect.tree.1.1",
        ] {
            assert!(
                controls
                    .iter()
                    .any(|(key, _)| key == &format!("copy {path}")),
                "every row paints its copy control ({path}): {controls:?}"
            );
        }
        let mut controls = controls;
        let (texts, output) = click_control(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            &mut controls,
            "copy inspect.tree.1",
            None,
        );
        assert_eq!(
            copied_texts(&output),
            vec!["(pow (sym x) (rat 2 1))".to_string()],
            "the clipboard carries the engine canonical form of that node only"
        );
        assert!(
            texts
                .iter()
                .any(|text| text
                    .starts_with("Copied Pow · x^2 [inspect.tree.1] expanded (23 bytes)")),
            "the pane reports the copy with its size: {texts:?}"
        );
    }

    /// SYM-003: arrow keys traverse the expression tree. Down/Up move between rows in engine
    /// child order, Left collapses and Right expands the focused node, Home/End jump to the ends.
    #[test]
    fn sym_003_arrow_keys_traverse_the_tree_and_left_right_collapse_and_expand() {
        let ctx = egui::Context::default();
        let view = inspector_view(supplied_mathir());
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let _ = paint_inspector_pass(&ctx, &view, &mut expansion, &mut toggles, Vec::new(), None);
        // A pointer user reaches a row by clicking it; request the same focus the click would.
        let root = row_id(&toggles, "inspect.tree");
        ctx.memory_mut(|memory| memory.request_focus(root));
        let _ = paint_inspector_pass(&ctx, &view, &mut expansion, &mut toggles, Vec::new(), None);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(row_id(&toggles, "inspect.tree")),
            "the root row takes keyboard focus"
        );
        for (expected, key) in [
            ("inspect.tree.0", Key::ArrowDown),
            ("inspect.tree.1", Key::ArrowDown),
            ("inspect.tree.0", Key::ArrowUp),
            ("inspect.tree.1.1", Key::End),
            ("inspect.tree", Key::Home),
        ] {
            let _ = paint_inspector_pass(
                &ctx,
                &view,
                &mut expansion,
                &mut toggles,
                vec![key_press(key)],
                None,
            );
            let _ =
                paint_inspector_pass(&ctx, &view, &mut expansion, &mut toggles, Vec::new(), None);
            let focused = ctx.memory(|memory| memory.focused());
            let named = toggles
                .iter()
                .find(|row| Some(row.id) == focused)
                .map(|row| row.path.clone());
            assert_eq!(
                named.as_deref(),
                Some(expected),
                "{key:?} must move the focus to {expected} (focused id {focused:?})"
            );
            assert_eq!(focused, Some(row_id(&toggles, expected)), "stable ids");
        }
        // Left collapses the focused (expanded) node; Right expands it again.
        let _ = paint_inspector_pass(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![key_press(Key::ArrowLeft)],
            None,
        );
        assert!(
            expansion.is_collapsed("inspect.tree"),
            "ArrowLeft on an expanded node collapses it"
        );
        let collapsed =
            paint_inspector_frame(&ctx, &view, &mut expansion, &mut toggles, Vec::new());
        assert_eq!(
            collapsed
                .iter()
                .filter(|text| text.starts_with('\u{25be}'))
                .count(),
            0,
            "the collapsed tree paints no expanded row: {collapsed:?}"
        );
        let _ = paint_inspector_pass(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![key_press(Key::ArrowRight)],
            None,
        );
        assert!(
            !expansion.is_collapsed("inspect.tree"),
            "ArrowRight on a collapsed node expands it"
        );
        // A pointer click on the same header collapses it: the seam carries the painted rectangle.
        let pos = toggles
            .iter()
            .find(|row| row.path == "inspect.tree")
            .map(|row| row.rect.center())
            .expect("the root header was painted");
        let _ = paint_inspector_pass(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![egui::Event::PointerMoved(pos)],
            None,
        );
        let _ = paint_inspector_pass(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![pointer_button(pos, true)],
            None,
        );
        let _ = paint_inspector_pass(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            vec![pointer_button(pos, false)],
            None,
        );
        assert!(
            expansion.is_collapsed("inspect.tree"),
            "a pointer click on the painted header collapses the node"
        );
    }

    /// SYM-006: the status the engine publishes is painted where the result is, and a partial
    /// result never wears a completion word.
    #[test]
    fn sym_006_a_partial_result_is_labelled_partial_beside_the_result_and_in_the_inspector() {
        use lovelace_json::Json;
        let ctx = egui::Context::default();
        let enum_value = |variant: &str| {
            Json::obj([
                ("kind", Json::Str("Enum".to_string())),
                ("type", Json::Str("SolveStatus".to_string())),
                ("value", Json::Str(variant.to_string())),
            ])
        };
        let boolean = |value: bool| {
            Json::obj([
                ("kind", Json::Str("Boolean".to_string())),
                (
                    "value",
                    Json::Str(if value { "true" } else { "false" }.to_string()),
                ),
            ])
        };
        let solve_view = |variant: &str, complete: bool| {
            let mut view = inspector_view(supplied_mathir());
            view.result_structured = Some(Json::obj([
                ("kind", Json::Str("Record".to_string())),
                ("type", Json::Str("SolveResult".to_string())),
                (
                    "fields",
                    Json::Arr(vec![
                        Json::obj([
                            ("name", Json::Str("status".to_string())),
                            ("value", enum_value(variant)),
                        ]),
                        Json::obj([
                            ("name", Json::Str("complete".to_string())),
                            ("value", boolean(complete)),
                        ]),
                    ]),
                ),
            ]));
            view
        };

        let partial = solve_view("Partial", false);
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let result_texts =
            paint_result_frame(&ctx, &partial, &mut expansion, &mut toggles, Vec::new());
        assert!(
            result_texts.iter().any(|text| text == "PARTIAL"),
            "the result pane labels the partial result: {result_texts:?}"
        );
        assert!(
            result_texts
                .iter()
                .any(|text| text == "SolveResult.status = Partial · complete: false"),
            "the badge names the engine field it read: {result_texts:?}"
        );
        for claim in ["SOLVED", "EXACT"] {
            assert!(
                !result_texts.iter().any(|text| text == claim),
                "a partial result must not claim {claim}: {result_texts:?}"
            );
        }
        let mut inspector_expansion = InspectorExpansion::new();
        let mut inspector_rows = Vec::new();
        let inspector_texts = paint_inspector_frame(
            &ctx,
            &partial,
            &mut inspector_expansion,
            &mut inspector_rows,
            Vec::new(),
        );
        assert!(
            inspector_texts.iter().any(|text| text == "PARTIAL"),
            "the inspector carries the same status: {inspector_texts:?}"
        );

        // The same record with the engine's completion flag is the one case that may read SOLVED.
        let solved = solve_view("Solved", true);
        let mut expansion = ExpansionState::new();
        let solved_texts =
            paint_result_frame(&ctx, &solved, &mut expansion, &mut toggles, Vec::new());
        assert!(
            solved_texts.iter().any(|text| text == "SOLVED"),
            "a solved, complete result is labelled SOLVED: {solved_texts:?}"
        );
        assert!(
            !solved_texts.iter().any(|text| text == "PARTIAL"),
            "a complete result is not labelled PARTIAL: {solved_texts:?}"
        );
    }

    /// SYM-007: the MathIR view highlights its instructions, copies the exact IR bytes to the
    /// clipboard, and exports those same bytes to a file.
    #[test]
    fn sym_007_the_mathir_view_highlights_instructions_and_copies_and_exports_its_bytes() {
        let ctx = egui::Context::default();
        let view = inspector_view(supplied_mathir());
        let export_dir = scratch_dir("sym-007");
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 1200.0))),
            ..Default::default()
        };
        let mut controls: Vec<(String, Rect)> = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let _ = paint_inspector(
                    ui,
                    &view,
                    420.0,
                    &mut expansion,
                    &mut toggles,
                    &mut controls,
                    Some(&export_dir),
                );
            });
        });
        output.textures_delta.clear();
        let mut galleys: Vec<(String, Vec<Color32>)> = Vec::new();
        for clipped in &output.shapes {
            collect_shape_colors(&clipped.shape, &mut galleys);
        }

        let palette = theme::syntax();
        let (ir_text, colors) = galleys
            .iter()
            .find(|(text, _)| text.starts_with("#!mathir 2"))
            .expect("the IR galley is painted");
        assert_eq!(
            ir_text,
            &supplied_ir_text(),
            "the painted IR is the engine's text, byte for byte"
        );
        for (class, color) in [
            ("the #!mathir directive", palette.keyword),
            ("an instruction opcode", palette.builtin),
            ("an operand", palette.number),
        ] {
            assert!(
                colors.contains(&color),
                "{class} must carry its own syntax colour: {colors:?}"
            );
        }
        let distinct = colors
            .iter()
            .fold(Vec::new(), |mut seen: Vec<Color32>, color| {
                if !seen.contains(color) {
                    seen.push(*color);
                }
                seen
            });
        assert!(
            distinct.len() >= 3,
            "the IR view is highlighted, not one flat colour: {distinct:?}"
        );

        // Copy: the clipboard command carries the whole IR text.
        let (texts, output) = click_control(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            &mut controls,
            "copy mathir",
            Some(&export_dir),
        );
        assert_eq!(
            copied_texts(&output),
            vec![supplied_ir_text()],
            "Copy MathIR puts the engine's IR on the clipboard"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == &format!("Copied MathIR ({} bytes)", supplied_ir_text().len())),
            "the pane reports the copy: {texts:?}"
        );

        // Export: the file's bytes are the same IR text, unchanged.
        let (_texts, _output) = click_control(
            &ctx,
            &view,
            &mut expansion,
            &mut toggles,
            &mut controls,
            "export mathir",
            Some(&export_dir),
        );
        let path = export_dir.join(format!("mathir-rev-{}.ir", view.revision.0));
        let written = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("the exported IR file {}: {error}", path.display()));
        assert_eq!(written, supplied_ir_text(), "the exported IR is unchanged");
        let _ = std::fs::remove_dir_all(&export_dir);
    }

    /// SYM-011: a 10k-node expression paints only the rows in view, and a deep one paints the
    /// level bound's own control instead of the whole tree.
    #[test]
    fn sym_011_a_ten_thousand_node_tree_paints_only_the_rows_in_view() {
        let ctx = egui::Context::default();
        let view = wide_tree_view(10_000);
        let mut expansion = InspectorExpansion::new();
        let mut toggles = Vec::new();
        let started = std::time::Instant::now();
        let (texts, _output, controls) =
            paint_inspector_pass(&ctx, &view, &mut expansion, &mut toggles, Vec::new(), None);
        let elapsed = started.elapsed();
        let painted = texts
            .iter()
            .filter(|text| text.starts_with('\u{25be}') || text.starts_with('\u{25b8}'))
            .count();
        assert!(
            painted + 1 >= toggles.len(),
            "each laid-out row contributes its header, and a row clipped at the viewport edge \
             contributes the rectangle without its text: painted={painted} rows={}",
            toggles.len()
        );
        assert!(
            painted >= 4,
            "the visible rows paint: {painted} of {toggles:?}"
        );
        assert!(
            painted <= 40 && toggles.len() <= 40,
            "only the rows in view are laid out, not 10 001: painted={painted} rows={}",
            toggles.len()
        );
        assert!(
            controls.len() <= 40,
            "one copy control per laid-out row, not per tree node: {}",
            controls.len()
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "10001 rows · levels 2/2 · 10001 nodes"),
            "the header states the row count and the level bound: {texts:?}"
        );
        assert!(
            elapsed < std::time::Duration::from_millis(1500),
            "one pass over a 10 001-node tree took {elapsed:?}"
        );

        // The deep tree exposes the level bound: 15 rows painted, 32 752 named as hidden.
        let deep = deep_tree_view(14);
        let (texts, _output, _controls) =
            paint_inspector_pass(&ctx, &deep, &mut expansion, &mut toggles, Vec::new(), None);
        assert!(
            texts
                .iter()
                .any(|text| text == "Load 4 more levels (32752 nodes hidden)"),
            "the level bound paints its own control with the hidden count: {texts:?}"
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
        log: std::collections::VecDeque<Option<PathBuf>>,
    }

    impl ScriptedDialogs {
        fn opening(paths: impl IntoIterator<Item = Option<PathBuf>>) -> ScriptedDialogs {
            ScriptedDialogs {
                open: paths.into_iter().collect(),
                save: std::collections::VecDeque::new(),
                log: std::collections::VecDeque::new(),
            }
        }

        fn saving(paths: impl IntoIterator<Item = Option<PathBuf>>) -> ScriptedDialogs {
            ScriptedDialogs {
                open: std::collections::VecDeque::new(),
                save: paths.into_iter().collect(),
                log: std::collections::VecDeque::new(),
            }
        }

        /// LOG-005: a dialogs queue whose Save log answer is `paths`.
        fn saving_log(paths: impl IntoIterator<Item = Option<PathBuf>>) -> ScriptedDialogs {
            ScriptedDialogs {
                open: std::collections::VecDeque::new(),
                save: std::collections::VecDeque::new(),
                log: paths.into_iter().collect(),
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

        fn save_log(
            &mut self,
            _suggested: &str,
            _initial_dir: Option<&std::path::Path>,
        ) -> Option<PathBuf> {
            self.log.pop_front().flatten()
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

    // -----------------------------------------------------------------------------------------
    // DOC-006: the external-change conflict prompt.
    // -----------------------------------------------------------------------------------------

    #[test]
    fn an_external_edit_refuses_save_and_survives_cancel_unchanged() {
        let path = fixture("conflict-save", b"a = 1\nb = 2\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 2\nc = 3\n".to_string());
        let external = b"a = 1\nexternal = 99\n";
        std::fs::write(&path, external).expect("external edit");
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        // DOC-006.4: the tab is marked before the user tries to save.
        assert!(
            app.workbench.documents()[index]
                .tab_label()
                .contains("changed on disk"),
            "the tab strip warns about the external change: {}",
            app.workbench.documents()[index].tab_label()
        );

        app.editors[index].set_caret(2, 5);
        let caret = app.editors[index].caret();
        frame(&ctx, &mut app, Vec::new());

        // Ctrl+S is the user's save path. It must ask, not write.
        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);

        let summary = app
            .conflict_summary()
            .expect("Save must open the conflict prompt");
        assert_eq!(summary.headline, "File changed on disk");
        assert_eq!(summary.target, path);
        assert_eq!(
            conflict_choices(&app),
            vec!["Reload", "Keep mine", "Cancel"],
            "the prompt offers exactly the three answers"
        );
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            external,
            "a refused Save writes nothing"
        );
        assert_eq!(
            app.workbench.documents()[index].source,
            "a = 1\nb = 2\nc = 3\n"
        );
        assert!(app.workbench.documents()[index].dirty());

        // Escape is Cancel: the file, the buffer, the caret and the focus are untouched.
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::default())],
        );
        assert!(app.conflict.is_none(), "Cancel dismisses the prompt");
        assert_eq!(std::fs::read(&path).expect("read back"), external);
        assert_eq!(
            app.workbench.documents()[index].source,
            "a = 1\nb = 2\nc = 3\n"
        );
        assert!(app.workbench.documents()[index].dirty());
        assert_eq!(app.editors[index].caret(), caret, "Cancel keeps the caret");
        frame(&ctx, &mut app, Vec::new());
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(surface_id()),
            "Cancel gives the focus back to the editor"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn keep_mine_in_the_conflict_prompt_writes_only_after_the_user_saw_it() {
        let path = fixture("conflict-keep", b"a = 1\nb = 2\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 2\nc = 3\n".to_string());
        std::fs::write(&path, b"a = 1\nexternal = 99\n").expect("external edit");
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert!(app.conflict.is_some(), "the prompt is open");
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\nexternal = 99\n"
        );

        let keep = conflict_button(&app, ConflictAnswer::KeepMine);
        pointer_click(&ctx, &mut app, keep);

        assert!(app.conflict.is_none(), "Keep mine resolves the prompt");
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\nb = 2\nc = 3\n",
            "Keep mine writes the buffer deliberately"
        );
        let document = &app.workbench.documents()[index];
        assert!(!document.dirty());
        assert_eq!(document.disk_status, crate::document_io::DiskStatus::Clean);
        assert!(
            !document.tab_label().contains("changed on disk"),
            "the marker clears once the save re-records the snapshot"
        );
        assert_eq!(
            document.source_snapshot,
            crate::document_io::probe(&path).snapshot(),
            "the recorded snapshot is the file Keep mine wrote"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_in_the_conflict_prompt_replaces_the_editor_text_and_clears_dirty() {
        let path = fixture("conflict-reload", b"a = 1\nb = 2\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 2\nc = 3\n".to_string());
        std::fs::write(&path, b"a = 1\nexternal = 99\n").expect("external edit");
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        let reload = conflict_button(&app, ConflictAnswer::Reload);
        pointer_click(&ctx, &mut app, reload);
        settle(&ctx, &mut app);

        // The buffer is dirty, so Reload first asks whether to discard it.
        let summary = app.conflict_summary().expect("the confirmation is open");
        assert!(summary.confirm_reload);
        assert!(
            app.reload_buttons
                .iter()
                .any(|(choice, _)| *choice == ReloadAnswer::DiscardAndReload)
        );

        let discard = reload_button(&app, ReloadAnswer::DiscardAndReload);
        pointer_click(&ctx, &mut app, discard);
        settle(&ctx, &mut app);

        assert!(app.conflict.is_none(), "Reload resolves the prompt");
        assert_eq!(
            app.workbench.documents()[index].source,
            "a = 1\nexternal = 99\n"
        );
        assert_eq!(
            app.editors[index].source(),
            "a = 1\nexternal = 99\n",
            "the editor shows the disk text"
        );
        assert!(!app.workbench.documents()[index].dirty());
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\nexternal = 99\n",
            "Reload writes nothing"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_of_a_file_deleted_on_disk_reports_it_and_never_recreates_it() {
        let path = fixture("conflict-deleted", b"a = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 2\n".to_string());
        std::fs::remove_file(&path).expect("external delete");
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::S, ctrl_modifiers())]);
        settle(&ctx, &mut app);

        let summary = app
            .conflict_summary()
            .expect("Save must report the missing file");
        assert_eq!(summary.headline, "File missing on disk");
        assert!(!path.exists(), "Save must not recreate a deleted file");
        assert_eq!(app.workbench.documents()[index].source, "a = 2\n");
        // Cancel leaves the tab open and the file still absent.
        let cancel = conflict_button(&app, ConflictAnswer::Cancel);
        pointer_click(&ctx, &mut app, cancel);
        assert!(app.conflict.is_none());
        assert!(!path.exists(), "Cancel does not recreate a deleted file");
        assert_eq!(app.workbench.documents()[index].source, "a = 2\n");
    }

    #[test]
    fn save_as_onto_an_existing_file_opens_the_conflict_instead_of_overwriting() {
        let path = fixture("conflict-save-as", b"a = 1\n");
        let target = fixture("conflict-save-as-target", b"someone else's file\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 2\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        app.dialogs = Box::new(ScriptedDialogs::saving([Some(target.clone())]));
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        app.save_active(true, &ctx);

        let summary = app.conflict_summary().expect("Save As must ask first");
        assert_eq!(summary.headline, "File already exists");
        assert_eq!(summary.target, target);
        assert_eq!(
            std::fs::read(&target).expect("read back"),
            b"someone else's file\n",
            "Save As wrote nothing"
        );
        assert_eq!(
            app.workbench.documents()[index].path.as_deref(),
            Some(path.as_path()),
            "the tab keeps its own path"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn the_close_prompts_save_hits_the_conflict_and_keep_mine_finishes_the_close() {
        let path = fixture("conflict-close", b"a = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 2\n".to_string());
        let tabs = workbench.documents().len();
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);

        frame(&ctx, &mut app, vec![key(Key::W, ctrl_modifiers())]);
        settle(&ctx, &mut app);
        assert!(app.close.is_some(), "the close prompt is open");
        std::fs::write(&path, b"a = 1\nexternal = 99\n").expect("external edit");
        let save = prompt_button(&app, UnsavedAnswer::Save);
        pointer_click(&ctx, &mut app, save);
        settle(&ctx, &mut app);

        assert!(app.conflict.is_some(), "the prompt's Save checks the disk");
        assert!(
            app.close.is_some(),
            "the close question waits under the conflict"
        );
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 1\nexternal = 99\n",
            "the close prompt's Save wrote nothing"
        );

        let keep = conflict_button(&app, ConflictAnswer::KeepMine);
        pointer_click(&ctx, &mut app, keep);
        settle(&ctx, &mut app);

        assert!(app.conflict.is_none());
        assert!(app.close.is_none(), "Keep mine finished the close flow");
        assert_eq!(app.workbench.documents().len(), tabs - 1);
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 2\n",
            "the tab closed only after the deliberate overwrite"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn regaining_focus_marks_the_tab_before_any_save() {
        let path = fixture("conflict-focus", b"a = 1\n");
        let mut workbench = opened_workbench(&path);
        let index = workbench.active();
        workbench.set_source(index, "a = 1\nb = 2\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        settle(&ctx, &mut app);
        assert!(
            !app.workbench.documents()[index]
                .tab_label()
                .contains("changed on disk")
        );

        // The window was in the background while another program wrote the file; regaining focus
        // must probe immediately rather than wait for the 750 ms poll.
        app.window_focused = false;
        std::fs::write(&path, b"a = 1\nexternal = 99\n").expect("external edit");
        app.poll_disk_changes(&ctx);

        assert_eq!(
            app.workbench.documents()[index].disk_status,
            crate::document_io::DiskStatus::Changed
        );
        assert!(
            app.workbench.documents()[index]
                .tab_label()
                .contains("changed on disk")
        );
        assert!(
            app.workbench.documents()[index]
                .logs
                .iter()
                .any(|line| line.text.contains("changed on disk")),
            "the external change is logged so the user learns before saving"
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
            precision: 250,
            unicode_glyphs: true,
            schema_version: recovery::SCHEMA_VERSION,
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
                precision: 250,
                unicode_glyphs: true,
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

    /// DOC-009: selecting a tab selects that tab's precision and ASCII/Unicode notation. The two
    /// tabs in this test disagree on both, so any window-wide value would show the wrong notation
    /// or the wrong precision in the painted text.
    #[test]
    fn doc_009_switching_tabs_switches_each_tabs_notation_and_precision() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("doc-009");
        let mut app = a11y_app(plot_dir.clone());
        pump_text(&ctx, &mut app, 2);
        app.workbench.set_source(0, "first\n".to_string());
        app.workbench.set_source(1, "second\n".to_string());

        // Tab 1 runs and chooses Unicode notation and 40 digits.
        run_from_keyboard(&ctx, &mut app);
        let _ = press_action(&ctx, &mut app, Action::ToggleUnicodeGlyphs);
        app.workbench.set_precision(0, 40);
        let expected_unicode = glyphs::render("sqrt(2)*pi", GlyphMode::Unicode, &mut |c| {
            a11y::font_has_glyph(&ctx, c)
        });
        let texts = pump_text(&ctx, &mut app, 2);
        assert_eq!(app.glyph_mode, GlyphMode::Unicode);
        assert_eq!(app.precision_input, "40");
        assert!(
            texts.iter().any(|text| text == &expected_unicode),
            "tab 1 paints its own Unicode value: {texts:?}"
        );

        // Tab 2 runs in ASCII at 250 digits: both values are the tab's own.
        app.workbench.set_active(1);
        run_from_keyboard(&ctx, &mut app);
        app.workbench.set_precision(1, 250);
        let texts = pump_text(&ctx, &mut app, 2);
        assert_eq!(
            app.glyph_mode,
            GlyphMode::Ascii,
            "selecting tab 2 restores tab 2's ASCII notation"
        );
        assert_eq!(
            app.precision_input, "250",
            "tab 2's precision in the toolbar"
        );
        assert!(
            texts.iter().any(|text| text == "[result] = sqrt(2)*pi"),
            "tab 2 paints ASCII despite tab 1 being Unicode: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == &expected_unicode),
            "tab 1's notation must not leak into tab 2: {texts:?}"
        );

        // Back to tab 1: its Unicode notation and 40 digits come back, unchanged.
        app.workbench.set_active(0);
        let texts = pump_text(&ctx, &mut app, 2);
        assert_eq!(app.glyph_mode, GlyphMode::Unicode);
        assert_eq!(app.precision_input, "40");
        assert!(texts.iter().any(|text| text == &expected_unicode));
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// DOC-010: a restored tab's Workspace pane says the engine state was reset until the first
    /// run; the sentence is painted text (a real UIA capture reads the same string).
    #[test]
    fn doc_010_the_restored_tab_paints_the_engine_reset_notice_until_it_runs() {
        let (storage, directory) = app_storage("doc-010-notice");
        let file = directory.join("restored.lv");
        std::fs::write(&file, b"a = 1\n").expect("fixture");
        let mut settings = Settings::new();
        settings.set_session(crate::settings::SessionState::new(
            vec![crate::settings::SessionTab {
                path: Some(file.to_string_lossy().into_owned()),
                document_id: 1,
                precision: 40,
                unicode_glyphs: false,
            }],
            0,
        ));
        settings
            .save(storage.settings_path.as_ref().expect("settings path"))
            .expect("save settings");

        let ctx = egui::Context::default();
        let mut app = WorkbenchApp::launch(storage.clone(), None);
        settle(&ctx, &mut app);
        let texts = pump_text(&ctx, &mut app, 2);
        assert_eq!(
            app.workbench.active_document().engine_state_note.as_deref(),
            Some(crate::workbench::ENGINE_RESET_NOTICE)
        );
        assert_eq!(
            app.workbench.active_document().precision,
            40,
            "the tab's persisted precision came back"
        );
        assert!(
            texts.iter().any(|text| text == "Engine state was reset"),
            "the Workspace pane names the reset: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("not restored by the restart")),
            "the pane explains what was not restored: {texts:?}"
        );

        // The first run recomputes the workspace and the notice disappears from both state and paint.
        run_from_keyboard(&ctx, &mut app);
        assert_eq!(app.workbench.active_document().engine_state_note, None);
        let after = pump_text(&ctx, &mut app, 2);
        assert!(
            !after.iter().any(|text| text == "Engine state was reset"),
            "a run retires the reset notice: {after:?}"
        );
        assert!(
            after.iter().any(|text| text == "a"),
            "the Workspace pane paints the recomputed variable row: {after:?}"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// DOC-014: a corrupt settings document and a corrupt recovery record are both survived at
    /// launch: the application starts, names each problem, and preserves the original bytes beside
    /// the file before anything can overwrite them.
    #[test]
    fn doc_014_corrupt_persisted_state_never_stops_startup_and_is_backed_up() {
        let (storage, directory) = app_storage("doc-014-corrupt");
        let settings_path = storage.settings_path.clone().expect("settings path");
        let corrupt_settings = b"{ this is not a settings document".to_vec();
        std::fs::write(&settings_path, &corrupt_settings).expect("corrupt settings");
        let recovery_root = storage.recovery_root.clone().expect("recovery root");
        std::fs::create_dir_all(&recovery_root).expect("recovery directory");
        let corrupt_record = recovery_root.join("doc-9.lvrec");
        let corrupt_bytes = b"not a recovery record at all".to_vec();
        std::fs::write(&corrupt_record, &corrupt_bytes).expect("corrupt record");

        let ctx = egui::Context::default();
        let mut app = WorkbenchApp::launch(storage.clone(), None);
        settle(&ctx, &mut app);

        assert!(
            !app.workbench.documents().is_empty(),
            "the launch still opened a document"
        );
        assert_eq!(
            app.settings.window,
            Settings::new().window,
            "defaults are usable"
        );
        let logs: String = app
            .workbench
            .documents()
            .iter()
            .flat_map(|document| document.logs.iter())
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            logs.contains("settings are not valid JSON"),
            "the corrupt settings are named: {logs}"
        );
        assert!(
            logs.contains("corrupt recovery record") && logs.contains("doc-9.lvrec"),
            "the corrupt record is named: {logs}"
        );
        // Both originals survive byte-for-byte beside their source file.
        assert_eq!(
            std::fs::read(directory.join("desktop.json.bak")).expect("settings backup"),
            corrupt_settings,
            "the settings bytes are preserved"
        );
        assert_eq!(
            std::fs::read(recovery_root.join("doc-9.lvrec.bak")).expect("record backup"),
            corrupt_bytes,
            "the recovery-record bytes are preserved"
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
            KeyName::Char(letter) => match letter {
                'a' => Key::A,
                'b' => Key::B,
                'c' => Key::C,
                'd' => Key::D,
                'e' => Key::E,
                'f' => Key::F,
                'g' => Key::G,
                'h' => Key::H,
                'i' => Key::I,
                'j' => Key::J,
                'k' => Key::K,
                'l' => Key::L,
                'm' => Key::M,
                'n' => Key::N,
                'o' => Key::O,
                'p' => Key::P,
                'q' => Key::Q,
                'r' => Key::R,
                's' => Key::S,
                't' => Key::T,
                'u' => Key::U,
                'v' => Key::V,
                'w' => Key::W,
                'x' => Key::X,
                'y' => Key::Y,
                'z' => Key::Z,
                other => panic!("no egui key for {other:?}"),
            },
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
        let mut texts = press_action(ctx, app, Action::Workbench(keyboard::WorkbenchCommand::Run));
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
            if clipped
                .clip_rect
                .intersects(ring_rect.expand(a11y::FOCUS_RING_GAP + 4.0))
            {
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
                texts
                    .iter()
                    .any(|text| text == &scope.label().to_uppercase()),
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
        assert_eq!(
            app.theme,
            theme::ThemeId::Light,
            "the theme switch is a binding"
        );
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
            app.workbench
                .active_document()
                .title
                .starts_with("untitled"),
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
            texts
                .iter()
                .any(|text| text == "RESULT · structured projection"),
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
            format!("focused=run run_state={:?}", app.view.run_state)
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

    /// DOC-002: the file bar's own New button adds a tab and the *same* pass renders it.
    ///
    /// This is the click path, not the shortcut path: the file bar is painted before the toolbar,
    /// so the toolbar in the same pass reads the new active document. Before the fix the toolbar
    /// indexed `self.editors[active]` before `sync_editors` had created the editor for the tab the
    /// click had just added, and the product panicked with "index out of bounds: the len is 2 but
    /// the index is 2". The regression is a click, a full pass and the toolbar's own text.
    #[test]
    fn clicking_the_file_bar_new_button_adds_a_tab_and_the_same_pass_renders_it() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("new-button");
        let mut app = a11y_app(plot_dir.clone());
        // Paint one pass so the file bar records its button rectangles and focus settles.
        frame(&ctx, &mut app, Vec::new());
        let before = app.workbench.documents().len();
        let new_rect = app
            .file_buttons
            .iter()
            .find(|(label, _)| *label == "New")
            .map(|(_, rect)| *rect)
            .expect("the file bar paints a New button");
        assert!(
            new_rect.width() > 0.0 && new_rect.height() > 0.0,
            "the New button has real bounds: {new_rect:?}"
        );

        pointer_click(&ctx, &mut app, new_rect.center());

        assert_eq!(
            app.workbench.documents().len(),
            before + 1,
            "the New click adds exactly one document"
        );
        assert!(
            app.workbench
                .active_document()
                .title
                .starts_with("untitled"),
            "the new tab is untitled: {:?}",
            app.workbench.active_document().title
        );
        assert_eq!(
            app.editors.len(),
            app.workbench.documents().len(),
            "the click pass reconciled one editor per document"
        );
        let texts = pump_text(&ctx, &mut app, 2);
        assert!(
            texts.iter().any(|text| text.contains("1 lines / 0 bytes")),
            "the toolbar paints the new tab's own editor: {texts:?}"
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
        assert!(
            ascii_texts
                .iter()
                .any(|text| text == "[result] = sqrt(2)*pi")
        );
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
                        module: "User".to_string(),
                        summary: Some("A user function.".to_string()),
                    },
                    EngineFunction {
                        name: "sqrt".to_string(),
                        parameters: vec!["x".to_string()],
                        is_builtin: true,
                        module: "Core".to_string(),
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

    /// PLT-005/PLT-009: an engine whose figure is named after the `plot("…")` title in the source
    /// it evaluated, so two tabs hold visibly different charts and a tab switch can be checked
    /// for the right one.
    struct PerDocumentPlotEngine {
        title: String,
    }

    impl Default for PerDocumentPlotEngine {
        fn default() -> PerDocumentPlotEngine {
            PerDocumentPlotEngine {
                title: "untitled".to_string(),
            }
        }
    }

    impl Evaluator for PerDocumentPlotEngine {
        fn eval(&mut self, src: &str) -> Result<String, String> {
            if let Some(start) = src.find("plot(\"") {
                let rest = &src[start + 6..];
                if let Some(end) = rest.find('"') {
                    self.title = rest[..end].to_string();
                }
            }
            Ok("1".to_string())
        }

        fn last_plot(&self) -> Option<crate::engine::PlotCapture> {
            Some(crate::engine::PlotCapture {
                path: format!("{}.svg", self.title),
                title: self.title.clone(),
                svg: format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"600\">\
                     <rect width=\"800\" height=\"600\" fill=\"#ffffff\"/>\
                     <text x=\"20\" y=\"40\" fill=\"#000000\">{}</text>\
                     <polyline fill=\"none\" stroke=\"#e6194b\" stroke-width=\"2\" points=\"40,80 400,500\"/>\
                     </svg>",
                    self.title
                ),
            })
        }
    }

    /// A two-tab workbench whose figures are named by each tab's own script.
    fn two_plot_tabs() -> WorkbenchApp {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(PerDocumentPlotEngine::default())));
        workbench.set_source(0, "plot(\"alpha\")".to_string());
        workbench.new_document();
        workbench.set_source(1, "plot(\"beta\")".to_string());
        workbench.set_active(0);
        WorkbenchApp::with_workbench(workbench)
    }

    /// Runs the active tab and paints the frame that loads its capture.
    fn run_and_settle_plot(ctx: &egui::Context, app: &mut WorkbenchApp) {
        frame(ctx, app, Vec::new());
        let _ = run_from_keyboard(ctx, app);
        frame(ctx, app, Vec::new());
    }

    /// PLT-005: every chart appears with its title, its document and the run that produced it,
    /// and selecting another tab replaces the chart instead of leaving the previous tab's on
    /// screen.
    #[test]
    fn plt_005_the_plot_names_its_document_and_run_and_a_tab_switch_never_shows_the_other_chart() {
        let ctx = egui::Context::default();
        let mut app = two_plot_tabs();

        run_and_settle_plot(&ctx, &mut app);
        let first = {
            let document = &app.workbench.documents()[0];
            (
                document.title.clone(),
                document.id.0,
                document.run_id().map(|run| run.0),
            )
        };
        assert_eq!(app.plot.title.as_deref(), Some("alpha"));
        let provenance = app.plot.provenance.clone().expect("tab 1 provenance");
        assert_eq!(provenance.document, first.0, "the document title");
        assert_eq!(provenance.document_id, first.1, "the document id");
        assert_eq!(provenance.run, first.2, "the run id");
        assert!(
            provenance.label().contains("run "),
            "the provenance line does not name the run: {}",
            provenance.label()
        );

        // Selecting the other tab and running it loads that tab's own chart and identity.
        app.workbench.set_active(1);
        frame(&ctx, &mut app, Vec::new());
        assert!(
            app.plot.is_empty(),
            "a tab that has not run kept the other tab's chart"
        );
        run_and_settle_plot(&ctx, &mut app);
        let second = {
            let document = &app.workbench.documents()[1];
            (document.title.clone(), document.id.0)
        };
        assert_eq!(
            app.plot.title.as_deref(),
            Some("beta"),
            "the plot pane still shows the other tab's chart"
        );
        let provenance = app.plot.provenance.clone().expect("tab 2 provenance");
        assert_eq!(provenance.document, second.0);
        assert_eq!(provenance.document_id, second.1);

        // Back to the first tab: its own chart again.
        app.workbench.set_active(0);
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.plot.title.as_deref(), Some("alpha"));

        // A tab that has never run shows the empty state, not the previous tab's chart.
        app.workbench.new_document();
        app.workbench.set_active(2);
        frame(&ctx, &mut app, Vec::new());
        assert!(
            app.plot.is_empty(),
            "a tab without a capture kept the previous tab's chart"
        );
        assert!(
            app.plot.provenance.is_none(),
            "a chartless tab kept a provenance"
        );
        assert!(
            app.plot.title.is_none(),
            "a chartless tab kept the previous chart's title"
        );
    }

    /// PLT-009: a successful run's chart stays inspectable across edits and tab switches until a
    /// later run replaces it, and each tab keeps its own.
    #[test]
    fn plt_009_the_last_plot_stays_inspectable_until_replaced_and_is_isolated_between_tabs() {
        let ctx = egui::Context::default();
        let mut app = two_plot_tabs();

        run_and_settle_plot(&ctx, &mut app);
        assert_eq!(app.plot.title.as_deref(), Some("alpha"));

        // An edit does not run anything: the last chart must stay inspectable.
        frame(&ctx, &mut app, vec![egui::Event::Text(" ".to_string())]);
        assert_eq!(
            app.plot.title.as_deref(),
            Some("alpha"),
            "an edit cleared the last plot"
        );
        assert!(!app.plot.is_empty());

        // Switching tabs and back keeps each tab's own chart.
        app.workbench.set_active(1);
        frame(&ctx, &mut app, Vec::new());
        assert!(app.plot.is_empty(), "tab 2 has not run yet");
        run_and_settle_plot(&ctx, &mut app);
        assert_eq!(app.plot.title.as_deref(), Some("beta"));
        app.workbench.set_active(0);
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            app.plot.title.as_deref(),
            Some("alpha"),
            "tab 1 lost its plot while tab 2 ran"
        );

        // A later successful run replaces the chart with its own.
        app.workbench.set_source(0, "plot(\"gamma\")".to_string());
        run_and_settle_plot(&ctx, &mut app);
        assert_eq!(
            app.plot.title.as_deref(),
            Some("gamma"),
            "the newer run did not replace the chart"
        );
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
        panic!(
            "the run did not reach a terminal state: {:?}",
            app.view.run_state
        );
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
        let tables: Vec<&a11y::NodeRow> =
            rows.iter().filter(|row| row.role == Role::Table).collect();
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
        let data_rows: Vec<&a11y::NodeRow> =
            rows.iter().filter(|row| row.role == Role::Row).collect();
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
        // VIS-001 review: each table paints exactly ONE header row. The pane used to paint a
        // plain Button row above the table and then the semantic ColumnHeader row inside it, so
        // every screen showed the headers twice. The extra row is gone; a regression would add
        // plain buttons with the same labels back.
        assert_eq!(
            headers.len(),
            6,
            "each workspace table paints exactly one header row\n{dump}"
        );
        let duplicate_header_buttons: Vec<&str> = rows
            .iter()
            .filter(|row| row.role == Role::Button)
            .map(|row| row.name.as_str())
            .filter(|name| matches!(*name, "Name ▲" | "Kind" | "Value" | "Parameters" | "Origin"))
            .collect();
        assert!(
            duplicate_header_buttons.is_empty(),
            "the sortable headers must be the table's ColumnHeader row only, not extra buttons: \
             {duplicate_header_buttons:?}\n{dump}"
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
            assert!(
                tab.selected.is_some(),
                "a tab must expose its selection state"
            );
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
        assert!(
            plot.name.contains("SVG plot canvas"),
            "plot name: {}",
            plot.name
        );
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

        // Splitters: the four pane dividers have a role and a name.
        let splitters: Vec<&a11y::NodeRow> = rows
            .iter()
            .filter(|row| row.role == Role::Splitter)
            .collect();
        assert_eq!(splitters.len(), 4, "four pane splitters\n{dump}");
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
    /// adapter publishes. The 200% cases cover both the 900x600-point contract minimum and the
    /// 800x500-point client that a 1600x1000 window reaches by zooming in; neither may overflow.
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
        // A11Y-004's contract minimum is a 900x600-point client; at 200% that is an 1800x1200
        // physical client. The 1600x1000 client is below that minimum at 200% (800x500 points)
        // and is exactly the state a 1600x1000 window reaches by zooming in, so it stays as the
        // graceful-degradation case: elided text is allowed there, overflow is not.
        let cases = [
            (vec2(1600.0, 1000.0), 125u32),
            (vec2(1600.0, 1000.0), 150),
            (vec2(1600.0, 1000.0), 200),
            (vec2(1800.0, 1200.0), 200),
        ];
        let mut summary = String::new();
        for (physical, percent) in cases {
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
            // The five pane rectangles the frame painted. A `Ui` node's own accesskit bounds
            // are not filled in until a widget owns it, so containment is measured against the
            // same rectangles the panes paint into — the pane a control visibly sits in.
            let layout = app.layout.expect("the frame paints the pane layout");
            let panes = [
                ("Editor", layout.editor),
                ("Graph", layout.plot),
                ("Workspace", layout.workspace),
                ("Logs", layout.logs),
                ("Symbolic Inspector", layout.inspector),
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
                if *pane_name == "Editor" || *pane_name == "Graph" {
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
                "scale={percent}% physical={:.0}x{:.0} window_points={:.0}x{:.0} rows={} checked={checked}\n",
                physical.x,
                physical.y,
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
            let _ = frame_capturing(
                &ctx,
                &mut app,
                vec2(1600.0, 1000.0),
                Vec::new(),
                Default::default(),
            );
            let output = frame_capturing(
                &ctx,
                &mut app,
                vec2(1600.0, 1000.0),
                Vec::new(),
                Default::default(),
            );
            let mut painted = Painted::default();
            for clipped in &output.shapes {
                collect_painted(&clipped.shape, &mut painted);
            }
            let metrics = app.editors[0].metrics;
            assert_eq!(
                metrics.font_size, size as f32,
                "the configured size is painted"
            );
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
        assert_eq!(
            app.settings.theme,
            theme::ThemeId::Light,
            "the follow state persists"
        );
        // Nothing else may have moved.
        assert_eq!(
            app.workbench.documents()[0].source,
            source,
            "the document survives"
        );
        assert_eq!(app.editors[0].caret(), caret, "the caret survives");
        assert_eq!(app.view.run_state, run_state, "the run state survives");
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            focused,
            "the focus survives"
        );
        assert_eq!(app.workbench.documents().len(), tabs, "the tabs survive");
        assert_eq!(app.editors.len(), tabs);

        notify(&mut app, egui::Theme::Dark);
        assert_eq!(
            app.theme,
            theme::ThemeId::Dark,
            "a second OS change is followed back"
        );
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
        let output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1600.0, 1000.0),
            Vec::new(),
            Default::default(),
        );
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

    /// LAY-011 / VIS-016: the measured adaptive layout at the supported window sizes and scales.
    ///
    /// For every (window, scale) pair this asserts, from the frame the window really painted:
    /// the chosen mode, that the workbench and every pane rectangle are inside the client, that
    /// the editor keeps at least 45% of the workbench width, that Run/Apply/the precision field
    /// (and Cancel while in flight, see the test below) are inside the client, and that no
    /// non-scrolling interactive control or painted text leaves the client. In the compact mode
    /// it also opens each collapsed pane with one pointer click and re-checks the drawer.
    #[test]
    fn lay_011_adaptive_layout_keeps_every_pane_inside_every_supported_client() {
        let mut app = structural_app();
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let _ = press_action(
            &ctx,
            &mut app,
            Action::Workbench(keyboard::WorkbenchCommand::Run),
        );
        pump_until_terminal(&ctx, &mut app);
        let mut report = String::new();
        for physical in [vec2(900.0, 600.0), vec2(1280.0, 800.0)] {
            for percent in [100u32, 150, 200] {
                app.settings.ui_scale_percent = percent;
                let scale = percent as f32 / 100.0;
                let screen = physical / scale;
                // The zoom setting becomes active on the next pass and rescales that pass's
                // screen rect; three passes settle on the requested client size.
                frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
                for _ in 0..3 {
                    frame_sized(&ctx, &mut app, screen, Vec::new());
                }
                assert!(
                    (ctx.zoom_factor() - scale).abs() < 1e-4,
                    "the scale is applied: {} != {scale}",
                    ctx.zoom_factor()
                );
                let layout = app.layout.expect("the frame paints the layout");
                assert_eq!(
                    layout.mode,
                    LayoutMode::for_window(screen, ctx.pixels_per_point()),
                    "mode at {:.0}x{:.0} points",
                    screen.x,
                    screen.y
                );
                let window = Rect::from_min_size(pos2(0.0, 0.0), screen);
                assert!(
                    window.expand(0.5).contains_rect(layout.full),
                    "the workbench itself is inside the client: {:?}",
                    layout.full
                );
                let visible: Vec<(&str, Rect)> = match layout.mode {
                    LayoutMode::Split | LayoutMode::DenseSplit => vec![
                        ("editor", layout.editor),
                        ("plot", layout.plot),
                        ("workspace", layout.workspace),
                        ("logs", layout.logs),
                        ("inspector", layout.inspector),
                        ("split_a", layout.split_a),
                        ("split_b", layout.split_b),
                        ("split_c", layout.split_c),
                        ("split_d", layout.split_d),
                    ],
                    LayoutMode::Compact => {
                        let compact = layout.compact.expect("compact rectangles");
                        assert!(
                            window.expand(0.5).contains_rect(compact.tab_bar),
                            "the pane tab row is inside the client: {:?}",
                            compact.tab_bar
                        );
                        let mut rects = vec![("editor", compact.editor)];
                        if let Some(drawer) = compact.drawer {
                            rects.push(("drawer", drawer));
                        }
                        if let Some(splitter) = compact.splitter {
                            rects.push(("compact_splitter", splitter));
                        }
                        rects
                    }
                };
                for (name, rect) in &visible {
                    assert!(
                        rect.is_positive(),
                        "{percent}% {:.0}x{:.0}: the {name} pane has no area: {rect:?}",
                        physical.x,
                        physical.y
                    );
                    assert!(
                        window.expand(0.5).contains_rect(*rect),
                        "{percent}% {:.0}x{:.0}: the {name} pane {rect:?} leaves the client {window:?}",
                        physical.x,
                        physical.y
                    );
                }
                // LAY-011: the editor keeps at least ~45% of the workbench width.
                let editor_share = layout.editor.width() / layout.full.width().max(1.0);
                assert!(
                    editor_share >= 0.45,
                    "{percent}% {:.0}x{:.0}: the editor has {:.1}% of the width, needs 45%",
                    physical.x,
                    physical.y,
                    editor_share * 100.0
                );
                let rows = accessibility_rows(&ctx, &mut app, screen);
                let required_actions: &[&str] = match layout.mode {
                    LayoutMode::Split => &["Run", "Apply", "Precision (digits)", "Save", "New"],
                    LayoutMode::DenseSplit | LayoutMode::Compact => {
                        &["Run", "Apply", "Precision (digits)", "Save", "File"]
                    }
                };
                for required in required_actions {
                    let row = rows
                        .iter()
                        .find(|row| row.name == *required && row.interactive())
                        .unwrap_or_else(|| {
                            panic!(
                                "{percent}% {:.0}x{:.0}: the primary action {required:?} is missing",
                                physical.x, physical.y
                            )
                        });
                    assert!(
                        window.expand(0.5).contains_rect(row.bounds),
                        "{percent}% {:.0}x{:.0}: {required:?} at {:?} is outside the client",
                        physical.x,
                        physical.y,
                        row.bounds
                    );
                }
                // The dense split keeps all five panes on screen; the compact layout replaces
                // them with its drawer, so only the dense split is asserted here.
                if layout.mode == LayoutMode::DenseSplit {
                    for title in ["Graph", "Workspace", "Logs", "Symbolic Inspector"] {
                        assert!(
                            rows.iter()
                                .any(|row| row.role == egui::Role::Pane && row.name == title),
                            "{percent}% {:.0}x{:.0}: the dense split must paint the {title} pane",
                            physical.x,
                            physical.y
                        );
                    }
                }
                // Every pane tab is visible and one click opens its pane.
                if layout.mode == LayoutMode::Compact {
                    for tab in ["Graph", "Workspace", "Logs", "Symbolic Inspector"] {
                        assert!(
                            rows.iter().any(|row| row.name == tab && row.interactive()),
                            "{percent}%: the collapsed pane tab {tab:?} is missing"
                        );
                    }
                    assert_eq!(app.compact_tabs.len(), 4, "four pane tabs are painted");
                    for (pane, rect) in app.compact_tabs.clone() {
                        let pos = rect.center();
                        frame_sized(&ctx, &mut app, screen, vec![egui::Event::PointerMoved(pos)]);
                        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, true)]);
                        frame_sized(&ctx, &mut app, screen, vec![pointer_button(pos, false)]);
                        assert_eq!(
                            app.compact_pane,
                            Some(pane),
                            "{percent}%: one click must open the {} drawer",
                            pane.label()
                        );
                        for _ in 0..2 {
                            frame_sized(&ctx, &mut app, screen, Vec::new());
                        }
                        let opened = app.layout.expect("drawer layout");
                        let compact = opened.compact.expect("compact rectangles");
                        let drawer = compact.drawer.expect("the drawer is open");
                        assert!(
                            window.expand(0.5).contains_rect(drawer),
                            "{percent}%: the {} drawer leaves the client: {drawer:?}",
                            pane.label()
                        );
                        assert!(
                            compact.editor.width() / opened.full.width().max(1.0) >= 0.45,
                            "{percent}%: the {} drawer shrinks the editor below 45%",
                            pane.label()
                        );
                        report.push_str(&format!(
                            "  drawer={} window={:.0}x{:.0} scale={percent}% editor_drawer={:.1}%\n",
                            pane.label(),
                            physical.x,
                            physical.y,
                            compact.editor.width() / opened.full.width().max(1.0) * 100.0
                        ));
                        // The pane is painted with its own titled frame.
                        let pane_rows = accessibility_rows(&ctx, &mut app, screen);
                        let title = match pane {
                            CompactPane::Graph => "Graph",
                            CompactPane::Workspace => "Workspace",
                            CompactPane::Logs => "Logs",
                            CompactPane::Inspector => "Symbolic Inspector",
                        };
                        assert!(
                            pane_rows
                                .iter()
                                .any(|row| row.role == egui::Role::Pane && row.name == title),
                            "{percent}%: the {} pane is not painted\n{}",
                            pane.label(),
                            a11y::dump_table(&pane_rows)
                        );
                    }
                    // LAY-002: the Symbolic Inspector tab opens the dedicated inspector drawer,
                    // so the fifth pane is one pointer action away in the compact layout.
                    assert!(
                        app.compact_inspector.is_some(),
                        "{percent}%: the Symbolic Inspector tab is painted"
                    );
                    app.compact_pane = None;
                }
                // No non-scrolling interactive control may leave the client, and no painted
                // text may be cut horizontally at any edge it is clipped to.
                for row in &rows {
                    if row.bounds == Rect::NOTHING || !row.bounds.is_positive() {
                        continue;
                    }
                    if row.interactive() && !a11y::in_scroll_container(&rows, row) {
                        assert!(
                            window.expand(0.5).contains_rect(row.bounds),
                            "{percent}% {:.0}x{:.0}: {:?} '{}' at {:?} leaves the client",
                            physical.x,
                            physical.y,
                            row.role,
                            row.name,
                            row.bounds
                        );
                    }
                }
                let output =
                    frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
                let mut shapes = 0usize;
                for clipped in &output.shapes {
                    let mut painted = Painted::default();
                    collect_painted(&clipped.shape, &mut painted);
                    for (text, rect) in painted.texts {
                        if !rect.intersects(window) || !rect.is_positive() {
                            continue;
                        }
                        shapes += 1;
                        let clip = clipped.clip_rect;
                        assert!(
                            rect.left() >= clip.left() - 0.5 && rect.right() <= clip.right() + 0.5,
                            "{percent}% {:.0}x{:.0}: the text {text:?} at {rect:?} is \
                             horizontally clipped by {clip:?}",
                            physical.x,
                            physical.y
                        );
                    }
                }
                assert!(shapes > 20, "{percent}%: only {shapes} text shapes painted");
                report.push_str(&format!(
                    "window={:.0}x{:.0} scale={percent}% points={:.0}x{:.0} mode={:?} \
                     editor={:.1}% panes={} text_shapes={shapes}\n",
                    physical.x,
                    physical.y,
                    screen.x,
                    screen.y,
                    layout.mode,
                    editor_share * 100.0,
                    visible.len(),
                ));
            }
        }
        eprintln!("LAY-011 layout assertions:\n{report}");
    }

    /// LAY-011: Cancel joins Run and the precision controls in the visible toolbar while a run is
    /// in flight, at both minimum-window scales, without any of them leaving the client.
    #[test]
    fn lay_011_run_cancel_and_precision_stay_visible_while_running() {
        let ctx = egui::Context::default();
        let gate = Arc::new(Gate::default());
        let workbench = gated_workbench(gate.clone());
        let mut app = WorkbenchApp::with_workbench(workbench);
        for (physical, percent) in [
            (vec2(900.0, 600.0), 200u32),
            (vec2(1280.0, 800.0), 200u32),
            (vec2(900.0, 600.0), 150u32),
        ] {
            app.settings.ui_scale_percent = percent;
            let screen = physical / (percent as f32 / 100.0);
            frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
            for _ in 0..3 {
                frame_sized(&ctx, &mut app, screen, Vec::new());
            }
            let _ = press_action(
                &ctx,
                &mut app,
                Action::Workbench(keyboard::WorkbenchCommand::Run),
            );
            frame_sized(&ctx, &mut app, screen, Vec::new());
            let rows = accessibility_rows(&ctx, &mut app, screen);
            let window = Rect::from_min_size(pos2(0.0, 0.0), screen);
            // Round 61: the Actions menu shares this row with Run and Cancel; at 200% in the
            // 900x600 minimum window its button was the one control that left the client.
            for required in ["Run", "Cancel", "Apply", "Precision (digits)", "Actions"] {
                let row = rows
                    .iter()
                    .find(|row| row.name == required && row.interactive())
                    .unwrap_or_else(|| {
                        panic!(
                            "{percent}% {:.0}x{:.0}: {required:?} is missing while running",
                            physical.x, physical.y
                        )
                    });
                assert!(
                    window.expand(0.5).contains_rect(row.bounds),
                    "{percent}%: {required:?} at {:?} leaves the client",
                    row.bounds
                );
            }
            app.workbench.cancel_active();
        }
        gate.release();
    }

    // -----------------------------------------------------------------------------------------
    // LAY-001 / LAY-002 / LAY-003 / LAY-004: the five-pane layout, its splitters and persistence.
    // -----------------------------------------------------------------------------------------

    /// VIS-008: the Workspace pane's functions table paints its own empty state when the
    /// variables section above it leaves the functions section inside the 1600x1000 viewport.
    /// (At 1280x800 the functions section starts below the scroll viewport, so LAY-002 cannot
    /// assert it; this test is the wider window the VIS-008 capture uses.)
    #[test]
    fn vis_008_the_functions_empty_state_is_visible_at_1600x1000() {
        let ctx = egui::Context::default();
        let screen = vec2(1600.0, 1000.0);
        let mut workbench = Workbench::new();
        workbench.set_source(0, String::new());
        let mut app = WorkbenchApp::with_workbench(workbench);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        assert!(app.workbench.clear_log_active() >= 1);
        let texts = painted_texts(&ctx, &mut app, screen);
        for empty in [
            "No variables yet",
            "No functions yet",
            "The last run published no functions.",
            "Run to populate",
        ] {
            assert!(
                texts.iter().any(|text| text == empty),
                "the workspace pane must paint {empty:?} at 1600x1000; texts: {texts:?}"
            );
        }
    }

    /// VIS-008: the find bar's no-match state names the query and the action that may reveal a
    /// match, instead of leaving the user with a bare count.
    #[test]
    fn vis_008_the_search_empty_state_names_the_query_and_the_next_step() {
        let mut app = app_with_source("alpha beta", 0, 0);
        let ctx = egui::Context::default();
        let screen = vec2(1600.0, 1000.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let ctrl = ctrl_modifiers();
        frame_sized(
            &ctx,
            &mut app,
            screen,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::F, ctrl)],
        );
        frame_sized(&ctx, &mut app, screen, Vec::new());
        frame_sized(
            &ctx,
            &mut app,
            screen,
            vec![egui::Event::Text("zzz".to_string())],
        );
        assert_eq!(
            app.editors[0].find.count_text().as_deref(),
            Some("No matches"),
            "the query really has no match"
        );
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts.iter().any(|text| text.contains("No match for")),
            "the search empty state must name the reason; texts: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("turn off Aa, W or .*")),
            "the search empty state must name the next step; texts: {texts:?}"
        );
    }

    /// Every text shape a frame painted, so a test can assert what the window really shows.
    fn painted_texts(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
    ) -> Vec<String> {
        let output = frame_capturing(ctx, app, screen, Vec::new(), Default::default());
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        texts
    }

    /// A pointer click at `pos` at an explicit client size (the same move/press/release sequence
    /// a mouse produces).
    fn pointer_click_sized(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
        pos: egui::Pos2,
    ) {
        frame_sized(ctx, app, screen, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame_sized(ctx, app, screen, vec![pointer_button(pos, pressed)]);
        }
    }

    /// A real pointer drag: press on `from`, move through intermediate positions while the
    /// button is down, release on `to`. Every event is a genuine pointer event delivered to the
    /// splitter widget; no test seam sets the fraction directly.
    fn pointer_drag_sized(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        frame_sized(ctx, app, screen, vec![egui::Event::PointerMoved(from)]);
        frame_sized(ctx, app, screen, vec![pointer_button(from, true)]);
        for step in [0.25_f32, 0.5, 0.75, 1.0] {
            let pos = from + (to - from) * step;
            frame_sized(ctx, app, screen, vec![egui::Event::PointerMoved(pos)]);
        }
        frame_sized(ctx, app, screen, vec![pointer_button(to, false)]);
    }

    /// Two real pointer clicks at `pos`, the gesture egui classifies as a double-click.
    fn pointer_double_click_sized(
        ctx: &egui::Context,
        app: &mut WorkbenchApp,
        screen: egui::Vec2,
        pos: egui::Pos2,
    ) {
        for _ in 0..2 {
            frame_sized(ctx, app, screen, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                frame_sized(ctx, app, screen, vec![pointer_button(pos, pressed)]);
            }
        }
    }

    /// LAY-001: at the default 1280x800 window the editor is the primary surface.
    ///
    /// The assertion is measured from the frame the window painted (the same `PaneLayout` the
    /// panes are painted into), and it also proves the other four panes stay reachable, so the
    /// editor's width is not won by hiding results or the graph.
    #[test]
    fn lay_001_the_editor_is_the_primary_surface_at_1280x800() {
        let mut app = structural_app();
        let ctx = egui::Context::default();
        let screen = vec2(1280.0, 800.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let layout = app.layout.expect("the frame paints the pane layout");
        assert_eq!(
            layout.mode,
            LayoutMode::Split,
            "1280x800 is the normal split"
        );
        let share = layout.editor.width() / layout.full.width().max(1.0);
        eprintln!(
            "LAY-001 editor share={:.1}% editor={:.1}x{:.1} full={:.1}x{:.1}",
            share * 100.0,
            layout.editor.width(),
            layout.editor.height(),
            layout.full.width(),
            layout.full.height()
        );
        assert!(
            share >= 0.45,
            "the editor must keep at least 45% of the client width, has {:.1}%",
            share * 100.0
        );
        // Visual priority: the editor is wider than either ancillary top column, and every other
        // pane keeps a usable rectangle.
        assert!(
            layout.editor.width() > layout.plot.width(),
            "the editor must be wider than the graph: editor={:?} graph={:?}",
            layout.editor,
            layout.plot
        );
        assert!(
            layout.editor.width() > layout.workspace.width(),
            "the editor must be wider than the workspace: editor={:?} workspace={:?}",
            layout.editor,
            layout.workspace
        );
        for (name, rect) in [
            ("graph", layout.plot),
            ("workspace", layout.workspace),
            ("logs", layout.logs),
            ("symbolic inspector", layout.inspector),
        ] {
            assert!(
                rect.width() >= 150.0 && rect.height() >= 90.0,
                "the {name} pane must stay usable: {rect:?}"
            );
        }
    }

    /// LAY-002: the five Studio panes are visible, each carries its own title, and each paints
    /// its own empty state before a run and its own populated content after one. The Symbolic
    /// Inspector's dedicated pane is a sibling of the Logs pane, not a tab hidden in Workspace.
    #[test]
    fn lay_002_the_five_panes_are_titled_and_own_their_empty_and_populated_states() {
        let ctx = egui::Context::default();
        let screen = vec2(1280.0, 800.0);
        // A fresh window whose document is empty (the starter text cleared through the same
        // workbench path an edit uses) leaves every pane in its empty state in one frame: no
        // run has published anything and the editor buffer has no content.
        let mut workbench = Workbench::new();
        workbench.set_source(0, String::new());
        let mut app = WorkbenchApp::with_workbench(workbench);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        // The engine's boot lines are always in the log, so the Logs pane's empty state is
        // reached through the pane's own Clear log command (the next frame paints it empty).
        assert!(app.workbench.clear_log_active() >= 1);
        let texts = painted_texts(&ctx, &mut app, screen);
        // VIS-008: every pane states what it is for (headline), why it is empty (reason) and
        // what to do next (the action's guidance or button). The assertions name one fact of
        // each of the eight states, not a single reworded sentence.
        for empty in [
            "Empty document",
            "This document has no text yet.",
            "No plot yet",
            "No run has produced a plot for this document.",
            "No result yet",
            "No run has published a result for this document.",
            "No variables yet",
            "No run has published a workspace for this document.",
            "No log lines yet",
            "This document's log is empty.",
            "Nothing to inspect",
            "No run has published an inspection payload.",
        ] {
            assert!(
                texts.iter().any(|text| text == empty),
                "each pane must paint its own empty state; missing {empty:?}\ntexts: {texts:?}"
            );
        }
        // The run-fed panes offer the one command that fills them, and the editor offers the
        // first keystroke; both labels are painted buttons, so the pointer (and UIA) can reach
        // them.
        for action in ["Run to populate", "Start typing"] {
            assert!(
                texts.iter().any(|text| text == action),
                "the empty state must offer {action:?}; texts: {texts:?}"
            );
        }
        // Every pane has a visible title and the same string is its accessible name. The editor's
        // title carries the document name.
        let rows = accessibility_rows(&ctx, &mut app, screen);
        let pane_names: Vec<&str> = rows
            .iter()
            .filter(|row| row.role == egui::Role::Pane && !row.name.is_empty())
            .map(|row| row.name.as_str())
            .collect();
        for title in ["Workspace", "Graph", "Logs", "Symbolic Inspector"] {
            assert!(
                pane_names.contains(&title),
                "the {title} pane is missing; painted panes: {pane_names:?}"
            );
        }
        assert!(
            pane_names.iter().any(|name| name.starts_with("Editor - ")),
            "the editor pane is titled with its document: {pane_names:?}"
        );
        // The titles are visible text, not only accessible names.
        for title in ["Workspace", "Graph", "Logs", "Symbolic Inspector"] {
            assert!(
                texts.iter().any(|text| text == title),
                "the pane title {title:?} must be painted: {texts:?}"
            );
        }

        // Populated: a run with a symbolic result, a plot, variables and log lines fills all five
        // panes from the one committed snapshot.
        let plot_dir = scratch_dir("lay-002-populated");
        let mut app = a11y_app(plot_dir.clone());
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let texts = run_from_keyboard(&ctx, &mut app);
        for populated in [
            "Expression tree",
            "Pretty",
            "Canonical",
            "a11y journey plot",
            "sqrt(2)*pi",
        ] {
            assert!(
                texts.iter().any(|text| text.contains(populated)),
                "the populated window must show {populated:?}\ntexts: {texts:?}"
            );
        }
        // The graph pane holds the engine SVG canvas; the inspector tree has a node header.
        let rows = accessibility_rows(&ctx, &mut app, screen);
        assert!(
            rows.iter()
                .any(|row| row.role == egui::Role::Image && row.name.contains("SVG plot canvas")),
            "the graph pane must carry the engine plot canvas"
        );
        assert!(
            rows.iter().any(|row| row.name.contains("Expression tree")),
            "the Symbolic Inspector must publish its expression-tree section"
        );
        assert!(
            rows.iter()
                .any(|row| row.name.starts_with("x | Symbolic | sqrt(2)*pi")),
            "the Workspace pane must publish the committed variable row"
        );

        // The inspector's own error state: an engine that publishes no inspection payload paints
        // the specific reason in the dedicated pane instead of an empty box.
        let mut app = structural_app();
        frame_sized(&ctx, &mut app, screen, Vec::new());
        run_from_keyboard(&ctx, &mut app);
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts.iter().any(|text| text
                .starts_with("The engine published no inspection payload for this result.")),
            "the inspector's missing-payload error must paint in its own pane: {texts:?}"
        );
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// LAY-003: a splitter resizes its adjacent panes by a real pointer drag and by arrow keys on
    /// the focused splitter, clamps at every pane minimum, and a double-click or the reset
    /// command restores the shipped sizes.
    #[test]
    fn lay_003_splitters_resize_by_pointer_and_keyboard_then_reset() {
        let mut app = structural_app();
        let ctx = egui::Context::default();
        let screen = vec2(1280.0, 800.0);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let start = app.layout.expect("the initial layout");
        let start_editor = start.editor.width();

        // Pointer drag: editor/graph divider +120 points.
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            start.split_a.center(),
            start.split_a.center() + vec2(120.0, 0.0),
        );
        let dragged = app.layout.expect("the dragged layout");
        assert!(
            dragged.editor.width() >= start_editor + 100.0,
            "a pointer drag must widen the editor: {start_editor} -> {}",
            dragged.editor.width()
        );

        // Double-click restores the shipped sizes exactly. The preceding gesture is a drag (its
        // release is not a click), so this is the first click pair the pointer state sees.
        pointer_double_click_sized(&ctx, &mut app, screen, dragged.split_a.center());
        assert_eq!(
            app.editor_frac, DEFAULT_EDITOR_FRACTION,
            "a splitter double-click must restore the default editor fraction"
        );
        assert_eq!(app.workspace_frac, DEFAULT_WORKSPACE_FRACTION);
        assert_eq!(app.logs_frac, DEFAULT_LOGS_FRACTION);
        assert_eq!(app.inspector_frac, DEFAULT_INSPECTOR_FRACTION);

        // Keyboard: focus the editor/graph splitter with a real click, then resize with the
        // arrow key its focus path handles.
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let splitter_center = app.layout.expect("the reset layout").split_a.center();
        pointer_click_sized(&ctx, &mut app, screen, splitter_center);
        let before_key = app
            .layout
            .expect("the layout before the arrow key")
            .editor
            .width();
        frame_sized(
            &ctx,
            &mut app,
            screen,
            vec![key(Key::ArrowRight, egui::Modifiers::default())],
        );
        // The splitter applies the arrow after this pass's layout was computed, so the next
        // painted frame is the one that shows the new size.
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let after_key = app.layout.expect("the layout after the arrow key");
        assert!(
            after_key.editor.width() >= before_key + 6.0,
            "ArrowRight on the focused splitter must widen the editor: {before_key} -> {}",
            after_key.editor.width()
        );
        assert!(
            app.editor_frac > DEFAULT_EDITOR_FRACTION,
            "the keyboard resize must move the saved fraction: {}",
            app.editor_frac
        );

        // The command path (Ctrl+Shift+L) restores the same defaults the double-click did.
        let _ = press_action(&ctx, &mut app, Action::ResetLayout);
        assert_eq!(
            app.editor_frac, DEFAULT_EDITOR_FRACTION,
            "the reset command must restore the default editor fraction"
        );
        assert_eq!(app.inspector_frac, DEFAULT_INSPECTOR_FRACTION);
        frame_sized(&ctx, &mut app, screen, Vec::new());

        // Direction: each divider follows the pointer. The Symbolic Inspector divider moves
        // right to shrink the inspector; the graph/workspace divider moves right to shrink the
        // workspace. Both are real pointer drags, not fraction writes.
        let direction = app.layout.expect("the default layout");
        let inspector_before = direction.inspector.width();
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            direction.split_d.center(),
            direction.split_d.center() + vec2(60.0, 0.0),
        );
        let moved_inspector = app.layout.expect("the inspector-divider layout");
        assert!(
            moved_inspector.split_d.left() >= direction.split_d.left() + 40.0,
            "the inspector divider must follow a rightward pointer drag: {} -> {}",
            direction.split_d.left(),
            moved_inspector.split_d.left()
        );
        assert!(
            moved_inspector.inspector.width() <= inspector_before - 40.0,
            "a rightward inspector drag must shrink the inspector: {inspector_before} -> {}",
            moved_inspector.inspector.width()
        );
        let workspace_before = moved_inspector.workspace.width();
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            moved_inspector.split_b.center(),
            moved_inspector.split_b.center() + vec2(60.0, 0.0),
        );
        let moved_workspace = app.layout.expect("the workspace-divider layout");
        assert!(
            moved_workspace.split_b.left() >= moved_inspector.split_b.left() + 40.0,
            "the workspace divider must follow a rightward pointer drag: {} -> {}",
            moved_inspector.split_b.left(),
            moved_workspace.split_b.left()
        );
        assert!(
            moved_workspace.workspace.width() <= workspace_before - 40.0,
            "a rightward workspace drag must shrink the workspace: {workspace_before} -> {}",
            moved_workspace.workspace.width()
        );
        // The command path restores the defaults again before the minimum-size drags.
        let _ = press_action(&ctx, &mut app, Action::ResetLayout);
        frame_sized(&ctx, &mut app, screen, Vec::new());

        // Pointer drag to the far left: the minimum-size clamp stops the editor at 320 points.
        let current = app.layout.expect("the layout before the clamp drags");
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            current.split_a.center(),
            pos2(4.0, current.split_a.center().y),
        );
        let clamped = app.layout.expect("the clamped layout");
        eprintln!(
            "LAY-003 min clamp: editor={:.1} graph={:.1} workspace={:.1} logs_h={:.1} inspector_w={:.1}",
            clamped.editor.width(),
            clamped.plot.width(),
            clamped.workspace.width(),
            clamped.logs.height(),
            clamped.inspector.width()
        );
        assert!(
            clamped.editor.width() >= MIN_EDITOR - 0.5,
            "the editor must clamp at {MIN_EDITOR}: {}",
            clamped.editor.width()
        );
        assert!(
            clamped.plot.width() >= MIN_PLOT - 0.5,
            "the graph must clamp at {MIN_PLOT}: {}",
            clamped.plot.width()
        );
        assert!(
            clamped.workspace.width() >= MIN_WORKSPACE - 0.5,
            "the workspace must clamp at {MIN_WORKSPACE}: {}",
            clamped.workspace.width()
        );
        // Workspace minimum: drag its divider right until the pane stops shrinking.
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            clamped.split_b.center(),
            clamped.split_b.center() + vec2(200.0, 0.0),
        );
        let workspace_min = app.layout.expect("the workspace-minimum layout");
        assert!(
            workspace_min.workspace.width() >= MIN_WORKSPACE - 0.5
                && workspace_min.workspace.width() <= MIN_WORKSPACE + 20.0,
            "the workspace must stop at its {MIN_WORKSPACE} minimum: {}",
            workspace_min.workspace.width()
        );
        // Graph minimum: drag the same divider left until the graph stops shrinking.
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            workspace_min.split_b.center(),
            workspace_min.split_b.center() - vec2(700.0, 0.0),
        );
        let graph_min = app.layout.expect("the graph-minimum layout");
        assert!(
            graph_min.plot.width() >= MIN_PLOT - 0.5 && graph_min.plot.width() <= MIN_PLOT + 10.0,
            "the graph must stop at its {MIN_PLOT} minimum: {}",
            graph_min.plot.width()
        );
        // The logs/inspector divider clamps the inspector at its own minimum.
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            graph_min.split_d.center(),
            pos2(screen.x - 4.0, graph_min.split_d.center().y),
        );
        let bottom = app.layout.expect("the bottom-strip layout");
        assert!(
            bottom.inspector.width() >= MIN_INSPECTOR_WIDTH - 0.5
                && bottom.inspector.width() <= MIN_INSPECTOR_WIDTH + 20.0,
            "the inspector must stop at its {MIN_INSPECTOR_WIDTH} minimum: {}",
            bottom.inspector.width()
        );
        // And the top/bottom divider clamps the Logs strip's height.
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            bottom.split_c.center(),
            pos2(bottom.split_c.center().x, screen.y - 4.0),
        );
        let vertical = app.layout.expect("the vertical layout");
        assert!(
            vertical.logs.height() >= MIN_LOGS - 0.5 && vertical.logs.height() <= MIN_LOGS + 20.0,
            "the logs strip must stop at its {MIN_LOGS} minimum: {}",
            vertical.logs.height()
        );
    }

    /// LAY-009: Ctrl/Cmd+Tab switches documents, F6 / Shift+F6 rotate keyboard focus between the
    /// pane groups, the focus visibly lands in the group it names (the status strip says so, a
    /// real widget in that group owns the keyboard and the focus ring is painted around it), and
    /// both chords are documented in the one binding table the Help reference renders.
    #[test]
    fn lay_009_ctrl_tab_switches_documents_and_f6_rotates_pane_focus_visibly() {
        let ctx = egui::Context::default();
        let plot_dir = scratch_dir("lay-009");
        let mut app = a11y_app(plot_dir.clone());
        // Three tabs, so "next" and "previous" are both observable and ordered.
        let first = app.workbench.new_document();
        let second = app.workbench.new_document();
        app.sync_after_tab_change(second, &ctx);
        pump_text(&ctx, &mut app, 2);
        assert_eq!(app.workbench.active(), second, "the newest tab is active");

        // The documented chords, from the same table the Help window paints (A11Y-007).
        let rows = keyboard::reference_rows();
        let row = |chord: &str| {
            rows.iter()
                .find(|row| row.chord == chord)
                .unwrap_or_else(|| panic!("{chord} must be documented"))
        };
        assert_eq!(row("Ctrl+Tab").description, "Select the next document tab");
        assert_eq!(
            row("Ctrl+Shift+Tab").description,
            "Select the previous document tab"
        );
        assert_eq!(
            row("F6").description,
            "Move keyboard focus to the next pane group"
        );
        assert_eq!(
            row("Shift+F6").description,
            "Move keyboard focus to the previous pane group"
        );

        // Ctrl+Tab switches documents from the keyboard; Ctrl+Shift+Tab goes back.
        let ctrl = ctrl_modifiers();
        let _ = a11y_frame(
            &ctx,
            &mut app,
            vec![egui::Event::ModifiersChanged(ctrl), key(Key::Tab, ctrl)],
        );
        assert_eq!(app.workbench.active(), 0, "Ctrl+Tab wraps to the first tab");
        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            command: true,
            shift: true,
            ..Default::default()
        };
        let _ = a11y_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::ModifiersChanged(ctrl_shift),
                key(Key::Tab, ctrl_shift),
            ],
        );
        assert_eq!(
            app.workbench.active(),
            second,
            "Ctrl+Shift+Tab returns to the previous tab"
        );
        let _ = first;

        // F6 walks the groups; each landing is visible and real.
        let ring = theme::tokens_for(theme::ThemeId::Dark).focus_ring;
        assert_eq!(app.focused_pane, Pane::Editor);
        for expected in [Pane::Plot, Pane::Workspace, Pane::Logs, Pane::Toolbar] {
            let texts = press_action(&ctx, &mut app, Action::NextPane);
            assert_eq!(app.focused_pane, expected, "F6 order");
            assert!(
                texts
                    .iter()
                    .any(|text| text == &format!("focus: {}", expected.label())),
                "the status strip names the focused pane: {texts:?}"
            );
            let output = a11y_frame(&ctx, &mut app, Vec::new());
            let focus = ctx
                .memory(|memory| memory.focused())
                .unwrap_or_else(|| panic!("{} must own the keyboard", expected.label()));
            let expected_id = match expected {
                Pane::Toolbar => app.run_button_id,
                Pane::Editor => Some(surface_id()),
                Pane::Plot => app.plot_fit_id,
                Pane::Workspace => app.workspace_tab_ids[0],
                Pane::Logs => Some(quick_eval_id()),
            };
            assert_eq!(
                Some(focus),
                expected_id,
                "the {} group's first control takes focus",
                expected.label()
            );
            let rect = ctx
                .read_response(focus)
                .map(|response| response.rect)
                .expect("the focused widget was painted");
            let mut strokes = Vec::new();
            for clipped in &output.shapes {
                if clipped
                    .clip_rect
                    .intersects(rect.expand(a11y::FOCUS_RING_GAP + 4.0))
                {
                    rect_stroke_colors(&clipped.shape, &mut strokes);
                }
            }
            assert!(
                strokes.iter().any(|(color, _)| *color == ring),
                "the focus ring is painted around the {} group's control: {strokes:?}",
                expected.label()
            );
        }
        // Shift+F6 steps back one group.
        press_action(&ctx, &mut app, Action::PreviousPane);
        assert_eq!(app.focused_pane, Pane::Logs, "Shift+F6 goes back one pane");
        let _ = std::fs::remove_dir_all(&plot_dir);
    }

    /// ARC-010: every typed projection the engine publishes reaches the panes — numeric, array,
    /// record and symbolic here, with the typed diagnostic fields checked on a failing run — and
    /// the *display* string is only ever painted as text.
    #[test]
    #[cfg(feature = "real-engine")]
    fn arc_010_the_panes_render_every_typed_projection() {
        let mut app = WorkbenchApp::with_workbench(Workbench::new());
        let ctx = egui::Context::default();
        let screen = vec2(2500.0, 1380.0);
        let cases = [
            // (source, projection kind, a field the pane must paint from the projection)
            ("m = [[1, 2], [3, 4]]\nm", "Array", "2 x 2"),
            ("divrem(7, 2)", "Record", "DivRemResult"),
            (
                "x = symbol(\"x\")\n3*x^2 + 4*x + 5",
                "Symbolic",
                "3*x^2 + 4*x + 5",
            ),
            ("evalf(1/3, 1500)", "Real", "truncated: digit-cap"),
        ];
        for (source, kind, needle) in cases {
            app.workbench.set_source(0, source.to_string());
            assert!(app.workbench.run_active(), "the {kind} run must start");
            assert!(
                app.workbench.wait_until_idle(Duration::from_secs(30)),
                "the {kind} run must finish"
            );
            let texts = painted_texts(&ctx, &mut app, screen);
            assert!(
                app.view.result_structured.is_some(),
                "the engine published a structured projection for {kind}"
            );
            assert!(
                texts.iter().any(|text| text.contains(kind)),
                "the Workspace result paints the projection kind {kind}: {texts:?}"
            );
            assert!(
                texts.iter().any(|text| text.contains(needle)),
                "the {kind} pane paints {needle:?} from the projection: {texts:?}"
            );
        }

        // The diagnostic projection is typed too: a failing run paints the engine's code,
        // category and position, not a string reconstructed from the message.
        app.workbench.set_source(0, "1 / 0".to_string());
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(30)));
        let texts = painted_texts(&ctx, &mut app, screen);
        let diagnostic = app
            .view
            .diagnostic
            .clone()
            .expect("the failing run published a typed diagnostic");
        assert_eq!(diagnostic.code.as_deref(), Some("DivisionByZero"));
        assert_eq!(diagnostic.category.as_deref(), Some("DomainError"));
        assert_eq!((diagnostic.line, diagnostic.column), (1, 1));
        assert!(
            texts
                .iter()
                .any(|text| text.contains("DivisionByZero") && text.contains("DomainError")),
            "the diagnostic's own code and category are painted: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("error at Ln 1, Col 1")),
            "the diagnostic's typed position is painted: {texts:?}"
        );
    }

    /// ARC-010: a display string that *looks* like a record cannot make the pane render a record
    /// when the engine's projection says 2x2 array. The same value painted twice with two
    /// contradictory display strings yields the same structure: the display is a label, never a
    /// source of structure.
    #[test]
    fn arc_010_a_misleading_display_string_cannot_change_the_painted_result() {
        struct MisleadingEngine {
            display: String,
        }
        impl crate::engine::Evaluator for MisleadingEngine {
            fn eval(&mut self, _src: &str) -> Result<String, String> {
                Ok(self.display.clone())
            }
            fn last_structured(&self) -> Option<lovelace_json::Json> {
                use lovelace_json::Json;
                Some(Json::obj([
                    ("kind", Json::Str("Array".to_string())),
                    ("type", Json::Str("Array".to_string())),
                    ("rank", Json::Num("2".to_string())),
                    ("shape", Json::str_array(["2", "2"])),
                    (
                        "elements",
                        Json::Arr(vec![
                            Json::obj([
                                ("kind", Json::Str("Natural".to_string())),
                                ("value", Json::Str("1".to_string())),
                            ]),
                            Json::obj([
                                ("kind", Json::Str("Natural".to_string())),
                                ("value", Json::Str("2".to_string())),
                            ]),
                            Json::obj([
                                ("kind", Json::Str("Natural".to_string())),
                                ("value", Json::Str("3".to_string())),
                            ]),
                            Json::obj([
                                ("kind", Json::Str("Natural".to_string())),
                                ("value", Json::Str("4".to_string())),
                            ]),
                        ]),
                    ),
                ]))
            }
        }

        let mut painted = Vec::new();
        for display in [
            "DivRemResult { quotient: 9, remainder: 9 }",
            "[[9, 9], [9, 9]]",
        ] {
            let display = display.to_string();
            let workbench = Workbench::with_factory(Box::new(move || {
                Box::new(MisleadingEngine {
                    display: display.clone(),
                })
            }));
            let mut app = WorkbenchApp::with_workbench(workbench);
            assert!(app.workbench.run_active());
            assert!(app.workbench.wait_until_idle(Duration::from_secs(10)));
            let ctx = egui::Context::default();
            let texts = painted_texts(&ctx, &mut app, vec2(2500.0, 1380.0));
            painted.push(texts);
        }
        let [array_display, record_display] = &painted[..] else {
            unreachable!("two paints")
        };
        // The tree rows (the projection's kinds and element values) must be identical whatever the
        // display says. Timing and status texts are excluded: they are not structure.
        let structure = |texts: &Vec<String>| -> std::collections::BTreeSet<String> {
            texts
                .iter()
                .filter(|text| {
                    text.contains("Array") || text.contains("Natural") || text.contains("2 x 2")
                })
                .cloned()
                .collect()
        };
        let from_array_display = structure(array_display);
        let from_record_display = structure(record_display);
        assert_eq!(
            from_array_display, from_record_display,
            "two contradictory display strings must paint the identical structure"
        );
        assert!(
            from_array_display.iter().any(|text| text.contains("Array")),
            "the projection's array kind is painted: {from_array_display:?}"
        );
        assert!(
            from_array_display.iter().any(|text| text.contains("2 x 2")),
            "the projection's shape is painted: {from_array_display:?}"
        );
        assert!(
            !from_array_display
                .iter()
                .any(|text| text.contains("quotient") || text.contains("remainder")),
            "no record field row is created from the display: {from_array_display:?}"
        );
        // The display string is still painted as text — as the log's result line, not as a tree.
        assert!(
            array_display
                .iter()
                .any(|text| text.starts_with("[result] = DivRemResult { quotient: 9")),
            "the display text is painted as the log's result line: {array_display:?}"
        );
    }

    /// LAY-004: pane sizes and the compact drawer's visible pane are written to the settings file
    /// when the user changes them and restored by a fresh launch from that file.
    #[test]
    fn lay_004_pane_sizes_and_visibility_persist_across_relaunch() {
        let ctx = egui::Context::default();
        let (storage, directory) = app_storage("layout-persist");
        let settings_path = storage.settings_path.clone().expect("settings path");
        let screen = vec2(1280.0, 800.0);
        let mut app = WorkbenchApp::launch(storage.clone(), None);
        frame_sized(&ctx, &mut app, screen, Vec::new());
        let before = app.layout.expect("the initial layout");
        pointer_drag_sized(
            &ctx,
            &mut app,
            screen,
            before.split_a.center(),
            before.split_a.center() + vec2(90.0, 0.0),
        );
        let after = app.layout.expect("the dragged layout");
        assert!(
            after.editor.width() > before.editor.width() + 60.0,
            "the drag must change the size that is persisted"
        );
        let saved = Settings::load(&settings_path);
        assert!(
            saved.warnings.is_empty(),
            "the settings file must stay valid: {:?}",
            saved.warnings
        );
        assert_eq!(
            saved.workbench_layout.editor_permille,
            app.layout_settings().editor_permille,
            "the dragged editor size must be written to the settings file"
        );
        assert!(
            directory.join("desktop.json").exists(),
            "the settings file must exist after the drag"
        );

        // Relaunch from the same settings file: the sizes come back before any frame paints.
        let expected = app.layout_settings();
        drop(app);
        let mut relaunched = WorkbenchApp::launch(storage.clone(), None);
        assert_eq!(
            relaunched.layout_settings(),
            expected,
            "a fresh launch must restore the persisted pane sizes"
        );
        frame_sized(&ctx, &mut relaunched, screen, Vec::new());
        let restored = relaunched.layout.expect("the restored layout");
        assert!(
            (restored.editor.width() - after.editor.width()).abs() < 0.5,
            "the restored geometry must match the pre-close geometry: {} vs {}",
            restored.editor.width(),
            after.editor.width()
        );

        // Visibility: in the compact layout the drawer either shows one pane or is collapsed.
        // Opening the Symbolic Inspector is a visible-pane choice; it must survive a relaunch.
        let compact = vec2(450.0, 300.0);
        frame_sized(&ctx, &mut relaunched, compact, Vec::new());
        assert_eq!(
            relaunched.layout.expect("the compact layout").mode,
            LayoutMode::Compact
        );
        let inspector_tab = relaunched
            .compact_tabs
            .iter()
            .find(|(pane, _)| *pane == CompactPane::Inspector)
            .map(|(_, rect)| *rect)
            .expect("the Symbolic Inspector tab is painted");
        pointer_click_sized(&ctx, &mut relaunched, compact, inspector_tab.center());
        for _ in 0..2 {
            frame_sized(&ctx, &mut relaunched, compact, Vec::new());
        }
        assert_eq!(
            relaunched.compact_pane,
            Some(CompactPane::Inspector),
            "the click must open the Symbolic Inspector drawer"
        );
        let opened = relaunched.layout.expect("the open drawer layout");
        assert!(
            opened.inspector.is_positive(),
            "the opened drawer paints the Symbolic Inspector pane"
        );
        let expected_visibility = relaunched.layout_settings();
        assert_eq!(
            expected_visibility.compact_pane,
            settings::CompactPaneSetting::Inspector
        );
        drop(relaunched);

        let mut third = WorkbenchApp::launch(storage, None);
        frame_sized(&ctx, &mut third, compact, Vec::new());
        assert_eq!(
            third.compact_pane,
            Some(CompactPane::Inspector),
            "the visible pane choice must be restored by the relaunch"
        );
        let restored_layout = third.layout.expect("the restored compact layout");
        assert_eq!(restored_layout.mode, LayoutMode::Compact);
        assert!(
            restored_layout.inspector.is_positive(),
            "the restored drawer must paint the Symbolic Inspector pane"
        );
        assert_eq!(
            third.layout_settings().editor_permille,
            expected_visibility.editor_permille,
            "the sizes and the visibility must come from the same settings document"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// WKS-003 at the paint layer: every variable row carries its own delete control, and
    /// clicking it removes exactly that variable, leaves the function rows and the result alone,
    /// and moves the workspace revision. The click is a real pointer click on the painted rect.
    #[test]
    fn wks_003_the_painted_row_delete_removes_only_that_variable() {
        let mut app = fake_app_with_source("alpha = 1\nbeta = 2\n");
        let ctx = egui::Context::default();
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        let function_count = app.functions_table.rows.len();
        let revision = app.view.workspace_revision;
        let controls: Vec<String> = app
            .variable_action_rects
            .iter()
            .map(|(key, _)| key.clone())
            .collect();
        assert!(
            controls.iter().any(|key| key == "delete beta"),
            "the beta row must paint its delete control: {controls:?}"
        );
        assert!(
            controls.iter().any(|key| key == "details beta"),
            "and its detail control: {controls:?}"
        );
        let beta = app
            .variable_action_rects
            .iter()
            .find(|(key, _)| key == "delete beta")
            .map(|(_, rect)| *rect)
            .expect("the delete control of beta");
        pointer_click_sized(&ctx, &mut app, vec2(1600.0, 1000.0), beta.center());
        // The tables are rebuilt from the committed capture at the start of the next pass, so the
        // frame after the click is the one that shows the deletion.
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        let names: Vec<String> = app
            .variables_table
            .rows
            .iter()
            .map(|row| row.name.clone())
            .collect();
        assert!(
            !names.iter().any(|name| name == "beta"),
            "only beta is deleted: {names:?}"
        );
        assert!(
            names.iter().any(|name| name == "alpha"),
            "alpha survives: {names:?}"
        );
        assert_eq!(
            app.functions_table.rows.len(),
            function_count,
            "the function table is unchanged"
        );
        assert_eq!(
            app.view.workspace_revision,
            revision + 1,
            "the workspace revision moved"
        );
        assert!(
            app.view
                .logs
                .iter()
                .any(|line| line.text.contains("Variable deleted: beta")),
            "the delete is reported in the log: {:?}",
            app.view.logs.last()
        );
    }

    /// WKS-004 at the paint layer: Clear Variables empties the variables table and keeps every
    /// user function row in the functions table, and the counts follow the same snapshot.
    #[test]
    fn wks_004_clear_variables_keeps_the_painted_function_rows() {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::RealEngine::new())));
        workbench.set_source(
            0,
            "func twice(x) = x * 2\nanswer = 21\ntwice(answer)\n".to_string(),
        );
        assert!(workbench.run_active());
        assert!(workbench.wait_until_idle(Duration::from_secs(10)));
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        assert!(
            app.variables_table
                .rows
                .iter()
                .any(|row| row.name == "answer"),
            "the run published answer"
        );
        let functions_before = app.functions_table.rows.len();
        assert!(app.workbench.clear_variables_active().is_some());
        frame(&ctx, &mut app, Vec::new());
        assert!(
            app.variables_table.rows.is_empty(),
            "the variables table is emptied: {:?}",
            app.variables_table.rows
        );
        assert_eq!(
            app.functions_table.rows.len(),
            functions_before,
            "the functions table keeps every row"
        );
        assert!(
            app.functions_table
                .rows
                .iter()
                .any(|row| row.name == "twice" && row.origin_label() == "user"),
            "the user function row survives"
        );
        assert_eq!(
            app.view.workspace_revision, 2,
            "the clear moved the workspace revision once"
        );
    }

    /// WKS-006 at the paint layer: the variables table paints a type-aware preview, and the
    /// row's detail view opens the full value the preview shortened.
    #[test]
    fn wks_006_the_table_paints_a_typed_preview_and_the_detail_opens_the_full_value() {
        let mut workbench =
            Workbench::with_factory(Box::new(|| Box::new(crate::engine::RealEngine::new())));
        workbench.set_source(0, "m = [[1, 2], [3, 4]]\ndivrem(7, 2)\n".to_string());
        assert!(workbench.run_active());
        assert!(workbench.wait_until_idle(Duration::from_secs(10)));
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        let m = app
            .variables_table
            .rows
            .iter()
            .find(|row| row.name == "m")
            .expect("m is a committed variable");
        assert_eq!(m.preview.type_label, "2 x 2 matrix");
        assert_eq!(m.preview.text, "[1, 2]; [3, 4]");
        let details = app
            .variable_action_rects
            .iter()
            .find(|(key, _)| key == "details m")
            .map(|(_, rect)| *rect)
            .expect("the m row paints its detail control");
        pointer_click_sized(&ctx, &mut app, vec2(1600.0, 1000.0), details.center());
        assert_eq!(
            app.variable_detail.as_deref(),
            Some("m"),
            "the detail control opens the row's detail: rects={:?}",
            app.variable_action_rects
        );
        let mut texts = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                ..Default::default()
            },
            |ui| {
                let view = app.view.clone();
                let row = app
                    .variables_table
                    .rows
                    .iter()
                    .find(|row| row.name == "m")
                    .cloned()
                    .expect("the row");
                let _ = view;
                paint_variable_detail(ui, &row);
            },
        );
        output.textures_delta.clear();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            texts.iter().any(|text| text == "Details · m"),
            "the detail view names the variable: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "structured · Array"),
            "the detail view names the published structure: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "shape      2 x 2"),
            "the detail view states the shape: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.trim_start().starts_with("[0]  Natural")),
            "the detail view lists the element kinds: {texts:?}"
        );
    }

    /// WKS-009: a tab switch repaints the workspace and the plot from the newly active document
    /// in one pass, even while the other tab's run is still in flight.
    #[test]
    fn wks_009_a_tab_switch_matches_the_workspace_and_plot_while_the_other_tab_runs() {
        let gate = Arc::new(Gate::default());
        let mut workbench = gated_workbench(Arc::clone(&gate));
        workbench.set_source(0, "alpha = 1\n".to_string());
        workbench.set_source(1, "beta = 2\n".to_string());
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        // Tab A's engine blocks inside the run, so tab B's frame happens during A's run.
        assert!(app.workbench.run_active());
        assert_eq!(app.workbench.documents()[0].state(), RunState::Queued);
        app.workbench.set_active(1);
        let mut texts = Vec::new();
        let mut output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            Vec::new(),
            Default::default(),
        );
        output.textures_delta.clear();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            app.workbench.documents()[0].state().is_in_flight(),
            "tab A is still running during the switch"
        );
        assert_eq!(
            app.view.buffer_revision,
            crate::execution::SourceRevision(1),
            "the frame's view is the active document's"
        );
        assert!(
            app.variables_table
                .rows
                .iter()
                .all(|row| row.name != "alpha"),
            "tab A's variables are not shown on tab B: {:?}",
            app.variables_table.rows
        );
        assert!(
            app.plot.provenance.is_none(),
            "tab B has no plot capture, so no other tab's chart can be shown"
        );
        assert!(
            !texts.iter().any(|text| text.contains("alpha")),
            "the painted frame shows no tab-A workspace text: {texts:?}"
        );
        gate.release();
    }

    /// RES-002 at the paint layer: the result section states the kind, the exactness, the
    /// precision and any truncation reason the projection published.
    #[test]
    fn res_002_the_result_paints_exactness_precision_and_truncation() {
        use lovelace_json::Json;
        let ctx = egui::Context::default();
        let mut view = result_view("3.14159...");
        view.precision = 1000;
        view.result_structured = Some(Json::obj([
            ("kind", Json::Str("Real".to_string())),
            ("value", Json::Str("3.14159265".to_string())),
            ("exact", Json::Bool(false)),
            ("truncated", Json::Bool(true)),
            ("truncationReason", Json::Str("digit-cap".to_string())),
            ("budget", Json::Num("1000".to_string())),
        ]));
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let _ = paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
                });
            },
        );
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            texts.iter().any(|text| text
                == "Real · approximate · precision 1000 · truncated: digit-cap (budget 1000)"),
            "the provenance line: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "approximate"),
            "the scalar row states approximate, not exact: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "truncated: digit-cap"),
            "the reason is in words: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "exact=true"),
            "no raw exact=true field is painted: {texts:?}"
        );
    }

    /// RES-005 at the paint layer: the symbolic result's pretty text, canonical form, domain,
    /// free symbols and node count are all painted from the projection.
    #[test]
    fn res_005_the_symbolic_result_paints_pretty_canonical_domain_free_symbols_and_node_count() {
        use lovelace_json::Json;
        let ctx = egui::Context::default();
        let mut view = symbolic_result_view();
        view.result_structured = Some(Json::obj([
            ("kind", Json::Str("Symbolic".to_string())),
            ("pretty", Json::Str("x^2 - 1".to_string())),
            (
                "canonical",
                Json::Str("(add (pow (sym x) (rat 2 1)) (rat -1 1))".to_string()),
            ),
            ("domain", Json::Str("real".to_string())),
            ("freeSymbols", Json::str_array(["x"])),
            ("nodeCount", Json::Str("5".to_string())),
            ("exact", Json::Bool(true)),
        ]));
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let _ = paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
                });
            },
        );
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            texts.iter().any(|text| text == "pretty     x^2 - 1"),
            "pretty: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "canonical  (add (pow (sym x) (rat 2 1)) (rat -1 1))"),
            "canonical: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "domain     real"),
            "domain: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "free       x"),
            "free symbols: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "nodes      5"),
            "node count: {texts:?}"
        );
    }

    /// An evaluator that counts how often it is asked to evaluate and publishes one structured
    /// Natural value per evaluation. Used to prove that inspecting a captured result is not an
    /// evaluation.
    struct CountingEvaluator {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl crate::engine::Evaluator for CountingEvaluator {
        fn eval(&mut self, _src: &str) -> Result<String, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok("41".to_string())
        }

        fn variables(&self) -> Vec<(String, String)> {
            vec![("answer".to_string(), "41".to_string())]
        }

        fn state(&self) -> Option<crate::engine::EngineState> {
            Some(crate::engine::EngineState {
                variables: vec![crate::engine::EngineVariable {
                    name: "answer".to_string(),
                    kind: "Natural".to_string(),
                    display: "41".to_string(),
                    structured: Some(lovelace_json::Json::obj([
                        ("kind", lovelace_json::Json::Str("Natural".to_string())),
                        ("value", lovelace_json::Json::Str("41".to_string())),
                        ("exact", lovelace_json::Json::Bool(true)),
                    ])),
                }],
                functions: Vec::new(),
            })
        }

        fn last_structured(&self) -> Option<lovelace_json::Json> {
            Some(lovelace_json::Json::obj([
                ("kind", lovelace_json::Json::Str("Natural".to_string())),
                ("value", lovelace_json::Json::Str("41".to_string())),
                ("exact", lovelace_json::Json::Bool(true)),
            ]))
        }
    }

    /// RES-006: selecting result detail uses the captured value and revision; painting and
    /// expanding it evaluates nothing, and an edit after the run does not recompute the value
    /// being inspected.
    #[test]
    fn res_006_inspecting_a_result_never_evaluates_again_and_survives_a_source_edit() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let mut workbench = Workbench::with_factory(Box::new(move || {
            Box::new(CountingEvaluator {
                calls: Arc::clone(&counter),
            })
        }));
        workbench.set_source(0, "answer = 1 + 1\nanswer + 1\n".to_string());
        assert!(workbench.run_active());
        assert!(workbench.wait_until_idle(Duration::from_secs(5)));
        let evaluations = calls.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(evaluations, 2, "the two statements evaluated once each");
        let mut app = WorkbenchApp::with_workbench(workbench);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());
        let captured = app.view.result.clone();
        let revision = app.view.revision;
        // Expand every painted result header: inspection is a model read, never an evaluation.
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let view = app.view.clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let _ = paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
                });
            },
        );
        output.textures_delta.clear();
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            evaluations,
            "painting the result performed no evaluation"
        );
        // Edit the source after the run: the inspected value and its revision stay as committed.
        app.workbench.set_source(0, "answer = 99\n".to_string());
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            evaluations,
            "editing the source performed no evaluation"
        );
        assert_eq!(
            app.view.result, captured,
            "the inspected value is the captured one, not recomputed"
        );
        assert_eq!(
            app.view.revision, revision,
            "the inspected result keeps its committed revision"
        );
        assert!(
            app.view.buffer_revision.0 > app.view.revision.0,
            "the buffer moved on while the inspected value stayed committed"
        );
    }

    /// RES-003 / RES-010 at the paint layer: a 100 000-cell matrix is painted as a virtualised
    /// window (a bounded number of widgets naming its shape and element kinds), and a
    /// million-digit value paints a bounded prefix with a note while the full value stays in the
    /// model.
    #[test]
    fn res_010_a_huge_result_paints_a_bounded_number_of_widgets() {
        use lovelace_json::Json;
        let ctx = egui::Context::default();
        let cell = |index: usize| {
            Json::obj([
                ("kind", Json::Str("Natural".to_string())),
                ("value", Json::Str(index.to_string())),
                ("exact", Json::Bool(true)),
            ])
        };
        let mut view = result_view("huge");
        view.result_structured = Some(Json::obj([
            ("kind", Json::Str("Array".to_string())),
            ("type", Json::Str("Array".to_string())),
            ("shape", Json::num_array(vec![316, 316])),
            ("elements", Json::Arr((0..99_856).map(cell).collect())),
            ("exact", Json::Bool(true)),
        ]));
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let started = std::time::Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let _ = paint_result_section(ui, &view, 420.0, &mut expansion, &mut toggles);
                });
            },
        );
        let elapsed = started.elapsed();
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(
            texts.iter().any(|text| text.starts_with("virtualised:")),
            "the large array states its virtualised presentation: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("316 x 316")),
            "the shape is visible: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("Natural")),
            "the element kinds are visible: {texts:?}"
        );
        assert!(
            texts.len() < 400,
            "a 100 000-cell matrix must not allocate a widget per element: {} painted texts",
            texts.len()
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the huge matrix paint must not block the UI: {elapsed:?}"
        );

        // A million-digit value: one bounded label plus a note, never a million-glyph galley.
        let digits = "9".repeat(1_000_000);
        let mut big = result_view(&digits);
        big.result_structured = Some(Json::obj([
            ("kind", Json::Str("Natural".to_string())),
            ("value", Json::Str(digits.clone())),
            ("exact", Json::Bool(true)),
        ]));
        let mut expansion = ExpansionState::new();
        let mut toggles = Vec::new();
        let started = std::time::Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(460.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let _ = paint_result_section(ui, &big, 420.0, &mut expansion, &mut toggles);
                });
            },
        );
        let elapsed = started.elapsed();
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        let painted: usize = texts.iter().map(String::len).sum();
        assert!(
            texts.iter().any(|text| text.starts_with("999")),
            "the visible prefix of the value is painted"
        );
        assert!(
            !texts
                .iter()
                .any(|text| text.chars().count() > MAX_PAINTED_VALUE_CHARS + 256),
            "no painted label is longer than the paint bound"
        );
        assert_eq!(
            bounded_value_text(&digits).1,
            1_000_000 - MAX_PAINTED_VALUE_CHARS,
            "the bounded painter states exactly how many characters it withheld"
        );
        assert!(
            painted < 40_000,
            "the million-digit value paints a bounded prefix ({painted} chars)"
        );
        assert_eq!(
            big.result.chars().count(),
            1_000_000,
            "the full value stays in the model"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the million-digit paint must not block the UI: {elapsed:?}"
        );
    }

    /// LOG-004: clicking a diagnostic log line opens the editor at the reported position, selects
    /// the span there and puts the keyboard focus in the source.
    #[test]
    fn log_004_clicking_a_diagnostic_line_opens_and_highlights_its_source() {
        let mut app = fake_app_with_source("alpha = 1\n1 / 0\ngamma = 3\n");
        let ctx = egui::Context::default();
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        // Two passes let the terminal event's buffered lines land, so the rects describe the log
        // the click pass paints.
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        let index = app
            .view
            .logs
            .iter()
            .rposition(|line| {
                line.kind == crate::log_kind::LogKind::Diagnostic && line.location.is_some()
            })
            .expect("the failed run logged its diagnostic with a source position");
        assert_eq!(
            app.view.logs[index].location,
            Some((2, 1)),
            "the diagnostic log line names its source position"
        );
        // The pinned diagnostic row is a real painted control (it is the row a user clicks, and
        // the UIA capture invokes the same label through UI Automation). Click it with pointer
        // events: the click selects the line and opens/highlights the source it names.
        let row = app
            .log_pinned_rects
            .iter()
            .find(|(painted, _)| *painted == index)
            .map(|(_, rect)| *rect)
            .expect("the pinned diagnostic row is painted");
        pointer_click_sized(&ctx, &mut app, vec2(1600.0, 1000.0), row.center());
        assert!(
            app.log_selection.contains(&index),
            "the clicked diagnostic line becomes the selection: {:?}",
            app.log_selection
        );
        let caret = app.editors[0].caret();
        assert_eq!(
            (caret.row, caret.col),
            (1, 0),
            "the caret moved to the reported line and column"
        );
        assert!(
            app.editors[0].selection.is_some(),
            "the diagnostic's span is highlighted"
        );
        assert_eq!(
            app.focused_pane,
            Pane::Editor,
            "the source holds the pane focus after the click"
        );
        assert!(
            app.log_status
                .as_deref()
                .unwrap_or_default()
                .contains("opened source at 2:1"),
            "the pane reports the jump: {:?}",
            app.log_status
        );
    }

    /// LOG-005: copy selected, copy all, clear and save all work from the keyboard (and the same
    /// functions back the header buttons and the row context menu), with the exported file
    /// holding the same lines the pane paints.
    #[test]
    fn log_005_copy_selected_copy_all_clear_and_save_log_work_from_the_keyboard() {
        let mut app = fake_app_with_source("alpha = 1\nbeta = 2\n");
        assert!(app.workbench.run_active());
        assert!(app.workbench.wait_until_idle(Duration::from_secs(5)));
        let target = std::env::temp_dir().join("lovelace-round59-log.txt");
        let _ = std::fs::remove_file(&target);
        app.dialogs = Box::new(ScriptedDialogs::saving_log([Some(target.clone())]));
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, Vec::new());

        let chord = |key: Key, shift: bool, alt: bool| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                alt,
                ..Default::default()
            },
        };

        // Select one line by clicking its pinned row (the row-click seam), then Copy selected
        // (Ctrl+Shift+C).
        frame_sized(&ctx, &mut app, vec2(1600.0, 1000.0), Vec::new());
        let row = app
            .log_pinned_rects
            .first()
            .map(|(_, rect)| *rect)
            .expect("the pane paints its pinned rows");
        pointer_click_sized(&ctx, &mut app, vec2(1600.0, 1000.0), row.center());
        assert_eq!(
            app.log_selection.len(),
            1,
            "one row is selected: pinned={:?} logs={}",
            app.log_pinned_rects,
            app.view.logs.len()
        );
        let mut output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            vec![chord(Key::C, true, false)],
            Default::default(),
        );
        let copied = copied_texts(&output.platform_output);
        assert_eq!(copied.len(), 1, "one clipboard command: {copied:?}");
        let selected_line = app.view.logs[*app.log_selection.iter().next().unwrap()].render();
        assert_eq!(
            copied[0],
            format!("{selected_line}\r\n"),
            "the clipboard carries exactly the selected line"
        );

        // Copy all (Ctrl+Shift+A) carries every retained line, in painted order.
        output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            vec![chord(Key::A, true, false)],
            Default::default(),
        );
        let copied = copied_texts(&output.platform_output);
        assert_eq!(copied.len(), 1, "one clipboard command: {copied:?}");
        let expected: Vec<String> = app.view.logs.iter().map(|line| line.render()).collect();
        let mut all = expected.join("\r\n");
        all.push_str("\r\n");
        assert_eq!(copied[0], all, "copy all carries every retained line");

        // Save log (Ctrl+Alt+L) writes the same lines to the chosen file.
        let _ = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            vec![chord(Key::L, false, true)],
            Default::default(),
        );
        let written = std::fs::read_to_string(&target).expect("the log file was written");
        assert_eq!(
            written, all,
            "the exported log holds exactly the painted lines"
        );

        // Clear log (Ctrl+Shift+K) empties the view; the selection goes with it.
        let _ = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            vec![chord(Key::K, true, false)],
            Default::default(),
        );
        assert!(app.view.logs.is_empty(), "the log view is empty");
        assert!(app.log_selection.is_empty(), "the selection is gone");
        let _ = std::fs::remove_file(&target);
    }

    /// LOG-006: when the storage cap discarded earlier records the pane states that the history
    /// is truncated, keeps the current fatal error on screen, and the export names the missing
    /// prefix instead of presenting a partial log as the whole run.
    #[test]
    fn log_006_the_truncated_history_is_stated_and_the_export_names_it() {
        let mut app = fake_app_with_source("alpha = 1\n");
        let target = std::env::temp_dir().join("lovelace-round59-truncated.txt");
        let _ = std::fs::remove_file(&target);
        app.dialogs = Box::new(ScriptedDialogs::saving_log([Some(target.clone())]));
        app.workbench
            .flood_log_active(crate::workbench::LOG_CAP + 5_000);
        let ctx = egui::Context::default();
        let mut texts = Vec::new();
        let mut output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1280.0, 800.0),
            Vec::new(),
            Default::default(),
        );
        output.textures_delta.clear();
        for clipped in &output.shapes {
            collect_shape_text(&clipped.shape, &mut texts);
        }
        assert!(app.view.logs_dropped > 0, "the flood exceeded the cap");
        assert!(
            texts.iter().any(|text| text.contains("truncated history:")),
            "the pane states that its history is truncated: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("were discarded")),
            "the discarded count is stated: {texts:?}"
        );
        // The current fatal error is the newest record: the cap drops only the oldest, so the
        // run's fatal diagnostic is retained, painted in the tail the virtualised pane shows, and
        // exported verbatim.
        assert_eq!(
            app.view.logs.last().map(|line| line.kind),
            Some(crate::log_kind::LogKind::Diagnostic),
            "the newest record is the fatal diagnostic"
        );
        assert!(
            app.view
                .logs
                .last()
                .is_some_and(|line| line.text.contains("LOGCAP-900 fatal")),
            "the fatal error is retained verbatim"
        );
        assert!(
            texts.iter().any(|text| text.contains("LOGCAP-900 fatal"))
                || app
                    .log_row_rects
                    .iter()
                    .any(|(index, _)| *index + 1 == app.view.logs.len()),
            "the fatal diagnostic is painted (it is the newest row of the virtualised list)"
        );
        app.save_log();
        let written = std::fs::read_to_string(&target).expect("the truncated log was written");
        let mut lines = written.lines();
        assert!(
            lines
                .next()
                .unwrap_or_default()
                .starts_with("# truncated history:"),
            "the export names the missing prefix: {written:.200}"
        );
        assert!(
            written.contains("LOGCAP-900 fatal"),
            "the export retains the fatal error"
        );
        let _ = std::fs::remove_file(&target);
    }

    /// LOG-010: a 100 000-record log paints a bounded number of row widgets in one pass (the
    /// viewport, not the log) and the pass stays responsive.
    #[test]
    fn log_010_a_hundred_thousand_records_paint_a_bounded_number_of_rows() {
        let mut app = fake_app_with_source("alpha = 1\n");
        let boot = app.workbench.documents()[0].logs.len();
        app.workbench
            .flood_log_active(crate::workbench::LOG_CAP - boot - 1);
        let ctx = egui::Context::default();
        let started = std::time::Instant::now();
        let mut output = frame_capturing(
            &ctx,
            &mut app,
            vec2(1600.0, 1000.0),
            Vec::new(),
            Default::default(),
        );
        let elapsed = started.elapsed();
        output.textures_delta.clear();
        assert_eq!(app.view.logs.len(), crate::workbench::LOG_CAP);
        assert!(
            app.log_row_rects.len() < 200,
            "only the viewport rows are allocated as widgets: {} rows for {} records",
            app.log_row_rects.len(),
            app.view.logs.len()
        );
        assert!(
            app.log_row_rects
                .iter()
                .any(|(index, _)| *index + 1 == app.view.logs.len()),
            "the list is scrolled to the newest record, which is addressable: {:?}",
            app.log_row_rects
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>()
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the 100 000-record pass must not block: {elapsed:?}"
        );
        // The whole log is in the model, not in the widget tree: a second pass repaints the same
        // bounded window without rebuilding 100 000 rows.
        let started = std::time::Instant::now();
        let _ = frame_capturing(
            &ctx,
            &mut app,
            vec2(1600.0, 1000.0),
            Vec::new(),
            Default::default(),
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a live update repaint stays bounded"
        );
        assert_eq!(app.view.logs.len(), crate::workbench::LOG_CAP);
    }

    /// LOG-010: the live window follows the tail by default and keeps following only while the
    /// view is at the bottom. A wheel scroll back ends follow mode; the records that arrive
    /// afterwards do not move the view, they raise the footer's count, and the footer's
    /// *Jump to newest* control returns to the tail.
    ///
    /// Every event below is a real egui input: a pointer over the list and a wheel delta, then a
    /// pointer press/release on the control the pane painted. No field is set to fake the
    /// scroll-away; `log_follow` is read to report, never assigned by the test.
    #[test]
    fn log_010_the_live_list_follows_the_tail_only_at_the_bottom_and_offers_a_jump_back() {
        let mut app = fake_app_with_source("alpha = 1\n");
        let boot = app.workbench.documents()[0].logs.len();
        app.workbench
            .flood_log_active(crate::workbench::LOG_CAP - boot - 1);
        let ctx = egui::Context::default();
        let screen = vec2(1600.0, 1000.0);

        // A launch paints the newest records, without any scroll request from the user.
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert!(app.log_follow, "a fresh log view follows the newest record");
        assert!(
            app.log_row_rects
                .iter()
                .any(|(index, _)| *index + 1 == app.view.logs.len()),
            "the newest record is painted: rows {:?}",
            app.log_row_rects
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>()
        );
        assert_eq!(app.log_unseen, 0);
        assert_eq!(app.log_jump_rect, None, "no jump control while following");

        // Live lines keep the view at the tail while it is already there.
        app.workbench.flood_log_active(25);
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert!(app.log_follow, "appending at the bottom keeps following");
        assert_eq!(app.log_unseen, 0, "nothing is unseen while following");
        assert!(
            app.log_row_rects
                .iter()
                .any(|(index, _)| *index + 1 == app.view.logs.len()),
            "the newly appended record is painted"
        );
        let rows_before_scroll: Vec<usize> =
            app.log_row_rects.iter().map(|(index, _)| *index).collect();

        // A real wheel scroll over the list moves the view away from the bottom. egui applies the
        // delta at the end of the pass, so the next pass paints the scrolled window. The pointer
        // must be over the rows this pass paints: a previous pass's rectangle can name a row that
        // has since moved out of the list, and then the wheel is not over the scroll area at all.
        let over_rows = app.log_row_rects[app.log_row_rects.len() / 2].1.center();
        let _ = frame_capturing(
            &ctx,
            &mut app,
            screen,
            vec![egui::Event::PointerMoved(over_rows)],
            Default::default(),
        );
        let over_rows = app.log_row_rects[app.log_row_rects.len() / 2].1.center();
        let _ = frame_capturing(
            &ctx,
            &mut app,
            screen,
            vec![
                egui::Event::PointerMoved(over_rows),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 420.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            Default::default(),
        );
        assert!(
            !app.log_follow,
            "scrolling away from the bottom ends follow mode; painted before: {rows_before_scroll:?}"
        );
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        let scrolled_rows: Vec<usize> = app.log_row_rects.iter().map(|(index, _)| *index).collect();
        assert!(
            scrolled_rows
                .iter()
                .all(|index| index + 1 < app.view.logs.len()),
            "the scrolled window is above the tail: {scrolled_rows:?}"
        );
        assert!(
            app.log_jump_rect.is_some(),
            "a scrolled-away view paints the jump-to-newest control"
        );

        // Records arriving while the user is reading history must not move the viewport.
        app.workbench.flood_log_active(40);
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert!(!app.log_follow, "new records never re-arm follow mode");
        assert_eq!(
            app.log_unseen, 41,
            "the footer counts every record that arrived while scrolled back"
        );
        let after_rows: Vec<usize> = app.log_row_rects.iter().map(|(index, _)| *index).collect();
        assert!(
            after_rows
                .iter()
                .all(|index| index + 1 < app.view.logs.len()),
            "the viewport stayed where the user left it: {after_rows:?}"
        );
        let jump = app
            .log_jump_rect
            .expect("the jump control is still painted after the append");
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts.iter().any(|text| text == "scrolled back · 41 new"),
            "the header states how many records arrived: {texts:?}"
        );

        // The control the pane painted is what a user presses.
        pointer_click_sized(&ctx, &mut app, screen, jump.center());
        // The request is carried out by the next painted pass (that is what the repaint request
        // is for); this is the pass the user sees after the click.
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert!(
            app.log_follow,
            "Jump to newest returns the view to the tail and re-arms follow mode"
        );
        assert_eq!(app.log_unseen, 0, "the unseen count is cleared by the jump");
        assert_eq!(app.log_jump_rect, None, "the control is gone at the tail");
        assert!(
            app.log_row_rects
                .iter()
                .any(|(index, _)| *index + 1 == app.view.logs.len()),
            "the newest record is painted after the jump: rows {:?}",
            app.log_row_rects
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>()
        );
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts.iter().any(|text| text == "at newest record"),
            "the pane says the view follows the newest record again: {texts:?}"
        );
    }

    /// PERF-011: the window shows the measured estimate before an extreme precision change, and
    /// the change happens only when the dialog's *Set precision* control is pressed. Cancel keeps
    /// the document's precision; the log names the estimate on the confirm path.
    #[test]
    fn perf_011_the_window_shows_the_estimate_and_applies_only_after_confirmation() {
        let mut app = fake_app_with_source("alpha = 1\n");
        app.workbench.set_precision_curve(
            crate::workbench::PrecisionCostCurve::from_samples(vec![
                (100, 0.001),
                (1_000, 0.010),
                (10_000, 0.100),
            ])
            .expect("the injected curve fits"),
        );
        let ctx = egui::Context::default();
        let screen = vec2(1600.0, 1000.0);
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        let apply = app
            .precision_apply_rect
            .expect("the toolbar paints its Apply control");
        assert_eq!(app.view.precision, 30, "the document starts at the default");

        // The typed value is above the threshold: the toolbar itself states the estimate before
        // any click, with no dialog.
        app.precision_input = "20000".to_string();
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts
                .iter()
                .any(|text| text == "estimate ≈ 200 ms and ≈ 312.8 KB"),
            "the live estimate is painted beside the field: {texts:?}"
        );

        // Pressing Apply opens the estimate and changes nothing.
        pointer_click_sized(&ctx, &mut app, screen, apply.center());
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert_eq!(
            app.view.precision, 30,
            "an extreme precision must not be applied before it is confirmed"
        );
        let cost = app
            .workbench
            .pending_precision()
            .cloned()
            .expect("the window holds the pending estimate");
        assert_eq!(cost.digits, 20_000);
        assert!((cost.seconds - 0.2).abs() < 0.02, "{cost:?}");
        let keys: Vec<&str> = app
            .precision_confirm_rects
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert!(
            keys.contains(&"Set precision") && keys.contains(&"Cancel"),
            "the dialog paints both decisions: {keys:?}"
        );
        let texts = painted_texts(&ctx, &mut app, screen);
        assert!(
            texts
                .iter()
                .any(|text| text == "Estimated ≈ 200 ms and ≈ 312.8 KB for one sqrt(2) evaluation."),
            "the dialog states the estimate in the tool's words: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("measured sqrt(2): 100 digits")),
            "the dialog names the measurements the estimate came from: {texts:?}"
        );

        // Cancel leaves the document exactly as it was and closes the dialog.
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        let cancel = app
            .precision_confirm_rects
            .iter()
            .find(|(key, _)| key == "Cancel")
            .map(|(_, rect)| *rect)
            .expect("the dialog paints Cancel");
        pointer_click_sized(&ctx, &mut app, screen, cancel.center());
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert_eq!(app.view.precision, 30, "Cancel keeps the precision");
        assert!(app.workbench.pending_precision().is_none());
        assert!(app.precision_confirm_rects.is_empty(), "the dialog is gone");
        assert!(
            app.view
                .logs
                .iter()
                .any(|line| line.text.contains("cancelled; the precision stays 30")),
            "the log says the change was cancelled"
        );

        // Confirm applies the value and logs the estimate it was confirmed against.
        app.precision_input = "20000".to_string();
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        pointer_click_sized(&ctx, &mut app, screen, apply.center());
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        let set = app
            .precision_confirm_rects
            .iter()
            .find(|(key, _)| key == "Set precision")
            .map(|(_, rect)| *rect)
            .expect("the dialog paints Set precision");
        pointer_click_sized(&ctx, &mut app, screen, set.center());
        let _ = frame_capturing(&ctx, &mut app, screen, Vec::new(), Default::default());
        assert_eq!(
            app.view.precision, 20_000,
            "Set precision applies the value"
        );
        assert!(app.workbench.pending_precision().is_none());
        assert!(
            app.view.logs.iter().any(|line| line
                .text
                .contains("Precision 20000 digits confirmed (≈ 200 ms")),
            "the log carries the estimate with the confirmed change"
        );
    }

    /// VIS-001 review: the variables table's Kind column must show the engine's kinds at the
    /// shipped default workspace width, and the three columns plus the row actions must never
    /// overflow at the 900x600 minimum window.
    #[test]
    fn vis_001_the_variable_columns_fit_the_engine_kinds_and_never_overflow_the_pane() {
        // 294 px is the Workspace body width observed in the 1600x1000 final captures.
        let default_available = 294.0 - ROW_ACTION_WIDTH * 2.0;
        let [name, kind, display] = variable_column_widths(default_available);
        let usable = (default_available - 16.0).max(96.0);
        assert!(
            name + kind + display <= usable + 0.5,
            "the default columns overflow their usable width: {name} + {kind} + {display} > {usable}"
        );
        // 13 pt monospace at the shipped font: "Symbolic" (8 chars) advances about 62.4 points;
        // 70 leaves room for the cell's padding and the truncate guard, so no kind is cut.
        assert!(
            kind >= 70.0,
            "the Kind column ({kind} pt) cannot fit 'Symbolic'; the table would paint Sym…"
        );

        // 900x600 at 100%: the Workspace column is 180 px, its body loses 16 px of padding.
        let minimum_available = 164.0 - ROW_ACTION_WIDTH * 2.0;
        let [n, k, d] = variable_column_widths(minimum_available);
        let minimum_usable = (minimum_available - 16.0).max(96.0);
        assert!(
            n + k + d <= minimum_usable + 0.5,
            "the columns overflow at the 900x600 minimum: {n} + {k} + {d} > {minimum_usable}"
        );
    }
}
