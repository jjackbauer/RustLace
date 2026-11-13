//! The persistent settings document.
//!
//! Persistence is deliberately boring: one human-readable JSON object, a schema number, and a
//! loader that treats every problem as "use the defaults and record a warning". A corrupt file,
//! a file written by a newer build, or a field of the wrong type must never be able to stop the
//! application from starting, so from_json has no error return at all.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use lovelace_json::Json;

use crate::engine::DEFAULT_PRECISION;

/// The schema version this build writes and understands.
pub const SCHEMA_VERSION: u32 = 1;

/// Smallest window width the settings loader will accept, in pixels.
pub const MIN_WINDOW_WIDTH: i32 = 900;
/// Smallest window height the settings loader will accept, in pixels.
pub const MIN_WINDOW_HEIGHT: i32 = 600;
/// Largest window dimension the settings loader will accept, in pixels.
pub const MAX_WINDOW_DIMENSION: i32 = 10_000;
/// Largest number of recent files kept.
pub const MAX_RECENT_FILES: usize = 20;
/// Smallest accepted display precision, in decimal places.
pub const MIN_PRECISION: usize = 1;
/// Largest accepted display precision, in decimal places.
pub const MAX_PRECISION: usize = 100_000;
/// Smallest accepted editor indent width, in columns.
pub const MIN_INDENT_WIDTH: usize = 1;
/// Largest accepted editor indent width, in columns.
pub const MAX_INDENT_WIDTH: usize = 8;

/// A11Y-004: the smallest interface scale the window offers, in percent of the OS DPI scale.
pub const MIN_UI_SCALE_PERCENT: u32 = 100;
/// A11Y-004: the largest interface scale the window offers, in percent of the OS DPI scale.
pub const MAX_UI_SCALE_PERCENT: u32 = 200;

/// A11Y-005: the smallest editor font the typography setting offers, in points.
pub const MIN_FONT_SIZE_PT: u32 = 12;
/// A11Y-005: the largest editor font the typography setting offers, in points.
pub const MAX_FONT_SIZE_PT: u32 = 24;
/// A11Y-005: the tightest line height offered, in percent of the font size.
pub const MIN_LINE_HEIGHT_PERCENT: u32 = 100;
/// A11Y-005: the loosest line height offered, in percent of the font size.
pub const MAX_LINE_HEIGHT_PERCENT: u32 = 200;

/// VIS-002: the persisted colour theme.
///
/// This lives in the settings module rather than in the Windows-only `ui` module so the settings
/// document stays cross-platform and every build can round-trip it. The UI maps it to its token
/// set; the strings are the persisted values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePreference {
    /// The shipped default: the dark token set.
    #[default]
    Dark,
    /// The light token set.
    Light,
}

impl ThemePreference {
    /// The string written to the settings document.
    pub fn as_str(self) -> &'static str {
        match self {
            ThemePreference::Dark => "dark",
            ThemePreference::Light => "light",
        }
    }

    /// Parses a persisted theme; unknown text is rejected by the caller, which warns and keeps
    /// the default.
    pub fn parse(text: &str) -> Option<ThemePreference> {
        match text {
            "dark" => Some(ThemePreference::Dark),
            "light" => Some(ThemePreference::Light),
            _ => None,
        }
    }

    /// The other theme, for the runtime toggle.
    pub fn toggled(self) -> ThemePreference {
        match self {
            ThemePreference::Dark => ThemePreference::Light,
            ThemePreference::Light => ThemePreference::Dark,
        }
    }
}

/// Saved window geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowGeometry {
    /// Outer window width, in pixels.
    pub width: i32,
    /// Outer window height, in pixels.
    pub height: i32,
}

impl Default for WindowGeometry {
    fn default() -> WindowGeometry {
        WindowGeometry {
            width: 1280,
            height: 800,
        }
    }
}

/// The layout fractions that the shell turns into pane sizes on every resize.
///
/// They are stored in permille (thousandths) rather than as floats so that the saved document,
/// the layout arithmetic and the tests are all exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutFractions {
    /// Editor column width, as a fraction of the client width.
    pub editor_width_permille: u32,
    /// Logs strip height, as a fraction of the content height.
    pub logs_height_permille: u32,
    /// Plot pane height, as a fraction of the editor column height.
    pub plot_height_permille: u32,
}

impl Default for LayoutFractions {
    fn default() -> LayoutFractions {
        LayoutFractions {
            editor_width_permille: 720,
            logs_height_permille: 220,
            plot_height_permille: 460,
        }
    }
}

/// LAY-004: which ancillary pane the compact layout's drawer shows.
///
/// The compact layout (LAY-011) is the window's one pane-visibility mechanism: the drawer is
/// either collapsed to the tab row or shows exactly one ancillary pane. Persisting which state
/// the user left means a relaunch restores the same visible pane set, not only the same sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompactPaneSetting {
    /// The drawer is collapsed; only the editor and the tab row are visible.
    #[default]
    Hidden,
    /// The drawer shows the engine graph.
    Graph,
    /// The drawer shows the Workspace pane.
    Workspace,
    /// The drawer shows the Logs pane.
    Logs,
    /// The drawer shows the Symbolic Inspector pane.
    Inspector,
}

impl CompactPaneSetting {
    /// The string written to the settings document.
    pub fn as_str(self) -> &'static str {
        match self {
            CompactPaneSetting::Hidden => "hidden",
            CompactPaneSetting::Graph => "graph",
            CompactPaneSetting::Workspace => "workspace",
            CompactPaneSetting::Logs => "logs",
            CompactPaneSetting::Inspector => "inspector",
        }
    }

    /// Parses a persisted choice; `None` means the text is not a pane the build knows, and the
    /// caller keeps the default and records a warning.
    pub fn parse(text: &str) -> Option<CompactPaneSetting> {
        match text {
            "hidden" => Some(CompactPaneSetting::Hidden),
            "graph" => Some(CompactPaneSetting::Graph),
            "workspace" => Some(CompactPaneSetting::Workspace),
            "logs" => Some(CompactPaneSetting::Logs),
            "inspector" => Some(CompactPaneSetting::Inspector),
            _ => None,
        }
    }
}

/// LAY-004: the workbench's pane sizes and pane visibility, persisted across relaunch.
///
/// The fractions are permille of the client (width fractions for the editor, Workspace and
/// Symbolic Inspector columns; height fractions for the Logs strip and the compact drawer's
/// editor). The default values are exactly the ones the window starts with, so a double-click on
/// a splitter or the reset command writes the same numbers a fresh launch uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkbenchLayout {
    /// Editor column width, permille of the client width.
    pub editor_permille: u32,
    /// Workspace column width, permille of the client width.
    pub workspace_permille: u32,
    /// Bottom strip height, permille of the client height.
    pub logs_permille: u32,
    /// Symbolic Inspector width, permille of the client width.
    pub inspector_permille: u32,
    /// LAY-011: the editor's share of the compact layout while the drawer is open.
    pub compact_editor_permille: u32,
    /// LAY-004: the pane the compact drawer shows, or `Hidden`.
    pub compact_pane: CompactPaneSetting,
}

impl Default for WorkbenchLayout {
    fn default() -> WorkbenchLayout {
        WorkbenchLayout {
            editor_permille: 480,
            workspace_permille: 200,
            logs_permille: 220,
            inspector_permille: 300,
            compact_editor_permille: 550,
            compact_pane: CompactPaneSetting::Hidden,
        }
    }
}

/// One tab of the last session: the file it showed, or just its identity for an unsaved buffer.
///
/// DOC-004: a tab with a path is restored by reading that file again; a tab without one can only
/// be restored when DOC-005's recovery journal still holds its buffer, which is why the identity
/// is persisted beside the path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTab {
    /// The file the tab showed, when it had one.
    pub path: Option<String>,
    /// The document identity the tab had, so a recovery record can be matched to it.
    pub document_id: u64,
}

/// The tabs that were open at the last persist, in tab order, and which one was active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionState {
    /// The open tabs, left to right.
    pub documents: Vec<SessionTab>,
    /// The index of the tab that was selected.
    pub active: usize,
}

impl SessionState {
    /// Builds a session value from `(path, document identity)` pairs and the active index.
    pub fn new(documents: Vec<SessionTab>, active: usize) -> SessionState {
        let active = if documents.is_empty() {
            0
        } else {
            active.min(documents.len() - 1)
        };
        SessionState { documents, active }
    }
}

/// A11Y-005: the monospace family the editor paints with.
///
/// The variant list is fixed (rather than a free-form family name) because every value must name
/// a font file the window can load on Windows; [`crate::ui::typography`] resolves each one to a
/// file under the Windows font directory and falls back to the shipped Hack face when the file is
/// not installed. `Cascadia` is the one family that ships a ligature face (Cascadia Code) and a
/// plain one (Cascadia Mono), which is what makes the optional ligature setting observable: the
/// same family has both a ligature and a non-ligature rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorFont {
    /// The font shipped with egui; no ligature face.
    #[default]
    Hack,
    /// Cascadia Code (ligatures) / Cascadia Mono (no ligatures), when installed.
    Cascadia,
    /// Consolas, when installed.
    Consolas,
    /// Courier New, when installed.
    CourierNew,
    /// Lucida Console, when installed.
    LucidaConsole,
}

impl EditorFont {
    /// Every family the settings window offers, in menu order.
    pub const ALL: [EditorFont; 5] = [
        EditorFont::Hack,
        EditorFont::Cascadia,
        EditorFont::Consolas,
        EditorFont::CourierNew,
        EditorFont::LucidaConsole,
    ];

    /// The string written to the settings document.
    pub fn as_str(self) -> &'static str {
        match self {
            EditorFont::Hack => "hack",
            EditorFont::Cascadia => "cascadia",
            EditorFont::Consolas => "consolas",
            EditorFont::CourierNew => "courier-new",
            EditorFont::LucidaConsole => "lucida-console",
        }
    }

    /// Parses a persisted family; unknown text is rejected by the caller, which warns and keeps
    /// the default.
    pub fn parse(text: &str) -> Option<EditorFont> {
        match text {
            "hack" => Some(EditorFont::Hack),
            "cascadia" => Some(EditorFont::Cascadia),
            "consolas" => Some(EditorFont::Consolas),
            "courier-new" => Some(EditorFont::CourierNew),
            "lucida-console" => Some(EditorFont::LucidaConsole),
            _ => None,
        }
    }

    /// The name the settings window and the pane header show.
    pub fn label(self) -> &'static str {
        match self {
            EditorFont::Hack => "Hack",
            EditorFont::Cascadia => "Cascadia",
            EditorFont::Consolas => "Consolas",
            EditorFont::CourierNew => "Courier New",
            EditorFont::LucidaConsole => "Lucida Console",
        }
    }
}

/// A11Y-005: the editor typography the settings document configures.
///
/// Sizes are stored as integers so the whole settings document remains `Eq` and its round-trip is
/// exact. `line_height_percent` is a percentage of the font size, which keeps the row geometry and
/// every decoration (gutter number, diagnostic underline, bracket cell, caret) derived from the
/// same two numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypographySettings {
    /// The monospace family.
    pub font: EditorFont,
    /// Font size in points, 12-24.
    pub font_size_pt: u32,
    /// Line height as a percentage of the font size, 100-200.
    pub line_height_percent: u32,
    /// True: use the family's ligature face (only Cascadia has one in this build).
    pub ligatures: bool,
}

impl Default for TypographySettings {
    fn default() -> TypographySettings {
        TypographySettings {
            font: EditorFont::Hack,
            font_size_pt: 13,
            line_height_percent: 125,
            ligatures: false,
        }
    }
}

impl TypographySettings {
    /// The font size as points.
    pub fn font_size(&self) -> f32 {
        self.font_size_pt.clamp(MIN_FONT_SIZE_PT, MAX_FONT_SIZE_PT) as f32
    }

    /// The line height as a multiple of the font size.
    pub fn line_height(&self) -> f32 {
        self.line_height_percent.clamp(MIN_LINE_HEIGHT_PERCENT, MAX_LINE_HEIGHT_PERCENT) as f32
            / 100.0
    }

    /// The row height in points: the one number the editor, the gutter, the diagnostic underline,
    /// the bracket cell and the caret all derive their vertical geometry from.
    pub fn row_height(&self) -> f32 {
        (self.font_size() * self.line_height()).max(self.font_size() + 2.0)
    }
}

/// A11Y-005: the editor behaviour and typography the settings document configures.
///
/// `indent_width` is both the number of columns one indentation level occupies and the width a
/// literal tab is painted at. `use_spaces` decides whether Tab and indentation write spaces or a
/// literal tab. `auto_close` enables bracket and quote auto-closing with skip-over.
/// `typography` is the A11Y-005 font, size, line height and ligature choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSettings {
    /// Columns per indentation level and the displayed width of a tab.
    pub indent_width: usize,
    /// True: indentation is spaces; false: indentation is a literal tab.
    pub use_spaces: bool,
    /// True: typing `(`, `[`, `{`, `"` or `'` also inserts its closing character.
    pub auto_close: bool,
    /// A11Y-005: the font family, size, line height and ligature choice.
    pub typography: TypographySettings,
}

impl Default for EditorSettings {
    fn default() -> EditorSettings {
        EditorSettings {
            indent_width: 4,
            use_spaces: true,
            auto_close: true,
            typography: TypographySettings::default(),
        }
    }
}

impl EditorSettings {
    /// The one indentation unit inserted by Tab and by smart Enter.
    pub fn indent_unit(&self) -> String {
        if self.use_spaces {
            " ".repeat(self.indent_width)
        } else {
            "\t".to_string()
        }
    }
}

/// Everything the application persists between runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Schema number of the file this value was loaded from (or wrote).
    pub schema_version: u32,
    /// Saved window geometry.
    pub window: WindowGeometry,
    /// Saved layout fractions.
    pub layout: LayoutFractions,
    /// LAY-004: the workbench's pane sizes and the compact drawer's visible pane.
    pub workbench_layout: WorkbenchLayout,
    /// EDT-008: indentation and auto-close behaviour.
    pub editor: EditorSettings,
    /// VIS-002: the colour theme the window paints; dark is the shipped default.
    pub theme: ThemePreference,
    /// A11Y-006: when true the window follows the OS application light/dark theme, and `theme`
    /// is only the value used until the first OS notification arrives. When false, `theme` is an
    /// explicit user override and an OS change is ignored.
    pub theme_follows_system: bool,
    /// A11Y-004: interface scale in percent, applied on top of the OS DPI scale (100-200).
    pub ui_scale_percent: u32,
    /// A11Y-014: render mathematical values in Unicode notation (`√(2)×π`) instead of the
    /// engine's ASCII notation (`sqrt(2)*pi`). The default is ASCII, the engine's own default.
    pub unicode_glyphs: bool,
    /// Most-recently used file paths, newest first.
    pub recent_files: Vec<String>,
    /// Display precision of the document that was active at exit.
    pub active_precision: usize,
    /// DOC-004: the tabs that were open at the last persist, or `None` when nothing was saved.
    pub session: Option<SessionState>,
    /// Problems found while loading. Never serialised; the shell copies them into the log.
    pub warnings: Vec<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            schema_version: SCHEMA_VERSION,
            window: WindowGeometry::default(),
            layout: LayoutFractions::default(),
            workbench_layout: WorkbenchLayout::default(),
            editor: EditorSettings::default(),
            theme: ThemePreference::default(),
            theme_follows_system: false,
            ui_scale_percent: MIN_UI_SCALE_PERCENT,
            unicode_glyphs: false,
            recent_files: Vec::new(),
            active_precision: DEFAULT_PRECISION,
            session: None,
            warnings: Vec::new(),
        }
    }
}

impl Settings {
    /// A settings value with every field at its default.
    pub fn new() -> Settings {
        Settings::default()
    }

    /// Serialises the settings as pretty-printed JSON. Warnings are not persisted: they
    /// describe this load, not the document.
    pub fn to_json(&self) -> String {
        Json::obj(vec![
            ("schema_version", Json::Num(SCHEMA_VERSION.to_string())),
            (
                "window",
                Json::obj(vec![
                    ("width", Json::Num(self.window.width.to_string())),
                    ("height", Json::Num(self.window.height.to_string())),
                ]),
            ),
            (
                "layout",
                Json::obj(vec![
                    (
                        "editor_width_permille",
                        Json::Num(self.layout.editor_width_permille.to_string()),
                    ),
                    (
                        "logs_height_permille",
                        Json::Num(self.layout.logs_height_permille.to_string()),
                    ),
                    (
                        "plot_height_permille",
                        Json::Num(self.layout.plot_height_permille.to_string()),
                    ),
                ]),
            ),
            (
                "workbench_layout",
                Json::obj(vec![
                    (
                        "editor_permille",
                        Json::Num(self.workbench_layout.editor_permille.to_string()),
                    ),
                    (
                        "workspace_permille",
                        Json::Num(self.workbench_layout.workspace_permille.to_string()),
                    ),
                    (
                        "logs_permille",
                        Json::Num(self.workbench_layout.logs_permille.to_string()),
                    ),
                    (
                        "inspector_permille",
                        Json::Num(self.workbench_layout.inspector_permille.to_string()),
                    ),
                    (
                        "compact_editor_permille",
                        Json::Num(self.workbench_layout.compact_editor_permille.to_string()),
                    ),
                    (
                        "compact_pane",
                        Json::Str(self.workbench_layout.compact_pane.as_str().to_string()),
                    ),
                ]),
            ),
            (
                "recent_files",
                Json::str_array(self.recent_files.iter().cloned()),
            ),
            (
                "editor",
                Json::obj(vec![
                    (
                        "indent_width",
                        Json::Num(self.editor.indent_width.to_string()),
                    ),
                    ("use_spaces", Json::Bool(self.editor.use_spaces)),
                    ("auto_close", Json::Bool(self.editor.auto_close)),
                    // A11Y-005: the editor typography survives a restart.
                    (
                        "font",
                        Json::Str(self.editor.typography.font.as_str().to_string()),
                    ),
                    (
                        "font_size_pt",
                        Json::Num(self.editor.typography.font_size_pt.to_string()),
                    ),
                    (
                        "line_height_percent",
                        Json::Num(self.editor.typography.line_height_percent.to_string()),
                    ),
                    (
                        "ligatures",
                        Json::Bool(self.editor.typography.ligatures),
                    ),
                ]),
            ),
            (
                "active_precision",
                Json::Num(self.active_precision.to_string()),
            ),
            ("theme", Json::Str(self.theme.as_str().to_string())),
            // A11Y-006: the follow-the-OS choice and an explicit override are different states.
            (
                "theme_follows_system",
                Json::Bool(self.theme_follows_system),
            ),
            // A11Y-004: the interface scale survives a restart.
            (
                "ui_scale_percent",
                Json::Num(self.ui_scale_percent.to_string()),
            ),
            // A11Y-014: the ASCII/Unicode mathematical-notation choice survives a restart.
            ("unicode_glyphs", Json::Bool(self.unicode_glyphs)),
            (
                "session",
                match &self.session {
                    Some(session) => Json::obj(vec![
                        ("active", Json::Num(session.active.to_string())),
                        (
                            "documents",
                            Json::Arr(
                                session
                                    .documents
                                    .iter()
                                    .map(|tab| {
                                        let mut fields = vec![(
                                            "document_id",
                                            Json::Num(tab.document_id.to_string()),
                                        )];
                                        if let Some(path) = &tab.path {
                                            fields.push(("path", Json::Str(path.clone())));
                                        }
                                        Json::obj(fields)
                                    })
                                    .collect(),
                            ),
                        ),
                    ]),
                    None => Json::Null,
                },
            ),
        ])
        .to_pretty()
    }

    /// Parses a settings document. Any parse failure, wrong schema, or unusable field falls
    /// back to a default and appends a warning; this function never panics and never fails.
    pub fn from_json(text: &str) -> Settings {
        let mut settings = Settings::new();
        let root = match Json::parse(text) {
            Ok(Json::Obj(map)) => Json::Obj(map),
            Ok(_) => {
                settings
                    .warnings
                    .push("settings root is not a JSON object; defaults used.".to_string());
                return settings;
            }
            Err(error) => {
                settings.warnings.push(format!(
                    "settings are not valid JSON ({error}); defaults used."
                ));
                return settings;
            }
        };

        let version = match root.get("schema_version") {
            Some(Json::Num(n)) => n.parse::<u32>().ok(),
            _ => None,
        };
        let version = match version {
            Some(v) => v,
            None => {
                settings.warnings.push(
                    "settings schema_version is missing or not a number; defaults used."
                        .to_string(),
                );
                return settings;
            }
        };
        if version > SCHEMA_VERSION {
            settings.warnings.push(format!(
                "settings schema_version {version} is newer than supported {SCHEMA_VERSION}; defaults used."
            ));
            return settings;
        }
        if version < SCHEMA_VERSION {
            settings.warnings.push(format!(
                "settings schema_version {version} is older than supported {SCHEMA_VERSION}; missing fields use defaults."
            ));
        }

        let mut warnings: Vec<String> = Vec::new();
        if let Some(window) = root.get("window") {
            if let Json::Obj(_) = window {
                settings.window.width = read_i64(
                    window,
                    "width",
                    settings.window.width as i64,
                    MIN_WINDOW_WIDTH as i64,
                    MAX_WINDOW_DIMENSION as i64,
                    &mut warnings,
                ) as i32;
                settings.window.height = read_i64(
                    window,
                    "height",
                    settings.window.height as i64,
                    MIN_WINDOW_HEIGHT as i64,
                    MAX_WINDOW_DIMENSION as i64,
                    &mut warnings,
                ) as i32;
            } else {
                warnings.push("window is not an object; default geometry used.".to_string());
            }
        }
        if let Some(layout) = root.get("layout") {
            if let Json::Obj(_) = layout {
                settings.layout.editor_width_permille = read_i64(
                    layout,
                    "editor_width_permille",
                    settings.layout.editor_width_permille as i64,
                    300,
                    900,
                    &mut warnings,
                ) as u32;
                settings.layout.logs_height_permille = read_i64(
                    layout,
                    "logs_height_permille",
                    settings.layout.logs_height_permille as i64,
                    100,
                    600,
                    &mut warnings,
                ) as u32;
                settings.layout.plot_height_permille = read_i64(
                    layout,
                    "plot_height_permille",
                    settings.layout.plot_height_permille as i64,
                    150,
                    850,
                    &mut warnings,
                ) as u32;
            } else {
                warnings.push("layout is not an object; default fractions used.".to_string());
            }
        }
        // LAY-004: absent workbench_layout is a settings file from before the workbench persisted
        // its pane sizes; the defaults apply with no warning, the same rule the editor typography
        // fields follow.
        if let Some(workbench) = root.get("workbench_layout") {
            if let Json::Obj(_) = workbench {
                settings.workbench_layout.editor_permille = read_i64(
                    workbench,
                    "editor_permille",
                    settings.workbench_layout.editor_permille as i64,
                    300,
                    800,
                    &mut warnings,
                ) as u32;
                settings.workbench_layout.workspace_permille = read_i64(
                    workbench,
                    "workspace_permille",
                    settings.workbench_layout.workspace_permille as i64,
                    100,
                    450,
                    &mut warnings,
                ) as u32;
                settings.workbench_layout.logs_permille = read_i64(
                    workbench,
                    "logs_permille",
                    settings.workbench_layout.logs_permille as i64,
                    100,
                    450,
                    &mut warnings,
                ) as u32;
                settings.workbench_layout.inspector_permille = read_i64(
                    workbench,
                    "inspector_permille",
                    settings.workbench_layout.inspector_permille as i64,
                    150,
                    600,
                    &mut warnings,
                ) as u32;
                settings.workbench_layout.compact_editor_permille = read_i64(
                    workbench,
                    "compact_editor_permille",
                    settings.workbench_layout.compact_editor_permille as i64,
                    480,
                    820,
                    &mut warnings,
                ) as u32;
                if let Some(pane) = workbench.get("compact_pane") {
                    match pane.as_str().and_then(CompactPaneSetting::parse) {
                        Some(parsed) => settings.workbench_layout.compact_pane = parsed,
                        None => warnings.push(
                            "workbench_layout.compact_pane is not a known pane; the drawer starts \
                             collapsed."
                                .to_string(),
                        ),
                    }
                }
            } else {
                warnings.push(
                    "workbench_layout is not an object; default pane sizes used.".to_string(),
                );
            }
        }
        if let Some(editor) = root.get("editor") {
            if let Json::Obj(_) = editor {
                settings.editor.indent_width = read_i64(
                    editor,
                    "indent_width",
                    settings.editor.indent_width as i64,
                    MIN_INDENT_WIDTH as i64,
                    MAX_INDENT_WIDTH as i64,
                    &mut warnings,
                ) as usize;
                settings.editor.use_spaces = read_bool(
                    editor,
                    "use_spaces",
                    settings.editor.use_spaces,
                    &mut warnings,
                );
                settings.editor.auto_close = read_bool(
                    editor,
                    "auto_close",
                    settings.editor.auto_close,
                    &mut warnings,
                );
                // A11Y-005: an absent typography field is the shipped default, not a warning:
                // settings files written before the setting existed are still valid.
                if let Some(font) = editor.get("font") {
                    match font.as_str().and_then(EditorFont::parse) {
                        Some(parsed) => settings.editor.typography.font = parsed,
                        None => warnings.push(
                            "editor.font is not a known monospace family; Hack is used."
                                .to_string(),
                        ),
                    }
                }
                if editor.get("font_size_pt").is_some() {
                    settings.editor.typography.font_size_pt = read_i64(
                        editor,
                        "font_size_pt",
                        settings.editor.typography.font_size_pt as i64,
                        MIN_FONT_SIZE_PT as i64,
                        MAX_FONT_SIZE_PT as i64,
                        &mut warnings,
                    ) as u32;
                }
                if editor.get("line_height_percent").is_some() {
                    settings.editor.typography.line_height_percent = read_i64(
                        editor,
                        "line_height_percent",
                        settings.editor.typography.line_height_percent as i64,
                        MIN_LINE_HEIGHT_PERCENT as i64,
                        MAX_LINE_HEIGHT_PERCENT as i64,
                        &mut warnings,
                    ) as u32;
                }
                if editor.get("ligatures").is_some() {
                    settings.editor.typography.ligatures = read_bool(
                        editor,
                        "ligatures",
                        settings.editor.typography.ligatures,
                        &mut warnings,
                    );
                }
            } else {
                warnings.push("editor is not an object; default editor settings used.".to_string());
            }
        }
        settings.active_precision = read_i64(
            &root,
            "active_precision",
            settings.active_precision as i64,
            MIN_PRECISION as i64,
            MAX_PRECISION as i64,
            &mut warnings,
        ) as usize;
        // VIS-002: an absent theme is the dark default (older settings files are not warnings);
        // a present but unknown value is a real problem the user should see.
        if let Some(theme) = root.get("theme") {
            match theme.as_str().and_then(ThemePreference::parse) {
                Some(preference) => settings.theme = preference,
                None => warnings.push(
                    "theme is not \"dark\" or \"light\"; the dark theme is used.".to_string(),
                ),
            }
        }
        // A11Y-014: an absent `unicode_glyphs` is the ASCII default, not a problem: settings
        // files written before the setting existed are still valid.
        if let Some(unicode) = root.get("unicode_glyphs") {
            match unicode {
                Json::Bool(enabled) => settings.unicode_glyphs = *enabled,
                _ => warnings.push(
                    "unicode_glyphs is not a boolean; ASCII mathematical notation is used."
                        .to_string(),
                ),
            }
        }
        // A11Y-006: an absent field keeps the explicit-override behaviour, so an older file
        // (and the shipped default) does not start following the OS unexpectedly.
        if let Some(value) = root.get("theme_follows_system") {
            match value {
                Json::Bool(follows) => settings.theme_follows_system = *follows,
                _ => warnings.push(
                    "theme_follows_system is not a boolean; the explicit theme override is used."
                        .to_string(),
                ),
            }
        }
        // A11Y-004: an absent field is the 100% default, not a warning.
        if root.get("ui_scale_percent").is_some() {
            settings.ui_scale_percent = read_i64(
                &root,
                "ui_scale_percent",
                settings.ui_scale_percent as i64,
                MIN_UI_SCALE_PERCENT as i64,
                MAX_UI_SCALE_PERCENT as i64,
                &mut warnings,
            ) as u32;
        }
        if let Some(recent) = root.get("recent_files") {
            match recent.as_arr() {
                Some(items) => {
                    // Preserve document order (newest first); add_recent would reverse it.
                    let mut loaded: Vec<String> = Vec::new();
                    for item in items {
                        if let Some(path) = item.as_str() {
                            let path = path.trim();
                            if !path.is_empty() && !loaded.iter().any(|existing| existing == path) {
                                loaded.push(path.to_string());
                            }
                        }
                    }
                    loaded.truncate(MAX_RECENT_FILES);
                    settings.recent_files = loaded;
                }
                None => warnings.push("recent_files is not an array; ignored.".to_string()),
            }
        }
        if let Some(session) = root.get("session") {
            // `null` is how a settings file with no session is written; it is not a problem.
            // A malformed session never fails the load either: read_session records the warning.
            if !matches!(session, Json::Null) {
                if let Some(parsed) = read_session(session, &mut warnings) {
                    settings.session = Some(parsed);
                }
            }
        }
        settings.warnings.extend(warnings);
        settings
    }

    /// Reads a settings file, returning defaults when it is missing or unreadable.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => Settings::from_json(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::new(),
            Err(error) => {
                let mut settings = Settings::new();
                settings.warnings.push(format!(
                    "cannot read settings at {}: {error}; defaults used.",
                    path.display()
                ));
                settings
            }
        }
    }

    /// Writes the settings file, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut text = self.to_json();
        text.push('\n');
        std::fs::write(path, text)
    }

    /// Records a recently used path, newest first, without duplicates.
    pub fn add_recent(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        self.recent_files.retain(|existing| existing != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    /// DOC-004: drops every recent path that no longer exists on disk, returning how many went.
    ///
    /// This runs when the settings are loaded into the running application, so the File bar's
    /// Recent menu never offers a file that was deleted between two launches. A path that exists
    /// but cannot be read is kept: the Open path reports that failure, which is more useful than
    /// silently forgetting the file.
    pub fn prune_missing_recent(&mut self) -> usize {
        let before = self.recent_files.len();
        self.recent_files.retain(|path| Path::new(path).exists());
        before - self.recent_files.len()
    }

    /// Replaces the persisted session with `session`.
    pub fn set_session(&mut self, session: SessionState) {
        self.session = Some(session);
    }

    /// Takes the load warnings, leaving the list empty.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }
}

/// Reads a persisted session (DOC-004).
///
/// A malformed session never fails the load: it is ignored with a warning and the next launch
/// simply opens the starter documents. Entries without a usable document identity are skipped,
/// because an unsaved buffer could not be matched to its recovery record without one.
fn read_session(session: &Json, warnings: &mut Vec<String>) -> Option<SessionState> {
    let mut documents = Vec::new();
    match session.get("documents").and_then(Json::as_arr) {
        Some(items) => {
            for (index, item) in items.iter().enumerate() {
                let Some(document_id) = item.get("document_id").and_then(|value| match value {
                    Json::Num(text) => text.parse::<u64>().ok(),
                    _ => None,
                }) else {
                    warnings.push(format!(
                        "session document {index} has no document_id; it was skipped."
                    ));
                    continue;
                };
                let path = match item.get("path") {
                    None | Some(Json::Null) => None,
                    Some(Json::Str(path)) => {
                        let path = path.trim();
                        if path.is_empty() {
                            warnings.push(format!(
                                "session document {index} has an empty path; it was skipped."
                            ));
                            continue;
                        }
                        Some(path.to_string())
                    }
                    Some(_) => {
                        warnings.push(format!(
                            "session document {index} has a path that is not a string; it was skipped."
                        ));
                        continue;
                    }
                };
                documents.push(SessionTab { path, document_id });
            }
        }
        None => {
            warnings
                .push("session.documents is not an array; the session was ignored.".to_string());
            return None;
        }
    }
    if documents.is_empty() {
        return None;
    }
    let active = match session.get("active") {
        Some(Json::Num(text)) => text.parse::<usize>().unwrap_or(0),
        None => 0,
        Some(_) => {
            warnings.push("session.active is not a number; using tab 1.".to_string());
            0
        }
    };
    Some(SessionState::new(documents, active))
}

/// Reads an integer field, clamping it into range and recording a warning when it is missing,
/// not a number, or out of range.
fn read_i64(
    container: &Json,
    name: &str,
    default: i64,
    lo: i64,
    hi: i64,
    warnings: &mut Vec<String>,
) -> i64 {
    match container.get(name) {
        Some(Json::Num(text)) => match text.parse::<i64>() {
            Ok(value) if (lo..=hi).contains(&value) => value,
            Ok(value) => {
                let clamped = value.clamp(lo, hi);
                warnings.push(format!(
                    "{name}={value} is outside {lo}..={hi}; using {clamped}."
                ));
                clamped
            }
            Err(_) => {
                warnings.push(format!("{name} is not an integer; using {default}."));
                default
            }
        },
        Some(_) => {
            warnings.push(format!("{name} is not a number; using {default}."));
            default
        }
        None => {
            warnings.push(format!("{name} is missing; using {default}."));
            default
        }
    }
}

/// Reads a boolean field, recording a warning when it is missing or not a boolean.
fn read_bool(container: &Json, name: &str, default: bool, warnings: &mut Vec<String>) -> bool {
    match container.get(name) {
        Some(Json::Bool(value)) => *value,
        Some(_) => {
            warnings.push(format!("{name} is not a boolean; using {default}."));
            default
        }
        None => {
            warnings.push(format!("{name} is missing; using {default}."));
            default
        }
    }
}

/// The per-user application directory: %APPDATA%\Lovelace on Windows, and the usual XDG
/// location elsewhere. Every file the application writes outside a document lives below it,
/// so the settings file and the plot directory are always siblings resolved the same way.
fn app_dir_from(
    appdata: Option<&OsStr>,
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(appdata) = appdata {
        return Some(PathBuf::from(appdata).join("Lovelace"));
    }
    if let Some(xdg) = xdg_config_home {
        return Some(PathBuf::from(xdg).join("lovelace"));
    }
    let home = home?;
    Some(PathBuf::from(home).join(".config").join("lovelace"))
}

/// The per-user application directory for this process.
fn app_dir() -> Option<PathBuf> {
    app_dir_from(
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// The per-user settings path: %APPDATA%\Lovelace\desktop.json on Windows, and the usual
/// XDG location elsewhere.
pub fn default_config_path() -> Option<PathBuf> {
    app_dir().map(|directory| directory.join("desktop.json"))
}

/// The per-user plot directory: %APPDATA%\Lovelace\plots on Windows, and the usual XDG
/// location elsewhere. It is derived from the same application directory as
/// [`default_config_path`], so the product never writes beside the executable or in the
/// process working directory (PLF-006).
pub fn default_plot_dir() -> Option<PathBuf> {
    app_dir().map(|directory| directory.join("plots"))
}

/// DOC-005: the per-user recovery directory, `%APPDATA%\Lovelace\recovery` on Windows.
///
/// It is the sibling of the settings file, so every file the application writes outside a
/// document lives under the one application-scoped directory (PLF-006). One record file per
/// journaled document lives here and nowhere else.
pub fn default_recovery_dir() -> Option<PathBuf> {
    app_dir().map(|directory| directory.join("recovery"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_preference_round_trips_and_rejects_unknown_values() {
        assert_eq!(
            Settings::new().theme,
            ThemePreference::Dark,
            "dark is the shipped default"
        );
        let mut settings = Settings::new();
        settings.theme = ThemePreference::Light;
        let again = Settings::from_json(&settings.to_json());
        assert_eq!(again.theme, ThemePreference::Light);
        assert!(
            again.warnings.is_empty(),
            "a valid theme must round-trip without warnings: {:?}",
            again.warnings
        );
        // A file written before VIS-002 has no theme field: dark, and no warning (the field was
        // not lost, it simply did not exist).
        let older = Settings::from_json(r#"{"schema_version": 1, "active_precision": 30}"#);
        assert_eq!(older.theme, ThemePreference::Dark);
        assert!(
            !older.warnings.iter().any(|warning| warning.contains("theme")),
            "a missing theme is not a warning: {:?}",
            older.warnings
        );
        // An unknown value is a real problem, so it warns and keeps the safe default.
        let unknown = Settings::from_json(r#"{"schema_version": 1, "theme": "blue"}"#);
        assert_eq!(unknown.theme, ThemePreference::Dark);
        assert!(
            unknown.warnings.iter().any(|warning| warning.contains("theme")),
            "warnings: {:?}",
            unknown.warnings
        );
        assert_eq!(ThemePreference::Dark.toggled(), ThemePreference::Light);
        assert_eq!(ThemePreference::Light.toggled(), ThemePreference::Dark);
    }

    #[test]
    fn settings_round_trip_through_json() {
        let mut settings = Settings::new();
        settings.window = WindowGeometry {
            width: 1440,
            height: 900,
        };
        settings.layout.editor_width_permille = 640;
        settings.layout.logs_height_permille = 250;
        settings.layout.plot_height_permille = 400;
        // LAY-004: the workbench's own pane sizes and its visible compact pane round-trip.
        settings.workbench_layout = WorkbenchLayout {
            editor_permille: 530,
            workspace_permille: 240,
            logs_permille: 300,
            inspector_permille: 410,
            compact_editor_permille: 620,
            compact_pane: CompactPaneSetting::Inspector,
        };
        settings.active_precision = 2500;
        settings.add_recent("C:/scripts/a.ls");
        settings.add_recent("C:/scripts/b.ls");

        let text = settings.to_json();
        let again = Settings::from_json(&text);
        assert_eq!(again, settings);
        assert!(
            again.warnings.is_empty(),
            "round trip must be warning-free: {:?}",
            again.warnings
        );
    }

    /// LAY-004: the workbench layout survives the settings document and rejects bad values
    /// field by field. A settings file written before the workbench persisted its layout loads
    /// its default pane sizes without a warning.
    #[test]
    fn the_workbench_pane_layout_round_trips_and_clamps() {
        let settings = Settings::from_json(r#"{"schema_version": 1}"#);
        assert_eq!(settings.workbench_layout, WorkbenchLayout::default());
        assert!(
            !settings
                .warnings
                .iter()
                .any(|warning| warning.contains("workbench_layout")),
            "an older settings file is not a warning: {:?}",
            settings.warnings
        );

        let bad = Settings::from_json(
            r#"{
                "schema_version": 1,
                "workbench_layout": {
                    "editor_permille": 9999,
                    "workspace_permille": 0,
                    "logs_permille": 1,
                    "inspector_permille": 10,
                    "compact_editor_permille": 9000,
                    "compact_pane": "not-a-pane"
                }
            }"#,
        );
        assert_eq!(bad.workbench_layout.editor_permille, 800);
        assert_eq!(bad.workbench_layout.workspace_permille, 100);
        assert_eq!(bad.workbench_layout.logs_permille, 100);
        assert_eq!(bad.workbench_layout.inspector_permille, 150);
        assert_eq!(bad.workbench_layout.compact_editor_permille, 820);
        assert_eq!(bad.workbench_layout.compact_pane, CompactPaneSetting::Hidden);
        assert!(
            bad.warnings
                .iter()
                .any(|warning| warning.contains("compact_pane")),
            "an unknown pane is reported: {:?}",
            bad.warnings
        );

        // Round trip: every fraction and the visible drawer pane survive exactly.
        let mut settings = Settings::new();
        settings.workbench_layout = WorkbenchLayout {
            editor_permille: 505,
            workspace_permille: 215,
            logs_permille: 260,
            inspector_permille: 340,
            compact_editor_permille: 580,
            compact_pane: CompactPaneSetting::Logs,
        };
        let again = Settings::from_json(&settings.to_json());
        assert_eq!(again.workbench_layout, settings.workbench_layout);
        assert!(again.warnings.is_empty(), "{:?}", again.warnings);
        assert_eq!(
            CompactPaneSetting::parse(CompactPaneSetting::Graph.as_str()),
            Some(CompactPaneSetting::Graph)
        );
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults_with_a_warning() {
        let settings = Settings::from_json("{ this is not json");
        let defaults = Settings::new();
        assert_eq!(settings.window, defaults.window);
        assert_eq!(settings.layout, defaults.layout);
        assert_eq!(settings.active_precision, defaults.active_precision);
        assert!(settings.recent_files.is_empty());
        assert_eq!(settings.warnings.len(), 1);
        assert!(settings.warnings[0].contains("not valid JSON"));
    }

    #[test]
    fn non_object_root_falls_back_to_defaults() {
        let settings = Settings::from_json("[1, 2, 3]");
        assert_eq!(settings.window, WindowGeometry::default());
        assert!(settings.warnings[0].contains("not a JSON object"));
    }

    #[test]
    fn newer_schema_refuses_to_guess_and_uses_defaults() {
        let text = r#"{"schema_version": 99, "window": {"width": 100, "height": 100}}"#;
        let settings = Settings::from_json(text);
        assert_eq!(settings.window, WindowGeometry::default());
        assert!(
            settings.warnings[0].contains("newer than supported"),
            "unexpected warning: {:?}",
            settings.warnings
        );
    }

    #[test]
    fn out_of_range_and_wrong_typed_fields_are_clamped_with_warnings() {
        let text = r#"{
            "schema_version": 1,
            "window": {"width": 10, "height": "tall"},
            "layout": {"editor_width_permille": 9999, "logs_height_permille": 0, "plot_height_permille": 460},
            "recent_files": ["a.ls", "b.ls", "a.ls"],
            "active_precision": -5
        }"#;
        let settings = Settings::from_json(text);
        assert_eq!(settings.window.width, MIN_WINDOW_WIDTH);
        assert_eq!(settings.window.height, WindowGeometry::default().height);
        assert_eq!(settings.layout.editor_width_permille, 900);
        assert_eq!(settings.layout.logs_height_permille, 100);
        assert_eq!(settings.active_precision, MIN_PRECISION);
        assert_eq!(
            settings.recent_files,
            vec!["a.ls".to_string(), "b.ls".to_string()]
        );
        assert!(
            settings.warnings.len() >= 4,
            "warnings: {:?}",
            settings.warnings
        );
    }

    #[test]
    fn editor_settings_round_trip_and_defaults() {
        let mut settings = Settings::new();
        assert_eq!(
            settings.editor,
            EditorSettings::default(),
            "the shipped default is a four-space indent with auto-close on"
        );
        settings.editor = EditorSettings {
            indent_width: 8,
            use_spaces: false,
            auto_close: false,
            ..EditorSettings::default()
        };
        let again = Settings::from_json(&settings.to_json());
        assert_eq!(again.editor, settings.editor);
        assert_eq!(again.editor.indent_unit(), "\t");
        assert!(again.warnings.is_empty(), "{:?}", again.warnings);

        // A file written before EDT-008 has no editor object: the defaults apply, and the rest
        // of the document still loads.
        let old = Settings::from_json(r#"{"schema_version": 1, "active_precision": 250}"#);
        assert_eq!(old.editor, EditorSettings::default());
        assert_eq!(old.active_precision, 250);
    }

    #[test]
    fn malformed_editor_settings_fall_back_field_by_field_with_warnings() {
        let text = r#"{
            "schema_version": 1,
            "editor": {"indent_width": 99, "use_spaces": "yes", "auto_close": false}
        }"#;
        let settings = Settings::from_json(text);
        assert_eq!(
            settings.editor,
            EditorSettings {
                indent_width: MAX_INDENT_WIDTH,
                use_spaces: true,
                auto_close: false,
                ..EditorSettings::default()
            },
            "indent_width is clamped, use_spaces falls back, the valid bool is kept"
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("indent_width=99")),
            "warnings: {:?}",
            settings.warnings
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("use_spaces is not a boolean")),
            "warnings: {:?}",
            settings.warnings
        );
    }

    #[test]
    fn missing_schema_version_is_corruption() {
        let settings = Settings::from_json(r#"{"window": {"width": 1000, "height": 700}}"#);
        assert_eq!(settings.window, WindowGeometry::default());
        assert!(settings.warnings[0].contains("schema_version"));
    }

    #[test]
    fn recent_files_deduplicate_and_cap() {
        let mut settings = Settings::new();
        for index in 0..(MAX_RECENT_FILES + 5) {
            settings.add_recent(&format!("file-{index}.ls"));
        }
        assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
        assert_eq!(
            settings.recent_files[0],
            format!("file-{}.ls", MAX_RECENT_FILES + 4)
        );
        settings.add_recent("file-29.ls");
        assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
        assert_eq!(
            settings
                .recent_files
                .iter()
                .filter(|path| path.as_str() == "file-29.ls")
                .count(),
            1
        );
    }

    #[test]
    fn app_dir_prefers_appdata_then_xdg_then_home() {
        let appdata = OsStr::new(r"C:\Users\example\AppData\Roaming");
        let xdg = OsStr::new("/home/example/.config");
        let home = OsStr::new("/home/example");

        let from_appdata =
            app_dir_from(Some(appdata), Some(xdg), Some(home)).expect("appdata wins");
        assert_eq!(
            from_appdata,
            PathBuf::from(appdata).join("Lovelace"),
            "the Windows application directory is %APPDATA%\\Lovelace"
        );
        assert_eq!(
            from_appdata.join("desktop.json"),
            PathBuf::from(appdata).join("Lovelace").join("desktop.json")
        );

        let from_xdg = app_dir_from(None, Some(xdg), Some(home)).expect("xdg second");
        assert_eq!(from_xdg, PathBuf::from(xdg).join("lovelace"));

        let from_home = app_dir_from(None, None, Some(home)).expect("home last");
        assert_eq!(
            from_home,
            PathBuf::from(home).join(".config").join("lovelace")
        );

        assert!(app_dir_from(None, None, None).is_none());
    }

    #[test]
    fn plot_dir_is_derived_from_the_settings_application_directory() {
        let appdata = OsStr::new(r"C:\Users\example\AppData\Roaming");
        let directory = app_dir_from(Some(appdata), None, None).expect("appdata");
        assert_eq!(
            directory.join("plots"),
            directory
                .join("desktop.json")
                .parent()
                .unwrap()
                .join("plots"),
            "the plot directory is the sibling of desktop.json, not the working directory"
        );
        assert!(directory.join("plots").ends_with("plots"));
        assert!(directory.join("plots").starts_with(PathBuf::from(appdata)));
    }

    #[test]
    fn default_plot_dir_matches_default_config_path_parent() {
        let Some(config) = default_config_path() else {
            // No APPDATA/XDG/HOME in this environment: the derivation itself is covered above.
            return;
        };
        let expected = config.parent().expect("config has a parent").join("plots");
        assert_eq!(default_plot_dir(), Some(expected));
    }

    #[test]
    fn save_and_load_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("lovelace-settings-{}", std::process::id()));
        let path = dir.join("desktop.json");
        let mut settings = Settings::new();
        settings.active_precision = 50;
        settings.add_recent("C:/x/y.ls");
        settings.save(&path).expect("save settings");
        let loaded = Settings::load(&path);
        assert_eq!(loaded, settings);
        assert!(loaded.warnings.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DOC-004: a session saves and loads with its tab order and active tab intact.
    #[test]
    fn session_tab_order_and_active_tab_round_trip_through_disk() {
        let directory =
            std::env::temp_dir().join(format!("lovelace-session-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("scratch directory");
        let settings_path = directory.join("desktop.json");
        let first = directory.join("first.lv");
        let second = directory.join("second.lv");
        let third = directory.join("third.lv");
        for path in [&first, &second, &third] {
            std::fs::write(path, "a = 1\n").expect("fixture");
        }

        let mut settings = Settings::new();
        settings.add_recent(&second.to_string_lossy());
        settings.add_recent(&first.to_string_lossy());
        settings.set_session(SessionState::new(
            vec![
                SessionTab {
                    path: Some(first.to_string_lossy().into_owned()),
                    document_id: 7,
                },
                SessionTab {
                    path: None,
                    document_id: 8,
                },
                SessionTab {
                    path: Some(third.to_string_lossy().into_owned()),
                    document_id: 9,
                },
            ],
            1,
        ));
        settings.save(&settings_path).expect("save settings");

        let loaded = Settings::load(&settings_path);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let session = loaded
            .session
            .as_ref()
            .expect("session survives a save/load");
        assert_eq!(session.active, 1, "the active tab is persisted");
        assert_eq!(session.documents.len(), 3, "tab order is persisted");
        assert_eq!(session.documents[0].document_id, 7);
        assert_eq!(session.documents[2].document_id, 9);
        assert_eq!(
            session.documents[2].path.as_deref(),
            Some(third.to_string_lossy().as_ref())
        );
        assert_eq!(
            session.documents[1].path, None,
            "a pathless tab stays pathless"
        );
        assert_eq!(session.documents[1].document_id, 8);
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// DOC-004: reopening a file moves it to the top of the recent list and never duplicates it.
    #[test]
    fn recent_files_move_a_reopened_file_to_the_top_without_duplicating_it() {
        let mut settings = Settings::new();
        settings.add_recent("C:/scripts/a.lv");
        settings.add_recent("C:/scripts/b.lv");
        settings.add_recent("C:/scripts/c.lv");
        assert_eq!(
            settings.recent_files,
            vec![
                "C:/scripts/c.lv".to_string(),
                "C:/scripts/b.lv".to_string(),
                "C:/scripts/a.lv".to_string()
            ]
        );
        settings.add_recent("C:/scripts/a.lv");
        assert_eq!(settings.recent_files[0], "C:/scripts/a.lv");
        assert_eq!(
            settings
                .recent_files
                .iter()
                .filter(|path| path.as_str() == "C:/scripts/a.lv")
                .count(),
            1,
            "a re-opened file must appear exactly once"
        );
        assert_eq!(settings.recent_files.len(), 3);
    }

    /// DOC-004: a recent path that no longer exists is pruned when the settings are loaded.
    #[test]
    fn recent_paths_that_no_longer_exist_are_pruned_on_load() {
        let directory =
            std::env::temp_dir().join(format!("lovelace-recent-prune-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("scratch directory");
        let kept = directory.join("kept.lv");
        let gone = directory.join("gone.lv");
        std::fs::write(&kept, "a = 1\n").expect("fixture");
        let _ = std::fs::remove_file(&gone);

        let mut settings = Settings::new();
        settings.recent_files = vec![
            gone.to_string_lossy().into_owned(),
            kept.to_string_lossy().into_owned(),
        ];
        let pruned = settings.prune_missing_recent();
        assert_eq!(pruned, 1);
        assert_eq!(
            settings.recent_files,
            vec![kept.to_string_lossy().into_owned()],
            "the existing file is kept in its position"
        );
        // A second prune finds nothing to do.
        assert_eq!(settings.prune_missing_recent(), 0);
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// DOC-004: a malformed session falls back to no session with a warning, never a crash.
    #[test]
    fn a_malformed_session_is_ignored_with_a_warning() {
        let text = r#"{
            "schema_version": 1,
            "session": { "documents": "not an array" }
        }"#;
        let settings = Settings::from_json(text);
        assert!(settings.session.is_none());
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("session.documents")),
            "{:?}",
            settings.warnings
        );

        let text = r#"{
            "schema_version": 1,
            "session": { "active": 9, "documents": [
                { "document_id": 4 },
                { "path": "C:/x.lv" }
            ] }
        }"#;
        let settings = Settings::from_json(text);
        let session = settings.session.expect("one valid entry remains");
        assert_eq!(
            session.documents.len(),
            1,
            "the entry without an id is skipped"
        );
        assert_eq!(session.active, 0, "the active index is clamped to the tabs");
    }

    /// DOC-005: the recovery directory is the sibling of the settings file.
    #[test]
    fn recovery_dir_is_the_sibling_of_the_settings_file() {
        assert_eq!(
            default_recovery_dir(),
            default_config_path().map(|path| path.parent().unwrap().join("recovery"))
        );
    }

    #[test]
    fn missing_file_is_not_a_warning() {
        let path = std::env::temp_dir().join("lovelace-does-not-exist-9c1f/desktop.json");
        let loaded = Settings::load(&path);
        assert_eq!(loaded, Settings::new());
        assert!(loaded.warnings.is_empty());
    }

    /// A11Y-014: the ASCII/Unicode choice is persisted, and a settings file written before the
    /// setting existed loads as the engine's own ASCII default without a warning.
    #[test]
    fn unicode_glyphs_round_trips_and_an_absent_key_is_the_ascii_default() {
        let mut settings = Settings::new();
        assert!(!settings.unicode_glyphs, "the engine's default is ASCII");
        settings.unicode_glyphs = true;
        let json = settings.to_json();
        assert!(
            json.contains("\"unicode_glyphs\": true"),
            "the choice is serialised: {json}"
        );
        let loaded = Settings::from_json(&json);
        assert!(loaded.unicode_glyphs);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);

        let older = Settings::from_json(r#"{"schema_version": 1}"#);
        assert!(!older.unicode_glyphs, "an older file keeps the ASCII default");
        assert!(
            !older
                .warnings
                .iter()
                .any(|warning| warning.contains("unicode_glyphs")),
            "an absent key is not a problem: {:?}",
            older.warnings
        );

        let malformed = Settings::from_json(r#"{"schema_version": 1, "unicode_glyphs": "yes"}"#);
        assert!(!malformed.unicode_glyphs);
        assert!(
            malformed
                .warnings
                .iter()
                .any(|warning| warning.contains("unicode_glyphs")),
            "a malformed value is reported: {:?}",
            malformed.warnings
        );
    }

    /// A11Y-004/A11Y-005/A11Y-006: the interface scale, editor typography and follow-the-OS
    /// choice round-trip; an older settings file keeps the shipped defaults without warnings,
    /// and an out-of-range or malformed value is clamped/reported.
    #[test]
    fn the_scale_typography_and_theme_follow_settings_round_trip_and_clamp() {
        let mut settings = Settings::new();
        assert_eq!(settings.ui_scale_percent, MIN_UI_SCALE_PERCENT);
        assert!(!settings.theme_follows_system, "the shipped default is an explicit theme");
        assert_eq!(settings.editor.typography, TypographySettings::default());
        assert_eq!(TypographySettings::default().font, EditorFont::Hack);
        assert_eq!(TypographySettings::default().font_size_pt, 13);
        assert_eq!(TypographySettings::default().line_height_percent, 125);
        assert!(!TypographySettings::default().ligatures);

        settings.ui_scale_percent = MAX_UI_SCALE_PERCENT;
        settings.theme_follows_system = true;
        settings.editor.typography = TypographySettings {
            font: EditorFont::Cascadia,
            font_size_pt: MAX_FONT_SIZE_PT,
            line_height_percent: 150,
            ligatures: true,
        };
        let again = Settings::from_json(&settings.to_json());
        assert_eq!(again, settings, "the new fields round-trip exactly");
        assert!(again.warnings.is_empty(), "{:?}", again.warnings);

        // A settings file written before these settings existed: defaults, no warnings.
        let older = Settings::from_json(
            r#"{"schema_version": 1, "editor": {"indent_width": 4, "use_spaces": true, "auto_close": true}}"#,
        );
        assert_eq!(older.editor.typography, TypographySettings::default());
        assert_eq!(older.ui_scale_percent, MIN_UI_SCALE_PERCENT);
        assert!(!older.theme_follows_system);
        assert!(
            !older.warnings.iter().any(|warning| {
                warning.contains("font")
                    || warning.contains("line_height")
                    || warning.contains("ligatures")
                    || warning.contains("ui_scale")
                    || warning.contains("theme_follows")
            }),
            "an absent key is not a problem: {:?}",
            older.warnings
        );

        // Out-of-range numbers are clamped and malformed values fall back with a warning.
        let clamped = Settings::from_json(
            r#"{
                "schema_version": 1,
                "ui_scale_percent": 900,
                "theme_follows_system": "yes",
                "editor": {"font": "comic-sans", "font_size_pt": 99, "line_height_percent": 5, "ligatures": "on"}
            }"#,
        );
        assert_eq!(clamped.ui_scale_percent, MAX_UI_SCALE_PERCENT);
        assert!(!clamped.theme_follows_system);
        assert_eq!(clamped.editor.typography.font, EditorFont::Hack);
        assert_eq!(clamped.editor.typography.font_size_pt, MAX_FONT_SIZE_PT);
        assert_eq!(
            clamped.editor.typography.line_height_percent,
            MIN_LINE_HEIGHT_PERCENT
        );
        assert!(!clamped.editor.typography.ligatures);
        assert!(
            clamped
                .warnings
                .iter()
                .any(|warning| warning.contains("theme_follows_system")),
            "{:?}",
            clamped.warnings
        );
        assert!(
            clamped
                .warnings
                .iter()
                .any(|warning| warning.contains("editor.font")),
            "{:?}",
            clamped.warnings
        );

        let too_small = Settings::from_json(r#"{"schema_version": 1, "ui_scale_percent": 10}"#);
        assert_eq!(
            too_small.ui_scale_percent, MIN_UI_SCALE_PERCENT,
            "below the range is the 100% floor"
        );
    }
}
