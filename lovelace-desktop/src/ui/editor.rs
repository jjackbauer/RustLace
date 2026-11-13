//! Virtualised code editor: only the visible rows are laid out.
//!
//! Adapted from `rust/lovelace-desktop-spike/src/editor.rs` (unchanged behaviour) to edit one
//! document buffer supplied by the workbench instead of owning a list of sample documents.
//! The pane paints a line-number gutter, a current-line highlight, a caret and the syntax
//! layouter from [`super::syntax`]; the 1-based line and UTF-16 column readout is derived from
//! [`TextEditor::line_col_1based`].
//!
//! Why not `TextEdit`? `TextEdit::layouter` is handed the *whole* buffer once per pass and has
//! no viewport argument, so a 5 000-line document is laid out in full on every frame. This pane
//! lays out only `metrics.laid_out_rows` rows per pass.

use std::time::Instant;

use egui::{
    Align2, Color32, CursorIcon, FontId, Rect, RichText, Sense, Stroke, TextStyle, Ui, pos2, vec2,
};

use crate::completion::{self, CaretContext, CompletionItem, CompletionKind, CompletionSession};
use crate::diagnostic_map::{self, DiagnosticRange};
use crate::document_io::NewlineStyle;
use crate::execution::Diagnostic;
use crate::find::{FindSession, MatchSpan};
use crate::history::{self, Caret, EditKind, History, HistoryCommand, Snapshot};
use crate::settings::EditorSettings;

use super::syntax;

/// Editor background.
pub const EDITOR_BG: Color32 = Color32::from_rgb(0x14, 0x18, 0x1d);
/// Gutter background.
pub const GUTTER_BG: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);
/// Current-line highlight.
pub const CURRENT_LINE: Color32 = Color32::from_rgb(0x1f, 0x2a, 0x33);
/// Gutter line numbers.
pub const GUTTER_TEXT: Color32 = Color32::from_rgb(0x5c, 0x6b, 0x7d);
/// Caret colour.
pub const CARET: Color32 = Color32::from_rgb(0xf0, 0xf4, 0xf8);
/// Selection highlight, painted behind the selected text.
pub const SELECTION: Color32 = Color32::from_rgb(0x28, 0x4a, 0x5c);
/// EDT-007: the cell background of a matched bracket pair.
pub const BRACKET_MATCH_BG: Color32 = Color32::from_rgb(0x1d, 0x4f, 0x4a);
/// EDT-007: the matched pair's underline / box colour.
pub const BRACKET_MATCH: Color32 = Color32::from_rgb(0x4e, 0xc9, 0xb0);
/// EDT-007: the cell background of a bracket with no mate.
pub const BRACKET_UNMATCHED_BG: Color32 = Color32::from_rgb(0x5a, 0x25, 0x25);
/// EDT-007: the unmatched bracket's underline / box colour, distinct from the matched teal.
pub const BRACKET_UNMATCHED: Color32 = Color32::from_rgb(0xff, 0x7b, 0x72);
/// Diagnostic underline colour.
pub const DIAGNOSTIC_UNDERLINE: Color32 = Color32::from_rgb(0xff, 0x7b, 0x72);
/// Diagnostic gutter-marker colour; a little deeper than the underline so it reads as a marker.
pub const DIAGNOSTIC_MARKER: Color32 = Color32::from_rgb(0xe0, 0x4b, 0x45);
/// Completion popup background: one step above the pane background so it reads as an overlay.
pub const COMPLETION_BG: Color32 = Color32::from_rgb(0x1b, 0x22, 0x2a);
/// Completion popup border, in the accent colour.
pub const COMPLETION_BORDER: Color32 = Color32::from_rgb(0x4e, 0xc9, 0xb0);
/// The highlighted completion row.
pub const COMPLETION_SELECTED: Color32 = Color32::from_rgb(0x2b, 0x4a, 0x47);
/// The detail column of a completion row.
pub const COMPLETION_DETAIL: Color32 = Color32::from_rgb(0x8b, 0x98, 0xa9);
/// EDT-009: the background of every match in the buffer.
pub const FIND_MATCH_BG: Color32 = Color32::from_rgb(0x4a, 0x3f, 0x1f);
/// EDT-009: the background of the highlighted (current) match.
pub const FIND_CURRENT_BG: Color32 = Color32::from_rgb(0x7a, 0x64, 0x20);
/// EDT-009: the underline of the highlighted match, so it stays distinct from the other hits.
pub const FIND_CURRENT_MARK: Color32 = Color32::from_rgb(0xf0, 0xc8, 0x60);
/// EDT-004: signature popup background, one step above the pane background.
pub const SIGNATURE_BG: Color32 = Color32::from_rgb(0x1b, 0x22, 0x2a);
/// EDT-004: signature popup border, in the accent colour.
pub const SIGNATURE_BORDER: Color32 = Color32::from_rgb(0x4e, 0xc9, 0xb0);
/// EDT-004: the parameter name the caret is currently editing.
pub const SIGNATURE_ACTIVE: Color32 = Color32::from_rgb(0xf0, 0xc8, 0x60);
/// EDT-004: the rest of the signature.
pub const SIGNATURE_TEXT: Color32 = Color32::from_rgb(0xd8, 0xde, 0xe9);
/// User-defined and host functions: a distinct accent from the builtins, which use the
/// highlighter's builtin blue. This is how the popup marks user functions apart.
pub const COMPLETION_FUNCTION: Color32 = Color32::from_rgb(0x4e, 0xc9, 0xb0);
/// Variables use the plain text colour; their detail carries the kind.
pub const COMPLETION_VARIABLE: Color32 = Color32::from_rgb(0xd8, 0xde, 0xe9);
/// Height of one completion row, in pixels.
const COMPLETION_ROW_H: f32 = 18.0;
/// The popup never grows past this many rows; the highlight scrolls the window inside it.
const COMPLETION_MAX_ROWS: usize = 9;

/// What one editor pass laid out, for the status strip and for measurement.
#[allow(dead_code)] // the benchmark-side fields stay part of the measured editor surface
#[derive(Clone, Copy, Default)]
pub struct EditorMetrics {
    /// First visible row (0-based).
    pub first_row: usize,
    /// Rows that fit in the viewport.
    pub visible_rows: usize,
    /// Rows actually laid out this pass.
    pub laid_out_rows: usize,
    /// Row height in pixels.
    pub row_h: f32,
    /// Monospace character width in pixels.
    pub char_w: f32,
    /// Gutter width in pixels.
    pub gutter_w: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Time the syntax refresh took this pass, in milliseconds (EDT-016 instrumentation).
    pub syntax_ms: f64,
    /// Time the visible rows' layout and paint took this pass, in milliseconds.
    pub rows_ms: f64,
}

/// EDT-007: the bracket next to the caret and, when one exists, its mate.
///
/// `bracket` is the bracket on the caret's side: the character at the caret when it is a bracket,
/// otherwise the character immediately before the caret. `mate` is `None` exactly when the
/// bracket has no matching partner in the buffer, which is the editor's distinct "unmatched"
/// state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracketMatch {
    /// The bracket character the caret is adjacent to.
    pub bracket: char,
    /// The bracket's buffer position, 0-based row and char column.
    pub at: (usize, usize),
    /// The matching bracket's position, or `None` when it is unmatched.
    pub mate: Option<(usize, usize)>,
}

/// EDT-010: one identifier occurrence the in-file symbol finder can jump to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolHit {
    /// The identifier text (the first occurrence of each name is kept).
    pub name: String,
    /// 0-based row of the identifier.
    pub row: usize,
    /// 0-based char column of the identifier's first character.
    pub col: usize,
}

impl BracketMatch {
    /// True when the bracket has a mate and therefore paints in the matched colour.
    pub fn is_matched(&self) -> bool {
        self.mate.is_some()
    }
}

/// EDT-004: one function's live signature help, built from the engine's registered metadata.
///
/// `name` and `parameters` are copied from the engine snapshot's [`crate::engine::EngineFunction`]
/// for the call the caret is inside; there is no signature table in the desktop crate. `active`
/// is the zero-based parameter the caret's argument position selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureHelp {
    /// The function's registered name.
    pub name: String,
    /// Its declared parameter names, in declaration order.
    pub parameters: Vec<String>,
    /// The active parameter's index, clamped into `parameters`.
    pub active: usize,
    /// The registry's own one-line summary, when the engine published one.
    pub summary: Option<String>,
    /// Buffer position of the call's opening `(`, used to keep an Escape dismissal scoped to
    /// this call instead of suppressing the next one.
    pub open: (usize, usize),
}

impl SignatureHelp {
    /// The signature as `name(p1, p2)`, exactly the parameter names the engine registered.
    pub fn signature(&self) -> String {
        format!("{}({})", self.name, self.parameters.join(", "))
    }

    /// The active parameter's name, if there is one.
    pub fn active_parameter(&self) -> Option<&str> {
        self.parameters.get(self.active).map(String::as_str)
    }

    /// The one-line state the pane header publishes (and a UIA capture records).
    pub fn status(&self) -> String {
        match self.active_parameter() {
            Some(parameter) => format!(
                "Signature: {} - parameter {}/{} '{}'",
                self.signature(),
                self.active + 1,
                self.parameters.len(),
                parameter
            ),
            None => format!("Signature: {} - no parameters", self.signature()),
        }
    }
}

/// One editable buffer with its own caret and scroll position.
pub struct TextEditor {
    /// Source lines, always at least one (an empty document has one empty line).
    ///
    /// A line ending is not part of a line: `\r` from a CRLF file is stripped on load and put back
    /// by [`TextEditor::source`] using [`Self::newline`], so a CRLF document is editable and
    /// saves with its own line endings.
    pub lines: Vec<String>,
    /// The line ending new lines are written with, copied from the document (DOC-002).
    pub newline: NewlineStyle,
    /// Caret row, 0-based.
    pub caret_row: usize,
    /// Caret column in characters, 0-based.
    pub caret_col: usize,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Last pass's metrics.
    pub metrics: EditorMetrics,
    /// Time the last pass took, in milliseconds.
    pub last_ui_ms: f64,
    /// Number of source-mutating edits, used by the window to detect a changed buffer.
    pub edits: u64,
    /// The failed run's diagnostic to decorate, while it describes the current buffer.
    ///
    /// The window copies this from [`crate::workbench::Workbench::diagnostic`], which is already
    /// revision-gated; the editor additionally drops it on the same pass as an edit and refuses
    /// to paint a line past the buffer.
    pub diagnostic: Option<Diagnostic>,
    /// The live completion popup: filtered entries, highlight and the prefix acceptance replaces.
    pub completion: CompletionSession,
    /// This document's undo/redo history.
    ///
    /// The window owns one editor per tab, so the history belongs to exactly one document: an
    /// undo in one tab can never pop a step recorded in another. A typing burst is coalesced by
    /// [`self::history`], and every step restores the buffer, the caret and the selection.
    pub history: History,
    /// The fixed end of the current selection; the caret is the moving end.
    ///
    /// `None` means no selection. When this equals the caret position the selection is empty and
    /// [`TextEditor::selection_range`] reports none.
    pub selection: Option<(usize, usize)>,
    /// EDT-008: indentation and auto-close behaviour copied from the persisted settings.
    pub preferences: EditorSettings,
    /// EDT-009: the find bar's query, toggles, matches and highlighted match for this document.
    pub find: FindSession,
    /// EDT-004: the signature help drawn by the last pass, if any.
    ///
    /// It is kept so the pane header can publish the same signature the popup paints; the popup
    /// itself is painter-only, like the completion popup.
    pub last_signature: Option<SignatureHelp>,
    /// EDT-004: the opening parenthesis of a signature the user dismissed with Escape.
    ///
    /// The dismissal belongs to that one call. Typing a new `(` (or leaving the call) computes a
    /// different open position and the help appears again.
    signature_dismissed: Option<(usize, usize)>,
    /// The builtins and symbols the current paint pass classifies. Refreshed from the live
    /// engine snapshot and the document's `symbol("...")` calls, so the highlighter holds no
    /// second builtin or symbol list.
    syntax: syntax::SyntaxContext,
    /// True when an edit could have changed a `symbol("...")` declaration.
    ///
    /// EDT-016: rediscovering the whole document's symbols on every keystroke costs the same as
    /// a full-document scan (about 20 ms in a debug build for 500 KB). Only edits that touch the
    /// `symbol(` name can change that set, so typing an ordinary character leaves the cache
    /// valid and the frame cheap. Undo, redo and replacement set the flag unconditionally.
    symbols_dirty: bool,
    /// The builtin labels the cached context was built from, for the per-pass change check.
    syntax_builtins: Vec<String>,
}

/// The stable egui id of the editor surface.
///
/// The window requests focus for this id on its first frame so a script typed immediately after
/// startup goes into the editor without a click. `Id::unique` is deterministic, so the same id
/// is used by every pass.
pub fn surface_id() -> egui::Id {
    egui::Id::unique("editor_surface")
}

fn split_lines(source: &str) -> Vec<String> {
    // A CRLF file splits on `\n` and leaves the `\r` at the end of every line. It is a line
    // ending, not a character: strip exactly one so the buffer holds the text the user sees.
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

impl TextEditor {
    /// Creates an editor for `source`, with the caret at the start and LF line endings.
    pub fn from_source(source: &str) -> TextEditor {
        TextEditor::from_source_with_newline(source, NewlineStyle::Lf)
    }

    /// Creates an editor for `source` that writes new lines with `newline`.
    ///
    /// DOC-002: the window constructs one of these per document from the newline style recorded
    /// when the file was opened, so an edit adds line endings of the file's own kind.
    pub fn from_source_with_newline(source: &str, newline: NewlineStyle) -> TextEditor {
        TextEditor {
            lines: split_lines(source),
            newline,
            caret_row: 0,
            caret_col: 0,
            scroll_y: 0.0,
            metrics: EditorMetrics::default(),
            last_ui_ms: 0.0,
            edits: 0,
            diagnostic: None,
            completion: CompletionSession::default(),
            history: History::new(),
            selection: None,
            preferences: EditorSettings::default(),
            find: FindSession::default(),
            last_signature: None,
            signature_dismissed: None,
            syntax: syntax::SyntaxContext::new(),
            symbols_dirty: true,
            syntax_builtins: Vec::new(),
        }
    }

    /// Replaces the buffer from an external source, keeping the current newline style.
    ///
    /// The history is discarded: its snapshots name rows and columns of the text that was just
    /// replaced, so keeping them would let an undo move the caret into unrelated text.
    #[cfg_attr(not(test), allow(dead_code))] // the window always supplies a newline style
    pub fn load(&mut self, source: &str) {
        let newline = self.newline;
        self.load_with_newline(source, newline);
    }

    /// Replaces the buffer and the newline style from an external source (a file load).
    ///
    /// The history is discarded: its snapshots name rows and columns of the text that was just
    /// replaced, so keeping them would let an undo move the caret into unrelated text.
    pub fn load_with_newline(&mut self, source: &str, newline: NewlineStyle) {
        self.lines = split_lines(source);
        self.newline = newline;
        self.caret_row = self.caret_row.min(self.line_count().saturating_sub(1));
        self.caret_col = self.caret_col.min(self.line_chars(self.caret_row));
        self.scroll_y = 0.0;
        self.selection = None;
        self.history.reset();
        self.completion.dismiss();
        self.find.mark_dirty();
        self.last_signature = None;
        self.signature_dismissed = None;
        // The source changed without going through an edit method, so the cached symbol set must
        // be rediscovered on the next paint pass.
        self.symbols_dirty = true;
    }

    /// The current buffer as source text, with the document's own line endings.
    ///
    /// The text supplied to [`TextEditor::load_with_newline`] round-trips when it used one line
    /// ending throughout, including a trailing newline. A file that mixed endings loses that mix
    /// here (the editor has one separator); the workbench's clean save writes the loaded bytes, so
    /// an unmodified mixed file still round-trips on disk.
    pub fn source(&self) -> String {
        self.lines.join(self.newline.ending().as_str())
    }

    /// Number of lines (an empty buffer has one empty line).
    pub fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    /// Byte size of the current source.
    pub fn bytes(&self) -> usize {
        let newlines = self.lines.len().saturating_sub(1);
        self.lines.iter().map(|line| line.len()).sum::<usize>() + newlines
    }

    /// Character count of a row.
    pub fn line_chars(&self, row: usize) -> usize {
        self.lines
            .get(row)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    /// The caret as 1-based line and 1-based **UTF-16** column.
    ///
    /// Columns count UTF-16 code units, the engine's and the diagnostic mapping's unit, so an
    /// astral character is two columns wide. The caret itself is a 0-based char index (the
    /// editor's cell unit); this converts at the readout boundary.
    pub fn line_col_1based(&self) -> (usize, usize) {
        (self.caret_row + 1, self.caret_column_utf16() + 1)
    }

    /// The caret's 0-based UTF-16 code-unit column on its line.
    ///
    /// The caret is stored as a 0-based char index (the editor's cell unit), but the engine,
    /// [`TextEditor::diagnostic_range`] and the status strip all count columns in UTF-16 code
    /// units: an astral character is two units wide. This is the conversion at that boundary.
    pub fn caret_column_utf16(&self) -> usize {
        diagnostic_map::utf16_column(&self.lines, self.caret_row, self.caret_col)
            .unwrap_or_default()
    }

    /// The active diagnostic resolved to a concrete line span, if its line is still in the buffer.
    pub fn diagnostic_range(&self) -> Option<DiagnosticRange> {
        let diagnostic = self.diagnostic.as_ref()?;
        diagnostic_map::range_for(&self.lines, diagnostic.line, diagnostic.column)
    }

    /// Moves the caret to the diagnostic's reported position and returns true when one was drawn.
    ///
    /// This is the gutter-marker click and F8 action: it puts the editor's caret on the exact
    /// 1-based line and UTF-16 column the engine named, not merely somewhere on the line.
    pub fn jump_to_diagnostic(&mut self) -> bool {
        let Some(range) = self.diagnostic_range() else {
            return false;
        };
        self.caret_row = range.line - 1;
        self.caret_col = range.start_char;
        self.selection = None;
        self.history.break_burst();
        true
    }

    /// Moves the caret, clamping row and column into the buffer and clearing any selection.
    #[allow(dead_code)] // part of the editor surface; the window reads the caret instead of setting it
    pub fn set_caret(&mut self, row: usize, col: usize) {
        self.caret_row = row.min(self.line_count().saturating_sub(1));
        self.caret_col = col.min(self.line_chars(self.caret_row));
        self.selection = None;
        self.history.break_burst();
    }

    /// The caret as a [`Caret`] value.
    pub fn caret(&self) -> Caret {
        Caret {
            row: self.caret_row,
            col: self.caret_col,
        }
    }

    /// The character at a 0-based buffer position, when the position is inside the buffer.
    fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.lines.get(row)?.chars().nth(col)
    }

    /// The bracket the caret is adjacent to: the character at the caret, else the one before it.
    ///
    /// Both sides are checked so a caret immediately before `)` and a caret immediately after
    /// `(` both name that bracket. A non-bracket character on both sides yields `None`.
    fn bracket_at_caret(&self) -> Option<(usize, usize, char)> {
        let at = self.char_at(self.caret_row, self.caret_col).filter(|ch| is_bracket(*ch));
        if let Some(ch) = at {
            return Some((self.caret_row, self.caret_col, ch));
        }
        if self.caret_col > 0 {
            if let Some(ch) = self
                .char_at(self.caret_row, self.caret_col - 1)
                .filter(|ch| is_bracket(*ch))
            {
                return Some((self.caret_row, self.caret_col - 1, ch));
            }
        }
        None
    }

    /// EDT-007: the bracket adjacent to the caret and its mate, if any.
    ///
    /// Opening brackets scan forward and closing brackets scan backward, counting only brackets
    /// of the same kind, so nested pairs of the same kind are skipped and a missing partner is
    /// reported instead of pairing with the first bracket of another kind. The scan ignores
    /// brackets inside string literals.
    pub fn bracket_match(&self) -> Option<BracketMatch> {
        let (row, col, bracket) = self.bracket_at_caret()?;
        let mate = self.find_mate(row, col, bracket);
        Some(BracketMatch {
            bracket,
            at: (row, col),
            mate,
        })
    }

    /// Finds the mate of the bracket at `(row, col)`, or `None` when the buffer has none.
    fn find_mate(&self, row: usize, col: usize, bracket: char) -> Option<(usize, usize)> {
        let close = closing_bracket(bracket);
        if let Some(close) = close {
            let mut depth = 0usize;
            for line_index in row..self.line_count() {
                let line = &self.lines[line_index];
                let start = if line_index == row { col } else { 0 };
                let mut in_string = false;
                let mut escaped = false;
                for (char_col, ch) in line.chars().enumerate().skip(start) {
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if ch == '\\' {
                            escaped = true;
                        } else if ch == '"' {
                            in_string = false;
                        }
                        continue;
                    }
                    if ch == '"' {
                        in_string = true;
                    } else if ch == bracket {
                        depth += 1;
                    } else if ch == close {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some((line_index, char_col));
                        }
                    }
                }
            }
            return None;
        }
        let open = opening_bracket(bracket)?;
        let mut depth = 0usize;
        for line_index in (0..=row).rev() {
            let line = &self.lines[line_index];
            let end = if line_index == row {
                col + 1
            } else {
                line.chars().count()
            };
            let chars: Vec<char> = line.chars().take(end).collect();
            let mut in_string = false;
            let mut escaped = false;
            for char_col in (0..chars.len()).rev() {
                let ch = chars[char_col];
                if in_string {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        in_string = false;
                    }
                    continue;
                }
                if ch == '"' {
                    // Opening a string we are now scanning backwards through; the quote parity
                    // is approximate, but a bracket inside a string never matches outside it.
                    in_string = true;
                } else if ch == bracket {
                    depth += 1;
                } else if ch == open {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some((line_index, char_col));
                    }
                }
            }
        }
        None
    }

    /// EDT-010: moves the caret to the mate of the bracket adjacent to it.
    ///
    /// Returns false when the caret is not adjacent to a bracket or the bracket is unmatched;
    /// the caret and selection are untouched in that case.
    pub fn jump_to_matching_bracket(&mut self) -> bool {
        let Some(matched) = self.bracket_match() else {
            return false;
        };
        let Some((row, col)) = matched.mate else {
            return false;
        };
        self.caret_row = row;
        self.caret_col = col.min(self.line_chars(row));
        self.selection = None;
        self.history.break_burst();
        true
    }

    /// EDT-007: the one-line bracket state the pane header publishes (and a capture records).
    ///
    /// A matched pair names both positions and the two bracket characters; an unmatched bracket
    /// says so explicitly, which is the distinct state the row requires.
    pub fn bracket_status(&self) -> Option<String> {
        let matched = self.bracket_match()?;
        match matched.mate {
            Some((row, col)) => Some(format!(
                "Bracket {} at {}:{} matched by {} at {}:{}",
                matched.bracket,
                matched.at.0 + 1,
                matched.at.1 + 1,
                matching_partner(matched.bracket),
                row + 1,
                col + 1
            )),
            None => Some(format!(
                "Unmatched {} at {}:{}",
                matched.bracket,
                matched.at.0 + 1,
                matched.at.1 + 1
            )),
        }
    }

    /// EDT-010: moves the caret to a 1-based line, or 1-based line and column.
    ///
    /// Accepts `"12"`, `"12:4"` and surrounding whitespace. The line must exist; a column past
    /// the line's end is clamped to the end of that line. Returns the reached 1-based caret
    /// position on success and a user-readable reason on failure.
    pub fn go_to_line(&mut self, spec: &str) -> Result<(usize, usize), String> {
        let text = spec.trim();
        if text.is_empty() {
            return Err("Go to line needs a line number.".to_string());
        }
        let (line_text, column_text) = match text.split_once(':') {
            Some((line, column)) => (line.trim(), column.trim()),
            None => (text, ""),
        };
        let line: usize = line_text
            .parse()
            .map_err(|_| format!("'{line_text}' is not a line number."))?;
        if line == 0 || line > self.line_count() {
            return Err(format!(
                "Line {line} is outside 1..={}.",
                self.line_count()
            ));
        }
        let row = line - 1;
        let col = if column_text.is_empty() {
            0
        } else {
            let column: usize = column_text
                .parse()
                .map_err(|_| format!("'{column_text}' is not a column number."))?;
            if column == 0 {
                return Err("Column numbers are 1-based.".to_string());
            }
            (column - 1).min(self.line_chars(row))
        };
        self.caret_row = row;
        self.caret_col = col;
        self.selection = None;
        self.history.break_burst();
        Ok(self.line_col_1based())
    }

    /// EDT-010: every identifier in the document, first occurrence per name, keywords excluded.
    ///
    /// Identifiers inside plain and interpolated string literals are text, not symbols, and are
    /// skipped. The document order is preserved, so the finder lists symbols top to bottom.
    pub fn document_symbols(&self) -> Vec<SymbolHit> {
        let mut hits: Vec<SymbolHit> = Vec::new();
        for (row, line) in self.lines.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut col = 0usize;
            let mut in_string = false;
            let mut escaped = false;
            while col < chars.len() {
                let ch = chars[col];
                if in_string {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        in_string = false;
                    }
                    col += 1;
                    continue;
                }
                if ch == '"' {
                    in_string = true;
                    col += 1;
                    continue;
                }
                if is_ident_start(ch) {
                    let start = col;
                    col += 1;
                    while col < chars.len() && is_ident_continue(chars[col]) {
                        col += 1;
                    }
                    let name: String = chars[start..col].iter().collect();
                    let keyword = crate::language::KEYWORDS.contains(&name.as_str());
                    if !keyword && !hits.iter().any(|hit| hit.name == name) {
                        hits.push(SymbolHit {
                            name,
                            row,
                            col: start,
                        });
                    }
                    continue;
                }
                col += 1;
            }
        }
        hits
    }

    /// EDT-010: filters [`TextEditor::document_symbols`] by a case-insensitive substring.
    ///
    /// An empty query returns every identifier, which is the list the prompt shows when it opens.
    pub fn find_symbols(&self, query: &str) -> Vec<SymbolHit> {
        let needle = query.trim().to_ascii_lowercase();
        self.document_symbols()
            .into_iter()
            .filter(|hit| hit.name.to_ascii_lowercase().contains(&needle))
            .collect()
    }

    /// EDT-010: moves the caret to a symbol occurrence. Returns false for a stale position.
    pub fn jump_to_symbol(&mut self, hit: &SymbolHit) -> bool {
        if hit.row >= self.line_count() {
            return false;
        }
        self.caret_row = hit.row;
        self.caret_col = hit.col.min(self.line_chars(hit.row));
        self.selection = None;
        self.history.break_burst();
        true
    }

    /// The display column of a char column, expanding a tab to the configured indent width.
    ///
    /// The caret is stored in chars (one cell per char); tabs are painted `indent_width` cells
    /// wide, so every screen coordinate (caret, selection, diagnostic underline, bracket box)
    /// goes through this conversion.
    pub fn display_column(&self, row: usize, col: usize) -> usize {
        let width = self.preferences.indent_width.max(1);
        self.lines
            .get(row)
            .map(|line| {
                line.chars()
                    .take(col)
                    .map(|ch| if ch == '\t' { width } else { 1 })
                    .sum()
            })
            .unwrap_or(0)
    }

    /// The char column under a display column, the inverse of [`TextEditor::display_column`].
    fn char_column_at(&self, row: usize, display_col: usize) -> usize {
        let Some(line) = self.lines.get(row) else {
            return 0;
        };
        let width = self.preferences.indent_width.max(1);
        let mut column = 0usize;
        for (index, ch) in line.chars().enumerate() {
            let cells = if ch == '\t' { width } else { 1 };
            if display_col < column + cells {
                return if display_col - column < cells / 2 {
                    index
                } else {
                    index + 1
                };
            }
            column += cells;
        }
        line.chars().count()
    }

    /// The current editor state, which is exactly what one undo step stores.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            caret: self.caret(),
            selection: self.selection.map(|(row, col)| Caret { row, col }),
        }
    }

    /// Restores one history snapshot: the text, the caret and the selection together.
    ///
    /// The snapshot was a valid editor state, but the row/column clamps are still applied so a
    /// future edit to the snapshot format can never point past the buffer. The completion popup
    /// is dismissed because the caret it was positioned for has moved.
    fn restore_snapshot(&mut self, snapshot: Snapshot) {
        self.lines = snapshot.lines;
        self.caret_row = snapshot.caret.row.min(self.line_count().saturating_sub(1));
        self.caret_col = snapshot.caret.col.min(self.line_chars(self.caret_row));
        self.selection = snapshot.selection.map(|anchor| {
            let row = anchor.row.min(self.line_count().saturating_sub(1));
            (row, anchor.col.min(self.line_chars(row)))
        });
        if self.selection == Some((self.caret_row, self.caret_col)) {
            self.selection = None;
        }
        self.completion.dismiss();
        self.find.mark_dirty();
        self.signature_dismissed = None;
        self.last_signature = None;
        self.symbols_dirty = true;
        self.edits += 1;
    }

    /// Starts one undoable edit, recording the pre-edit state when it opens a new step.
    ///
    /// A coalesced typing character records nothing: the open burst's step already holds the
    /// state the run started from, so typing does not clone the buffer per keystroke.
    fn begin_edit(&mut self, kind: EditKind) {
        self.begin_edit_at(kind, Instant::now());
    }

    /// [`TextEditor::begin_edit`] with an explicit clock, for deterministic burst tests.
    fn begin_edit_at(&mut self, kind: EditKind, now: Instant) {
        // EDT-009: every edit can invalidate the find bar's cached matches, so the next pass
        // rescans them. `FindSession::refresh` is a no-op while the bar is closed.
        self.find.mark_dirty();
        if self.history.begin(kind, now) {
            let before = self.snapshot();
            self.history.push(before);
        }
    }

    /// Applies one undo or redo command. Returns true when the buffer changed.
    pub fn command(&mut self, command: HistoryCommand) -> bool {
        match command {
            HistoryCommand::Undo => self.undo(),
            HistoryCommand::Redo => self.redo(),
        }
    }

    /// Restores the state before the last edit. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        if self.history.undo_steps() == 0 {
            return false;
        }
        let current = self.snapshot();
        let Some(previous) = self.history.undo(current) else {
            return false;
        };
        self.restore_snapshot(previous);
        true
    }

    /// Restores the state the last undo replaced. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        if self.history.redo_steps() == 0 {
            return false;
        }
        let current = self.snapshot();
        let Some(next) = self.history.redo(current) else {
            return false;
        };
        self.restore_snapshot(next);
        true
    }

    /// The selection as `(start, end)` buffer positions in reading order, when non-empty.
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.ordered_selection()
    }

    /// The selection as byte offsets in [`TextEditor::source`], when non-empty.
    ///
    /// [`TextEditor::selection_range`] is stored in char columns (the editor paints one char per
    /// cell); the engine and the diagnostics use UTF-16 code units and the buffer is UTF-8. This
    /// converts both endpoints from char space to byte space in one place, so the byte range
    /// slices exactly the characters the editor highlights.
    #[allow(dead_code)] // part of the editor surface; tests and a future find/replace read it
    pub fn selection_bytes(&self) -> Option<(usize, usize)> {
        let (start, end) = self.ordered_selection()?;
        diagnostic_map::selection_byte_range(&self.lines, start, end)
    }

    /// The selected text, when a non-empty range is selected.
    #[allow(dead_code)] // part of the editor surface; tests and a future find/replace read it
    pub fn selected_text(&self) -> Option<String> {
        let ((start_row, start_col), (end_row, end_col)) = self.ordered_selection()?;
        if start_row == end_row {
            let line = self.lines.get(start_row)?;
            return Some(
                line.chars()
                    .skip(start_col)
                    .take(end_col - start_col)
                    .collect(),
            );
        }
        let mut parts = vec![
            self.lines
                .get(start_row)?
                .chars()
                .skip(start_col)
                .collect::<String>(),
        ];
        for row in start_row + 1..end_row {
            parts.push(self.lines.get(row)?.clone());
        }
        parts.push(
            self.lines
                .get(end_row)?
                .chars()
                .take(end_col)
                .collect::<String>(),
        );
        Some(parts.join("\n"))
    }

    /// Selects from the caret to `(row, col)` (shift-click semantics) and moves the caret there.
    ///
    /// The existing anchor is kept when a selection is already active, so a second shift-click
    /// extends from the original fixed end rather than from the moved caret.
    pub fn select_to(&mut self, row: usize, col: usize) {
        let anchor = self.selection.unwrap_or((self.caret_row, self.caret_col));
        let row = row.min(self.line_count().saturating_sub(1));
        let col = col.min(self.line_chars(row));
        self.caret_row = row;
        self.caret_col = col;
        self.selection = if anchor == (row, col) {
            None
        } else {
            Some(anchor)
        };
        self.completion.dismiss();
        self.history.break_burst();
    }

    /// The selection ordered from the start of the buffer to its end, when non-empty.
    fn ordered_selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.selection?;
        let caret = (self.caret_row, self.caret_col);
        if anchor == caret {
            return None;
        }
        Some(if anchor < caret {
            (anchor, caret)
        } else {
            (caret, anchor)
        })
    }

    /// Removes the selected range as part of the caller's edit. Returns true when it removed one.
    fn delete_selection(&mut self) -> bool {
        let Some(((start_row, start_col), (end_row, end_col))) = self.ordered_selection() else {
            return false;
        };
        // A selection that covers `symbol` text, or whose boundary lines mention it, can change
        // the document's symbol declarations even when the remaining text does not say so
        // (`symbol("x")` losing its quote). Mark the cache stale before the splice.
        if self.lines[start_row].contains("symbol")
            || self.lines[end_row].contains("symbol")
            || self
                .selected_text()
                .is_some_and(|text| text.contains("symbol"))
        {
            self.symbols_dirty = true;
        }
        let prefix: String = self.lines[start_row].chars().take(start_col).collect();
        let suffix: String = self.lines[end_row].chars().skip(end_col).collect();
        self.lines
            .splice(start_row..=end_row, [format!("{prefix}{suffix}")]);
        self.caret_row = start_row;
        self.caret_col = start_col;
        self.selection = None;
        true
    }

    /// Inserts typed text at the caret, splitting on newlines.
    ///
    /// Consecutive typed insertions coalesce into one undo step until a pause or a non-typing
    /// action. A non-empty selection is replaced as part of the same step. With auto-close on
    /// (EDT-008), a typed opening bracket or quote also inserts its closing character and leaves
    /// the caret between them, and typing the closing character that is already at the caret
    /// steps over it instead of inserting a duplicate.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() && self.ordered_selection().is_none() {
            return;
        }
        let replaced_selection = self.ordered_selection().is_some();
        self.begin_edit(EditKind::Typing);
        self.delete_selection();
        if self.preferences.auto_close && !replaced_selection && !text.contains('\n') {
            for ch in text.chars() {
                self.type_character(ch);
            }
        } else {
            self.insert_raw(text);
        }
        self.note_symbol_edit(text);
        self.edits += 1;
    }

    /// Inserts one typed character, applying auto-close and skip-over when configured.
    fn type_character(&mut self, ch: char) {
        if is_bracket(ch) || ch == '"' || ch == '\'' {
            if self.char_at(self.caret_row, self.caret_col) == Some(ch) && is_closing(ch) {
                // The configured pair is already there: step over it rather than duplicate it.
                self.caret_col += 1;
                return;
            }
        }
        self.insert_char_raw(ch);
        if let Some(close) = closing_bracket(ch).or_else(|| quote_pair(ch)) {
            if self.should_auto_close(ch) {
                self.insert_char_raw(close);
                // Leave the caret between the pair, where the next typed character belongs.
                self.caret_col -= 1;
            }
        }
    }

    /// True when typing `ch` should also insert its closing partner.
    ///
    /// Brackets always pair. A quote does not pair after an identifier character (the `'` in a
    /// word is text, not an opening quote) nor when the character at the caret is an identifier
    /// character (the quote would split a word rather than open a literal).
    fn should_auto_close(&self, ch: char) -> bool {
        if is_bracket(ch) {
            return true;
        }
        if ch != '"' && ch != '\'' {
            return false;
        }
        let before = if self.caret_col >= 2 {
            self.char_at(self.caret_row, self.caret_col - 2)
        } else {
            None
        };
        let after = self.char_at(self.caret_row, self.caret_col);
        let identifier = |character: char| is_ident_continue(character);
        !before.map(identifier).unwrap_or(false) && !after.map(identifier).unwrap_or(false)
    }

    /// Inserts one character at the caret without history or edit-counter work.
    fn insert_char_raw(&mut self, ch: char) {
        let col = self.caret_col;
        let line = &mut self.lines[self.caret_row];
        let byte = byte_index(line, col);
        line.insert(byte, ch);
        self.caret_col += 1;
    }

    /// Inserts `text` verbatim, splitting on newlines, without auto-close.
    fn insert_raw(&mut self, text: &str) {
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                self.split_line_at_caret();
            }
            if part.is_empty() {
                continue;
            }
            let col = self.caret_col;
            let line = &mut self.lines[self.caret_row];
            let byte = byte_index(line, col);
            line.insert_str(byte, part);
            self.caret_col += part.chars().count();
        }
    }

    /// Inserts `text` as one undoable edit of `kind`, without auto-close.
    fn insert_text(&mut self, text: &str, kind: EditKind) {
        self.insert_text_at(text, kind, Instant::now());
    }

    /// [`TextEditor::insert_text`] with an explicit clock, for deterministic burst tests.
    fn insert_text_at(&mut self, text: &str, kind: EditKind, now: Instant) {
        if text.is_empty() && self.ordered_selection().is_none() {
            return;
        }
        self.begin_edit_at(kind, now);
        self.delete_selection();
        self.insert_raw(text);
        self.note_symbol_edit(text);
        self.edits += 1;
    }

    /// Splits the current line at the caret without touching the history or the edit counter.
    fn split_line_at_caret(&mut self) {
        let col = self.caret_col;
        let row = self.caret_row;
        let line = &mut self.lines[row];
        let byte = byte_index(line, col);
        let tail = line.split_off(byte);
        self.lines.insert(row + 1, tail);
        self.caret_row = row + 1;
        self.caret_col = 0;
    }

    /// EDT-008: smart Enter - keep the current indentation and add one level after `{`.
    ///
    /// The new line starts with the current line's leading whitespace; when the text before the
    /// caret (ignoring trailing spaces) ends with an opening brace, one configured indentation
    /// level is appended. The caret lands after that prefix, so typing continues at the right
    /// depth without any pointer input.
    pub fn newline(&mut self) {
        self.begin_edit(EditKind::Atomic);
        self.delete_selection();
        if self.lines[self.caret_row].contains("symbol") {
            // Splitting a `symbol(` call across lines must be re-examined.
            self.symbols_dirty = true;
        }
        let indent: String = self.lines[self.caret_row]
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        let before: String = self.lines[self.caret_row]
            .chars()
            .take(self.caret_col)
            .collect();
        let extra = if before.trim_end().ends_with('{') {
            self.preferences.indent_unit()
        } else {
            String::new()
        };
        let prefix: String = format!("{indent}{extra}");
        self.split_line_at_caret();
        if !prefix.is_empty() {
            self.lines[self.caret_row].insert_str(0, &prefix);
            self.caret_col = prefix.chars().count();
        }
        self.edits += 1;
    }

    /// EDT-008: the rows a selection (or the caret when there is none) acts on.
    fn edit_rows(&self) -> (usize, usize, bool) {
        match self.selection_range() {
            Some(((start_row, _), (end_row, _))) => (start_row, end_row, true),
            None => (self.caret_row, self.caret_row, false),
        }
    }

    /// EDT-008: the Tab key's action, shared by the editor's own event loop and the window.
    ///
    /// An open completion popup owns Tab (it accepts the highlighted entry). Otherwise Tab
    /// indents the selection (or inserts a level at the caret) and Shift+Tab outdents it.
    pub fn handle_tab(&mut self, shift: bool) {
        if self.completion.is_open() {
            self.accept_completion();
        } else if shift {
            self.outdent();
        } else {
            self.indent();
        }
    }

    /// EDT-008: Tab - indent every selected line, or insert one level at the caret.
    ///
    /// With a non-empty selection every line it touches gets one indentation unit prepended and
    /// the selection endpoints move with the text they covered, so the same words stay selected.
    /// With no selection this is an ordinary insertion at the caret.
    pub fn indent(&mut self) {
        let (first, last, has_selection) = self.edit_rows();
        if !has_selection {
            let unit = self.preferences.indent_unit();
            self.insert_text(&unit, EditKind::Atomic);
            return;
        }
        let unit = self.preferences.indent_unit();
        let width = unit.chars().count();
        self.begin_edit(EditKind::Atomic);
        for row in first..=last {
            self.lines[row].insert_str(0, &unit);
        }
        if let Some((anchor_row, anchor_col)) = self.selection {
            if (first..=last).contains(&anchor_row) {
                self.selection = Some((anchor_row, anchor_col + width));
            }
        }
        if (first..=last).contains(&self.caret_row) {
            self.caret_col += width;
        }
        self.edits += 1;
    }

    /// EDT-008: Shift+Tab - outdent every selected line, or the caret's line when none is
    /// selected. At most one indentation level (one tab, or up to `indent_width` spaces) goes.
    pub fn outdent(&mut self) {
        let (first, last, _has_selection) = self.edit_rows();
        let removals: Vec<usize> = (first..=last)
            .map(|row| outdent_amount(&self.lines[row], self.preferences.indent_width.max(1)))
            .collect();
        if removals.iter().all(|removed| *removed == 0) {
            self.history.break_burst();
            return;
        }
        self.begin_edit(EditKind::Atomic);
        for (offset, row) in (first..=last).enumerate() {
            let removed = removals[offset];
            if removed == 0 {
                continue;
            }
            let byte = byte_index(&self.lines[row], removed);
            self.lines[row].drain(..byte);
        }
        let adjust = |row: usize, col: usize| -> usize {
            if row >= first && row <= last {
                col.saturating_sub(removals[row - first])
            } else {
                col
            }
        };
        if let Some((anchor_row, anchor_col)) = self.selection {
            self.selection = Some((anchor_row, adjust(anchor_row, anchor_col)));
        }
        self.caret_col = adjust(self.caret_row, self.caret_col);
        self.edits += 1;
    }

    /// Deletes the character before the caret (or joins with the previous line).
    pub fn backspace(&mut self) {
        if self.ordered_selection().is_some() {
            self.begin_edit(EditKind::Atomic);
            self.delete_selection();
            self.note_symbol_edit("");
            self.edits += 1;
            return;
        }
        if self.caret_col == 0 && self.caret_row == 0 {
            // Nothing to delete, but the key press still ends an open typing burst.
            self.history.break_burst();
            return;
        }
        self.begin_edit(EditKind::Atomic);
        if self.caret_col > 0 {
            let col = self.caret_col;
            let line = &mut self.lines[self.caret_row];
            let end = byte_index(line, col);
            let start = line[..end]
                .char_indices()
                .last()
                .map(|(index, _)| index)
                .unwrap_or(0);
            line.replace_range(start..end, "");
            self.caret_col -= 1;
        } else if self.caret_row > 0 {
            let row = self.caret_row;
            let tail = self.lines.remove(row);
            let previous_len = self.lines[row - 1].chars().count();
            self.lines[row - 1].push_str(&tail);
            self.caret_row = row - 1;
            self.caret_col = previous_len;
        }
        self.note_symbol_edit("");
        self.edits += 1;
    }

    /// Deletes the character after the caret (or joins with the next line).
    pub fn delete_forward(&mut self) {
        if self.ordered_selection().is_some() {
            self.begin_edit(EditKind::Atomic);
            self.delete_selection();
            self.note_symbol_edit("");
            self.edits += 1;
            return;
        }
        let row = self.caret_row;
        let col = self.caret_col;
        let can_delete = col < self.line_chars(row) || row + 1 < self.line_count();
        if !can_delete {
            self.history.break_burst();
            return;
        }
        self.begin_edit(EditKind::Atomic);
        if col < self.line_chars(row) {
            let line = &mut self.lines[row];
            let start = byte_index(line, col);
            let end = line[start..]
                .char_indices()
                .nth(1)
                .map(|(index, _)| start + index)
                .unwrap_or(line.len());
            line.replace_range(start..end, "");
        } else if row + 1 < self.line_count() {
            let tail = self.lines.remove(row + 1);
            self.lines[row].push_str(&tail);
        }
        self.note_symbol_edit("");
        self.edits += 1;
    }

    /// Extends the selection back to where the caret was, or collapses it when they meet.
    ///
    /// This is the Shift+arrow / Shift+Home / Shift+End rule: the first extended move makes the
    /// old caret the anchor, and every following one moves only the caret.
    fn extend_selection_from(&mut self, before: (usize, usize)) {
        if self.selection.is_none() {
            self.selection = Some(before);
        }
        if self.selection == Some((self.caret_row, self.caret_col)) {
            self.selection = None;
        }
        self.history.break_burst();
    }

    /// Moves the caret up one row. A caret move ends an open typing burst.
    pub fn move_up(&mut self) {
        self.history.break_burst();
        if self.caret_row > 0 {
            self.caret_row -= 1;
            self.caret_col = self.caret_col.min(self.line_chars(self.caret_row));
        }
    }

    /// Moves the caret down one row. A caret move ends an open typing burst.
    pub fn move_down(&mut self) {
        self.history.break_burst();
        if self.caret_row + 1 < self.line_count() {
            self.caret_row += 1;
            self.caret_col = self.caret_col.min(self.line_chars(self.caret_row));
        }
    }

    /// Moves the caret left, wrapping at the start of a line. A caret move ends a typing burst.
    pub fn move_left(&mut self) {
        self.history.break_burst();
        if self.caret_col > 0 {
            self.caret_col -= 1;
        } else if self.caret_row > 0 {
            self.caret_row -= 1;
            self.caret_col = self.line_chars(self.caret_row);
        }
    }

    /// Moves the caret right, wrapping at the end of a line. A caret move ends a typing burst.
    pub fn move_right(&mut self) {
        self.history.break_burst();
        if self.caret_col < self.line_chars(self.caret_row) {
            self.caret_col += 1;
        } else if self.caret_row + 1 < self.line_count() {
            self.caret_row += 1;
            self.caret_col = 0;
        }
    }

    /// Opens completion for the caret's identifier prefix; Ctrl+Space also opens it with an
    /// empty prefix (the whole catalogue). Strings and comments suppress it even explicitly.
    fn open_completion(&mut self, catalogue: &[CompletionItem]) {
        let line = self.lines[self.caret_row].clone();
        if completion::caret_context(&line, self.caret_col) != CaretContext::Code {
            self.completion.dismiss();
            return;
        }
        let (prefix, start) = completion::identifier_prefix(&line, self.caret_col)
            .map(|prefix| (prefix.text, prefix.start_col))
            .unwrap_or_else(|| (String::new(), self.caret_col));
        self.completion.show(catalogue, &prefix, start);
    }

    /// Refreshes completion after typed text: opens on an identifier prefix, closes otherwise.
    ///
    /// This is the "automatically after typing an identifier prefix of at least one character"
    /// trigger. A character typed inside a string or comment (or after punctuation, or into a
    /// number) closes the popup instead of showing a stale filter.
    fn refresh_completion(&mut self, catalogue: &[CompletionItem]) {
        let line = self.lines[self.caret_row].clone();
        if completion::caret_context(&line, self.caret_col) != CaretContext::Code {
            self.completion.dismiss();
            return;
        }
        match completion::identifier_prefix(&line, self.caret_col) {
            Some(prefix) => {
                self.completion
                    .show(catalogue, &prefix.text, prefix.start_col);
            }
            None => self.completion.dismiss(),
        }
    }

    /// Accepts the highlighted entry, replacing exactly the identifier prefix.
    ///
    /// The acceptance is one atomic undo step: the snapshot is taken before the prefix is
    /// touched, so one Ctrl+Z restores the typed prefix and its caret and does not also undo the
    /// typing that produced it.
    fn accept_completion(&mut self) {
        let before = self.snapshot();
        if let Some((row, col)) =
            self.completion
                .accept(&mut self.lines, self.caret_row, self.caret_col)
        {
            if self.history.begin(EditKind::Atomic, Instant::now()) {
                self.history.push(before);
            }
            self.find.mark_dirty();
            self.caret_row = row;
            self.caret_col = col;
            self.selection = None;
            self.note_symbol_edit("");
            self.edits += 1;
        }
    }

    /// The one-line completion state the pane header publishes (and the capture records).
    ///
    /// It is a plain egui label in the header, so the typed prefix, the selected signature and
    /// the entry's origin are part of the window's accessibility tree even though the popup
    /// itself is painted.
    pub fn completion_summary(&self) -> Option<String> {
        let item = self.completion.selected()?;
        Some(format!(
            "Completion: {} -> {} [{}] {}/{}",
            self.completion.prefix(),
            item.detail,
            item.kind.tag(),
            self.completion.selected_index() + 1,
            self.completion.items().len()
        ))
    }

    /// EDT-009: opens the find bar with the caret as the search anchor.
    pub fn find_open(&mut self) {
        self.find.begin((self.caret_row, self.caret_col));
        self.find_refresh();
        self.history.break_burst();
    }

    /// EDT-009: closes the find bar and drops its highlights.
    pub fn find_close(&mut self) {
        self.find.close();
    }

    /// EDT-009: recomputes the cached matches when the buffer or the query changed.
    pub fn find_refresh(&mut self) {
        self.find
            .refresh(&self.lines, (self.caret_row, self.caret_col));
    }

    /// EDT-009: moves the highlight to the next match, wrapping at the end of the document.
    pub fn find_next(&mut self) -> bool {
        self.find_refresh();
        let Some(span) = self.find.select_next() else {
            return false;
        };
        self.reveal_find_match(span);
        true
    }

    /// EDT-009: moves the highlight to the previous match, wrapping at the start.
    pub fn find_previous(&mut self) -> bool {
        self.find_refresh();
        let Some(span) = self.find.select_previous() else {
            return false;
        };
        self.reveal_find_match(span);
        true
    }

    /// Moves the caret to the match the find bar is pointing at; the caret move ends a burst.
    fn reveal_find_match(&mut self, span: MatchSpan) {
        if span.row >= self.line_count() {
            return;
        }
        self.caret_row = span.row;
        self.caret_col = span.start.min(self.line_chars(span.row));
        self.selection = None;
        self.history.break_burst();
    }

    /// EDT-009: replaces the highlighted match as exactly one undo step.
    ///
    /// Returns true when a match was replaced. The replacement is literal text; the find bar's
    /// fields are single-line, so a replacement can never span lines.
    pub fn replace_current(&mut self) -> bool {
        self.find_refresh();
        let Some(span) = self.find.selected() else {
            return false;
        };
        let Some(line) = self.lines.get(span.row) else {
            return false;
        };
        let chars: Vec<char> = line.chars().collect();
        if span.start > span.end || span.end > chars.len() {
            return false;
        }
        let replacement = self.find.replacement.clone();
        // One snapshot for this replacement: Ctrl+Z restores the pre-replace buffer and caret.
        self.begin_edit(EditKind::Atomic);
        let mut rebuilt: String = chars[..span.start].iter().collect();
        rebuilt.push_str(&replacement);
        rebuilt.extend(chars[span.end..].iter());
        self.lines[span.row] = rebuilt;
        self.caret_row = span.row;
        self.caret_col = span.start + replacement.chars().count();
        self.selection = None;
        self.symbols_dirty = true;
        self.edits += 1;
        let caret = (self.caret_row, self.caret_col);
        self.find.reselect_from(caret);
        self.find_refresh();
        true
    }

    /// EDT-009: replaces every match and records the whole rewrite as one undo step.
    ///
    /// Returns the number of replacements. Each line is rebuilt from its pre-replace characters
    /// and the matches are applied left to right, so a replacement can never be matched again in
    /// the same pass: every match is rewritten exactly once.
    pub fn replace_all(&mut self) -> usize {
        self.find_refresh();
        let rows = self.find.by_row();
        let Some(first) = rows
            .values()
            .next()
            .and_then(|spans| spans.first())
            .copied()
        else {
            return 0;
        };
        let replacement = self.find.replacement.clone();
        // One snapshot for the whole operation: the undo step is the complete rewrite.
        self.begin_edit(EditKind::Atomic);
        let mut replaced = 0usize;
        for (row, spans) in rows {
            let Some(line) = self.lines.get(row) else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            let mut out = String::with_capacity(line.len());
            let mut cursor = 0usize;
            for span in spans {
                if span.start < cursor || span.end > chars.len() {
                    continue;
                }
                out.extend(chars[cursor..span.start].iter());
                out.push_str(&replacement);
                cursor = span.end;
                replaced += 1;
            }
            out.extend(chars[cursor..].iter());
            self.lines[row] = out;
        }
        self.caret_row = first.row;
        self.caret_col = first.start.min(self.line_chars(first.row));
        self.selection = None;
        self.symbols_dirty = true;
        self.edits += 1;
        let caret = (self.caret_row, self.caret_col);
        self.find.reselect_from(caret);
        self.find_refresh();
        replaced
    }

    /// EDT-009: the find bar's one-line state for the pane header (and the UIA capture).
    pub fn find_summary(&self) -> Option<String> {
        self.find.summary()
    }

    /// EDT-004: the innermost call the caret is inside, as its open parenthesis, callee name and
    /// the number of top-level commas between the two.
    ///
    /// The scan is bounded to the caret's line and the 20 lines above it, ignores parentheses
    /// inside string literals and `#` comments, and treats a closed call as no call. That bound
    /// keeps the per-pass cost independent of a 5 000-line document's size.
    fn call_context(&self) -> Option<((usize, usize), String, usize)> {
        // A caret inside a string or comment is inside text, not inside an argument list.
        if completion::caret_context(&self.lines[self.caret_row], self.caret_col) != CaretContext::Code
        {
            return None;
        }
        let start_row = self.caret_row.saturating_sub(20);
        let mut depth = 0usize;
        let mut open: Option<(usize, usize)> = None;
        'rows: for row in (start_row..=self.caret_row).rev() {
            let Some(line) = self.lines.get(row) else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            let mask = code_mask(line);
            let end = if row == self.caret_row {
                self.caret_col.min(chars.len())
            } else {
                chars.len()
            };
            for col in (0..end).rev() {
                if !mask.get(col).copied().unwrap_or(false) {
                    continue;
                }
                match chars[col] {
                    ')' => depth += 1,
                    '(' if depth == 0 => {
                        open = Some((row, col));
                        break 'rows;
                    }
                    '(' => depth -= 1,
                    _ => {}
                }
            }
        }
        let (open_row, open_col) = open?;
        let chars: Vec<char> = self.lines.get(open_row)?.chars().collect();
        let mut name_end = open_col;
        while name_end > 0 && chars[name_end - 1].is_whitespace() {
            name_end -= 1;
        }
        let mut name_start = name_end;
        while name_start > 0 && is_ident_continue(chars[name_start - 1]) {
            name_start -= 1;
        }
        if name_start == name_end || !is_ident_start(chars[name_start]) {
            return None;
        }
        let name: String = chars[name_start..name_end].iter().collect();
        // Count the commas that separate arguments: only depth-zero commas after the open paren.
        let mut commas = 0usize;
        let mut call_depth = 0usize;
        for row in open_row..=self.caret_row {
            let Some(line) = self.lines.get(row) else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            let mask = code_mask(line);
            let from = if row == open_row { open_col + 1 } else { 0 };
            let to = if row == self.caret_row {
                self.caret_col.min(chars.len())
            } else {
                chars.len()
            };
            for col in from..to {
                if !mask.get(col).copied().unwrap_or(false) {
                    continue;
                }
                match chars[col] {
                    '(' => call_depth += 1,
                    ')' => call_depth = call_depth.saturating_sub(1),
                    ',' if call_depth == 0 => commas += 1,
                    _ => {}
                }
            }
        }
        Some(((open_row, open_col), name, commas))
    }

    /// EDT-004: signature help for the call at the caret, sourced from the engine snapshot.
    ///
    /// The parameter names are the `EngineFunction.parameters` the engine's own registry
    /// published (and the summary is the registry's `summary`); the desktop has no signature
    /// table. `None` outside a call or for a name the engine does not register.
    pub fn signature_help(&self, engine: &crate::engine::EngineState) -> Option<SignatureHelp> {
        let (open, name, commas) = self.call_context()?;
        let function = engine.functions.iter().find(|function| function.name == name)?;
        let parameters = function.parameters.clone();
        let active = if parameters.is_empty() {
            0
        } else {
            commas.min(parameters.len() - 1)
        };
        Some(SignatureHelp {
            name,
            parameters,
            active,
            summary: function.summary.clone(),
            open,
        })
    }

    /// EDT-004: the signature help the last paint pass showed, for the pane header.
    pub fn signature_summary(&self) -> Option<String> {
        self.last_signature.as_ref().map(SignatureHelp::status)
    }

    /// EDT-004: dismisses the visible signature help for this call (Escape).
    ///
    /// The window calls this for an Escape press that reaches the window before the editor's
    /// own event loop; the editor's loop calls the same rule directly. Either path records the
    /// call's opening parenthesis, so only this call is suppressed and the next `(` shows help
    /// again.
    pub fn dismiss_signature(&mut self) {
        if let Some(help) = self.last_signature.take() {
            self.signature_dismissed = Some(help.open);
        }
    }

    /// The buffer position under a pointer position, clamped into the buffer.
    fn caret_at(
        &self,
        position: egui::Pos2,
        body: Rect,
        row_h: f32,
        text_left: f32,
        char_w: f32,
    ) -> (usize, usize) {
        let total = self.line_count().max(1);
        let row = row_at(body, self.scroll_y, row_h, total, position.y);
        let display_col = (((position.x - text_left) / char_w).max(0.0)) as usize;
        (row, self.char_column_at(row, display_col))
    }

    /// Places the caret under a pointer position and clears any selection.
    fn set_caret_from_pos(
        &mut self,
        position: egui::Pos2,
        body: Rect,
        row_h: f32,
        text_left: f32,
        char_w: f32,
    ) {
        let (row, col) = self.caret_at(position, body, row_h, text_left, char_w);
        self.caret_row = row;
        self.caret_col = col;
        self.selection = None;
    }

    /// Refreshes the syntax context from the live engine snapshot and the current buffer.
    ///
    /// The snapshot's `is_builtin` functions are the engine registry's own labels (the same
    /// capture completion offers), so the highlighter has no builtin table of its own. The
    /// snapshot is used directly rather than the deduplicated catalogue: a variable named like a
    /// builtin must not hide that builtin's colour. The symbol set is rediscovered only when the
    /// buffer's edit counter moved; the builtin comparison is a cheap per-pass scan.
    fn refresh_syntax(&mut self, engine: &crate::engine::EngineState) {
        let builtins: Vec<&str> = engine
            .functions
            .iter()
            .filter(|function| function.is_builtin)
            .map(|function| function.name.as_str())
            .collect();
        let unchanged = builtins.len() == self.syntax_builtins.len()
            && builtins
                .iter()
                .zip(self.syntax_builtins.iter())
                .all(|(live, cached)| *live == cached.as_str());
        if !unchanged {
            self.syntax.set_builtins(builtins.iter().copied());
            self.syntax_builtins = builtins.into_iter().map(str::to_string).collect();
        }
        if self.symbols_dirty {
            self.syntax
                .set_symbols(syntax::discover_symbols(&self.source()));
            self.symbols_dirty = false;
        }
    }

    /// Marks the cached symbol set stale when `text` or the caret's line mentions `symbol`.
    ///
    /// `symbol("name")` is line-local text in this language, so a line that never mentions the
    /// word cannot declare a symbol; an edit elsewhere leaves the cached set valid.
    fn note_symbol_edit(&mut self, text: &str) {
        if text.contains("symbol")
            || self
                .lines
                .get(self.caret_row)
                .is_some_and(|line| line.contains("symbol"))
        {
            self.symbols_dirty = true;
        }
    }

    /// Draws and edits the buffer inside `rect`.
    ///
    /// `catalogue` is the live completion catalogue for the active document (the window
    /// rebuilds it from the engine's registry and the last committed snapshot each pass);
    /// `engine` is the same capture the catalogue was built from, and supplies the highlighter's
    /// builtin names.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        catalogue: &[CompletionItem],
        engine: &crate::engine::EngineState,
    ) -> EditorMetrics {
        let started = Instant::now();
        let edits_before = self.edits;
        // EDT-009/EDT-016: the caret is only forced into the viewport when it actually moved (or
        // the buffer was edited). A wheel scroll that leaves the caret where it is must be able
        // to move the view; otherwise every scroll snaps straight back to the caret's row and a
        // 5 000-line document cannot be scrolled at all.
        let caret_before = (self.caret_row, self.caret_col);
        let font = FontId::monospace(13.0);
        let row_h = ui.text_style_height(&TextStyle::Monospace) + 3.0;
        let char_w = ui
            .painter()
            .layout_no_wrap("M".to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
            .max(1.0);
        let total = self.line_count();
        let digits = total.to_string().len().max(4);
        let gutter_w = 10.0 + digits as f32 * char_w;
        let text_left = rect.left() + gutter_w + 6.0;
        let max_scroll = (total as f32 * row_h - rect.height()).max(0.0);

        let id = surface_id();
        let response = ui.interact(rect, id, Sense::click_and_drag());
        if response.clicked() {
            response.request_focus();
        }
        let focused = ui.memory(|memory| memory.has_focus(id));
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.scroll_y = (self.scroll_y - scroll).clamp(0.0, max_scroll);
            }
        }

        if focused {
            let events = ui.input(|input| input.events.clone());
            // Ctrl+Space opens the popup explicitly, with the caret's prefix or the whole
            // catalogue when the prefix is empty. Some backends also deliver the space as a
            // Text event; that one is swallowed so the shortcut can never insert a space.
            let mut explicit = false;
            for event in &events {
                if let egui::Event::Key {
                    key: egui::Key::Space,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    if modifiers.command || modifiers.ctrl {
                        explicit = true;
                    }
                }
            }
            let mut swallow_space = explicit;
            if explicit {
                // Ctrl+Space is a non-typing action: it ends an open typing burst.
                self.history.break_burst();
                self.open_completion(catalogue);
            }
            for event in events {
                match event {
                    egui::Event::Text(text) => {
                        if swallow_space && text == " " {
                            swallow_space = false;
                            continue;
                        }
                        self.insert_str(&text);
                        self.refresh_completion(catalogue);
                    }
                    egui::Event::Paste(text) => {
                        // A paste is one atomic step: it never joins an open typing burst.
                        self.insert_text(&text, EditKind::Atomic);
                        self.completion.dismiss();
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        // Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y are the editor's own history
                        // shortcuts. They are read through the shared mapping so the window
                        // and `--gui-selftest` exercise the same rule.
                        if let Some(command) = history::command_for_shortcut(
                            matches!(key, egui::Key::Z),
                            matches!(key, egui::Key::Y),
                            modifiers.shift,
                            modifiers.command || modifiers.ctrl,
                        ) {
                            self.command(command);
                            self.completion.dismiss();
                            continue;
                        }
                        // Only keys the editor treats as actions end a typing burst. A letter
                        // key also arrives as a Key event, but its Text event is the typing
                        // edit: breaking on the Key would split every typed word.
                        match key {
                            egui::Key::Backspace => {
                                self.completion.dismiss();
                                self.backspace();
                            }
                            egui::Key::Delete => {
                                self.completion.dismiss();
                                self.delete_forward();
                            }
                            egui::Key::Enter => {
                                // A command or a newline both end the typing burst; Ctrl+Enter
                                // (Run) must not let the next typed character join it.
                                self.history.break_burst();
                                // Ctrl+Enter is the workbench's Run shortcut; it must not also
                                // insert a newline into the document, and it never accepts a
                                // completion.
                                if !(modifiers.command || modifiers.ctrl) {
                                    if self.completion.is_open() {
                                        self.accept_completion();
                                    } else {
                                        self.newline();
                                    }
                                }
                            }
                            egui::Key::Tab => {
                                self.handle_tab(modifiers.shift);
                            }
                            egui::Key::Escape => {
                                // Dismiss only: the buffer and the caret stay exactly as they
                                // were. A visible signature belongs to the call at the caret, so
                                // the dismissal is scoped to that one opening parenthesis.
                                self.history.break_burst();
                                self.completion.dismiss();
                                if let Some(help) = self.signature_help(engine) {
                                    self.signature_dismissed = Some(help.open);
                                    self.last_signature = None;
                                }
                            }
                            egui::Key::ArrowLeft => {
                                self.history.break_burst();
                                self.completion.dismiss();
                                let before = (self.caret_row, self.caret_col);
                                self.move_left();
                                if modifiers.shift {
                                    self.extend_selection_from(before);
                                } else {
                                    self.selection = None;
                                }
                            }
                            egui::Key::ArrowRight => {
                                self.history.break_burst();
                                self.completion.dismiss();
                                let before = (self.caret_row, self.caret_col);
                                self.move_right();
                                if modifiers.shift {
                                    self.extend_selection_from(before);
                                } else {
                                    self.selection = None;
                                }
                            }
                            egui::Key::ArrowUp => {
                                self.history.break_burst();
                                if self.completion.is_open() {
                                    self.completion.move_selection(-1);
                                } else {
                                    let before = (self.caret_row, self.caret_col);
                                    self.move_up();
                                    if modifiers.shift {
                                        self.extend_selection_from(before);
                                    } else {
                                        self.selection = None;
                                    }
                                }
                            }
                            egui::Key::ArrowDown => {
                                self.history.break_burst();
                                if self.completion.is_open() {
                                    self.completion.move_selection(1);
                                } else {
                                    let before = (self.caret_row, self.caret_col);
                                    self.move_down();
                                    if modifiers.shift {
                                        self.extend_selection_from(before);
                                    } else {
                                        self.selection = None;
                                    }
                                }
                            }
                            egui::Key::Home => {
                                self.history.break_burst();
                                self.completion.dismiss();
                                let before = (self.caret_row, self.caret_col);
                                self.caret_col = 0;
                                if modifiers.shift {
                                    self.extend_selection_from(before);
                                } else {
                                    self.selection = None;
                                }
                            }
                            egui::Key::End => {
                                self.history.break_burst();
                                self.completion.dismiss();
                                let before = (self.caret_row, self.caret_col);
                                self.caret_col = self.line_chars(self.caret_row);
                                if modifiers.shift {
                                    self.extend_selection_from(before);
                                } else {
                                    self.selection = None;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        if focused {
            // Keep the arrow keys for the caret. egui's focus system otherwise treats a bare
            // arrow as "move keyboard focus to the closest widget in that direction" before the
            // editor can consume it, which drops focus out of the editor after one Up/Down
            // (DEF-002). Tab is always locked too: EDT-008 makes Tab indent the selection (and
            // Shift+Tab outdent it), so egui must not use it for focus traversal. Escape is
            // locked as well: the editor uses it to dismiss the completion and signature popups
            // (EDT-004/EDT-009), and losing focus to egui's "surrender focus" rule would mean the
            // key never reaches the editor at all.
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        escape: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                    },
                );
            });
        }

        // An edit invalidates the diagnostic's coordinates: it names a line and column in the
        // text that was run. Drop it in this same pass so a deleted line cannot be decorated and
        // no marker is ever painted against text the failure never named.
        if self.edits != edits_before {
            self.diagnostic = None;
        }

        // Editing shrinks or grows the buffer (Backspace at a line start removes a line). The
        // loop above may have changed `lines`, so recompute the bounds this pass paints with;
        // the values captured at the top of the pass describe the pre-edit buffer and would let
        // the paint loop index a line that no longer exists (DEF-001).
        let total = self.line_count();
        let max_scroll = (total as f32 * row_h - rect.height()).max(0.0);

        // Resolve the diagnostic against the post-edit buffer. `range_for` refuses a line past
        // the buffer, so the decoration can never outlive the text it describes.
        let decoration: Option<(DiagnosticRange, String)> =
            self.diagnostic.as_ref().and_then(|diagnostic| {
                diagnostic_map::range_for(&self.lines, diagnostic.line, diagnostic.column)
                    .map(|range| (range, diagnostic.render()))
            });

        // A pointer click in the gutter marker jumps to the reported line and column; any other
        // pointer click positions the caret as before and dismisses the popup (the caret left the
        // prefix it was filtered for). A keyboard activation of this focused surface also reports
        // `clicked()`, but has no pointer position; it must not be mistaken for a click here —
        // Ctrl+Space is exactly such an activation.
        if response.clicked() {
            if let Some(position) = response.interact_pointer_pos() {
                self.completion.dismiss();
                self.history.break_burst();
                let row = row_at(rect, self.scroll_y, row_h, total, position.y);
                let on_marker = position.x < text_left
                    && decoration
                        .as_ref()
                        .is_some_and(|(range, _)| range.line == row + 1);
                if on_marker {
                    self.jump_to_diagnostic();
                } else if ui.input(|input| input.modifiers.shift) {
                    // Shift+click extends the selection from its fixed end (or from the old
                    // caret when none was selected).
                    let (row, col) = self.caret_at(position, rect, row_h, text_left, char_w);
                    self.select_to(row, col);
                } else {
                    self.set_caret_from_pos(position, rect, row_h, text_left, char_w);
                }
            }
        }

        // A pointer drag selects text: the press origin is the fixed end and the moving pointer
        // the caret. A plain click (no drag) positions the caret and clears the selection above.
        if response.drag_started() {
            if let Some(origin) = ui.input(|input| input.pointer.press_origin()) {
                let (row, col) = self.caret_at(origin, rect, row_h, text_left, char_w);
                self.completion.dismiss();
                self.selection = Some((row, col));
                self.caret_row = row;
                self.caret_col = col;
                self.history.break_burst();
            }
        }
        if response.dragged() {
            if let Some(position) = response.interact_pointer_pos() {
                let (row, col) = self.caret_at(position, rect, row_h, text_left, char_w);
                self.caret_row = row;
                self.caret_col = col;
                self.history.break_burst();
            }
        }
        if response.drag_stopped() && self.selection == Some((self.caret_row, self.caret_col)) {
            // A press that never moved past the drag threshold is not a selection.
            self.selection = None;
        }

        // Keep the caret inside the viewport when it moved or the text was edited. A scroll
        // gesture that did not move the caret is left alone (see `caret_before`).
        let caret_y = self.caret_row as f32 * row_h;
        let caret_moved = (self.caret_row, self.caret_col) != caret_before;
        if caret_moved || self.edits != edits_before {
            if caret_y < self.scroll_y {
                self.scroll_y = caret_y;
            } else if caret_y + row_h > self.scroll_y + rect.height() {
                self.scroll_y = caret_y + row_h - rect.height();
            }
        }
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll);

        let first_row = (self.scroll_y / row_h).floor().max(0.0) as usize;
        let visible_rows = ((rect.height() / row_h).ceil() as usize + 1).max(1);
        let last_row = (first_row + visible_rows).min(total);

        // Refresh after this pass's typing events, so a symbol declared by the character just
        // typed is painted in the same frame rather than one frame late.
        let syntax_started = Instant::now();
        self.refresh_syntax(engine);
        let syntax_ms = syntax_started.elapsed().as_secs_f64() * 1000.0;
        // EDT-009: recompute the find matches after this pass's edits and caret moves. `refresh`
        // is a no-op unless the query, a toggle or the buffer changed.
        self.find_refresh();
        // EDT-004: the signature help for the call at the caret, from the engine snapshot. An
        // Escape dismissal suppresses exactly the call it was pressed in.
        let call = self.call_context();
        let dismissed = self.signature_dismissed;
        self.last_signature = match (&call, dismissed) {
            (None, _) => {
                self.signature_dismissed = None;
                None
            }
            (Some((open, _, _)), Some(dismissed)) if *open == dismissed => None,
            _ => self.signature_help(engine),
        };

        // EDT-009: every match paints its own background; the highlighted one has the amber
        // highlight and underline so "current" is distinguishable from "also matched".
        let find_matches: &[MatchSpan] = self.find.matches();
        let find_selected = self.find.selected();

        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, EDITOR_BG);
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(gutter_w, rect.height())),
            0.0,
            GUTTER_BG,
        );

        // Current-line highlight.
        let row_top = rect.top() - self.scroll_y + self.caret_row as f32 * row_h;
        painter.rect_filled(
            Rect::from_min_size(pos2(rect.left(), row_top), vec2(rect.width(), row_h)),
            0.0,
            CURRENT_LINE,
        );

        let mut laid_out = 0usize;
        // EDT-007: resolve the bracket pair once for this pass. `bracket_match` is derived from
        // the caret alone, so every painted row sees the same pair.
        let bracket_match = self.bracket_match();
        let tab_width = self.preferences.indent_width.max(1);
        // The selection is painted under the text, one row at a time, so a multi-line range
        // reads as one continuous highlight and never covers a glyph.
        let selection = self.selection_range();
        let rows_started = Instant::now();
        for row in first_row..last_row {
            let y = rect.top() - self.scroll_y + row as f32 * row_h;
            if y + row_h < rect.top() || y > rect.bottom() {
                continue;
            }
            // EDT-009: paint every match on this row first, then the selection and the bracket
            // decoration above it, so the highlighted (current) match keeps its amber underline.
            for span in find_matches.iter().filter(|span| span.row == row) {
                let x0 = text_left + self.display_column(row, span.start) as f32 * char_w;
                let x1 = text_left + self.display_column(row, span.end) as f32 * char_w;
                let current = find_selected == Some(*span);
                painter.rect_filled(
                    Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + row_h - 1.0)),
                    1.0,
                    if current {
                        FIND_CURRENT_BG
                    } else {
                        FIND_MATCH_BG
                    },
                );
                if current {
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(x0, y + row_h - 2.0),
                            pos2(x1, y + row_h - 1.0),
                        ),
                        1.0,
                        FIND_CURRENT_MARK,
                    );
                }
            }
            if let Some((start, end)) = selection {
                if row >= start.0 && row <= end.0 {
                    let from = if row == start.0 { start.1 } else { 0 };
                    let to = if row == end.0 {
                        end.1
                    } else {
                        self.line_chars(row)
                    };
                    if to > from {
                        let from_x = text_left + self.display_column(row, from) as f32 * char_w;
                        let to_x = text_left + self.display_column(row, to) as f32 * char_w;
                        painter.rect_filled(
                            Rect::from_min_max(
                                pos2(from_x, y + 1.0),
                                pos2(to_x, y + row_h - 1.0),
                            ),
                            1.0,
                            SELECTION,
                        );
                    }
                }
            }
            // EDT-007: a matched bracket pair gets the teal cell background and underline; an
            // unmatched bracket gets the distinct red one. They paint under the glyph, so the
            // bracket character itself stays readable in the capture.
            if let Some(matched) = bracket_match {
                let paint_cell = |position: (usize, usize), background: Color32, line: Color32| {
                    if position.0 != row {
                        return;
                    }
                    let x = text_left + self.display_column(row, position.1) as f32 * char_w;
                    painter.rect_filled(
                        Rect::from_min_max(pos2(x, y + 1.0), pos2(x + char_w, y + row_h - 1.0)),
                        2.0,
                        background,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(x, y + row_h - 2.0),
                            pos2(x + char_w, y + row_h - 1.0),
                        ),
                        1.0,
                        line,
                    );
                };
                if matched.is_matched() {
                    paint_cell(matched.at, BRACKET_MATCH_BG, BRACKET_MATCH);
                    if let Some(mate) = matched.mate {
                        paint_cell(mate, BRACKET_MATCH_BG, BRACKET_MATCH);
                    }
                } else {
                    paint_cell(matched.at, BRACKET_UNMATCHED_BG, BRACKET_UNMATCHED);
                }
            }
            let line = &self.lines[row];
            let display_line = expand_tabs(line, tab_width);
            let job = syntax::job(&display_line, font.clone(), &syntax::DARK, &self.syntax);
            let galley = painter.layout_job(job);
            laid_out += 1;
            painter.galley(pos2(text_left, y), galley, syntax::DARK.plain);
            painter.text(
                pos2(rect.left() + gutter_w - 4.0, y),
                Align2::RIGHT_TOP,
                (row + 1).to_string(),
                FontId::monospace(11.0),
                GUTTER_TEXT,
            );
            if let Some((range, _)) = &decoration {
                if range.line == row + 1 {
                    // Underline the offending span, from the reported column to end-of-line.
                    // An empty span (column at EOL) still gets one cell so the failure is visible.
                    let x0 =
                        text_left + self.display_column(row, range.start_char) as f32 * char_w;
                    let cells = range.end_char.saturating_sub(range.start_char).max(1);
                    let x1 = x0 + cells as f32 * char_w;
                    painter.rect_filled(
                        Rect::from_min_max(pos2(x0, y + row_h - 3.0), pos2(x1, y + row_h - 1.0)),
                        1.0,
                        DIAGNOSTIC_UNDERLINE,
                    );
                    // A distinct gutter marker on the same line: a solid bar at the gutter edge,
                    // clear of the right-aligned line number.
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(rect.left() + 2.0, y + 2.0),
                            pos2(rect.left() + 6.0, y + row_h - 2.0),
                        ),
                        1.0,
                        DIAGNOSTIC_MARKER,
                    );
                }
            }
        }
        let rows_ms = rows_started.elapsed().as_secs_f64() * 1000.0;

        // Hovering the underlined line (or its gutter marker) shows the diagnostic message. This
        // is a tooltip attached to the pointer, not a widget, so it cannot steal the editor's
        // click or its keyboard focus.
        if response.hovered() {
            if let (Some((range, message)), Some(pointer)) = (
                decoration.as_ref(),
                ui.input(|input| input.pointer.hover_pos()),
            ) {
                let row = row_at(rect, self.scroll_y, row_h, total, pointer.y);
                let row_top = rect.top() - self.scroll_y + row as f32 * row_h;
                let on_decorated_line =
                    range.line == row + 1 && pointer.y >= row_top && pointer.y < row_top + row_h;
                if on_decorated_line {
                    let _ = egui::Tooltip::always_open(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Id::unique("editor_diagnostic_tooltip"),
                        egui::PopupAnchor::Pointer,
                    )
                    .at_pointer()
                    .show(|ui| {
                        ui.set_max_width(520.0);
                        ui.label(RichText::new(message).monospace());
                    });
                }
            }
        }

        if focused {
            let caret_x =
                text_left + self.display_column(self.caret_row, self.caret_col) as f32 * char_w;
            let caret_y = rect.top() - self.scroll_y + self.caret_row as f32 * row_h;
            painter.line_segment(
                [
                    pos2(caret_x, caret_y + 1.0),
                    pos2(caret_x, caret_y + row_h - 1.0),
                ],
                Stroke::new(1.5, CARET),
            );
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(0x4e, 0xc9, 0xb0)),
                egui::StrokeKind::Inside,
            );
        }

        // The completion popup paints last, above the text and the caret. It is a pure painter
        // overlay: no widget, no interaction, no focus, so it can never block typing or swallow
        // a click. It is anchored one row BELOW the caret line (or, when that side has no room,
        // above it), so it does not cover the line being typed on.
        self.paint_completion(ui, rect, text_left, row_h, char_w);
        // EDT-004: the signature popup paints after the completion popup (the two never appear
        // together: typing an identifier closes completion, and `(` then opens signature help).
        if let Some(help) = self.last_signature.as_ref() {
            paint_signature(
                ui,
                rect,
                text_left,
                row_h,
                char_w,
                self.caret_row,
                self.scroll_y,
                help,
            );
        }
        #[cfg(test)]
        eprintln!(
            "[dbg] editor end open={} items={} pass={}",
            self.completion.is_open(),
            self.completion.items().len(),
            ui.ctx().cumulative_pass_nr()
        );

        let (line, column) = self.line_col_1based();
        let label = format!(
            "Script editor, {} lines, caret line {} column {}",
            total, line, column
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::MultilineTextInput, true, label.clone())
        });

        self.metrics = EditorMetrics {
            first_row,
            visible_rows,
            laid_out_rows: laid_out,
            row_h,
            char_w,
            gutter_w,
            scroll_y: self.scroll_y,
            syntax_ms,
            rows_ms,
        };
        self.last_ui_ms = started.elapsed().as_secs_f64() * 1000.0;
        self.metrics
    }

    /// Paints the completion popup over the editor surface.
    ///
    /// The popup is painter-only on purpose: it owns no widget, requests no focus and senses no
    /// pointer, so typing continues to reach the editor and no click is swallowed. Placement
    /// prefers the free space below the caret row and falls back to the free space above it;
    /// the number of rows is capped by that free space, so the popup never covers the caret
    /// line unless the pane is shorter than one row (in which case nothing is painted).
    fn paint_completion(&self, ui: &Ui, rect: Rect, text_left: f32, row_h: f32, char_w: f32) {
        let items = self.completion.items();
        if !self.completion.is_open() || items.is_empty() {
            return;
        }
        let selected = self.completion.selected_index();
        let font = FontId::monospace(12.0);
        let detail_font = FontId::monospace(11.0);
        let label_char_w = ui
            .painter()
            .layout_no_wrap("M".to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
            .max(1.0);
        let detail_char_w = ui
            .painter()
            .layout_no_wrap("M".to_owned(), detail_font.clone(), Color32::WHITE)
            .size()
            .x
            .max(1.0);
        let label_cols = items
            .iter()
            .map(|item| item.label.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let detail_cols = items
            .iter()
            .map(|item| item.detail.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let padding = 8.0;
        let column_gap = 14.0;
        let wanted = label_cols * label_char_w
            + column_gap
            + detail_cols * detail_char_w
            + padding * 2.0
            + 4.0;
        let max_width = (rect.width() - 8.0).max(120.0);
        let width = wanted.clamp(180.0f32.min(max_width), max_width);
        let height_for = |rows: usize| rows as f32 * COMPLETION_ROW_H + 6.0;

        let caret_top = rect.top() - self.scroll_y + self.caret_row as f32 * row_h;
        let below_top = caret_top + row_h + 3.0;
        let above_top = rect.top() + 4.0;
        let below_space = (rect.bottom() - 4.0 - below_top).max(0.0);
        let above_space = (caret_top - 3.0 - above_top).max(0.0);
        let place_below = below_space >= above_space;
        let space = if place_below {
            below_space
        } else {
            above_space
        };
        let capacity = ((space - 6.0) / COMPLETION_ROW_H).floor().max(0.0) as usize;
        let visible = items.len().min(COMPLETION_MAX_ROWS).min(capacity);
        if visible == 0 {
            return;
        }
        let height = height_for(visible);
        let y = if place_below {
            below_top
        } else {
            caret_top - height - 3.0
        };
        let caret_x =
            text_left + self.display_column(self.caret_row, self.caret_col) as f32 * char_w;
        let mut x = caret_x;
        let right_limit = rect.right() - 4.0 - width;
        if x > right_limit {
            x = right_limit;
        }
        x = x.max(rect.left() + 4.0);

        // Scroll the visible window so the highlighted row stays inside it.
        let first = if selected < visible {
            0
        } else {
            (selected + 1 - visible).min(items.len() - visible)
        };

        let painter = ui.painter().with_clip_rect(rect);
        let popup = Rect::from_min_size(pos2(x, y), vec2(width, height));
        painter.rect_filled(popup, 3.0, COMPLETION_BG);
        painter.rect_stroke(
            popup,
            3.0,
            Stroke::new(1.0, COMPLETION_BORDER),
            egui::StrokeKind::Inside,
        );
        for (offset, item) in items[first..first + visible].iter().enumerate() {
            let index = first + offset;
            let row_top = popup.top() + 3.0 + offset as f32 * COMPLETION_ROW_H;
            let row = Rect::from_min_size(
                pos2(popup.left() + 1.0, row_top),
                vec2(popup.width() - 2.0, COMPLETION_ROW_H),
            );
            if index == selected {
                painter.rect_filled(row, 2.0, COMPLETION_SELECTED);
            }
            let accent = completion_colour(item.kind);
            // A two-pixel kind bar: builtins keep the highlighter's blue, user functions and
            // variables get their own colour, keywords the highlighter's keyword colour.
            painter.rect_filled(
                Rect::from_min_size(
                    pos2(row.left() + 3.0, row_top + 3.0),
                    vec2(2.0, COMPLETION_ROW_H - 6.0),
                ),
                1.0,
                accent,
            );
            painter.text(
                pos2(popup.left() + padding, row_top + 2.0),
                Align2::LEFT_TOP,
                &item.label,
                font.clone(),
                accent,
            );
            painter.text(
                pos2(
                    popup.left() + padding + label_cols * label_char_w + column_gap,
                    row_top + 3.0,
                ),
                Align2::LEFT_TOP,
                &item.detail,
                detail_font.clone(),
                COMPLETION_DETAIL,
            );
        }
    }
}

/// The colour and kind bar one completion entry is painted with.
fn completion_colour(kind: CompletionKind) -> Color32 {
    match kind {
        CompletionKind::Keyword => syntax::DARK.keyword,
        CompletionKind::Builtin => syntax::DARK.builtin,
        CompletionKind::Function => COMPLETION_FUNCTION,
        CompletionKind::Variable => COMPLETION_VARIABLE,
    }
}

/// EDT-004: paints the signature popup above or below the caret row.
///
/// The popup is painter-only, like the completion popup: no widget, no focus, no pointer sense,
/// so it can never block typing. The active parameter is painted in the amber accent with an
/// underline; the status line names it with its engine-declared name, and the registry's summary
/// is shown (truncated to the popup) when the engine published one.
#[allow(clippy::too_many_arguments)]
fn paint_signature(
    ui: &Ui,
    rect: Rect,
    text_left: f32,
    row_h: f32,
    char_w: f32,
    caret_row: usize,
    scroll_y: f32,
    help: &SignatureHelp,
) {
    let font = FontId::monospace(12.0);
    let detail_font = FontId::monospace(10.5);
    let label_char_w = ui
        .painter()
        .layout_no_wrap("M".to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x
        .max(1.0);
    let detail_char_w = ui
        .painter()
        .layout_no_wrap("M".to_owned(), detail_font.clone(), Color32::WHITE)
        .size()
        .x
        .max(1.0);
    let padding = 8.0;
    let signature = help.signature();
    let status = match help.active_parameter() {
        Some(parameter) => format!(
            "parameter {} of {}: {}",
            help.active + 1,
            help.parameters.len(),
            parameter
        ),
        None => "no parameters".to_string(),
    };
    let summary = help.summary.clone().unwrap_or_default();
    let max_width = (rect.width() - 8.0).max(160.0);
    let signature_width = signature.chars().count() as f32 * label_char_w;
    let status_width = status.chars().count() as f32 * detail_char_w;
    let summary_width = summary.chars().count() as f32 * detail_char_w;
    let wanted = signature_width.max(status_width).max(summary_width) + padding * 2.0 + 4.0;
    let width = wanted.clamp(220.0f32.min(max_width), max_width);
    let height = if summary.is_empty() { 36.0 } else { 50.0 };

    let caret_top = rect.top() - scroll_y + caret_row as f32 * row_h;
    let below_top = caret_top + row_h + 3.0;
    let above_top = rect.top() + 4.0;
    let below_space = (rect.bottom() - 4.0 - below_top).max(0.0);
    let above_space = (caret_top - 3.0 - above_top).max(0.0);
    let place_below = below_space >= height || below_space >= above_space;
    let y = if place_below {
        below_top.min(rect.bottom() - height - 2.0).max(rect.top() + 2.0)
    } else {
        (caret_top - height - 3.0)
            .max(rect.top() + 2.0)
            .min(rect.bottom() - height - 2.0)
    };
    let caret_x = text_left + help.open.1 as f32 * char_w;
    let mut x = caret_x;
    let right_limit = rect.right() - 4.0 - width;
    if x > right_limit {
        x = right_limit;
    }
    x = x.max(rect.left() + 4.0);

    let painter = ui.painter().with_clip_rect(rect);
    let popup = Rect::from_min_size(pos2(x, y), vec2(width, height));
    painter.rect_filled(popup, 3.0, SIGNATURE_BG);
    painter.rect_stroke(
        popup,
        3.0,
        Stroke::new(1.0, SIGNATURE_BORDER),
        egui::StrokeKind::Inside,
    );

    let mut before = format!("{}(", help.name);
    for parameter in help.parameters.iter().take(help.active) {
        before.push_str(parameter);
        before.push_str(", ");
    }
    let active_text = help.active_parameter().unwrap_or("").to_string();
    let mut after = String::new();
    for parameter in help.parameters.iter().skip(help.active + 1) {
        after.push_str(", ");
        after.push_str(parameter);
    }
    after.push(')');

    let text_y = popup.top() + 5.0;
    let mut text_x = popup.left() + padding;
    painter.text(
        pos2(text_x, text_y),
        Align2::LEFT_TOP,
        &before,
        font.clone(),
        SIGNATURE_TEXT,
    );
    text_x += before.chars().count() as f32 * label_char_w;
    if !active_text.is_empty() {
        painter.text(
            pos2(text_x, text_y),
            Align2::LEFT_TOP,
            &active_text,
            font.clone(),
            SIGNATURE_ACTIVE,
        );
        let active_w = active_text.chars().count() as f32 * label_char_w;
        painter.rect_filled(
            Rect::from_min_max(
                pos2(text_x, text_y + 15.0),
                pos2(text_x + active_w, text_y + 16.0),
            ),
            0.0,
            SIGNATURE_ACTIVE,
        );
        text_x += active_w;
    }
    painter.text(
        pos2(text_x, text_y),
        Align2::LEFT_TOP,
        &after,
        font,
        SIGNATURE_TEXT,
    );
    painter.text(
        pos2(popup.left() + padding, text_y + 19.0),
        Align2::LEFT_TOP,
        &status,
        detail_font.clone(),
        COMPLETION_DETAIL,
    );
    if !summary.is_empty() {
        let max_chars = (((width - padding * 2.0) / detail_char_w).floor() as usize).max(8);
        let mut shortened: String = summary.chars().take(max_chars).collect();
        if summary.chars().count() > max_chars {
            shortened.push('…');
        }
        painter.text(
            pos2(popup.left() + padding, text_y + 33.0),
            Align2::LEFT_TOP,
            shortened,
            detail_font,
            COMPLETION_DETAIL,
        );
    }
}

/// True for the three bracket pairs EDT-007 matches.
fn is_bracket(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}')
}

/// The closing partner of an opening bracket, if `ch` is one.
fn closing_bracket(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

/// The opening partner of a closing bracket, if `ch` is one.
fn opening_bracket(ch: char) -> Option<char> {
    match ch {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

/// The closing character of a quote pair, if `ch` starts one.
fn quote_pair(ch: char) -> Option<char> {
    match ch {
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

/// True when `ch` can be the closing half of an auto-closed pair.
fn is_closing(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}' | '"' | '\'')
}

/// The bracket that closes `ch` (or opens it), for the status line.
fn matching_partner(ch: char) -> char {
    closing_bracket(ch)
        .or_else(|| opening_bracket(ch))
        .unwrap_or(ch)
}

/// True at the first character of a Lovelace identifier.
fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// True inside a Lovelace identifier after its first character.
fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// True for each character that is code: outside a string literal and outside a `#` comment.
///
/// The language's strings and comments are line-local, so a per-line mask is exact. Signature
/// help uses it so a parenthesis written inside `"..."` or after `#` is never read as a call.
fn code_mask(line: &str) -> Vec<bool> {
    let chars: Vec<char> = line.chars().collect();
    let mut mask = vec![true; chars.len()];
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in chars.iter().enumerate() {
        if in_string {
            mask[index] = false;
            if escaped {
                escaped = false;
            } else if *ch == '\\' {
                escaped = true;
            } else if *ch == '"' {
                in_string = false;
            }
            continue;
        }
        if *ch == '"' {
            mask[index] = false;
            in_string = true;
        } else if *ch == '#' {
            for slot in mask.iter_mut().skip(index) {
                *slot = false;
            }
            break;
        }
    }
    mask
}

/// How many leading characters one outdent removes: one tab, or up to `indent_width` spaces.
fn outdent_amount(line: &str, indent_width: usize) -> usize {
    let mut removed = 0usize;
    for ch in line.chars() {
        match ch {
            '\t' if removed == 0 => return 1,
            ' ' if removed < indent_width => removed += 1,
            _ => break,
        }
    }
    removed
}

/// Expands tabs to `width` spaces for layout, leaving every other character unchanged.
fn expand_tabs(line: &str, width: usize) -> String {
    let width = width.max(1);
    let mut expanded = String::with_capacity(line.len());
    for ch in line.chars() {
        if ch == '\t' {
            expanded.push_str(&" ".repeat(width));
        } else {
            expanded.push(ch);
        }
    }
    expanded
}

/// The byte index of a char column in one line.
fn byte_index(line: &str, char_col: usize) -> usize {
    line.char_indices()
        .nth(char_col)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

/// The 0-based buffer row under a screen `y` position, clamped into a one-line-minimum buffer.
fn row_at(rect: Rect, scroll_y: f32, row_h: f32, total: usize, y: f32) -> usize {
    (((y - rect.top() + scroll_y) / row_h).floor() as isize).clamp(0, total.max(1) as isize - 1)
        as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::build_catalogue;
    use crate::engine::{EngineFunction, EngineState};
    use std::time::Duration;

    fn builtin(name: &str, parameters: &[&str]) -> EngineFunction {
        EngineFunction {
            name: name.to_string(),
            parameters: parameters.iter().map(|p| p.to_string()).collect(),
            is_builtin: true,
            plugin: None,
            summary: None,
        }
    }

    fn caret(editor: &TextEditor) -> (usize, usize) {
        (editor.caret_row, editor.caret_col)
    }

    #[test]
    fn syntax_context_refreshes_builtins_from_the_engine_and_symbols_from_the_buffer() {
        let state = crate::engine::EngineState {
            variables: Vec::new(),
            functions: vec![builtin("squeeze", &["a"])],
        };
        let mut editor = TextEditor::from_source("t = symbol(\"t\")");
        editor.refresh_syntax(&state);
        assert!(
            editor.syntax.is_builtin("squeeze"),
            "the engine's builtin is used"
        );
        assert!(
            editor.syntax.is_symbol("t"),
            "the buffer's symbol is discovered"
        );
        assert!(
            !editor.syntax.is_builtin("round"),
            "a name absent from the engine stays plain"
        );

        // A buffer replacement invalidates the cached symbol set.
        editor.load("u = symbol(\"u\")");
        editor.refresh_syntax(&state);
        assert!(editor.syntax.is_symbol("u"));
        assert!(!editor.syntax.is_symbol("t"));

        // A changed engine snapshot refreshes the builtin set without a buffer edit.
        let state = crate::engine::EngineState {
            variables: Vec::new(),
            functions: vec![builtin("round", &["x"])],
        };
        editor.refresh_syntax(&state);
        assert!(editor.syntax.is_builtin("round"));
        assert!(!editor.syntax.is_builtin("squeeze"));
    }

    #[test]
    fn typing_burst_is_one_step_and_undo_redo_restore_the_caret() {
        let mut editor = TextEditor::from_source("");
        editor.insert_str("a");
        editor.insert_str("b");
        editor.insert_str("c");

        assert_eq!(editor.source(), "abc");
        assert_eq!(caret(&editor), (0, 3));
        assert_eq!(
            editor.history.undo_steps(),
            1,
            "three typed characters in one burst are one undo step"
        );

        assert!(editor.undo());
        assert_eq!(editor.source(), "", "Ctrl+Z restores the empty buffer");
        assert_eq!(caret(&editor), (0, 0), "and the caret at 0");

        assert!(editor.redo());
        assert_eq!(editor.source(), "abc", "Ctrl+Y restores the burst");
        assert_eq!(caret(&editor), (0, 3), "and the caret after the text");
    }

    #[test]
    fn a_pause_starts_a_new_typing_step() {
        let mut editor = TextEditor::from_source("");
        let start = Instant::now();
        editor.insert_text_at("ab", EditKind::Typing, start);
        editor.insert_text_at("cd", EditKind::Typing, start + Duration::from_millis(300));
        assert_eq!(
            editor.history.undo_steps(),
            1,
            "characters within the burst gap coalesce"
        );

        let after_pause = start
            + Duration::from_millis(300)
            + history::TYPING_BURST_GAP
            + Duration::from_millis(1);
        editor.insert_text_at("ef", EditKind::Typing, after_pause);
        assert_eq!(
            editor.history.undo_steps(),
            2,
            "a pause longer than the gap starts a new step"
        );
        assert!(editor.undo());
        assert_eq!(editor.source(), "abcd");
        assert_eq!(caret(&editor), (0, 4));
        assert!(editor.undo());
        assert_eq!(editor.source(), "");
    }

    #[test]
    fn a_non_typing_action_starts_a_new_step() {
        let mut editor = TextEditor::from_source("");
        editor.insert_str("ab");
        editor.newline();
        editor.insert_str("cd");
        assert_eq!(editor.source(), "ab\ncd");
        assert_eq!(editor.history.undo_steps(), 3);

        assert!(editor.undo());
        assert_eq!(editor.source(), "ab\n", "the second burst is its own step");
        assert_eq!(caret(&editor), (1, 0));
        assert!(editor.undo());
        assert_eq!(editor.source(), "ab");
        assert_eq!(caret(&editor), (0, 2));
        assert!(editor.undo());
        assert_eq!(editor.source(), "");
    }

    #[test]
    fn backspace_line_join_undo_restores_the_caret_on_line_two() {
        let mut editor = TextEditor::from_source("a\nb");
        editor.set_caret(1, 0);
        editor.backspace();
        assert_eq!(editor.source(), "ab");
        assert_eq!(caret(&editor), (0, 1));

        assert!(editor.undo());
        assert_eq!(editor.source(), "a\nb", "the joined newline is restored");
        assert_eq!(caret(&editor), (1, 0), "the caret is back on line 2");
        assert_eq!(editor.line_col_1based(), (2, 1));
    }

    #[test]
    fn a_caret_move_between_typing_runs_starts_a_new_step() {
        let mut editor = TextEditor::from_source("");
        editor.insert_str("ab");
        editor.move_left();
        editor.insert_str("X");
        assert_eq!(editor.source(), "aXb");
        assert_eq!(caret(&editor), (0, 2));
        assert_eq!(editor.history.undo_steps(), 2);

        assert!(editor.undo());
        assert_eq!(editor.source(), "ab", "the second run is its own step");
        assert_eq!(caret(&editor), (0, 1), "the moved caret is restored");
        assert!(editor.undo());
        assert_eq!(editor.source(), "");
    }

    #[test]
    fn deleting_a_selection_undoes_to_the_text_and_the_selection() {
        let mut editor = TextEditor::from_source("abcdef");
        editor.set_caret(0, 1);
        editor.select_to(0, 4);
        assert_eq!(editor.selected_text().as_deref(), Some("bcd"));

        editor.delete_forward();
        assert_eq!(editor.source(), "aef");
        assert_eq!(caret(&editor), (0, 1));
        assert!(editor.selection_range().is_none());

        assert!(editor.undo());
        assert_eq!(editor.source(), "abcdef", "the deleted text is restored");
        assert_eq!(
            editor.selection_range(),
            Some(((0, 1), (0, 4))),
            "the selection is restored with the text"
        );
        assert_eq!(editor.selected_text().as_deref(), Some("bcd"));
        assert_eq!(caret(&editor), (0, 4));
    }

    /// EDT-014 (2): an astral character is three selection chars but six bytes and four UTF-16
    /// units. The byte range must slice exactly `a😀b`, and the caret readout must be the UTF-16
    /// column (5, 1-based), not the 3-char editor cell.
    #[test]
    fn astral_selection_converts_to_exact_bytes_and_the_caret_utf16_column() {
        let mut editor = TextEditor::from_source("a\u{1f600}b");
        editor.select_to(0, 3); // the whole line in the editor's char cells
        assert_eq!(
            editor.selection_range(),
            Some(((0, 0), (0, 3))),
            "the selection is stored in char columns: 'a', the emoji and 'b'"
        );

        let (start, end) = editor.selection_bytes().expect("a non-empty selection");
        assert_eq!(
            (start, end),
            (0, 6),
            "'a' is 1 byte, U+1F600 is 4 bytes and 'b' is 1 byte"
        );
        assert_eq!(&editor.source()[start..end], "a\u{1f600}b");
        assert_eq!(editor.selected_text().as_deref(), Some("a\u{1f600}b"));

        // The caret sits after the selection, so it is four UTF-16 units into the line.
        assert_eq!(editor.caret_column_utf16(), 4);
        assert_eq!(
            editor.line_col_1based(),
            (1, 5),
            "the readout column is 1-based UTF-16, not the 3-char caret column"
        );
    }

    /// EDT-014: a combining mark is a second `char` in the same grapheme cluster. Selecting the
    /// cluster must cover its full three UTF-8 bytes, not only the base character's one byte.
    #[test]
    fn a_combining_mark_selection_covers_the_whole_clusters_bytes() {
        let mut editor = TextEditor::from_source("e\u{0301} = 1");
        editor.select_to(0, 2); // 'e' plus U+0301: two chars, one grapheme on screen
        let (start, end) = editor.selection_bytes().expect("a non-empty selection");
        assert_eq!((start, end), (0, 3), "'e' is 1 byte and U+0301 is 2 bytes");
        assert_eq!(&editor.source()[start..end], "e\u{0301}");
        assert_eq!(
            editor.caret_column_utf16(),
            2,
            "both code points are one UTF-16 unit, so the caret is at unit 2"
        );
    }

    #[test]
    fn a_multiline_paste_is_one_undo_step() {
        let mut editor = TextEditor::from_source("x");
        editor.set_caret(0, 1);
        // `Event::Paste` calls exactly this atomic insertion.
        editor.insert_text("a\nb", EditKind::Atomic);
        assert_eq!(editor.source(), "xa\nb");
        assert_eq!(caret(&editor), (1, 1));
        assert_eq!(editor.history.undo_steps(), 1);

        assert!(editor.undo());
        assert_eq!(editor.source(), "x", "the whole paste is one step");
        assert_eq!(caret(&editor), (0, 1));
    }

    #[test]
    fn a_completion_insertion_is_its_own_undo_step() {
        let mut editor = TextEditor::from_source("");
        let catalogue = build_catalogue(&[], &[builtin("sqrt", &["x"])], &[]);
        editor.insert_str("sq");
        assert!(editor.completion.show(&catalogue, "sq", 0));

        editor.accept_completion();
        assert_eq!(editor.source(), "sqrt");
        assert_eq!(caret(&editor), (0, 4));
        assert!(
            !editor.completion.is_open(),
            "acceptance dismisses the popup"
        );

        assert!(editor.undo());
        assert_eq!(
            editor.source(),
            "sq",
            "one undo removes the insertion without the typing that produced the prefix"
        );
        assert_eq!(caret(&editor), (0, 2));
        assert!(!editor.completion.is_open());

        assert!(editor.undo());
        assert_eq!(editor.source(), "", "the typing is the next step back");
    }

    #[test]
    fn each_editor_has_its_own_history() {
        let mut first = TextEditor::from_source("");
        let mut second = TextEditor::from_source("");
        first.insert_str("alpha");
        second.insert_str("beta");

        assert!(first.undo());
        assert_eq!(first.source(), "");
        assert_eq!(
            second.source(),
            "beta",
            "an undo in one document never changes another document"
        );
        assert!(second.undo());
        assert_eq!(second.source(), "");
        assert!(
            !second.undo(),
            "the other document's history was never shared"
        );
    }

    /// DOC-002: a CRLF file opens with its `\r` stripped for display and closes with CRLF again,
    /// so an unmodified buffer is the file's own text and an edit adds CRLF, not LF.
    #[test]
    fn a_crlf_buffer_round_trips_and_new_lines_use_crlf() {
        let source = "a = 1\r\nb = 2\r\n";
        let mut editor = TextEditor::from_source_with_newline(source, NewlineStyle::CrLf);
        assert_eq!(
            editor.lines,
            vec!["a = 1", "b = 2", ""],
            "no stray \\r is a cell"
        );
        assert_eq!(editor.line_chars(0), 5, "the caret sees five characters");
        assert_eq!(editor.source(), source, "an untouched buffer is byte-equal");

        editor.set_caret(0, 5);
        editor.newline();
        editor.insert_str("c = 3");
        assert_eq!(
            editor.source(),
            "a = 1\r\nc = 3\r\nb = 2\r\n",
            "a new line uses the document's own ending"
        );
    }

    /// DOC-002: an LF file stays LF when it is edited; the editor never normalises to a platform
    /// default and never rewrites the other style.
    #[test]
    fn an_lf_buffer_writes_new_lines_with_lf() {
        let mut editor = TextEditor::from_source_with_newline("a = 1\nb = 2\n", NewlineStyle::Lf);
        assert_eq!(editor.source(), "a = 1\nb = 2\n");
        editor.set_caret(0, 5);
        editor.newline();
        editor.insert_str("c = 3");
        assert_eq!(editor.source(), "a = 1\nc = 3\nb = 2\n");
    }

    /// The caret and the readout ignore the stripped line ending: a CRLF document's line 1 has
    /// the same character count as the same line in an LF document.
    #[test]
    fn a_crlf_line_has_no_phantom_column_at_its_end() {
        let editor = TextEditor::from_source_with_newline("ab\r\ncd\r\n", NewlineStyle::CrLf);
        assert_eq!(editor.line_chars(0), 2);
        let mut editor = editor;
        editor.set_caret(0, 99);
        assert_eq!(caret(&editor), (0, 2), "the caret clamps before the ending");
        assert_eq!(editor.line_col_1based(), (1, 3));
    }

    /// EDT-007: nested pairs of the same kind and brackets of another kind are skipped, and the
    /// caret is adjacent to a bracket from either side.
    #[test]
    fn nested_brackets_match_the_enclosing_pair_and_keep_the_exact_positions() {
        let mut editor = TextEditor::from_source("a(b[c]d)e");
        editor.set_caret(0, 1); // immediately before '('
        assert_eq!(
            editor.bracket_match(),
            Some(BracketMatch {
                bracket: '(',
                at: (0, 1),
                mate: Some((0, 7)),
            })
        );
        assert_eq!(
            editor.bracket_status().as_deref(),
            Some("Bracket ( at 1:2 matched by ) at 1:8")
        );

        // The caret immediately after ')' names the same pair.
        editor.set_caret(0, 8);
        assert_eq!(
            editor.bracket_match(),
            Some(BracketMatch {
                bracket: ')',
                at: (0, 7),
                mate: Some((0, 1)),
            })
        );

        // A same-kind nested pair: the inner '(' must not pair with the outer ')'.
        let mut nested = TextEditor::from_source("f(g(h), i)");
        nested.set_caret(0, 3);
        assert_eq!(
            nested.bracket_match(),
            Some(BracketMatch {
                bracket: '(',
                at: (0, 3),
                mate: Some((0, 5)),
            })
        );
        nested.set_caret(0, 1);
        assert_eq!(
            nested.bracket_match(),
            Some(BracketMatch {
                bracket: '(',
                at: (0, 1),
                mate: Some((0, 9)),
            })
        );
    }

    /// EDT-007: a bracket with no partner is a distinct state, not a pair with a guessed mate.
    #[test]
    fn an_unmatched_bracket_reports_the_distinct_unmatched_state() {
        let mut editor = TextEditor::from_source("(a(b)");
        editor.set_caret(0, 0);
        assert_eq!(
            editor.bracket_match(),
            Some(BracketMatch {
                bracket: '(',
                at: (0, 0),
                mate: None,
            })
        );
        assert_eq!(
            editor.bracket_status().as_deref(),
            Some("Unmatched ( at 1:1")
        );

        // The inner pair is still matched; only the outer bracket is unmatched.
        editor.set_caret(0, 2);
        assert_eq!(
            editor.bracket_match(),
            Some(BracketMatch {
                bracket: '(',
                at: (0, 2),
                mate: Some((0, 4)),
            })
        );

        // A closing bracket with no opener is unmatched too.
        let mut closing = TextEditor::from_source("a]b");
        closing.set_caret(0, 2);
        assert_eq!(
            closing.bracket_status().as_deref(),
            Some("Unmatched ] at 1:2")
        );
        assert!(!closing.jump_to_matching_bracket());
        assert_eq!(caret(&closing), (0, 2), "an unmatched jump must not move");
    }

    /// EDT-010: go to matching bracket moves the caret to the exact mate and back.
    #[test]
    fn go_to_matching_bracket_moves_the_caret_to_the_exact_mate() {
        let mut editor = TextEditor::from_source("f(ab[cd]e)");
        editor.set_caret(0, 1); // on '('
        assert!(editor.jump_to_matching_bracket());
        assert_eq!(caret(&editor), (0, 9), "the caret is on the closing ')'");
        assert!(editor.jump_to_matching_bracket());
        assert_eq!(caret(&editor), (0, 1), "and back on the opening '('");

        // A caret inside the brackets, not adjacent to one, jumps nowhere.
        editor.set_caret(0, 3);
        assert!(!editor.jump_to_matching_bracket());
        assert_eq!(caret(&editor), (0, 3));
    }

    /// EDT-008: Tab indents every line a multi-line selection touches and Shift+Tab undoes it,
    /// with the selection endpoints following the text they covered.
    #[test]
    fn indent_and_outdent_a_multiline_selection() {
        let mut editor = TextEditor::from_source("a\nb\nc");
        editor.set_caret(0, 0);
        editor.select_to(2, 1);

        editor.indent();
        assert_eq!(editor.source(), "    a\n    b\n    c");
        assert_eq!(
            editor.selection_range(),
            Some(((0, 4), (2, 5))),
            "the selection stays over the same three lines"
        );
        assert_eq!(caret(&editor), (2, 5));

        editor.outdent();
        assert_eq!(editor.source(), "a\nb\nc");
        assert_eq!(editor.selection_range(), Some(((0, 0), (2, 1))));
        assert_eq!(caret(&editor), (2, 1));

        // One more outdent does nothing: there is no leading whitespace left.
        editor.outdent();
        assert_eq!(editor.source(), "a\nb\nc");
    }

    /// EDT-008: smart Enter keeps the current indentation and adds one level after `{`.
    #[test]
    fn smart_enter_keeps_the_indent_and_adds_a_level_after_an_opening_brace() {
        let mut editor = TextEditor::from_source("    if (x) {\n        y = 1");
        editor.set_caret(0, 12); // end of `    if (x) {`
        editor.newline();
        assert_eq!(
            editor.source(),
            "    if (x) {\n        \n        y = 1",
            "the new line has the enclosing four spaces plus one four-space level"
        );
        assert_eq!(caret(&editor), (1, 8));

        // An ordinary indented line simply keeps its indentation.
        let mut plain = TextEditor::from_source("    a");
        plain.set_caret(0, 5);
        plain.newline();
        assert_eq!(plain.source(), "    a\n    ");
        assert_eq!(caret(&plain), (1, 4));
    }

    /// EDT-008: typing the closing character of an auto-closed pair steps over it instead of
    /// inserting a duplicate; disabling auto-close inserts the typed character literally.
    #[test]
    fn auto_close_skips_over_the_closing_character_and_is_configurable() {
        let mut editor = TextEditor::from_source("");
        assert_eq!(
            editor.preferences,
            EditorSettings {
                indent_width: 4,
                use_spaces: true,
                auto_close: true,
            }
        );
        editor.insert_str("(");
        assert_eq!(editor.source(), "()", "the closing bracket is inserted");
        assert_eq!(caret(&editor), (0, 1), "the caret is between the pair");

        editor.insert_str(")");
        assert_eq!(
            editor.source(),
            "()",
            "the typed ')' steps over the one already there"
        );
        assert_eq!(caret(&editor), (0, 2));

        editor.insert_str("{");
        assert_eq!(editor.source(), "(){}");
        assert_eq!(caret(&editor), (0, 3));

        let mut quoted = TextEditor::from_source("");
        quoted.insert_str("\"");
        assert_eq!(quoted.source(), "\"\"");
        assert_eq!(caret(&quoted), (0, 1));

        let mut disabled = TextEditor::from_source("");
        disabled.preferences.auto_close = false;
        disabled.insert_str("(");
        assert_eq!(disabled.source(), "(");
        assert_eq!(caret(&disabled), (0, 1));
    }

    /// EDT-008: the indent width and tabs-vs-spaces setting decide what Tab writes and how wide
    /// a literal tab is displayed.
    #[test]
    fn tab_width_and_tabs_vs_spaces_settings_are_honoured() {
        let mut spaces = TextEditor::from_source("");
        spaces.preferences = EditorSettings {
            indent_width: 2,
            use_spaces: true,
            auto_close: true,
        };
        spaces.indent();
        assert_eq!(spaces.source(), "  ");
        assert_eq!(caret(&spaces), (0, 2));
        assert_eq!(spaces.display_column(0, 2), 2);

        let mut tabs = TextEditor::from_source("");
        tabs.preferences = EditorSettings {
            indent_width: 8,
            use_spaces: false,
            auto_close: true,
        };
        tabs.indent();
        assert_eq!(tabs.source(), "\t", "tabs mode writes one literal tab");
        assert_eq!(
            tabs.display_column(0, 1),
            8,
            "the tab is eight display cells wide"
        );
        tabs.insert_str("x");
        assert_eq!(tabs.source(), "\tx");
        assert_eq!(
            tabs.display_column(0, 2),
            9,
            "the character after a tab starts in column 9"
        );
        assert_eq!(tabs.line_col_1based(), (1, 3), "the readout stays in chars");
    }

    /// EDT-010: go to line accepts 1-based `line` and `line:column`, and refuses bad input.
    #[test]
    fn go_to_line_moves_the_caret_to_the_exact_line_and_column() {
        let mut editor = TextEditor::from_source("one\ntwo\nthree\nfour");
        assert_eq!(editor.go_to_line("3:2"), Ok((3, 2)));
        assert_eq!(caret(&editor), (2, 1));
        assert_eq!(editor.go_to_line(" 4 "), Ok((4, 1)));
        assert_eq!(caret(&editor), (3, 0));

        assert_eq!(
            editor.go_to_line("0"),
            Err("Line 0 is outside 1..=4.".to_string())
        );
        assert_eq!(
            editor.go_to_line("9"),
            Err("Line 9 is outside 1..=4.".to_string())
        );
        assert_eq!(
            editor.go_to_line("two"),
            Err("'two' is not a line number.".to_string())
        );
        assert_eq!(caret(&editor), (3, 0), "a rejected jump leaves the caret");

        // A column past the line's end clamps to the line, not to the next line.
        assert_eq!(editor.go_to_line("1:99"), Ok((1, 4)));
        assert_eq!(caret(&editor), (0, 3));
    }

    /// EDT-010: find symbol filters the document's identifiers (strings and keywords excluded)
    /// and jumps to the first occurrence of the chosen name.
    #[test]
    fn find_symbol_filters_the_documents_identifiers_and_jumps() {
        let editor =
            TextEditor::from_source("func alpha(x) {\n    beta = alpha + x\n    \"alpha\"\n}");
        assert_eq!(
            editor.document_symbols(),
            vec![
                SymbolHit {
                    name: "alpha".to_string(),
                    row: 0,
                    col: 5,
                },
                SymbolHit {
                    name: "x".to_string(),
                    row: 0,
                    col: 11,
                },
                SymbolHit {
                    name: "beta".to_string(),
                    row: 1,
                    col: 4,
                },
            ],
            "keywords are excluded, duplicate names keep their first position, strings are skipped"
        );
        assert_eq!(
            editor.find_symbols("AL"),
            vec![SymbolHit {
                name: "alpha".to_string(),
                row: 0,
                col: 5,
            }],
            "the filter is case-insensitive and matches inside the name"
        );
        assert_eq!(
            editor.find_symbols("ta"),
            vec![SymbolHit {
                name: "beta".to_string(),
                row: 1,
                col: 4,
            }]
        );

        let mut editor = editor;
        let beta = editor.find_symbols("beta")[0].clone();
        assert!(editor.jump_to_symbol(&beta));
        assert_eq!(caret(&editor), (1, 4));
        assert_eq!(editor.line_col_1based(), (2, 5));
    }

    // -------------------------------------------------------------------------------------
    // EDT-009: find and replace
    // -------------------------------------------------------------------------------------

    fn find_editor(source: &str, query: &str) -> TextEditor {
        let mut editor = TextEditor::from_source(source);
        editor.find_open();
        editor.find.set_query(query);
        editor.find_refresh();
        editor
    }

    #[test]
    fn find_next_wraps_from_the_last_match_to_the_first() {
        let mut editor = find_editor("alpha beta alpha", "alpha");
        assert_eq!(editor.find.matches().len(), 2);
        assert_eq!(editor.find.selected_index(), 0);
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 2"));

        assert!(editor.find_next());
        assert_eq!(caret(&editor), (0, 11), "the second match is revealed");
        assert_eq!(editor.find.count_text().as_deref(), Some("2 of 2"));

        assert!(editor.find_next());
        assert_eq!(
            (editor.caret_row, editor.caret_col),
            (0, 0),
            "next past the last match wraps to the first"
        );
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 2"));

        assert!(editor.find_previous());
        assert_eq!(
            (editor.caret_row, editor.caret_col),
            (0, 11),
            "previous before the first match wraps to the last"
        );
    }

    #[test]
    fn find_is_case_sensitive_only_when_the_toggle_is_on() {
        let mut editor = find_editor("Alpha alpha ALPHA", "alpha");
        assert_eq!(editor.find.matches().len(), 3, "the default is insensitive");
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 3"));

        editor.find.set_option(true, false, false);
        editor.find_refresh();
        assert_eq!(editor.find.matches().len(), 1, "the toggle isolates 'alpha'");
        assert_eq!(editor.find.matches()[0].start, 6);
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 1"));
    }

    #[test]
    fn whole_word_matches_only_identifier_boundaries() {
        let mut editor = find_editor("cat category cat_1 cat2 cat", "cat");
        editor.find.set_option(false, true, false);
        editor.find_refresh();
        assert_eq!(
            editor
                .find
                .matches()
                .iter()
                .map(|span| (span.start, span.end))
                .collect::<Vec<_>>(),
            vec![(0, 3), (24, 27)],
            "category, cat_1 and cat2 are not whole-word matches"
        );
        assert!(editor.find_next());
        assert_eq!((editor.caret_row, editor.caret_col), (0, 24));
    }

    #[test]
    fn an_invalid_regex_shows_an_error_and_never_panics() {
        let mut editor = find_editor("alpha beta\ngamma", "(");
        editor.find.set_option(false, false, true);
        editor.find_refresh();
        let error = editor.find.error().expect("the invalid pattern has a reason");
        assert!(error.contains("Invalid regular expression"), "{error}");
        assert!(editor.find.matches().is_empty(), "no match list is published");
        assert_eq!(editor.find.count_text(), None, "no count is shown");
        assert!(!editor.find_next(), "there is nowhere to move");
        assert_eq!(
            editor.source(),
            "alpha beta\ngamma",
            "an invalid pattern never edits the buffer"
        );
        assert!(editor.find_summary().expect("open").starts_with("Find: invalid regex"));

        // A valid pattern in the same session recovers the count and clears the error.
        editor.find.set_query("a+");
        editor.find_refresh();
        assert!(editor.find.error().is_none());
        assert!(!editor.find.matches().is_empty());
    }

    #[test]
    fn replace_all_rewrites_every_match_exactly_once() {
        let mut editor = find_editor("one two one two one", "one");
        editor.find.set_replacement("1");
        assert_eq!(editor.replace_all(), 3);
        assert_eq!(editor.source(), "1 two 1 two 1");
        assert_eq!(
            editor.find.matches().len(),
            0,
            "the replacement text is not itself rematched in the same operation"
        );
    }

    #[test]
    fn one_undo_restores_the_pre_replace_buffer_in_one_step() {
        let mut editor = find_editor("one two one two one", "one");
        editor.find.set_replacement("1");
        let steps_before = editor.history.undo_steps();
        assert_eq!(editor.replace_all(), 3);
        assert_eq!(editor.source(), "1 two 1 two 1");
        assert_eq!(
            editor.history.undo_steps(),
            steps_before + 1,
            "replace-all is exactly one history step"
        );
        assert!(editor.undo());
        assert_eq!(
            editor.source(),
            "one two one two one",
            "one Ctrl+Z restores the whole pre-replace buffer"
        );
        assert_eq!(editor.history.undo_steps(), steps_before);
    }

    #[test]
    fn replace_current_is_one_undo_step_and_moves_to_the_next_match() {
        let mut editor = find_editor("a a a", "a");
        editor.find.set_replacement("X");
        let steps_before = editor.history.undo_steps();
        assert!(editor.replace_current());
        assert_eq!(editor.source(), "X a a");
        assert_eq!(editor.history.undo_steps(), steps_before + 1);
        assert_eq!(caret(&editor), (0, 1), "the caret is after the replacement");
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 2"));

        assert!(editor.replace_current());
        assert_eq!(editor.source(), "X X a");
        assert!(editor.undo());
        assert_eq!(editor.source(), "X a a", "one undo undoes one replacement");
    }

    #[test]
    fn regex_replace_all_uses_the_engine_and_counts_each_match() {
        let mut editor = find_editor("a1 b22 c333", "\\d+");
        editor.find.set_option(true, false, true);
        editor.find_refresh();
        editor.find.set_replacement("#");
        assert_eq!(editor.find.matches().len(), 3);
        assert_eq!(editor.replace_all(), 3);
        assert_eq!(editor.source(), "a# b# c#");
    }

    #[test]
    fn find_matches_refresh_after_a_buffer_edit_while_the_bar_is_open() {
        let mut editor = find_editor("alpha beta", "alpha");
        assert_eq!(editor.find.matches().len(), 1);
        editor.set_caret(0, 0);
        editor.insert_str("alpha ");
        editor.find_refresh();
        assert_eq!(
            editor.find.matches().len(),
            2,
            "an edit marks the cached matches stale and the next refresh sees the new match"
        );
        assert_eq!(editor.find.count_text().as_deref(), Some("1 of 2"));
    }

    #[test]
    fn find_close_removes_the_highlights_and_keeps_the_buffer() {
        let mut editor = find_editor("alpha beta alpha", "alpha");
        assert_eq!(editor.find.matches().len(), 2);
        editor.find_close();
        assert!(editor.find.matches().is_empty());
        assert_eq!(editor.source(), "alpha beta alpha");
        assert_eq!(editor.find_summary(), None);
    }

    // -------------------------------------------------------------------------------------
    // EDT-004: signature help
    // -------------------------------------------------------------------------------------

    fn signature_function(name: &str, parameters: &[&str]) -> EngineFunction {
        EngineFunction {
            name: name.to_string(),
            parameters: parameters.iter().map(|p| p.to_string()).collect(),
            is_builtin: true,
            plugin: None,
            summary: Some(format!("{name} is described by the engine registry.")),
        }
    }

    #[test]
    fn signature_help_comes_from_the_engine_function_metadata() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![signature_function("transform", &["alpha", "beta", "gamma"])],
        };
        let mut editor = TextEditor::from_source("transform(");
        editor.set_caret(0, 10);
        let help = editor.signature_help(&state).expect("a call to a known function");
        assert_eq!(help.name, "transform");
        assert_eq!(
            help.parameters,
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            "the parameter names are the engine's, not a desktop table"
        );
        assert_eq!(help.signature(), "transform(alpha, beta, gamma)");
        assert_eq!(help.active, 0);
        assert_eq!(help.active_parameter(), Some("alpha"));
        assert!(help.status().contains("parameter 1/3 'alpha'"));
    }

    #[test]
    fn signature_help_advances_the_active_parameter_with_commas() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![signature_function("transform", &["alpha", "beta", "gamma"])],
        };
        let mut editor = TextEditor::from_source("transform(");
        editor.set_caret(0, 10);
        assert_eq!(
            editor.signature_help(&state).map(|help| help.active),
            Some(0)
        );

        editor.insert_str("1, ");
        assert_eq!(
            editor.signature_help(&state).map(|help| help.active),
            Some(1),
            "the first comma activates beta"
        );
        editor.insert_str("2, ");
        assert_eq!(
            editor.signature_help(&state).map(|help| help.active),
            Some(2),
            "the second comma activates gamma"
        );
        editor.insert_str("3, ");
        assert_eq!(
            editor.signature_help(&state).map(|help| help.active),
            Some(2),
            "an extra comma stays on the last parameter"
        );

        editor.insert_str("4)");
        assert!(
            editor.signature_help(&state).is_none(),
            "a closed call dismisses the signature"
        );
    }

    #[test]
    fn a_parenthesis_inside_a_string_is_not_a_call() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![signature_function("transform", &["alpha"])],
        };
        let mut editor = TextEditor::from_source("\"transform(\"");
        editor.set_caret(0, 11);
        assert!(
            editor.signature_help(&state).is_none(),
            "the parenthesis is string text, not a call"
        );

        let mut commented = TextEditor::from_source("# transform(");
        commented.set_caret(0, 12);
        assert!(commented.signature_help(&state).is_none());
    }

    #[test]
    fn an_unknown_function_name_shows_no_signature() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![signature_function("transform", &["alpha"])],
        };
        let mut editor = TextEditor::from_source("unknown(");
        editor.set_caret(0, 8);
        assert!(editor.signature_help(&state).is_none());
    }

    /// The known builtin's signature is read from the real engine's own registry: the parameter
    /// names asserted here are not written anywhere in the desktop crate.
    #[cfg(feature = "real-engine")]
    #[test]
    fn known_builtins_sqrt_and_compile_read_their_parameters_from_the_engine_registry() {
        use crate::engine::Evaluator;
        let engine = crate::engine::RealEngine::new();
        let state = engine.state().expect("the real engine captures its registry");

        let sqrt = state
            .functions
            .iter()
            .find(|function| function.name == "sqrt")
            .expect("sqrt is a registered builtin");
        assert_eq!(sqrt.parameters, vec!["x".to_string()]);

        let mut editor = TextEditor::from_source("sqrt(");
        editor.set_caret(0, 5);
        let help = editor
            .signature_help(&state)
            .expect("signature help for the registered sqrt");
        assert_eq!(help.signature(), "sqrt(x)");
        assert_eq!(help.active_parameter(), Some("x"));

        let compile = state
            .functions
            .iter()
            .find(|function| function.name == "compile")
            .expect("compile is a registered builtin");
        assert_eq!(
            compile.parameters,
            vec!["f".to_string(), "params".to_string()]
        );
        let mut editor = TextEditor::from_source("compile(");
        editor.set_caret(0, 8);
        let help = editor.signature_help(&state).expect("compile's signature");
        assert_eq!(help.signature(), "compile(f, params)");
        assert_eq!(help.active, 0);
        editor.insert_str("x^2, ");
        let help = editor.signature_help(&state).expect("still inside the call");
        assert_eq!(help.active, 1, "the comma advances to the params parameter");
        assert_eq!(help.active_parameter(), Some("params"));
    }
}
