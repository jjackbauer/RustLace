//! A11Y-003 (visible focus) and A11Y-014 (glyph localization) for the window.
//!
//! Two window-wide concerns live here so `ui::app` stays the view it already was:
//!
//! - [`paint_focus_ring`] draws one 2 px ring in the active theme's `focus_ring` token around
//!   whatever widget holds keyboard focus. A single ring is what makes the indicator *consistent*:
//!   a button, a text field, a tree header and the editor surface all get the same outline, and
//!   none of them can forget one because the ring is painted once per frame from egui's focus
//!   state rather than by each widget.
//! - [`localize_view`] applies the ASCII/Unicode mathematical-notation setting to the one
//!   assembled `DocumentView` the frame paints, so the structured result, the workspace tables,
//!   the logs result line and the inspector tree cannot disagree. The same pass replaces a glyph
//!   the installed font cannot draw with its documented fallback (`crate::glyphs`), which is what
//!   keeps a missing glyph from becoming a tofu box.
//!
//! Both are testable without a window: the ring painter is exercised by the window's headless
//! tests (they read the painted shapes) and the localization is a pure walk over the view's
//! display fields with the font probe supplied by the caller.

#[cfg(test)]
use egui::Rect;
use egui::{Context, Id, LayerId, Order, Stroke, StrokeKind};
use lovelace_json::Json;

use crate::glyphs::{self, GlyphMode};
use crate::log_kind::LogKind;
use crate::workbench::DocumentView;

use super::theme;

/// The ring's width in logical pixels. Two pixels is a visible perimeter at 100-200% scaling.
pub const FOCUS_RING_WIDTH: f32 = 2.0;

/// The gap between the focused widget's own edge and the ring, in logical pixels.
///
/// The ring is outside the widget, so it never covers the widget's text, its control border or
/// its background fill; the colour behind it is the pane surface the widget sits on, which is the
/// surface the contrast test measures.
pub const FOCUS_RING_GAP: f32 = 2.0;

/// The radius of the ring's corners, matching the window's widget corner radius.
pub const FOCUS_RING_RADIUS: u8 = 3;

/// The layer the ring paints in: above panes, windows and dialogs, so a focused control inside a
/// prompt keeps its indicator.
pub fn focus_ring_layer() -> LayerId {
    LayerId::new(Order::Tooltip, Id::unique("a11y_focus_ring"))
}

/// Paints the keyboard-focus ring around whatever widget currently owns focus.
///
/// Nothing is painted when no widget is focused or when the focused widget was not painted this
/// pass (for example a control inside a window that just closed), so a stale ring can never
/// linger over unrelated content.
pub fn paint_focus_ring(ctx: &Context) {
    let Some(id) = ctx.memory(|memory| memory.focused()) else {
        return;
    };
    let Some(rect) = ctx.read_response(id).map(|response| response.rect) else {
        return;
    };
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let painter = ctx.layer_painter(focus_ring_layer());
    painter.rect_stroke(
        rect.expand(FOCUS_RING_GAP),
        egui::CornerRadius::same(FOCUS_RING_RADIUS),
        Stroke::new(FOCUS_RING_WIDTH, theme::focus_ring()),
        StrokeKind::Outside,
    );
}

/// The JSON keys whose string values are display text the panes paint.
///
/// Only these fields are localized: a structural field such as `kind`, `name` or `type` keeps its
/// exact spelling, because it is an identifier (a variable named `pi` must not become `π`) rather
/// than a rendering. The list is the union of the keys `results_tree` and `inspector` read as
/// text.
const DISPLAY_KEYS: &[&str] = &[
    "pretty",
    "display",
    "value",
    "label",
    "canonical",
    "detail",
    "text",
    "left",
    "right",
    "source",
    "opLabel",
    "classification",
    "conditions",
    "resultType",
    "summary",
    "reason",
    "expression",
];

/// True when a string under this key is display text.
fn is_display_key(key: &str) -> bool {
    DISPLAY_KEYS.contains(&key)
}

/// Renders `text` and returns `Some` only when the rendering differs, so an unchanged value keeps
/// its allocation and the per-frame pass stays cheap on an ASCII document.
fn localized(
    text: &str,
    mode: GlyphMode,
    has_glyph: &mut impl FnMut(char) -> bool,
) -> Option<String> {
    let rendered = glyphs::render(text, mode, has_glyph);
    (rendered != text).then_some(rendered)
}

/// Rewrites every display string in a structured projection.
fn localize_json(value: &mut Json, mode: GlyphMode, has_glyph: &mut impl FnMut(char) -> bool) {
    match value {
        Json::Str(text) => {
            if let Some(rendered) = localized(text, mode, has_glyph) {
                *text = rendered;
            }
        }
        Json::Arr(items) => {
            for item in items {
                localize_json(item, mode, has_glyph);
            }
        }
        Json::Obj(fields) => {
            for (key, field) in fields.iter_mut() {
                match field {
                    Json::Str(text) if is_display_key(key) => {
                        if let Some(rendered) = localized(text, mode, has_glyph) {
                            *text = rendered;
                        }
                    }
                    // A display field can hold a nested object or array (a record of symbolic
                    // values, a list of solutions); recurse so their display strings are reached
                    // while identifiers stay exact.
                    Json::Arr(_) | Json::Obj(_) => localize_json(field, mode, has_glyph),
                    _ => {}
                }
            }
        }
        Json::Null | Json::Bool(_) | Json::Num(_) => {}
    }
}

/// Applies the ASCII/Unicode setting to every display field of the frame's view.
///
/// A glyph whose character is not drawable by the installed font is replaced by its documented
/// fallback as part of the same call, so every surface shows the same string for the same value.
/// `has_glyph` is the window's font probe (cached by the caller).
///
/// The four A11Y-014 surfaces are covered by construction:
///
/// | surface | fields |
/// | --- | --- |
/// | structured result | `result`, `result_structured.display` |
/// | workspace | `variables[].display`, `state.variables[].display`/`structured` |
/// | logs | the value-bearing `[result]` line |
/// | inspector | `result_inspection` display fields |
///
/// Source echoes, `print(...)` output, diagnostics and the status strip are the user's own words.
/// They are left byte-exact, so the setting can never change what a script printed.
pub fn localize_view(
    view: &mut DocumentView,
    mode: GlyphMode,
    has_glyph: &mut impl FnMut(char) -> bool,
) {
    if let Some(rendered) = localized(&view.result, mode, has_glyph) {
        view.result = rendered;
    }
    if let Some(structured) = view.result_structured.as_mut() {
        localize_json(structured, mode, has_glyph);
    }
    if let Some(inspection) = view.result_inspection.as_mut() {
        localize_json(inspection, mode, has_glyph);
    }
    // Workspace: the variable display column and the same value inside the completion snapshot's
    // structured projection. Names and kinds are identifiers and are left exact.
    for (_, display) in view.variables.iter_mut() {
        if let Some(rendered) = localized(display, mode, has_glyph) {
            *display = rendered;
        }
    }
    for variable in view.state.variables.iter_mut() {
        if let Some(rendered) = localized(&variable.display, mode, has_glyph) {
            variable.display = rendered;
        }
        if let Some(structured) = variable.structured.as_mut() {
            localize_json(structured, mode, has_glyph);
        }
    }
    // Logs: only the value-bearing result line is a rendering of a value.
    for line in view.logs.iter_mut() {
        if line.kind != LogKind::Result {
            continue;
        }
        if let Some(rendered) = localized(&line.text, mode, has_glyph) {
            line.text = rendered;
        }
    }
}

/// True when the installed font can draw `c` in the UI's monospace or proportional family.
///
/// Asking egui's own font stack means the answer describes the shipped window, including any
/// system font the platform resolver added, rather than a guess from a glyph table.
pub fn font_has_glyph(ctx: &Context, c: char) -> bool {
    ctx.fonts_mut(|fonts| {
        fonts.has_glyph(&super::type_scale::Role::Code.font(), c)
            || fonts.has_glyph(&super::type_scale::Role::Body.font(), c)
    })
}

/// Every non-ASCII character in `sample` with its fallback and whether the font can draw it.
///
/// This is the A11Y-014 no-tofu report: a character with `has_glyph == false` and no fallback
/// would be drawn as `?` by [`glyphs::render`], so the report is empty of such characters for a
/// shipped sample.
pub fn glyph_report(
    sample: &str,
    has_glyph: &mut impl FnMut(char) -> bool,
) -> Vec<(char, bool, Option<&'static str>)> {
    glyphs::non_ascii(sample)
        .into_iter()
        .map(|c| (c, has_glyph(c), glyphs::fallback(c)))
        .collect()
}

/// A11Y-012: the WCAG relative luminance of a colour, computed from its sRGB channels.
///
/// The formula weights the channels by human sensitivity, so two colours with the same
/// luminance are equally bright in greyscale. It is the number a colour-blind reader's
/// perception reduces to: no hue information remains.
#[cfg(test)]
pub fn relative_luminance(color: egui::Color32) -> f32 {
    fn linear(channel: u8) -> f32 {
        let c = channel as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

/// A11Y-012: the WCAG contrast ratio between two colours, from luminance alone.
#[cfg(test)]
pub fn luminance_contrast(first: egui::Color32, second: egui::Color32) -> f32 {
    let (a, b) = (
        relative_luminance(first),
        relative_luminance(second),
    );
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

/// A11Y-012: the greyscale value a pixel becomes when its hue is removed.
///
/// It is the same 0.299/0.587/0.114 luma a desaturating capture uses, so a test can state the
/// greyscale pixel the error marker will have.
#[cfg(test)]
pub fn greyscale_luma(color: egui::Color32) -> u8 {
    (0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32).round() as u8
}

/// One node of the window's accessibility tree, as the A11Y-002 audit and the A11Y-004 layout
/// assertions read it.
#[derive(Clone, Debug)]
#[cfg(test)]
pub struct NodeRow {
    /// The node's identity, so a row's parent can be found again.
    pub id: egui::accesskit::NodeId,
    /// The parent node, or `None` for the root.
    pub parent: Option<egui::accesskit::NodeId>,
    /// Nesting depth, 0 for the root.
    pub depth: usize,
    /// The accessibility role.
    pub role: egui::accesskit::Role,
    /// The accessible name (`label`).
    pub name: String,
    /// The value, when the node publishes one (the editor's text, the plot's title, ...).
    pub value: Option<String>,
    /// The description, when the node publishes one.
    pub description: Option<String>,
    /// Selection state, when the node is selectable (tabs, column headers).
    pub selected: Option<bool>,
    /// Numeric value, when the node is a range (the progress bar).
    pub numeric_value: Option<f64>,
    /// Row and column counts, for a table node.
    pub table_size: Option<(usize, usize)>,
    /// True when the node can take keyboard focus.
    pub focusable: bool,
    /// True when the node is disabled.
    pub disabled: bool,
    /// The node's bounds, in points.
    pub bounds: Rect,
}

#[cfg(test)]
impl NodeRow {
    /// True when the node is one a user can operate (the A11Y-002 "interactive node" set).
    pub fn interactive(&self) -> bool {
        self.focusable
            || matches!(
                self.role,
                egui::Role::Button
                    | egui::Role::Tab
                    | egui::Role::TextInput
                    | egui::Role::MultilineTextInput
                    | egui::Role::Splitter
                    | egui::Role::ProgressIndicator
                    | egui::Role::ColumnHeader
                    | egui::Role::Row
            )
    }

    /// The `value/state` column of the dump: every state the node carries, as text.
    pub fn state_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(value) = &self.value {
            parts.push(format!("value={}", short(value, 40)));
        }
        if let Some(selected) = self.selected {
            parts.push(if selected {
                "selected".to_string()
            } else {
                "not selected".to_string()
            });
        }
        if let Some(numeric) = self.numeric_value {
            parts.push(format!("numeric={numeric:.2}"));
        }
        if let Some((rows, columns)) = self.table_size {
            parts.push(format!("{rows}x{columns}"));
        }
        if self.disabled {
            parts.push("disabled".to_string());
        }
        if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join(" ")
        }
    }
}

/// Truncates display text for the dump without cutting a character in half.
#[cfg(test)]
fn short(text: &str, limit: usize) -> String {
    let mut short: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        short.push('…');
    }
    short.replace('\n', "\\n")
}

/// The row for every node of one AccessKit tree update, in depth-first order.
///
/// The window's real Windows UIA tree is produced from exactly this update by the AccessKit
/// Windows adapter, so this dump and the `uia-tree.txt` capture are two views of one tree: the
/// test audit cannot pass on a tree the product does not expose.
#[cfg(test)]
pub fn accesskit_rows(update: &egui::accesskit::TreeUpdate) -> Vec<NodeRow> {
    use std::collections::HashMap;
    let nodes: HashMap<_, _> = update.nodes.iter().cloned().collect();
    let root = update
        .tree
        .as_ref()
        .map(|tree| tree.root)
        .or_else(|| {
            update
                .nodes
                .iter()
                .find(|(_, node)| node.role() == egui::accesskit::Role::Window)
                .map(|(id, _)| *id)
        });
    let Some(root) = root else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    let mut stack = vec![(root, None, 0usize)];
    let mut seen = std::collections::HashSet::new();
    while let Some((id, parent, depth)) = stack.pop() {
        if !seen.insert(id) || depth > 40 {
            continue;
        }
        let Some(node) = nodes.get(&id) else {
            continue;
        };
        let bounds = node.bounds().map_or(Rect::NOTHING, |rect| {
            Rect::from_min_max(
                egui::pos2(rect.x0 as f32, rect.y0 as f32),
                egui::pos2(rect.x1 as f32, rect.y1 as f32),
            )
        });
        rows.push(NodeRow {
            id,
            parent,
            depth,
            role: node.role(),
            // A `Label` (and a text run) is named by its value: that is the name the Windows
            // UIA adapter reports, so the dump and the real tree agree.
            name: match (node.label(), node.value()) {
                (Some(label), _) if !label.is_empty() => label.to_string(),
                (_, Some(value)) => value.to_string(),
                _ => String::new(),
            },
            value: node.value().map(str::to_string),
            description: node.description().map(str::to_string),
            selected: node.is_selected(),
            numeric_value: node.numeric_value(),
            table_size: match (node.row_count(), node.column_count()) {
                (Some(rows), Some(columns)) => Some((rows, columns)),
                (Some(rows), None) => Some((rows, 0)),
                _ => None,
            },
            focusable: node.supports_action(egui::accesskit::Action::Focus),
            disabled: node.is_disabled(),
            bounds,
        });
        for child in node.children().iter().rev() {
            stack.push((*child, Some(id), depth + 1));
        }
    }
    rows
}

/// The `role | name | value/state` table the A11Y-002 deliverable publishes, as text.
#[cfg(test)]
pub fn dump_table(rows: &[NodeRow]) -> String {
    let mut text = String::from("role | name | value/state\n");
    for row in rows {
        text.push_str(&format!(
            "{}{:?} | {} | {}\n",
            "  ".repeat(row.depth),
            row.role,
            if row.name.is_empty() {
                "(unnamed)"
            } else {
                row.name.as_str()
            },
            row.state_text()
        ));
    }
    text
}

/// True when `row` is a descendant of `ancestor` in the dumped tree.
#[cfg(test)]
pub fn is_descendant(rows: &[NodeRow], row: &NodeRow, ancestor: egui::accesskit::NodeId) -> bool {
    let mut current = row.parent;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = rows
            .iter()
            .find(|candidate| candidate.id == id)
            .and_then(|candidate| candidate.parent);
    }
    false
}

/// True when `row` lives inside a scrollable viewport.
///
/// egui reports a scrolled row's layout position, not the visually clipped one, so such a row
/// may legitimately sit outside the window or its pane; the scroll viewport clips it cleanly.
/// A scrollable viewport is recognised by the `ScrollBar` node a `ScrollArea` registers beside
/// its content.
#[cfg(test)]
pub fn in_scroll_container(rows: &[NodeRow], row: &NodeRow) -> bool {
    let mut current = row.parent;
    while let Some(id) = current {
        if rows
            .iter()
            .any(|candidate| candidate.parent == Some(id) && candidate.role == egui::Role::ScrollBar)
        {
            return true;
        }
        current = rows
            .iter()
            .find(|candidate| candidate.id == id)
            .and_then(|candidate| candidate.parent);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineState;
    use crate::execution::{Diagnostic, SourceRevision};
    use crate::log_kind::LogLine;
    use crate::run_state::RunState;
    use lovelace_json::Json;

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            message: "sqrt(2)*pi is not a number".to_string(),
            code: Some("TypeError".to_string()),
            category: Some("type".to_string()),
            suggestions: vec!["sqrt".to_string()],
            statement_index: 0,
            statement_line: 1,
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn view() -> DocumentView {
        DocumentView {
            revision: SourceRevision(1),
            has_snapshot: true,
            committed_steps: 2,
            total: 2,
            result: "sqrt(2)*pi".to_string(),
            result_structured: Some(Json::obj(vec![
                ("kind", Json::Str("symbolic".to_string())),
                ("pretty", Json::Str("sqrt(2)*pi".to_string())),
            ])),
            result_inspection: Some(Json::obj(vec![
                ("kind", Json::Str("tree".to_string())),
                (
                    "root",
                    Json::obj(vec![
                        ("kind", Json::Str("Fun".to_string())),
                        ("label", Json::Str("sqrt(2)*pi".to_string())),
                    ]),
                ),
            ])),
            variables: vec![("y".to_string(), "sqrt(2)*pi".to_string())],
            functions: Vec::new(),
            state: EngineState::default(),
            plot: None,
            output: vec!["sqrt(2)*pi".to_string()],
            logs: vec![
                LogLine::new(LogKind::Stdout, "sqrt(2)*pi").at_statement(0),
                LogLine::new(LogKind::Result, "sqrt(2)*pi"),
            ],
            logs_dropped: 0,
            workspace_revision: 1,
            diagnostic: Some(diagnostic()),
            run_state: RunState::Finished,
            buffer_revision: SourceRevision(1),
            progress: "2/2".to_string(),
            precision: 30,
            elapsed_ms: 1,
        }
    }

    fn localize(mode: GlyphMode) -> DocumentView {
        let mut view = view();
        // A font with full coverage, so the assertion is about the mode mapping itself; the
        // missing-glyph path is asserted separately with a probe that accepts no non-ASCII.
        localize_view(&mut view, mode, &mut |_c: char| true);
        view
    }

    #[test]
    fn the_same_value_renders_in_both_modes_on_every_surface() {
        let ascii = localize(GlyphMode::Ascii);
        let unicode = localize(GlyphMode::Unicode);
        // Structured result caption and tree.
        assert_eq!(ascii.result, "sqrt(2)*pi");
        assert_eq!(unicode.result, "\u{221a}(2)\u{00d7}\u{03c0}");
        let ascii_tree = ascii.result_structured.as_ref().unwrap();
        let unicode_tree = unicode.result_structured.as_ref().unwrap();
        assert_eq!(
            ascii_tree.get("pretty").and_then(Json::as_str),
            Some("sqrt(2)*pi")
        );
        assert_eq!(
            unicode_tree.get("pretty").and_then(Json::as_str),
            Some("\u{221a}(2)\u{00d7}\u{03c0}")
        );
        assert_eq!(
            ascii_tree.get("kind").and_then(Json::as_str),
            Some("symbolic"),
            "a structural field is never localized"
        );
        // Workspace display column.
        assert_eq!(ascii.variables[0].1, "sqrt(2)*pi");
        assert_eq!(unicode.variables[0].1, "\u{221a}(2)\u{00d7}\u{03c0}");
        // Logs result line.
        assert_eq!(
            ascii
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Result)
                .unwrap()
                .text,
            "sqrt(2)*pi"
        );
        assert_eq!(
            unicode
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Result)
                .unwrap()
                .text,
            "\u{221a}(2)\u{00d7}\u{03c0}"
        );
        // Inspector label.
        let ascii_root = ascii.result_inspection.as_ref().unwrap().get("root").unwrap();
        let unicode_root = unicode.result_inspection.as_ref().unwrap().get("root").unwrap();
        assert_eq!(
            ascii_root.get("label").and_then(Json::as_str),
            Some("sqrt(2)*pi")
        );
        assert_eq!(
            unicode_root.get("label").and_then(Json::as_str),
            Some("\u{221a}(2)\u{00d7}\u{03c0}")
        );
    }

    #[test]
    fn ascii_mode_is_byte_identical_to_the_engine_default() {
        // The default mode must not rewrite anything the engine already produced.
        let ascii = localize(GlyphMode::Ascii);
        assert_eq!(ascii.result, view().result);
        assert_eq!(ascii.variables, view().variables);
        assert_eq!(ascii.logs, view().logs);
        assert_eq!(ascii.result_structured, view().result_structured);
    }

    #[test]
    fn user_text_and_diagnostics_are_not_localized() {
        let unicode = localize(GlyphMode::Unicode);
        // print() output and a diagnostic message are the user's/source's own words.
        assert_eq!(unicode.output, view().output);
        assert_eq!(
            unicode.diagnostic.as_ref().unwrap().message,
            "sqrt(2)*pi is not a number"
        );
        assert_eq!(
            unicode
                .logs
                .iter()
                .find(|line| line.kind == LogKind::Stdout)
                .unwrap()
                .text,
            "sqrt(2)*pi",
            "stdout is not a value rendering"
        );
    }

    #[test]
    fn identifiers_are_never_rewritten_as_named_constants() {
        // A variable literally named `pi` keeps its name in the workspace table; only its value
        // is rendered in the selected notation.
        let mut view = view();
        view.variables = vec![("pi".to_string(), "sqrt(2)*pi".to_string())];
        localize_view(&mut view, GlyphMode::Unicode, &mut |_c: char| true);
        assert_eq!(view.variables[0].0, "pi");
        assert_eq!(view.variables[0].1, "\u{221a}(2)\u{00d7}\u{03c0}");
    }

    #[test]
    fn a_missing_glyph_never_reaches_the_painted_string() {
        let mut view = view();
        view.result = "\u{221a}(2)\u{00d7}\u{03c0} \u{2264} \u{221e}".to_string();
        localize_view(&mut view, GlyphMode::Unicode, &mut |_c: char| false);
        assert_eq!(view.result, "sqrt(2)*pi <= inf");
        assert!(view.result.is_ascii());
    }

    #[test]
    fn the_no_tofu_report_covers_the_engine_sample() {
        // A sample with the engine printer's glyphs, the two this window adds and an astral
        // mathematical-alphanumeric character.
        let sample = format!("{} and {}", glyphs::SAMPLE_GLYPHS, '\u{1D70B}');
        let mut probe = |_c: char| false;
        let report = glyph_report(&sample, &mut probe);
        assert!(!report.is_empty());
        for (glyph, drawable, fallback) in report {
            assert!(
                drawable || fallback.is_some(),
                "{glyph:?} has no coverage and no documented fallback"
            );
        }
        // A font with nothing at all still renders ASCII through the table.
        let rendered = glyphs::render(&sample, GlyphMode::Unicode, |_c: char| false);
        assert!(rendered.is_ascii(), "no tofu: {rendered:?}");
        assert!(!rendered.contains('\u{FFFD}'));
    }

    #[test]
    fn the_display_key_whitelist_covers_the_panes_text_fields() {
        for key in [
            "pretty",
            "value",
            "label",
            "canonical",
            "detail",
            "text",
            "left",
            "right",
            "source",
            "opLabel",
            "classification",
            "conditions",
            "resultType",
            "summary",
            "reason",
            "display",
            "expression",
        ] {
            assert!(is_display_key(key), "{key} must be localized");
        }
        for key in [
            "kind", "name", "type", "path", "code", "category", "ruleId", "shape",
        ] {
            assert!(
                !is_display_key(key),
                "{key} is structural and must stay exact"
            );
        }
    }

    #[test]
    fn the_ring_constants_are_visible_and_do_not_cover_the_control() {
        assert!(FOCUS_RING_WIDTH >= 2.0);
        assert!(FOCUS_RING_GAP >= 2.0);
        assert_eq!(FOCUS_RING_RADIUS, 3);
        assert_ne!(focus_ring_layer(), egui::LayerId::background());
    }

    /// A11Y-012: the contrast the error markers rely on is a luminance-only measure. Two colours
    /// with the same luminance are indistinguishable in greyscale (contrast 1:1) no matter how
    /// different their hues, and black/white is the 21:1 ceiling.
    #[test]
    fn the_greyscale_contrast_is_a_luminance_only_measure() {
        let black = egui::Color32::BLACK;
        let white = egui::Color32::WHITE;
        assert!(
            (luminance_contrast(black, white) - 21.0).abs() < 0.1,
            "black on white is 21:1: {}",
            luminance_contrast(black, white)
        );
        assert_eq!(
            luminance_contrast(black, white),
            luminance_contrast(white, black),
            "the ratio is symmetric"
        );
        // A saturated hue and a neutral grey of the same WCAG luminance: a greyscale rendering
        // cannot tell them apart, so an error marker that only differed in hue would vanish.
        let red = egui::Color32::from_rgb(255, 0, 0);
        let grey = egui::Color32::from_rgb(128, 128, 128);
        assert!(
            (relative_luminance(red) - relative_luminance(grey)).abs() < 0.01,
            "red {} and grey {} must have the same luminance",
            relative_luminance(red),
            relative_luminance(grey)
        );
        assert!(
            luminance_contrast(red, grey) < 1.02,
            "equal-luminance colours are one shade in greyscale: {}",
            luminance_contrast(red, grey)
        );
        // The 0.299/0.587/0.114 luma is what the desaturated capture shows, and it must keep the
        // theme's error marker clearly separated from the editor gutter.
        let dark = theme::tokens_for(crate::settings::ThemePreference::Dark);
        assert!(
            (greyscale_luma(dark.diagnostic_marker) as i32 - greyscale_luma(dark.gutter_bg) as i32)
                .abs()
                > 60,
            "the dark marker and gutter must differ in greyscale"
        );
        let light = theme::tokens_for(crate::settings::ThemePreference::Light);
        assert!(
            (greyscale_luma(light.diagnostic_marker) as i32
                - greyscale_luma(light.gutter_bg) as i32)
                .abs()
                > 60,
            "the light marker and gutter must differ in greyscale"
        );
    }
}
