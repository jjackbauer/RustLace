//! SYM-001: the typed model of the engine's symbolic inspection payload.
//!
//! The window renders this model; it never derives a tree, a trace or an IR from display text.
//! The payload is produced by the engine adapter at the moment a value is committed:
//!
//! | Section | Engine source (see `engine::real::inspection`) |
//! | --- | --- |
//! | pretty | `structured::project_value` of the symbolic subject |
//! | canonical | `structured::project_value` of the symbolic subject |
//! | expression tree | the engine's own `lovelace_symbolics::Expr` carried by `Value::Symbolic` |
//! | assumptions | `inspect(value).assumptions`, plus a `TransformResult`'s `conditions` |
//! | trace | a `TransformResult`'s `steps` (the `simplify_full` record) |
//! | MathIR | the engine's `mathir::compile_ordered` path over the subject |
//!
//! Every section is either supplied or carries the specific engine reason it is not. A missing
//! section is therefore a statement about this result, never an empty box.

use std::collections::BTreeMap;

use lovelace_json::Json;

/// One payload section: the engine's data, or the reason the engine supplied none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section<T> {
    /// The engine supplied this section.
    Supplied(T),
    /// The engine supplied no section; the string is the specific reason.
    Missing(String),
}

impl<T> Section<T> {
    /// The section's value, when supplied.
    #[allow(dead_code)] // model accessor: the tests read it
    pub fn supplied(&self) -> Option<&T> {
        match self {
            Section::Supplied(value) => Some(value),
            Section::Missing(_) => None,
        }
    }

    /// The specific reason, when the section is missing.
    #[allow(dead_code)] // model accessor: the tests read it
    pub fn reason(&self) -> Option<&str> {
        match self {
            Section::Supplied(_) => None,
            Section::Missing(reason) => Some(reason),
        }
    }
}

/// The value the inspector is bound to and where it sits in the last committed result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    /// The engine kind or record type (`Symbolic`, `TransformResult`, `Integer`, ...).
    pub kind: String,
    /// The path inside the result (`result` or `result.expression`).
    pub path: String,
    /// Optional engine detail, such as a `TransformResult`'s status.
    pub detail: Option<String>,
}

/// One typed assumption / condition row.
///
/// A relation is kept typed: `op` is the engine's relation operator code (`gt`, `le`, ...) and
/// `left`/`right` are its operands, so the pane never has to split a sentence. Records such as a
/// `FiniteCondition` keep their record `kind` and the engine's pretty rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assumption {
    /// Where the row came from: `session` (inspect), `result` (TransformResult conditions) or
    /// `required` (one trace step's required conditions).
    pub source: String,
    /// The engine's structured condition kind (`Relation`, `FiniteCondition`, ...).
    pub kind: String,
    /// The engine's human rendering of the condition.
    pub label: String,
    /// The engine's canonical S-expression, when the condition is symbolic.
    pub canonical: Option<String>,
    /// The relation operator code, for a symbolic relation (`gt`, `le`, ...).
    pub op: Option<String>,
    /// The relation operator as published (`>`, `<=`, ...).
    pub op_label: Option<String>,
    /// The relation's left operand, pretty-printed by the engine.
    pub left: Option<String>,
    /// The relation's right operand, pretty-printed by the engine.
    pub right: Option<String>,
}

impl Assumption {
    /// The typed description the pane paints before the rendering: `relation >` or the record
    /// kind. It is derived from the payload's own fields, not from the display text.
    pub fn type_label(&self) -> String {
        match (&self.kind[..], self.op_label.as_deref()) {
            ("Relation", Some(op)) => format!("relation {op}"),
            (kind, _) => kind.to_string(),
        }
    }
}

/// One rewrite step of a `TransformResult`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStep {
    /// The engine's stable rewrite-rule id, e.g. `logexp.exp-log`.
    pub rule_id: String,
    /// The engine's classification: `Universal`, `Conditional` or `DomainSpecific`.
    pub classification: String,
    /// SYM-005: whether this rewrite's preconditions were actually met, in the engine's own
    /// vocabulary (`Applicable` or `ApplicableWithConditions`). Empty when the engine published
    /// none for this step, which the pane states as such instead of implying applicability.
    pub applicability: String,
    /// The rewritten sub-expression, pretty-printed by the engine.
    pub before: String,
    /// The rewritten sub-expression's canonical form.
    pub before_canonical: String,
    /// The produced sub-expression, pretty-printed by the engine.
    pub after: String,
    /// The produced sub-expression's canonical form.
    pub after_canonical: String,
    /// The conditions this step requires (typed condition rows).
    pub conditions: Vec<Assumption>,
}

impl TraceStep {
    /// SYM-005: the applicability as the pane paints it, or the explicit absence when the engine
    /// published none. The two words the row requires — applied / conditioned — map from the
    /// engine's own enum, never from a guess about the conditions list.
    pub fn applicability_label(&self) -> String {
        match self.applicability.as_str() {
            "Applicable" => "applicable (preconditions met unconditionally)".to_string(),
            "ApplicableWithConditions" => {
                "conditioned (met under the required conditions)".to_string()
            }
            "" => "not published by this engine for this step".to_string(),
            other => other.to_string(),
        }
    }
}

/// SYM-005: one rule the engine tried and rejected, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleAttempt {
    /// The stable rewrite-rule id.
    pub rule_id: String,
    /// True when the rule's pattern matched the node (independently of applicability).
    pub pattern_matched: bool,
    /// The engine's applicability decision (`NotApplicable`, `Unknown`).
    pub precondition: String,
    /// The engine's reason the attempt did not fire.
    pub reason: String,
}

impl RuleAttempt {
    /// The applicability as the pane paints it: `not applicable` / `not decided`, with the
    /// engine's precondition name kept visible so the two never blur.
    pub fn status_label(&self) -> &'static str {
        match self.precondition.as_str() {
            "NotApplicable" => "not applicable",
            "Unknown" => "not decided",
            "Applicable" => "applicable",
            "ApplicableWithConditions" => "conditioned",
            _ => "not published",
        }
    }
}

/// The lowered MathIR text and its compilation contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathIr {
    /// The serialized IR, exactly as the engine's compile path emitted it.
    pub text: String,
    /// The parameter names the compile call was made with.
    pub parameters: Vec<String>,
    /// The engine's inferred result type, when it reported one.
    pub result_type: Option<String>,
}

/// One node of the engine's expression tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    /// The engine node kind (`Num`, `Sym`, `Add`, `Mul`, `Pow`, `Fun`, `Div`, `Rel`, `Real`).
    pub kind: String,
    /// The function name or relation operator for a `Fun`/`Rel` node.
    pub detail: Option<String>,
    /// The engine's pretty rendering of this sub-expression.
    pub label: String,
    /// SYM-003: the engine's canonical rendering of this sub-expression, so one node's canonical
    /// form can be copied without re-deriving it from the pretty text.
    pub canonical: String,
    /// The child nodes, in the engine's own child order.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// The number of nodes in this subtree, root included.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(TreeNode::count).sum::<usize>()
    }

    /// The header text: node kind, optional detail, then the engine rendering.
    #[allow(dead_code)] // model accessor: the row model renders the same header for paint
    pub fn header(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{} {} · {}", self.kind, detail, self.label),
            None => format!("{} · {}", self.kind, self.label),
        }
    }

    /// The number of levels in this subtree: 1 for a leaf, 1 + the deepest child's levels.
    pub fn depth(&self) -> usize {
        match self.children.iter().map(TreeNode::depth).max() {
            Some(child) => 1 + child,
            None => 1,
        }
    }
}

/// One engine inspection payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    /// The subject value and its path in the result.
    pub subject: Subject,
    /// `pretty`: the engine's pretty rendering of the subject.
    pub pretty: Section<String>,
    /// `canonical`: the engine's canonical rendering of the subject.
    pub canonical: Section<String>,
    /// `tree`: the engine expression tree.
    pub tree: Section<TreeNode>,
    /// `assumptions`: typed session assumptions and TransformResult conditions.
    pub assumptions: Section<Vec<Assumption>>,
    /// `trace`: the `simplify_full` rewrite steps.
    pub trace: Section<Vec<TraceStep>>,
    /// SYM-005: the rule applicability the result publishes — the fired rules' decisions and the
    /// rejected rules' reasons.
    pub applicability: Section<Vec<RuleAttempt>>,
    /// `mathir`: the compiled IR.
    pub mathir: Section<MathIr>,
}

impl Inspection {
    /// Reads the desktop-owned payload the engine adapter publishes.
    ///
    /// Returns `None` only when the payload is not an object or has no subject. A malformed
    /// section is `Missing` with a reason saying the payload was unreadable, so a schema mismatch
    /// is visible instead of silently blanking the pane.
    pub fn from_json(json: &Json) -> Option<Inspection> {
        let subject_json = json.get("subject")?;
        let subject = Subject {
            kind: subject_json
                .get("kind")
                .and_then(Json::as_str)
                .unwrap_or("Unknown")
                .to_string(),
            path: subject_json
                .get("path")
                .and_then(Json::as_str)
                .unwrap_or("result")
                .to_string(),
            detail: subject_json
                .get("detail")
                .and_then(Json::as_str)
                .map(str::to_string),
        };
        let sections = json.get("sections");
        let section = |name: &str| sections.and_then(|sections| sections.get(name));
        Some(Inspection {
            subject,
            pretty: text_section(section("pretty"), "pretty"),
            canonical: text_section(section("canonical"), "canonical"),
            tree: tree_section(section("tree")),
            assumptions: assumptions_section(section("assumptions")),
            trace: trace_section(section("trace")),
            applicability: applicability_section(section("applicability")),
            mathir: mathir_section(section("mathir")),
        })
    }

    /// The seven section names, in the order the inspector paints them.
    #[allow(dead_code)] // model vocabulary: the round document and tests name them
    pub const SECTION_NAMES: [&'static str; 7] = [
        "pretty",
        "canonical",
        "expression tree",
        "assumptions",
        "simplification trace",
        "applicability",
        "MathIR",
    ];
}

fn missing(name: &str, reason: &str) -> Section<String> {
    Section::Missing(format!("The engine supplied no {name}: {reason}"))
}

fn malformed(name: &str) -> Section<String> {
    Section::Missing(format!(
        "The engine's {name} section was not readable (unexpected payload shape)."
    ))
}

fn text_section(json: Option<&Json>, name: &str) -> Section<String> {
    let Some(json) = json else {
        return missing(name, "the payload has no such section.");
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    match json.get("text").and_then(Json::as_str) {
        Some(text) => Section::Supplied(text.to_string()),
        None => malformed(name),
    }
}

fn tree_section(json: Option<&Json>) -> Section<TreeNode> {
    let Some(json) = json else {
        return Section::Missing(
            "The engine supplied no expression tree: the payload has no such section.".to_string(),
        );
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    match json.get("root").and_then(tree_node) {
        Some(root) => Section::Supplied(root),
        None => Section::Missing(
            "The engine's expression tree section was not readable (unexpected payload shape)."
                .to_string(),
        ),
    }
}

fn tree_node(json: &Json) -> Option<TreeNode> {
    let kind = json.get("kind").and_then(Json::as_str)?.to_string();
    let label = json.get("label").and_then(Json::as_str)?.to_string();
    let canonical = json
        .get("canonical")
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_string();
    let detail = json
        .get("detail")
        .and_then(Json::as_str)
        .map(str::to_string);
    let children = json
        .get("children")
        .and_then(Json::as_arr)
        .map(|children| children.iter().filter_map(tree_node).collect())
        .unwrap_or_default();
    Some(TreeNode {
        kind,
        detail,
        label,
        canonical,
        children,
    })
}

fn assumptions_section(json: Option<&Json>) -> Section<Vec<Assumption>> {
    let Some(json) = json else {
        return Section::Missing(
            "The engine supplied no assumptions: the payload has no such section.".to_string(),
        );
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    match json.get("items").and_then(Json::as_arr) {
        Some(items) => Section::Supplied(items.iter().filter_map(assumption).collect()),
        None => Section::Missing(
            "The engine's assumptions section was not readable (unexpected payload shape)."
                .to_string(),
        ),
    }
}

fn assumption(json: &Json) -> Option<Assumption> {
    Some(Assumption {
        source: json
            .get("source")
            .and_then(Json::as_str)
            .unwrap_or("session")
            .to_string(),
        kind: json.get("kind").and_then(Json::as_str)?.to_string(),
        label: json
            .get("label")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string(),
        canonical: json
            .get("canonical")
            .and_then(Json::as_str)
            .map(str::to_string),
        op: json.get("op").and_then(Json::as_str).map(str::to_string),
        op_label: json
            .get("opLabel")
            .and_then(Json::as_str)
            .map(str::to_string),
        left: json.get("left").and_then(Json::as_str).map(str::to_string),
        right: json.get("right").and_then(Json::as_str).map(str::to_string),
    })
}

fn trace_section(json: Option<&Json>) -> Section<Vec<TraceStep>> {
    let Some(json) = json else {
        return Section::Missing(
            "The engine supplied no simplification trace: the payload has no such section."
                .to_string(),
        );
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    match json.get("steps").and_then(Json::as_arr) {
        Some(steps) => Section::Supplied(steps.iter().filter_map(trace_step).collect()),
        None => Section::Missing(
            "The engine's trace section was not readable (unexpected payload shape).".to_string(),
        ),
    }
}

fn trace_step(json: &Json) -> Option<TraceStep> {
    let pair = |key: &str, part: &str| -> String {
        json.get(key)
            .and_then(|value| value.get(part))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(TraceStep {
        rule_id: json.get("ruleId").and_then(Json::as_str)?.to_string(),
        classification: json
            .get("classification")
            .and_then(Json::as_str)
            .unwrap_or("Unclassified")
            .to_string(),
        applicability: json
            .get("applicability")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string(),
        before: pair("before", "pretty"),
        before_canonical: pair("before", "canonical"),
        after: pair("after", "pretty"),
        after_canonical: pair("after", "canonical"),
        conditions: json
            .get("conditions")
            .and_then(Json::as_arr)
            .map(|items| items.iter().filter_map(assumption).collect())
            .unwrap_or_default(),
    })
}

/// SYM-005: the applicability section.
///
/// The engine's `attempts` array carries one row per rejected rule; `counts` is its own summary.
/// A result whose rules all fired has an empty `attempts` array, which is `Supplied(empty)` — the
/// pane then says no rule was rejected, which is a fact, not a missing section.
fn applicability_section(json: Option<&Json>) -> Section<Vec<RuleAttempt>> {
    let Some(json) = json else {
        return Section::Missing(
            "The engine supplied no rewrite applicability: the payload has no such section."
                .to_string(),
        );
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    match json.get("attempts").and_then(Json::as_arr) {
        Some(attempts) => Section::Supplied(attempts.iter().filter_map(rule_attempt).collect()),
        None => Section::Missing(
            "The engine's applicability section was not readable (unexpected payload shape)."
                .to_string(),
        ),
    }
}

fn rule_attempt(json: &Json) -> Option<RuleAttempt> {
    Some(RuleAttempt {
        rule_id: json.get("ruleId").and_then(Json::as_str)?.to_string(),
        pattern_matched: matches!(json.get("patternMatched"), Some(Json::Bool(true))),
        precondition: json
            .get("precondition")
            .and_then(Json::as_str)
            .unwrap_or("Unknown")
            .to_string(),
        reason: json
            .get("reason")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn mathir_section(json: Option<&Json>) -> Section<MathIr> {
    let Some(json) = json else {
        return Section::Missing(
            "The engine supplied no MathIR: the payload has no such section.".to_string(),
        );
    };
    if let Some(reason) = json.get("reason").and_then(Json::as_str) {
        return Section::Missing(reason.to_string());
    }
    let Some(text) = json.get("text").and_then(Json::as_str) else {
        return Section::Missing(
            "The engine's MathIR section was not readable (unexpected payload shape).".to_string(),
        );
    };
    Section::Supplied(MathIr {
        text: text.to_string(),
        parameters: json
            .get("parameters")
            .and_then(Json::as_arr)
            .map(|parameters| {
                parameters
                    .iter()
                    .filter_map(Json::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        result_type: json
            .get("resultType")
            .and_then(Json::as_str)
            .map(str::to_string),
    })
}

/// SYM-011: the number of expression-tree levels painted on the first pass (root is level 1).
///
/// A tree is never opened whole: this bound is what keeps the first committed 10k-node expression
/// from laying out a hundred thousand wrapped labels before the user has asked for them.
pub const INITIAL_TREE_LEVELS: usize = 4;
/// SYM-011: the levels one "Load deeper" action adds.
pub const TREE_LEVELS_STEP: usize = 4;

/// The interaction state of the inspector's expression tree, scoped to one inspection payload.
///
/// The key is the payload's compact serialization, the same rule round 27 uses for the result
/// tree: an edit to the buffer cannot change it, while a new committed result can.
///
/// SYM-011: the state also carries how many levels are currently loaded (`levels`, 1 meaning only
/// the root) and the path a keyboard move has asked for. A node's children are only walked when
/// the node is expanded *and* its level is below `levels`, so expanding one node costs that node's
/// own children, never the whole expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorExpansion {
    identity: Option<String>,
    collapsed: BTreeMap<String, bool>,
    levels: usize,
    /// SYM-003: the tree path a keyboard traversal has asked the next pass to focus. It survives
    /// the pass that sets it so a row outside the painted range can be scrolled to first.
    pending_focus: Option<String>,
}

impl InspectorExpansion {
    /// A state map with no decisions: the root and the first levels start expanded.
    pub fn new() -> InspectorExpansion {
        InspectorExpansion {
            identity: None,
            collapsed: BTreeMap::new(),
            levels: INITIAL_TREE_LEVELS,
            pending_focus: None,
        }
    }

    /// Binds the map to one payload. Returns true when a previous result was replaced.
    pub fn bind(&mut self, inspection: &Json) -> bool {
        let identity = inspection.to_string();
        if self.identity.as_deref() == Some(identity.as_str()) {
            return false;
        }
        let replaced = self.identity.is_some();
        self.identity = Some(identity);
        self.collapsed.clear();
        self.levels = INITIAL_TREE_LEVELS;
        self.pending_focus = None;
        replaced
    }

    /// True when the node at `path` is collapsed. Unknown paths are expanded.
    pub fn is_collapsed(&self, path: &str) -> bool {
        self.collapsed.get(path).copied().unwrap_or(false)
    }

    /// Records one node's collapsed state without touching any other path.
    pub fn set_collapsed(&mut self, path: &str, collapsed: bool) {
        self.collapsed.insert(path.to_string(), collapsed);
    }

    /// Flips one node and returns its new collapsed state.
    pub fn toggle(&mut self, path: &str) -> bool {
        let collapsed = !self.is_collapsed(path);
        self.set_collapsed(path, collapsed);
        collapsed
    }

    /// SYM-011: the tree levels this state paints (level 1 is the root alone).
    pub fn levels(&self) -> usize {
        self.levels
    }

    /// SYM-011: loads one more step of levels and returns the new count.
    pub fn load_deeper(&mut self, deepest: usize) -> usize {
        self.levels = (self.levels + TREE_LEVELS_STEP).min(deepest.max(1));
        self.levels
    }

    /// SYM-011: true while the tree has levels the current bound hides.
    pub fn has_deeper(&self, deepest: usize) -> bool {
        self.levels < deepest
    }

    /// SYM-003: the path a keyboard move asked the next pass to focus.
    pub fn pending_focus(&self) -> Option<&str> {
        self.pending_focus.as_deref()
    }

    /// SYM-003: asks the next pass to focus one row and, when needed, scroll it into range.
    pub fn set_pending_focus(&mut self, path: &str) {
        self.pending_focus = Some(path.to_string());
    }

    /// SYM-003: clears the pending focus once a pass has honoured it.
    pub fn clear_pending_focus(&mut self) {
        self.pending_focus = None;
    }

    /// The paths the user has collapsed, in path order.
    #[allow(dead_code)] // model accessor: the tests read it
    pub fn collapsed_paths(&self) -> Vec<&str> {
        self.collapsed
            .iter()
            .filter(|(_, collapsed)| **collapsed)
            .map(|(path, _)| path.as_str())
            .collect()
    }
}

/// One painted row of the expression tree.
///
/// The rows are produced from the engine tree and the current expansion state, so the pane paints
/// exactly the rows it is asked for and can skip every other row of a giant expression (SYM-011).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    /// The node's path in the tree (`inspect.tree.0.1`).
    pub path: String,
    /// 0 for the root.
    pub level: usize,
    /// The engine node kind.
    pub kind: String,
    /// The engine detail (function name or relation operator).
    pub detail: Option<String>,
    /// The engine's pretty rendering of this sub-expression.
    pub label: String,
    /// The engine's canonical rendering of this sub-expression.
    pub canonical: String,
    /// Whether the node has children at all.
    pub has_children: bool,
    /// Whether the user collapsed this node.
    pub collapsed: bool,
    /// The descendant nodes this row hides because they are past the loaded level bound.
    pub deeper_nodes: usize,
}

impl TreeRow {
    /// The header text: node kind, optional detail, then the engine rendering.
    pub fn header(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{} {} · {}", self.kind, detail, self.label),
            None => format!("{} · {}", self.kind, self.label),
        }
    }

    /// The accessible name of the row's header: header text, node path and state.
    pub fn accessible_name(&self) -> String {
        format!("{} [{}] {}", self.header(), self.path, self.state_word())
    }

    /// `collapsed`, `expanded` or `leaf`, the word a UIA client reads for this row.
    pub fn state_word(&self) -> &'static str {
        if !self.has_children {
            "leaf"
        } else if self.collapsed {
            "collapsed"
        } else {
            "expanded"
        }
    }

    /// The visible expand/collapse indicator for this row.
    pub fn glyph(&self) -> &'static str {
        if self.collapsed {
            crate::results_tree::COLLAPSED_GLYPH
        } else {
            crate::results_tree::EXPANDED_GLYPH
        }
    }
}

/// Builds the painted rows of `root` under `expansion`, in engine child order.
///
/// A node's subtree is walked only while the node is expanded and its level is below the loaded
/// bound, so the work is proportional to the painted rows, not to the tree.
pub fn tree_rows(root: &TreeNode, expansion: &InspectorExpansion) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    push_rows(root, TREE_ROOT_PATH, 0, expansion, &mut rows);
    rows
}

fn push_rows(
    node: &TreeNode,
    path: &str,
    level: usize,
    expansion: &InspectorExpansion,
    rows: &mut Vec<TreeRow>,
) {
    let collapsed = expansion.is_collapsed(path);
    let within = level + 1 < expansion.levels();
    let deeper_nodes = if within {
        0
    } else {
        node.children.iter().map(TreeNode::count).sum()
    };
    rows.push(TreeRow {
        path: path.to_string(),
        level,
        kind: node.kind.clone(),
        detail: node.detail.clone(),
        label: node.label.clone(),
        canonical: node.canonical.clone(),
        has_children: !node.children.is_empty(),
        collapsed,
        deeper_nodes,
    });
    if collapsed || !within {
        return;
    }
    for (index, child) in node.children.iter().enumerate() {
        push_rows(child, &tree_path(path, index), level + 1, expansion, rows);
    }
}

/// The path of the tree root in the inspector.
pub const TREE_ROOT_PATH: &str = "inspect.tree";

/// The path of a child node: `inspect.tree.0`, `inspect.tree.0.1`.
pub fn tree_path(parent: &str, index: usize) -> String {
    format!("{parent}.{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Json {
        Json::obj([("text", Json::Str(text.to_string()))])
    }

    fn relation() -> Json {
        Json::obj([
            ("source", Json::Str("session".to_string())),
            ("kind", Json::Str("Relation".to_string())),
            ("op", Json::Str("gt".to_string())),
            ("opLabel", Json::Str(">".to_string())),
            ("left", Json::Str("x".to_string())),
            ("right", Json::Str("0".to_string())),
            ("label", Json::Str("x > 0".to_string())),
            ("canonical", Json::Str("(gt (sym x) (rat 0 1))".to_string())),
        ])
    }

    fn full_payload() -> Json {
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
                    ("pretty", text("x^2 - 1")),
                    (
                        "canonical",
                        text("(add (pow (sym x) (rat 2 1)) (rat -1 1))"),
                    ),
                    (
                        "tree",
                        Json::obj([
                            ("nodes", Json::Num("5".to_string())),
                            (
                                "root",
                                Json::obj([
                                    ("kind", Json::Str("Add".to_string())),
                                    ("label", Json::Str("x^2 - 1".to_string())),
                                    (
                                        "canonical",
                                        Json::Str(
                                            "(add (pow (sym x) (rat 2 1)) (rat -1 1))".to_string(),
                                        ),
                                    ),
                                    (
                                        "children",
                                        Json::Arr(vec![
                                            Json::obj([
                                                ("kind", Json::Str("Pow".to_string())),
                                                ("label", Json::Str("x^2".to_string())),
                                                (
                                                    "canonical",
                                                    Json::Str(
                                                        "(pow (sym x) (rat 2 1))".to_string(),
                                                    ),
                                                ),
                                                ("children", Json::Arr(Vec::new())),
                                            ]),
                                            Json::obj([
                                                ("kind", Json::Str("Num".to_string())),
                                                ("label", Json::Str("-1".to_string())),
                                                ("canonical", Json::Str("(rat -1 1)".to_string())),
                                                ("children", Json::Arr(Vec::new())),
                                            ]),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                    ),
                    (
                        "assumptions",
                        Json::obj([("items", Json::Arr(vec![relation()]))]),
                    ),
                    (
                        "trace",
                        Json::obj([(
                            "steps",
                            Json::Arr(vec![Json::obj([
                                (
                                    "ruleId",
                                    Json::Str("trig.pythagorean-sin2-cos2".to_string()),
                                ),
                                ("classification", Json::Str("Universal".to_string())),
                                ("applicability", Json::Str("Applicable".to_string())),
                                (
                                    "before",
                                    Json::obj([
                                        ("pretty", Json::Str("sin(x)^2 + cos(x)^2".to_string())),
                                        (
                                            "canonical",
                                            Json::Str(
                                                "(add (pow (fun sin (sym x)) ...))".to_string(),
                                            ),
                                        ),
                                    ]),
                                ),
                                (
                                    "after",
                                    Json::obj([
                                        ("pretty", Json::Str("1".to_string())),
                                        ("canonical", Json::Str("(rat 1 1)".to_string())),
                                    ]),
                                ),
                                ("conditions", Json::Arr(vec![relation()])),
                            ])]),
                        )]),
                    ),
                    (
                        "applicability",
                        Json::obj([
                            (
                                "applied",
                                Json::Arr(vec![Json::obj([
                                    (
                                        "ruleId",
                                        Json::Str("trig.pythagorean-sin2-cos2".to_string()),
                                    ),
                                    ("applicability", Json::Str("Applicable".to_string())),
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
                    (
                        "mathir",
                        Json::obj([
                            ("text", Json::Str("#!mathir 2\nparam x".to_string())),
                            ("parameters", Json::str_array(["x"])),
                            ("resultType", Json::Str("Integer".to_string())),
                        ]),
                    ),
                ]),
            ),
        ])
    }

    #[test]
    fn a_full_payload_reads_all_seven_sections_from_the_engine_data() {
        let inspection = Inspection::from_json(&full_payload()).expect("payload");
        assert_eq!(inspection.subject.kind, "Symbolic");
        assert_eq!(inspection.subject.path, "result");
        assert_eq!(
            inspection.pretty.supplied().map(String::as_str),
            Some("x^2 - 1")
        );
        assert!(inspection.canonical.supplied().is_some());
        let tree = inspection.tree.supplied().expect("tree");
        assert_eq!(tree.kind, "Add");
        assert_eq!(tree.count(), 3);
        assert_eq!(tree.children[0].kind, "Pow");
        // SYM-003: each node carries the engine's canonical form of that very sub-expression.
        assert_eq!(tree.canonical, "(add (pow (sym x) (rat 2 1)) (rat -1 1))");
        assert_eq!(tree.children[0].canonical, "(pow (sym x) (rat 2 1))");
        assert_eq!(tree.children[1].canonical, "(rat -1 1)");
        let assumption = &inspection.assumptions.supplied().expect("assumptions")[0];
        assert_eq!(assumption.type_label(), "relation >");
        assert_eq!(assumption.left.as_deref(), Some("x"));
        let step = &inspection.trace.supplied().expect("trace")[0];
        assert_eq!(step.rule_id, "trig.pythagorean-sin2-cos2");
        assert_eq!(step.classification, "Universal");
        // SYM-005: the step states its applicability, and the pane's wording distinguishes the
        // unconditional case from the conditioned one.
        assert_eq!(step.applicability, "Applicable");
        assert_eq!(
            step.applicability_label(),
            "applicable (preconditions met unconditionally)"
        );
        assert_eq!(step.before, "sin(x)^2 + cos(x)^2");
        assert_eq!(step.conditions[0].source, "session");
        // SYM-005: a rejected rule keeps the engine's applicability AND its reason.
        let attempt = &inspection.applicability.supplied().expect("applicability")[0];
        assert_eq!(attempt.rule_id, "pow.sqrt-square-nonnegative");
        assert!(attempt.pattern_matched);
        assert_eq!(attempt.precondition, "NotApplicable");
        assert_eq!(attempt.status_label(), "not applicable");
        assert!(
            attempt.reason.contains("not known to be nonnegative"),
            "the unmet precondition's reason must survive the payload: {:?}",
            attempt.reason
        );
        let mathir = inspection.mathir.supplied().expect("mathir");
        assert_eq!(mathir.parameters, vec!["x".to_string()]);
        assert_eq!(mathir.result_type.as_deref(), Some("Integer"));
    }

    #[test]
    fn a_missing_mathir_keeps_its_reason_and_leaves_the_other_sections() {
        let payload = Json::obj([
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
                    ("pretty", text("x < 0")),
                    ("canonical", text("(lt (sym x) (rat 0 1))")),
                    (
                        "tree",
                        Json::obj([
                            ("nodes", Json::Num("3".to_string())),
                            (
                                "root",
                                Json::obj([
                                    ("kind", Json::Str("Rel".to_string())),
                                    ("detail", Json::Str("lt".to_string())),
                                    ("label", Json::Str("x < 0".to_string())),
                                    ("children", Json::Arr(Vec::new())),
                                ]),
                            ),
                        ]),
                    ),
                    (
                        "mathir",
                        Json::obj([(
                            "reason",
                            Json::Str(
                                "Node kind Relation cannot be lowered to MathIR.".to_string(),
                            ),
                        )]),
                    ),
                ]),
            ),
        ]);
        let inspection = Inspection::from_json(&payload).expect("payload");
        assert_eq!(
            inspection.mathir.reason(),
            Some("Node kind Relation cannot be lowered to MathIR.")
        );
        assert!(inspection.pretty.supplied().is_some());
        assert!(inspection.canonical.supplied().is_some());
        assert!(inspection.tree.supplied().is_some());
        // A section the payload omits is still a specific missing state, not None.
        assert!(
            inspection
                .trace
                .reason()
                .expect("trace reason")
                .contains("has no such section")
        );
    }

    #[test]
    fn the_expansion_map_survives_the_same_payload_and_clears_on_a_new_one() {
        let payload = full_payload();
        let mut expansion = InspectorExpansion::new();
        assert!(!expansion.bind(&payload));
        expansion.set_collapsed("inspect.tree.0", true);
        assert!(!expansion.bind(&payload));
        assert_eq!(expansion.collapsed_paths(), vec!["inspect.tree.0"]);
        // A different inspection payload (one pretty text differs) is a new result identity.
        let other = Json::obj([
            (
                "subject",
                Json::obj([
                    ("kind", Json::Str("Symbolic".to_string())),
                    ("path", Json::Str("result".to_string())),
                ]),
            ),
            ("sections", Json::obj([("pretty", text("x + 1"))])),
        ]);
        assert!(expansion.bind(&other));
        assert!(expansion.collapsed_paths().is_empty());
    }

    #[test]
    fn tree_paths_name_child_order() {
        assert_eq!(tree_path(TREE_ROOT_PATH, 0), "inspect.tree.0");
        assert_eq!(tree_path("inspect.tree.0", 1), "inspect.tree.0.1");
    }

    /// One chain of `levels` binary nodes: 2*levels + 1 nodes, the deepest level index `levels`.
    fn chain(levels: usize) -> TreeNode {
        fn build(level: usize, total: usize) -> TreeNode {
            let leaf = |label: &str| TreeNode {
                kind: "Num".to_string(),
                detail: None,
                label: label.to_string(),
                canonical: format!("(rat {label} 1)"),
                children: Vec::new(),
            };
            if level == total {
                return leaf("1");
            }
            TreeNode {
                kind: "Add".to_string(),
                detail: None,
                label: format!("x + {level}"),
                canonical: format!("(add (sym x) (rat {level} 1))"),
                children: vec![leaf("x"), build(level + 1, total)],
            }
        }
        build(0, levels)
    }

    /// SYM-003: the rows are the engine's own nodes, with their kind, child order and canonical
    /// form, so a node can be traversed and its canonical form copied.
    #[test]
    fn tree_rows_carry_node_kind_child_order_and_the_engine_canonical_form() {
        let inspection = Inspection::from_json(&full_payload()).expect("payload");
        let tree = inspection.tree.supplied().expect("tree");
        let mut expansion = InspectorExpansion::new();
        let rows = tree_rows(tree, &expansion);
        let order: Vec<&str> = rows.iter().map(|row| row.kind.as_str()).collect();
        assert_eq!(order, vec!["Add", "Pow", "Num"]);
        assert_eq!(rows[0].path, "inspect.tree");
        assert_eq!(rows[1].path, "inspect.tree.0");
        assert_eq!(rows[2].path, "inspect.tree.1");
        assert_eq!(rows[0].level, 0);
        assert_eq!(rows[2].level, 1);
        assert_eq!(rows[1].canonical, "(pow (sym x) (rat 2 1))");
        assert_eq!(rows[0].header(), "Add · x^2 - 1");
        assert_eq!(rows[1].accessible_name(), "Pow · x^2 [inspect.tree.0] leaf");
        assert!(!rows[0].collapsed, "the tree starts expanded");
        // Collapsing a row hides exactly its children, and the row says so.
        expansion.toggle("inspect.tree.0");
        let collapsed = tree_rows(tree, &expansion);
        assert_eq!(
            collapsed.len(),
            3,
            "a collapsed leaf has no children to hide"
        );
        expansion.toggle("inspect.tree");
        assert_eq!(tree_rows(tree, &expansion).len(), 1);
        assert_eq!(tree_rows(tree, &expansion)[0].state_word(), "collapsed");
    }

    /// SYM-011: the first pass paints a bounded number of levels, and a row that hides a deeper
    /// subtree says how many nodes it is hiding. Loading deeper never re-derives the whole tree.
    #[test]
    fn tree_rows_stop_at_the_loaded_level_bound_and_report_what_they_hide() {
        let root = chain(19);
        let deepest = root.depth();
        assert_eq!(root.count(), 39);
        assert_eq!(deepest, 20);
        let mut expansion = InspectorExpansion::new();
        let rows = tree_rows(&root, &expansion);
        // A binary chain: levels 0..3 hold 1 + 2 + 2 + 2 rows.
        assert_eq!(rows.len(), 2 * INITIAL_TREE_LEVELS - 1);
        assert!(expansion.has_deeper(deepest));
        let last = rows.last().expect("the deepest painted row");
        assert_eq!(last.level, INITIAL_TREE_LEVELS - 1);
        let hidden: usize = rows.iter().map(|row| row.deeper_nodes).sum();
        assert_eq!(
            hidden,
            root.count() - rows.len(),
            "the rows that hide a deeper subtree name every node they hide"
        );
        assert_eq!(last.deeper_nodes, hidden);
        // One load-deeper step paints the next step of levels, not the whole tree.
        let loaded = expansion.load_deeper(deepest);
        assert_eq!(loaded, INITIAL_TREE_LEVELS + TREE_LEVELS_STEP);
        let rows = tree_rows(&root, &expansion);
        assert_eq!(rows.len(), 2 * (INITIAL_TREE_LEVELS + TREE_LEVELS_STEP) - 1);
        // Loading to the deepest level paints every node, in the engine's order.
        while expansion.has_deeper(deepest) {
            expansion.load_deeper(deepest);
        }
        assert_eq!(expansion.levels(), deepest);
        assert_eq!(tree_rows(&root, &expansion).len(), root.count());
    }

    /// SYM-011: a 10k-node tree is bounded in the two directions that matter. A deep expression
    /// is cut by the level bound, and a wide one is cut by the paint window: the row model is
    /// linear in the nodes it is asked for and building 10 001 rows is fast enough to stay
    /// interactive, while only the rows in view are ever painted (proved at the paint layer).
    #[test]
    fn a_ten_thousand_node_tree_is_bounded_by_depth_and_cheap_to_model() {
        // A wide tree: one root with 10 000 leaf children.
        let leaf = |index: usize| TreeNode {
            kind: "Sym".to_string(),
            detail: None,
            label: format!("x{index}"),
            canonical: format!("(sym x{index})"),
            children: Vec::new(),
        };
        let root = TreeNode {
            kind: "Add".to_string(),
            detail: None,
            label: "sum".to_string(),
            canonical: "(add ...)".to_string(),
            children: (0..10_000).map(leaf).collect(),
        };
        assert_eq!(root.count(), 10_001);
        assert_eq!(
            root.depth(),
            2,
            "the level bound cannot cut a wide tree; the paint window has to"
        );
        let expansion = InspectorExpansion::new();
        let start = std::time::Instant::now();
        let rows = tree_rows(&root, &expansion);
        let elapsed = start.elapsed();
        assert_eq!(rows.len(), 10_001);
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "modeling 10 001 rows took {elapsed:?}"
        );
        assert_eq!(rows[0].state_word(), "expanded");
        assert_eq!(rows[10_000].path, "inspect.tree.9999");

        // A deep, balanced tree of 32 767 nodes is cut by the level bound: the first pass paints
        // 1 + 2 + 4 + 8 rows and names every node it did not descend into.
        fn balanced(level: usize) -> TreeNode {
            if level == 0 {
                return TreeNode {
                    kind: "Sym".to_string(),
                    detail: None,
                    label: "x".to_string(),
                    canonical: "(sym x)".to_string(),
                    children: Vec::new(),
                };
            }
            TreeNode {
                kind: "Add".to_string(),
                detail: None,
                label: format!("d{level}"),
                canonical: format!("(add d{level})"),
                children: vec![balanced(level - 1), balanced(level - 1)],
            }
        }
        let deep = balanced(14);
        assert_eq!(deep.count(), 32_767);
        assert_eq!(deep.depth(), 15);
        let rows = tree_rows(&deep, &expansion);
        assert_eq!(rows.len(), 15, "four levels of a balanced tree");
        let hidden: usize = rows.iter().map(|row| row.deeper_nodes).sum();
        assert_eq!(hidden, deep.count() - rows.len());
    }

    /// SYM-011: the level bound and the pending keyboard focus reset with a new result, so a new
    /// expression is never opened at the old expression's depth.
    #[test]
    fn a_new_inspection_resets_the_loaded_levels_and_the_pending_focus() {
        let mut expansion = InspectorExpansion::new();
        let payload = full_payload();
        assert!(!expansion.bind(&payload));
        expansion.load_deeper(30);
        expansion.set_pending_focus("inspect.tree.1");
        assert!(expansion.levels() > INITIAL_TREE_LEVELS);
        assert!(
            !expansion.bind(&payload),
            "the same payload keeps its state"
        );
        assert!(expansion.levels() > INITIAL_TREE_LEVELS);
        let other = Json::obj([
            (
                "subject",
                Json::obj([
                    ("kind", Json::Str("Symbolic".to_string())),
                    ("path", Json::Str("result".to_string())),
                ]),
            ),
            ("sections", Json::obj([("pretty", text("x + 1"))])),
        ]);
        assert!(expansion.bind(&other));
        assert_eq!(expansion.levels(), INITIAL_TREE_LEVELS);
        assert_eq!(expansion.pending_focus(), None);
    }
}
