//! The deterministic models behind the Workspace pane's two tables.
//!
//! The pane used to paint two unlabelled lists: variables as `name = typed display` and
//! functions as `name(parameters)` read from the engine's *user* function list. This module
//! owns the table data instead, so the visible rows, the sort order and the counts are testable
//! without an egui pass:
//!
//! - [`VariableTable`] shows every variable the committed snapshot published (name, kind and
//!   display) in the engine's own order by default, and both directions of every column on a
//!   header click;
//! - [`FunctionTable`] shows the engine's registry snapshot (name, parameters, origin and the
//!   summary when the registry publishes one) with the same sorting contract.
//!
//! Both tables keep the capture's ordinal on every row. Sorting never drops a row and always
//! breaks ties by that ordinal, so two rows with the same kind (or the same display) keep the
//! engine's order in both directions: a repeated sort is a permutation of exactly the same set.

use crate::engine::{EngineFunction, EngineState, FunctionOrigin};
use lovelace_json::Json;

/// Longest text a cell paints; a longer value is cut with an ellipsis and its full text stays
/// available to the caller for the hover tooltip.
pub const MAX_CELL_CHARS: usize = 40;

/// The ellipsis appended to a cut cell.
const ELLIPSIS: char = '…';

/// Cuts `text` to at most [`MAX_CELL_CHARS`] characters, ending in an ellipsis when it was cut.
///
/// The function counts characters, not bytes, so a value containing non-ASCII text is never
/// split inside a character. A short value is returned unchanged.
pub fn cell_text(text: &str) -> String {
    if text.chars().count() <= MAX_CELL_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_CELL_CHARS - 1).collect();
    format!("{head}{ELLIPSIS}")
}

/// The direction arrow a sorted column shows: up for ascending, down for descending.
pub fn direction_arrow(ascending: bool) -> &'static str {
    if ascending { "▲" } else { "▼" }
}

/// A column's click state: which column sorts, and in which direction.
///
/// The contract is the requirement's: the default is the capture's ordinal (name) order, the
/// **first** click on a column sorts it ascending and the **second** click on that same column
/// sorts it descending; a third click returns to ascending. Selecting a different column starts
/// again at ascending. The `engaged` flag is what makes the first click on the already-default
/// name column stay ascending instead of reversing it before the user has asked for descending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortState<C> {
    /// The column currently sorted.
    pub key: C,
    /// True for ascending (the first click), false for descending (the second).
    pub ascending: bool,
    /// Whether the current column has received its first click.
    engaged: bool,
}

impl<C: Copy + PartialEq> SortState<C> {
    /// A state whose default column sorts ascending but has not been clicked yet.
    pub fn new(key: C) -> SortState<C> {
        SortState {
            key,
            ascending: true,
            engaged: false,
        }
    }

    /// Applies one header click: the first click on a column starts ascending, the next flips.
    pub fn clicked(&mut self, key: C) {
        if self.key == key {
            if self.engaged {
                self.ascending = !self.ascending;
            } else {
                self.engaged = true;
                self.ascending = true;
            }
        } else {
            self.key = key;
            self.engaged = true;
            self.ascending = true;
        }
    }

    /// True when `key` is the sorted column.
    pub fn is_active(&self, key: C) -> bool {
        self.key == key
    }

    /// The header text: the label plus an up/down arrow when the column is the sorted one.
    pub fn header_text(&self, key: C, label: &str) -> String {
        if self.is_active(key) {
            format!("{label} {}", direction_arrow(self.ascending))
        } else {
            label.to_string()
        }
    }

    /// The hovered direction wording for a sorted column.
    pub fn direction_label(&self) -> &'static str {
        if self.ascending {
            "ascending"
        } else {
            "descending"
        }
    }
}

/// One variable row: the engine's published name, kind and display value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableRow {
    /// The variable name as written in source (including `_`).
    pub name: String,
    /// The kind the engine published (`Natural`, `Real`, `Text`, `Record`, ...).
    pub kind: String,
    /// The untyped display rendering the engine produced.
    pub display: String,
    /// WKS-006: the type-aware preview derived from the engine's structured projection.
    pub preview: ValuePreview,
    /// WKS-006: the engine's structured projection itself, for the detail view.
    ///
    /// Carried so the detail view can show the published structure (shape, fields, pretty and
    /// canonical renderings, exactness) without asking the engine again or re-parsing the display.
    pub structured: Option<Json>,
    /// The position in the committed capture; every sort breaks ties by this value.
    pub ordinal: usize,
}

impl VariableRow {
    /// The display cell, cut for the pane and paired with the full text as its tooltip.
    pub fn display_cell(&self) -> String {
        cell_text(&self.display)
    }

    /// WKS-006: the preview cell, cut for the pane.
    pub fn preview_cell(&self) -> String {
        cell_text(&self.preview.text)
    }
}

/// WKS-006: a readable, type-aware one-line preview of one variable value.
///
/// The preview is derived from the engine's structured projection, never by re-parsing the
/// display text (the RES-001 rule): a 2-D array is published as rows and columns, a record as its
/// type and first fields, a complex number as `re + im i`, and a symbolic value as its pretty
/// rendering. Values the engine published no projection for keep the display text, labelled with
/// the published kind, so a preview can never invent structure the engine did not publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuePreview {
    /// The type/shape summary (`2 x 2 matrix`, `record DivRemResult`, `symbolic`, `real`, ...).
    pub type_label: String,
    /// The one-line readable preview.
    pub text: String,
    /// True when [`Self::text`] was shortened; the full value stays in the row and the detail view.
    pub shortened: bool,
}

/// Longest preview text: a table cell shows one short line, the detail view shows the full value.
pub const MAX_PREVIEW_CHARS: usize = 64;

/// The maximum number of elements/fields one preview names before it is shortened.
const PREVIEW_ITEMS: usize = 6;

impl ValuePreview {
    /// Builds the preview for one published variable.
    pub fn of(variable: &crate::engine::EngineVariable) -> ValuePreview {
        let Some(structured) = variable.structured.as_ref() else {
            return ValuePreview {
                type_label: variable.kind.to_lowercase(),
                text: cut_preview(&variable.display),
                shortened: variable.display.chars().count() > MAX_PREVIEW_CHARS,
            };
        };
        let kind = structured
            .get("kind")
            .and_then(|kind| kind.as_str())
            .unwrap_or(variable.kind.as_str())
            .to_string();
        match kind.as_str() {
            "Array" => preview_array(structured),
            "Record" => preview_record(structured),
            "Symbolic" => {
                let pretty =
                    text_field(structured, "pretty").unwrap_or_else(|| variable.display.clone());
                let shortened = pretty.chars().count() > MAX_PREVIEW_CHARS;
                ValuePreview {
                    type_label: "symbolic".to_string(),
                    text: cut_preview(&pretty),
                    shortened,
                }
            }
            "Complex" => {
                let re = text_field(structured, "re").unwrap_or_else(|| "0".to_string());
                let im = text_field(structured, "im").unwrap_or_else(|| "0".to_string());
                let text = complex_text(&re, &im);
                ValuePreview {
                    type_label: "complex".to_string(),
                    shortened: text.chars().count() > MAX_PREVIEW_CHARS,
                    text: cut_preview(&text),
                }
            }
            "Null" => ValuePreview {
                type_label: "null".to_string(),
                text: "no value".to_string(),
                shortened: false,
            },
            other => ValuePreview {
                type_label: other.to_lowercase(),
                text: cut_preview(&variable.display),
                shortened: variable.display.chars().count() > MAX_PREVIEW_CHARS,
            },
        }
    }
}

/// A 2-D array previews as rows (`[1, 2; 3, 4]`), a vector as a flat list, and a higher-rank array
/// as its shape plus the first elements — never as one unreadable run of every cell.
fn preview_array(structured: &Json) -> ValuePreview {
    let type_name = text_field(structured, "type").unwrap_or_else(|| "Array".to_string());
    let shape: Vec<usize> = structured
        .get("shape")
        .and_then(|shape| shape.as_arr())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    Json::Num(number) => number.parse().ok(),
                    Json::Str(text) => text.parse().ok(),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let elements: Vec<String> = structured
        .get("elements")
        .and_then(|elements| elements.as_arr())
        .map(|items| items.iter().map(scrub).collect())
        .unwrap_or_default();
    let mut shortened = false;
    let (type_label, text) = if shape.len() == 2 && !elements.is_empty() {
        let columns = shape[1].max(1);
        let rows = elements.len() / columns;
        let mut lines: Vec<String> = Vec::new();
        for row in 0..rows.min(PREVIEW_ITEMS) {
            let cells: Vec<String> = elements[row * columns..(row + 1) * columns]
                .iter()
                .map(|cell| cut_preview(cell))
                .collect();
            lines.push(format!("[{}]", cells.join(", ")));
        }
        if rows > PREVIEW_ITEMS {
            shortened = true;
            lines.push(format!("… {} more rows", rows - PREVIEW_ITEMS));
        }
        let body = lines.join("; ");
        shortened |= body.chars().count() > MAX_PREVIEW_CHARS;
        (
            format!("{} x {} matrix", shape[0], shape[1]),
            cut_preview(&body),
        )
    } else {
        let shown: Vec<String> = elements
            .iter()
            .take(PREVIEW_ITEMS)
            .map(|element| cut_preview(element))
            .collect();
        if elements.len() > PREVIEW_ITEMS {
            shortened = true;
        }
        let mut body = format!("[{}]", shown.join(", "));
        if elements.len() > PREVIEW_ITEMS {
            body.push_str(&format!(" … {} more", elements.len() - PREVIEW_ITEMS));
        }
        shortened |= body.chars().count() > MAX_PREVIEW_CHARS;
        let label = if shape.is_empty() {
            type_name.to_lowercase()
        } else {
            format!(
                "{} {}",
                type_name.to_lowercase(),
                shape
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<String>>()
                    .join(" x ")
            )
        };
        (label, cut_preview(&body))
    };
    ValuePreview {
        type_label,
        text,
        shortened,
    }
}

/// A record previews as its published type and first named fields.
fn preview_record(structured: &Json) -> ValuePreview {
    let type_name = text_field(structured, "type").unwrap_or_else(|| "Record".to_string());
    let fields: Vec<(String, String)> = structured
        .get("fields")
        .and_then(|fields| fields.as_arr())
        .map(|items| {
            items
                .iter()
                .filter_map(|field| {
                    let name = text_field(field, "name")?;
                    let value = field.get("value").map(scrub).unwrap_or_default();
                    Some((name, value))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut shortened = false;
    let shown: Vec<String> = fields
        .iter()
        .take(PREVIEW_ITEMS)
        .map(|(name, value)| format!("{name}={}", cut_preview(value)))
        .collect();
    if fields.len() > PREVIEW_ITEMS {
        shortened = true;
    }
    let mut body = format!("{} {{{}}}", type_name, shown.join(", "));
    if fields.len() > PREVIEW_ITEMS {
        body.push_str(&format!(
            " … {} more field(s)",
            fields.len() - PREVIEW_ITEMS
        ));
    }
    if fields.is_empty() {
        body = format!("{type_name} {{}}");
    }
    shortened |= body.chars().count() > MAX_PREVIEW_CHARS;
    ValuePreview {
        type_label: format!("record {type_name}"),
        text: cut_preview(&body),
        shortened,
    }
}

/// `3 + 4i`, `-2 - 1i`, `4i` — the engine's published real and imaginary parts.
fn complex_text(re: &str, im: &str) -> String {
    let re_zero = re.trim() == "0";
    let im_zero = im.trim() == "0";
    if im_zero {
        return re.to_string();
    }
    if re_zero {
        return format!("{im}i");
    }
    if let Some(rest) = im.strip_prefix('-') {
        format!("{re} - {rest}i")
    } else {
        format!("{re} + {im}i")
    }
}

/// One projected node as short text: the projection's `value`, `pretty` or `kind`.
fn scrub(json: &Json) -> String {
    match json {
        Json::Str(text) => text.clone(),
        Json::Num(text) => text.clone(),
        other => {
            let kind = other
                .get("kind")
                .and_then(|kind| kind.as_str())
                .unwrap_or("value");
            match kind {
                "Array" => {
                    let count = other
                        .get("elements")
                        .and_then(|elements| elements.as_arr())
                        .map(|items| items.len())
                        .unwrap_or(0);
                    format!("[{count} elements]")
                }
                "Record" => "record".to_string(),
                "Null" => "null".to_string(),
                _ => text_field(other, "value")
                    .or_else(|| text_field(other, "pretty"))
                    .unwrap_or_else(|| kind.to_string()),
            }
        }
    }
}

/// Cuts a preview to [`MAX_PREVIEW_CHARS`], ending in an ellipsis when it was cut.
fn cut_preview(text: &str) -> String {
    if text.chars().count() <= MAX_PREVIEW_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_PREVIEW_CHARS - 1).collect();
    format!("{head}{ELLIPSIS}")
}

fn text_field(json: &Json, key: &str) -> Option<String> {
    match json.get(key) {
        Some(Json::Str(text)) => Some(text.clone()),
        Some(Json::Num(text)) => Some(text.clone()),
        Some(Json::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    }
}

/// The sortable columns of the variables table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableColumn {
    /// Variable name.
    Name,
    /// Published kind.
    Kind,
    /// Display value.
    Display,
}

impl VariableColumn {
    /// Every column, in paint order.
    pub const ALL: [VariableColumn; 3] = [
        VariableColumn::Name,
        VariableColumn::Kind,
        VariableColumn::Display,
    ];

    /// The header label.
    pub fn label(self) -> &'static str {
        match self {
            VariableColumn::Name => "Name",
            VariableColumn::Kind => "Kind",
            VariableColumn::Display => "Value",
        }
    }

    /// The zero-based position, for the pane's recorded header rectangles.
    pub fn index(self) -> usize {
        match self {
            VariableColumn::Name => 0,
            VariableColumn::Kind => 1,
            VariableColumn::Display => 2,
        }
    }

    fn key(self, row: &VariableRow) -> &str {
        match self {
            VariableColumn::Name => row.name.as_str(),
            VariableColumn::Kind => row.kind.as_str(),
            VariableColumn::Display => row.display.as_str(),
        }
    }
}

/// One function row: the engine's registry entry, classified by origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionRow {
    /// The registered name.
    pub name: String,
    /// The declared parameter names, in declaration order.
    pub parameters: Vec<String>,
    /// `builtin` or `user`, from the engine's builtin flag.
    pub origin: FunctionOrigin,
    /// The static family label for a builtin (`Core`, `DSP`, `Symbolics`, `MathIR`, `Systems`),
    /// or `User` for a script definition.
    pub module: String,
    /// The registry summary, when the engine publishes one.
    pub summary: Option<String>,
    /// The position in the committed capture; every sort breaks ties by this value.
    pub ordinal: usize,
}

impl FunctionRow {
    /// The parameters as one comma-joined string (`a, b`).
    pub fn parameters_text(&self) -> String {
        self.parameters.join(", ")
    }

    /// The origin wording the pane paints.
    pub fn origin_label(&self) -> &'static str {
        self.origin.label()
    }

    /// The summary cell, cut for the pane; `None` when the registry published no summary.
    pub fn summary_cell(&self) -> Option<String> {
        self.summary.as_deref().map(cell_text)
    }
}

/// The sortable columns of the functions table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionColumn {
    /// Function name.
    Name,
    /// Comma-joined parameter names.
    Parameters,
    /// `builtin` / `user`.
    Origin,
}

impl FunctionColumn {
    /// Every column, in paint order.
    pub const ALL: [FunctionColumn; 3] = [
        FunctionColumn::Name,
        FunctionColumn::Parameters,
        FunctionColumn::Origin,
    ];

    /// The header label.
    pub fn label(self) -> &'static str {
        match self {
            FunctionColumn::Name => "Name",
            FunctionColumn::Parameters => "Parameters",
            FunctionColumn::Origin => "Origin",
        }
    }

    /// The zero-based position, for the pane's recorded header rectangles.
    pub fn index(self) -> usize {
        match self {
            FunctionColumn::Name => 0,
            FunctionColumn::Parameters => 1,
            FunctionColumn::Origin => 2,
        }
    }

    fn key(self, row: &FunctionRow) -> String {
        match self {
            FunctionColumn::Name => row.name.clone(),
            FunctionColumn::Parameters => row.parameters_text(),
            FunctionColumn::Origin => row.origin.label().to_string(),
        }
    }
}

/// Builds one row per captured variable, in the capture's ordinal order.
pub fn variable_rows(state: &EngineState) -> Vec<VariableRow> {
    state
        .variables
        .iter()
        .enumerate()
        .map(|(ordinal, variable)| VariableRow {
            name: variable.name.clone(),
            kind: variable.kind.clone(),
            display: variable.display.clone(),
            preview: ValuePreview::of(variable),
            structured: variable.structured.clone(),
            ordinal,
        })
        .collect()
}

/// Builds one row per captured function, in the capture's ordinal order.
pub fn function_rows(state: &EngineState) -> Vec<FunctionRow> {
    state
        .functions
        .iter()
        .enumerate()
        .map(|(ordinal, function)| function_row(ordinal, function))
        .collect()
}

fn function_row(ordinal: usize, function: &EngineFunction) -> FunctionRow {
    FunctionRow {
        name: function.name.clone(),
        parameters: function.parameters.clone(),
        origin: function.origin(),
        module: function.module.clone(),
        summary: function.summary.clone(),
        ordinal,
    }
}

/// The variable row indices in `sort` order.
///
/// Equal keys keep capture order in both directions: the ordinal is the final comparison, so
/// descending by kind cannot reverse two rows that share a kind, and every result is a
/// permutation of `0..rows.len()` (sorting never drops a row).
pub fn ordered_variables(rows: &[VariableRow], sort: SortState<VariableColumn>) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..rows.len()).collect();
    let descending = !sort.ascending;
    let column = sort.key;
    indices.sort_by(|&left, &right| {
        let left_row = &rows[left];
        let right_row = &rows[right];
        let ordering = column.key(left_row).cmp(column.key(right_row));
        let ordering = if descending {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| left_row.ordinal.cmp(&right_row.ordinal))
    });
    indices
}

/// The function row indices in `sort` order, with the same stability contract as
/// [`ordered_variables`].
pub fn ordered_functions(rows: &[FunctionRow], sort: SortState<FunctionColumn>) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..rows.len()).collect();
    let descending = !sort.ascending;
    let column = sort.key;
    indices.sort_by(|&left, &right| {
        let left_row = &rows[left];
        let right_row = &rows[right];
        let ordering = column.key(left_row).cmp(&column.key(right_row));
        let ordering = if descending {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| left_row.ordinal.cmp(&right_row.ordinal))
    });
    indices
}

/// The variables table, with its rows and the current sort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableTable {
    /// Every captured variable, in capture order; sorting never mutates this list.
    pub rows: Vec<VariableRow>,
    /// The current column and direction.
    pub sort: SortState<VariableColumn>,
}

impl Default for VariableTable {
    fn default() -> VariableTable {
        VariableTable {
            rows: Vec::new(),
            sort: SortState::new(VariableColumn::Name),
        }
    }
}

impl VariableTable {
    /// Builds the table from a capture; the default sort is name ascending (ordinal).
    pub fn new(state: &EngineState) -> VariableTable {
        VariableTable {
            rows: variable_rows(state),
            sort: SortState::new(VariableColumn::Name),
        }
    }

    /// Replaces the rows with a new capture, keeping the user's current sort.
    pub fn rebuild(&mut self, state: &EngineState) {
        self.rows = variable_rows(state);
    }

    /// The row indices in current sort order.
    ///
    /// Equal keys keep capture order in both directions: the ordinal is the final comparison,
    /// so descending by kind cannot reverse two rows that share a kind.
    #[cfg_attr(not(test), allow(dead_code))] // host/test accessor; the pane uses `ordered_variables`
    pub fn ordered(&self) -> Vec<usize> {
        ordered_variables(&self.rows, self.sort)
    }
}

/// The functions table, with its rows and the current sort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionTable {
    /// Every captured function, in capture order; sorting never mutates this list.
    pub rows: Vec<FunctionRow>,
    /// The current column and direction.
    pub sort: SortState<FunctionColumn>,
}

impl Default for FunctionTable {
    fn default() -> FunctionTable {
        FunctionTable {
            rows: Vec::new(),
            sort: SortState::new(FunctionColumn::Name),
        }
    }
}

impl FunctionTable {
    /// Builds the table from a capture; the default sort is name ascending (ordinal).
    pub fn new(state: &EngineState) -> FunctionTable {
        FunctionTable {
            rows: function_rows(state),
            sort: SortState::new(FunctionColumn::Name),
        }
    }

    /// Replaces the rows with a new capture, keeping the user's current sort.
    pub fn rebuild(&mut self, state: &EngineState) {
        self.rows = function_rows(state);
    }

    /// The row indices in current sort order; ties keep capture order like the variables table.
    #[cfg_attr(not(test), allow(dead_code))] // host/test accessor; the pane uses `ordered_functions`
    pub fn ordered(&self) -> Vec<usize> {
        ordered_functions(&self.rows, self.sort)
    }

    /// The engine registry rows, by origin. The table tests use it to check the visible split.
    #[cfg_attr(not(test), allow(dead_code))] // invariant accessor used by the table tests
    pub fn count_by_origin(&self, origin: FunctionOrigin) -> usize {
        self.rows.iter().filter(|row| row.origin == origin).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineFunction, EngineState, EngineVariable};

    fn variable(name: &str, kind: &str, display: &str) -> EngineVariable {
        EngineVariable {
            name: name.to_string(),
            kind: kind.to_string(),
            display: display.to_string(),
            structured: None,
        }
    }

    fn function(
        name: &str,
        parameters: &[&str],
        is_builtin: bool,
        module: &str,
        summary: Option<&str>,
    ) -> EngineFunction {
        EngineFunction {
            name: name.to_string(),
            parameters: parameters.iter().map(|p| p.to_string()).collect(),
            is_builtin,
            module: module.to_string(),
            summary: summary.map(str::to_string),
        }
    }

    fn names(table: &VariableTable) -> Vec<String> {
        table
            .ordered()
            .into_iter()
            .map(|index| table.rows[index].name.clone())
            .collect()
    }

    fn function_names(table: &FunctionTable) -> Vec<String> {
        table
            .ordered()
            .into_iter()
            .map(|index| table.rows[index].name.clone())
            .collect()
    }

    /// WKS-001: the default is ordinal name ascending, and `_` is a normal row with its kind
    /// and display value, not a special case that a filter or a dedup can lose.
    #[test]
    fn variables_default_to_name_ascending_and_include_the_underscore_global() {
        let state = EngineState {
            variables: vec![
                variable("b", "Text", "\"two\""),
                variable("_", "Natural", "42"),
                variable("a", "Natural", "1"),
            ],
            functions: Vec::new(),
        };
        let table = VariableTable::new(&state);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(names(&table), vec!["_", "a", "b"]);
        let underscore = table
            .rows
            .iter()
            .find(|row| row.name == "_")
            .expect("the answer global must be a row");
        assert_eq!(underscore.kind, "Natural");
        assert_eq!(underscore.display, "42");

        // Sort by kind: Natural rows tie, so their capture order (_, a) is kept; Text follows.
        let mut kinded = table.clone();
        kinded.sort.clicked(VariableColumn::Kind);
        let ordered: Vec<&VariableRow> =
            kinded.ordered().iter().map(|i| &kinded.rows[*i]).collect();
        assert_eq!(
            ordered
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["_", "a", "b"]
        );

        // The same set sorted both directions by name: `_` is first ascending and last
        // descending, and no row disappears in either direction.
        let mut by_name = table.clone();
        by_name.sort.clicked(VariableColumn::Name); // first click: ascending
        assert_eq!(names(&by_name), vec!["_", "a", "b"]);
        by_name.sort.clicked(VariableColumn::Name); // second click: descending
        assert_eq!(names(&by_name), vec!["b", "a", "_"]);
        by_name.sort.clicked(VariableColumn::Name); // third click: ascending
        assert_eq!(names(&by_name), vec!["_", "a", "b"]);
    }

    /// WKS-001: first click ascending, second descending, third ascending; a new column starts
    /// ascending instead of inheriting the previous column's direction.
    #[test]
    fn a_header_click_sorts_ascending_then_descending_and_a_new_column_restarts_ascending() {
        let state = EngineState {
            variables: vec![
                variable("a", "Text", "1"),
                variable("b", "Natural", "2"),
                variable("c", "Real", "3"),
            ],
            functions: Vec::new(),
        };
        let mut table = VariableTable::new(&state);
        assert_eq!(
            names(&table),
            vec!["a", "b", "c"],
            "default is name ascending"
        );
        assert!(table.sort.ascending);

        // The default column starts on its ascending side: its first click stays ascending and
        // its second click is the descending one.
        table.sort.clicked(VariableColumn::Name);
        assert!(table.sort.ascending);
        assert_eq!(
            names(&table),
            vec!["a", "b", "c"],
            "first click is ascending"
        );

        table.sort.clicked(VariableColumn::Name);
        assert!(!table.sort.ascending);
        assert_eq!(names(&table), vec!["c", "b", "a"], "second click descends");

        table.sort.clicked(VariableColumn::Name);
        assert!(table.sort.ascending);
        assert_eq!(
            names(&table),
            vec!["a", "b", "c"],
            "third click ascends again"
        );

        // A different column also starts at ascending even though Name was just descending.
        table.sort.clicked(VariableColumn::Display);
        assert_eq!(table.sort.key, VariableColumn::Display);
        assert!(table.sort.ascending, "a new column starts ascending");
        table.sort.clicked(VariableColumn::Display);
        assert!(!table.sort.ascending, "second click on Display descends");
        table.sort.clicked(VariableColumn::Name);
        assert_eq!(table.sort.key, VariableColumn::Name);
        assert!(table.sort.ascending, "returning to Name starts ascending");
        assert_eq!(names(&table), vec!["a", "b", "c"]);
    }

    /// Every sort is a permutation of the same rows, and tied keys stay in capture order in
    /// both directions (the ordinal tie-break; no unstable reorder and no lost row).
    #[test]
    fn sorting_is_stable_for_tied_keys_and_never_loses_a_row() {
        let state = EngineState {
            variables: vec![
                variable("z", "Natural", "7"),
                variable("m", "Natural", "7"),
                variable("a", "Natural", "7"),
                variable("n", "Text", "same"),
            ],
            functions: Vec::new(),
        };
        let mut table = VariableTable::new(&state);
        let ordinals = |table: &VariableTable| -> Vec<usize> {
            table
                .ordered()
                .into_iter()
                .map(|i| table.rows[i].ordinal)
                .collect()
        };

        // Descending by kind: Text before Natural, and the three Natural rows keep 0,1,2.
        table.sort.clicked(VariableColumn::Kind);
        table.sort.clicked(VariableColumn::Kind);
        assert!(!table.sort.ascending);
        let ordered = table.ordered();
        assert_eq!(ordered.len(), table.rows.len(), "no row may be dropped");
        let mut seen: Vec<usize> = ordered.iter().map(|i| table.rows[*i].ordinal).collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2, 3], "every row appears exactly once");
        assert_eq!(
            ordinals(&table),
            vec![3, 0, 1, 2],
            "the three Natural rows (capture order z, m, a) stay in capture order"
        );
    }

    /// WKS-002: the functions table reads origin from the engine's builtin flag and the static
    /// module label, joins the parameters with a comma, and carries the registry summary when
    /// present.
    #[test]
    fn functions_show_parameters_origin_and_summary() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![
                function("aaa", &["x"], false, "User", None),
                function("abs", &["x"], true, "Core", Some("Absolute value.")),
                function(
                    "acos",
                    &["x"],
                    true,
                    "Symbolics",
                    Some("The principal arccosine."),
                ),
            ],
        };
        let table = FunctionTable::new(&state);
        assert_eq!(function_names(&table), vec!["aaa", "abs", "acos"]);

        let user = table.rows.iter().find(|row| row.name == "aaa").unwrap();
        assert_eq!(user.origin, FunctionOrigin::User);
        assert_eq!(user.origin_label(), "user");
        assert_eq!(user.parameters_text(), "x");
        assert_eq!(user.summary_cell(), None);

        let builtin = table.rows.iter().find(|row| row.name == "abs").unwrap();
        assert_eq!(builtin.origin, FunctionOrigin::Builtin);
        assert_eq!(builtin.origin_label(), "builtin");
        assert_eq!(builtin.summary_cell().as_deref(), Some("Absolute value."));

        let symbolic = table.rows.iter().find(|row| row.name == "acos").unwrap();
        assert_eq!(symbolic.origin, FunctionOrigin::Builtin);
        assert_eq!(symbolic.origin_label(), "builtin");
        assert_eq!(symbolic.module, "Symbolics");
        assert_eq!(
            symbolic.summary_cell().as_deref(),
            Some("The principal arccosine.")
        );
        assert_eq!(table.count_by_origin(FunctionOrigin::User), 1);
        assert_eq!(table.count_by_origin(FunctionOrigin::Builtin), 2);
    }

    /// WKS-002 count agreement: the table is built from the snapshot's registry capture, so the
    /// displayed count is exactly the snapshot's function count (a legacy `functions` list that
    /// names only the user definition cannot shrink it).
    #[test]
    fn the_function_count_is_the_snapshot_registry_count() {
        let state = EngineState {
            variables: Vec::new(),
            functions: vec![
                function("aaa", &["x"], false, "User", None),
                function("abs", &["x"], true, "Core", Some("Absolute value.")),
                function("acos", &["x"], true, "Symbolics", None),
            ],
        };
        let legacy_functions = vec![("aaa".to_string(), vec!["x".to_string()])];
        let snapshot = crate::execution::WorkspaceSnapshot {
            result: String::new(),
            result_structured: None,
            result_inspection: None,
            variables: Vec::new(),
            functions: legacy_functions,
            state: state.clone(),
            plot: None,
            output: Vec::new(),
            revision: crate::execution::SourceRevision(1),
            committed_steps: 1,
            total: 1,
            precision: crate::engine::DEFAULT_PRECISION,
            diagnostic: None,
        };
        let table = FunctionTable::new(&snapshot.state);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows.len(), snapshot.state.functions.len());
        assert_ne!(
            table.rows.len(),
            snapshot.functions.len(),
            "the table must use the registry capture, not the legacy user-only list"
        );
    }

    /// WKS-006: numbers, matrices, complex, record and symbolic values each render a readable,
    /// type-aware preview derived from the engine's projection — never by re-parsing the display.
    #[test]
    fn value_previews_name_the_type_and_read_the_published_structure() {
        let json = |fields: Vec<(&str, Json)>| Json::obj(fields.into_iter().collect::<Vec<_>>());
        let scalar = |kind: &str, value: &str| {
            json(vec![
                ("kind", Json::Str(kind.to_string())),
                ("value", Json::Str(value.to_string())),
            ])
        };

        // A number: the published kind and value, not the display string.
        let number = EngineVariable {
            name: "n".to_string(),
            kind: "Natural".to_string(),
            display: "42".to_string(),
            structured: Some(scalar("Natural", "42")),
        };
        let preview = ValuePreview::of(&number);
        assert_eq!(preview.type_label, "natural");
        assert_eq!(preview.text, "42");
        assert!(!preview.shortened);

        // A 2x2 matrix: rows and columns from the published shape, element values row-major.
        let matrix = EngineVariable {
            name: "m".to_string(),
            kind: "Array".to_string(),
            display: "[[1, 2], [3, 4]]".to_string(),
            structured: Some(json(vec![
                ("kind", Json::Str("Array".to_string())),
                ("type", Json::Str("Array".to_string())),
                ("shape", Json::num_array(vec![2, 2])),
                (
                    "elements",
                    Json::Arr(vec![
                        scalar("Natural", "1"),
                        scalar("Natural", "2"),
                        scalar("Natural", "3"),
                        scalar("Natural", "4"),
                    ]),
                ),
            ])),
        };
        let preview = ValuePreview::of(&matrix);
        assert_eq!(preview.type_label, "2 x 2 matrix");
        assert_eq!(preview.text, "[1, 2]; [3, 4]");

        // A complex number: the published real and imaginary parts.
        let complex = EngineVariable {
            name: "c".to_string(),
            kind: "Complex".to_string(),
            display: "3 + 4i".to_string(),
            structured: Some(json(vec![
                ("kind", Json::Str("Complex".to_string())),
                ("re", Json::Str("3".to_string())),
                ("im", Json::Str("-4".to_string())),
                ("exact", Json::Bool(true)),
            ])),
        };
        let preview = ValuePreview::of(&complex);
        assert_eq!(preview.type_label, "complex");
        assert_eq!(preview.text, "3 - 4i");

        // A record: the published type and its first field names and values.
        let record = EngineVariable {
            name: "r".to_string(),
            kind: "Record".to_string(),
            display: "DivRemResult(quotient: 9, remainder: 1)".to_string(),
            structured: Some(json(vec![
                ("kind", Json::Str("Record".to_string())),
                ("type", Json::Str("DivRemResult".to_string())),
                (
                    "fields",
                    Json::Arr(vec![
                        json(vec![
                            ("name", Json::Str("quotient".to_string())),
                            ("value", scalar("Integer", "9")),
                        ]),
                        json(vec![
                            ("name", Json::Str("remainder".to_string())),
                            ("value", scalar("Integer", "1")),
                        ]),
                    ]),
                ),
            ])),
        };
        let preview = ValuePreview::of(&record);
        assert_eq!(preview.type_label, "record DivRemResult");
        assert_eq!(preview.text, "DivRemResult {quotient=9, remainder=1}");

        // A symbolic expression: the published pretty rendering.
        let symbolic = EngineVariable {
            name: "s".to_string(),
            kind: "Symbolic".to_string(),
            display: "3*x^2 + 4*x + 5".to_string(),
            structured: Some(json(vec![
                ("kind", Json::Str("Symbolic".to_string())),
                ("pretty", Json::Str("3*x^2 + 4*x + 5".to_string())),
                ("canonical", Json::Str("(add ...)".to_string())),
            ])),
        };
        let preview = ValuePreview::of(&symbolic);
        assert_eq!(preview.type_label, "symbolic");
        assert_eq!(preview.text, "3*x^2 + 4*x + 5");

        // No projection: the kind and the display text, with no invented structure.
        let bare = EngineVariable {
            name: "t".to_string(),
            kind: "Text".to_string(),
            display: "\"hi\"".to_string(),
            structured: None,
        };
        let preview = ValuePreview::of(&bare);
        assert_eq!(preview.type_label, "text");
        assert_eq!(preview.text, "\"hi\"");
    }

    /// WKS-006: a huge matrix previews a bounded window of rows and says it was shortened, while
    /// the row keeps the full display and the full projection for the detail view.
    #[test]
    fn a_huge_matrix_preview_is_bounded_and_the_full_value_stays_in_the_model() {
        let cell = |value: usize| {
            Json::obj(vec![
                ("kind", Json::Str("Natural".to_string())),
                ("value", Json::Str(value.to_string())),
            ])
        };
        let elements: Vec<Json> = (0..10_000).map(cell).collect();
        let display = "[[… 10000 elements …]]".to_string();
        let variable = EngineVariable {
            name: "big".to_string(),
            kind: "Array".to_string(),
            display: display.clone(),
            structured: Some(Json::obj(vec![
                ("kind", Json::Str("Array".to_string())),
                ("type", Json::Str("Array".to_string())),
                ("shape", Json::num_array(vec![100, 100])),
                ("elements", Json::Arr(elements)),
            ])),
        };
        let state = EngineState {
            variables: vec![variable],
            functions: Vec::new(),
        };
        let table = VariableTable::new(&state);
        let row = &table.rows[0];
        assert_eq!(row.preview.type_label, "100 x 100 matrix");
        assert!(row.preview.shortened, "a 10 000-cell preview is shortened");
        assert!(
            row.preview.text.chars().count() <= MAX_PREVIEW_CHARS,
            "the preview stays one bounded cell: {} chars",
            row.preview.text.chars().count()
        );
        assert_eq!(row.display, display, "the full display is not truncated");
        assert_eq!(
            row.structured
                .as_ref()
                .and_then(|structured| structured.get("elements"))
                .and_then(Json::as_arr)
                .map(<[Json]>::len),
            Some(10_000),
            "the full projection is kept for the detail view"
        );
    }

    /// Large values are cut in the cell but the model keeps the full text for the tooltip.
    #[test]
    fn a_large_value_is_truncated_for_the_cell_and_kept_for_the_tooltip() {
        let long = "1234567890".repeat(10);
        let row = VariableRow {
            name: "big".to_string(),
            kind: "Array".to_string(),
            display: long.clone(),
            preview: ValuePreview {
                type_label: "array".to_string(),
                text: cell_text(&long),
                shortened: true,
            },
            structured: None,
            ordinal: 0,
        };
        let cell = row.display_cell();
        assert_eq!(cell.chars().count(), MAX_CELL_CHARS);
        assert!(
            cell.ends_with(ELLIPSIS),
            "cut cells end in an ellipsis: {cell:?}"
        );
        assert_eq!(row.display, long, "the tooltip text is not truncated");
        assert_eq!(cell_text("short"), "short");
    }

    /// A multi-byte value is cut at a character boundary, never inside a UTF-8 sequence.
    #[test]
    fn truncation_counts_characters_not_bytes() {
        let text = "é".repeat(MAX_CELL_CHARS + 5);
        let cut = cell_text(&text);
        assert_eq!(cut.chars().count(), MAX_CELL_CHARS);
        assert!(cut.ends_with(ELLIPSIS));
        assert!(cut.chars().all(|ch| ch == 'é' || ch == ELLIPSIS));
    }
}
