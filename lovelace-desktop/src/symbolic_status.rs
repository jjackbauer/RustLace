//! SYM-006: the status a committed result publishes, read from the engine's own fields.
//!
//! A rich result (`SolveResult`, `TransformResult`, `IntegrationResult`, ...) publishes a `status`
//! enum and often a `complete` flag; a rendering can also publish `truncated` and `exact`. This
//! module turns exactly those published fields into the badge the window paints beside the result,
//! so a `Partial` or `Unevaluated` result can never be shown as a fully Solved/Exact one.
//!
//! Nothing here infers a status from the display text: a result that publishes no status field
//! gets no status badge (and the pane paints the absence, never a guessed "Solved").

use lovelace_json::Json;

/// What an incomplete result must never be painted as.
pub const COMPLETION_WORDS: [&str; 3] = ["SOLVED", "EXACT", "COMPLETE"];

/// How serious the badge is. It selects the colour and the announcement, nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// The engine states the result is complete and exact.
    Complete,
    /// The result is only valid when its published conditions hold.
    Conditional,
    /// The engine states the result is incomplete, partial or unevaluated.
    Incomplete,
}

impl Tone {
    /// The word a screen reader receives beside the badge.
    pub fn label(self) -> &'static str {
        match self {
            Tone::Complete => "complete",
            Tone::Conditional => "conditional",
            Tone::Incomplete => "incomplete",
        }
    }
}

/// One status badge painted beside the last committed result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusBadge {
    /// The upper-case word painted first (`SOLVED`, `PARTIAL`, `CONDITIONAL`, ...).
    pub word: String,
    /// The engine evidence: the published field and the value it carried.
    pub detail: String,
    /// The colour/announcement class.
    pub tone: Tone,
}

impl StatusBadge {
    /// True when this badge claims the result is fully solved, exact or complete.
    ///
    /// SYM-006: a badge can only make that claim when the engine published the field that says so;
    /// every other badge answers false here, and the paint path is checked against this method.
    pub fn claims_completion(&self) -> bool {
        COMPLETION_WORDS.contains(&self.word.as_str())
    }
}

/// The status badges for one committed result: the projection first, then the inspection payload.
///
/// `structured` is the engine's structured projection of the result (the same value the RESULT
/// tree is built from); `inspection` is the symbolic payload beside it, when the engine published
/// one. Both are read, never re-derived.
pub fn badges(structured: Option<&Json>, inspection: Option<&Json>) -> Vec<StatusBadge> {
    let mut out = Vec::new();
    if let Some(structured) = structured {
        status_badge(structured, &mut out);
        condition_badge(structured, &mut out);
        rendering_badges(structured, &mut out);
    }
    if out.is_empty() {
        // A result with no projection still has the inspector's subject line (for a
        // TransformResult that is its published status); read that before giving up.
        if let Some(detail) = inspection
            .and_then(|json| json.get("subject"))
            .and_then(|subject| subject.get("detail"))
            .and_then(Json::as_str)
        {
            out.push(status_word(detail, "subject detail", None));
        }
    }
    out
}

/// The headline the panes paint: every badge's word and evidence, in one line.
pub fn headline(badges: &[StatusBadge]) -> String {
    badges
        .iter()
        .map(|badge| format!("{} · {}", badge.word, badge.detail))
        .collect::<Vec<String>>()
        .join("   ")
}

/// Reads the record's `status` field, when the projection is a record that publishes one.
fn status_badge(structured: &Json, out: &mut Vec<StatusBadge>) {
    let Some(record_type) = structured.get("type").and_then(Json::as_str) else {
        return;
    };
    let Some(status) = record_field(structured, "status") else {
        return;
    };
    let Some(variant) = status.get("value").and_then(Json::as_str) else {
        return;
    };
    let complete = record_field(structured, "complete").and_then(text_value);
    out.push(status_word(
        variant,
        &format!("{record_type}.status"),
        complete,
    ));
}

/// Turns one published status variant into a badge.
///
/// The `complete` flag only ever makes the badge *more* cautious: a solved status that publishes
/// `complete: false` is a partial result, never a solved one.
fn status_word(variant: &str, source: &str, complete: Option<String>) -> StatusBadge {
    let complete_word = complete.as_deref() == Some("true");
    let complete_detail = match complete.as_deref() {
        Some("true") => "complete: true",
        Some("false") => "complete: false",
        Some(other) => other,
        None => "no complete flag",
    };
    let (word, tone) = match variant {
        "Solved" if complete_word => ("SOLVED".to_string(), Tone::Complete),
        "Solved" => ("PARTIAL".to_string(), Tone::Incomplete),
        "Satisfied" => ("SATISFIED".to_string(), Tone::Conditional),
        "Value" if complete_word => ("VALUE".to_string(), Tone::Complete),
        "Value" => ("PARTIAL".to_string(), Tone::Incomplete),
        "Partial" => ("PARTIAL".to_string(), Tone::Incomplete),
        "Unevaluated" => ("UNEVALUATED".to_string(), Tone::Incomplete),
        "NoSolutions" => ("NO SOLUTIONS".to_string(), Tone::Complete),
        "Unsatisfiable" => ("UNSATISFIABLE".to_string(), Tone::Incomplete),
        "BudgetExceeded" => ("INCOMPLETE".to_string(), Tone::Incomplete),
        "Exact" => ("EXACT".to_string(), Tone::Complete),
        "Conditional" => ("CONDITIONAL".to_string(), Tone::Conditional),
        other => (upper(other), Tone::Incomplete),
    };
    let detail = if variant == "BudgetExceeded" {
        format!("{source} = {variant} (budget exhausted) · {complete_detail}")
    } else {
        format!("{source} = {variant} · {complete_detail}")
    };
    StatusBadge {
        word: word.to_string(),
        detail,
        tone,
    }
}
/// The record's published conditions: a rewrite with conditions is a conditional result, never an
/// unconditional one, however the status word reads.
fn condition_badge(structured: &Json, out: &mut Vec<StatusBadge>) {
    for field in ["conditions", "assumptions"] {
        let Some(value) = record_field(structured, field) else {
            continue;
        };
        let count = value
            .get("elements")
            .and_then(Json::as_arr)
            .map(|elements| elements.len())
            .unwrap_or(0);
        if count == 0 {
            continue;
        }
        let record_type = structured
            .get("type")
            .and_then(Json::as_str)
            .unwrap_or("Record");
        out.push(StatusBadge {
            word: "CONDITIONAL".to_string(),
            detail: format!("{record_type}.{field} = {count} required condition(s)"),
            tone: Tone::Conditional,
        });
        return;
    }
}

/// `truncated` / `exact` as the projection publishes them for a rendering or a scalar.
fn rendering_badges(structured: &Json, out: &mut Vec<StatusBadge>) {
    let kind = structured
        .get("kind")
        .and_then(Json::as_str)
        .unwrap_or("value");
    if json_bool(structured.get("truncated")) == Some(true) {
        let reason = structured
            .get("truncationReason")
            .and_then(Json::as_str)
            .unwrap_or("unspecified");
        out.push(StatusBadge {
            word: "TRUNCATED".to_string(),
            detail: format!("{kind}.truncated = true · reason {reason}"),
            tone: Tone::Incomplete,
        });
    }
    if let Some(exact) = json_bool(structured.get("exact")) {
        out.push(StatusBadge {
            word: if exact { "EXACT" } else { "INEXACT" }.to_string(),
            detail: format!("{kind}.exact = {exact}"),
            tone: if exact {
                Tone::Complete
            } else {
                Tone::Incomplete
            },
        });
    }
}

fn record_field<'a>(structured: &'a Json, name: &str) -> Option<&'a Json> {
    structured
        .get("fields")?
        .as_arr()?
        .iter()
        .find(|field| field.get("name").and_then(Json::as_str) == Some(name))
        .and_then(|field| field.get("value"))
}

fn json_bool(value: Option<&Json>) -> Option<bool> {
    match value {
        Some(Json::Bool(flag)) => Some(*flag),
        _ => None,
    }
}

fn text_value(value: &Json) -> Option<String> {
    value
        .get("value")
        .and_then(Json::as_str)
        .map(str::to_string)
}

fn upper(text: &str) -> String {
    text.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enum_value(type_name: &str, variant: &str) -> Json {
        Json::obj([
            ("kind", Json::Str("Enum".to_string())),
            ("type", Json::Str(type_name.to_string())),
            ("value", Json::Str(variant.to_string())),
        ])
    }

    fn boolean(value: bool) -> Json {
        Json::obj([
            ("kind", Json::Str("Boolean".to_string())),
            (
                "value",
                Json::Str(if value { "true" } else { "false" }.to_string()),
            ),
        ])
    }

    fn record(type_name: &str, fields: Vec<(&str, Json)>) -> Json {
        Json::obj([
            ("kind", Json::Str("Record".to_string())),
            ("type", Json::Str(type_name.to_string())),
            (
                "fields",
                Json::Arr(
                    fields
                        .into_iter()
                        .map(|(name, value)| {
                            Json::obj([("name", Json::Str(name.to_string())), ("value", value)])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn array(kind: &str, elements: Vec<Json>) -> Json {
        Json::obj([
            ("kind", Json::Str(kind.to_string())),
            ("elements", Json::Arr(elements)),
        ])
    }

    /// SYM-006: a `Partial` solve result is painted as PARTIAL and never as SOLVED or EXACT.
    #[test]
    fn a_partial_solve_result_is_never_painted_as_solved_or_exact() {
        let projected = record(
            "SolveResult",
            vec![
                ("status", enum_value("SolveStatus", "Partial")),
                ("complete", boolean(false)),
                ("solutions", array("Array", Vec::new())),
            ],
        );
        let painted = badges(Some(&projected), None);
        assert_eq!(painted.len(), 1, "{painted:?}");
        assert_eq!(painted[0].word, "PARTIAL");
        assert_eq!(painted[0].tone, Tone::Incomplete);
        assert!(!painted[0].claims_completion());
        assert!(
            painted[0].detail.contains("SolveResult.status = Partial"),
            "the badge names the engine field it read: {painted:?}"
        );
        assert!(painted[0].detail.contains("complete: false"), "{painted:?}");
        let line = headline(&painted);
        assert!(
            !line.contains("SOLVED"),
            "the headline claims Solved: {line}"
        );
        assert!(!line.contains("EXACT"), "the headline claims Exact: {line}");
    }

    /// SYM-006: an `Unevaluated` result says so; it is not silently presented as a value.
    #[test]
    fn an_unevaluated_result_publishes_its_own_word() {
        let projected = record(
            "IntegrationResult",
            vec![
                ("status", enum_value("IntegrationStatus", "Unevaluated")),
                ("complete", boolean(false)),
            ],
        );
        let painted = badges(Some(&projected), None);
        assert_eq!(painted[0].word, "UNEVALUATED");
        assert!(!painted[0].claims_completion());
        assert!(painted[0].detail.contains("IntegrationResult.status"));
    }

    /// SYM-006: a solved result that publishes `complete: false` is still partial: the flag wins.
    #[test]
    fn a_solved_status_with_complete_false_is_a_partial_result() {
        let projected = record(
            "SolveResult",
            vec![
                ("status", enum_value("SolveStatus", "Solved")),
                ("complete", boolean(false)),
            ],
        );
        let painted = badges(Some(&projected), None);
        assert_eq!(painted[0].word, "PARTIAL");
        assert!(!painted[0].claims_completion());
    }

    /// SYM-006: a result whose conditions are published is conditional even when its status word
    /// reads Satisfied; the conditions badge is painted beside the status.
    #[test]
    fn a_rewrite_with_conditions_is_conditional_and_not_merely_satisfied() {
        let condition = Json::obj([
            ("kind", Json::Str("Symbolic".to_string())),
            ("pretty", Json::Str("x > 0".to_string())),
        ]);
        let projected = record(
            "TransformResult",
            vec![
                ("status", enum_value("TransformStatus", "Satisfied")),
                ("conditions", array("Array", vec![condition])),
            ],
        );
        let painted = badges(Some(&projected), None);
        let words: Vec<&str> = painted.iter().map(|badge| badge.word.as_str()).collect();
        assert_eq!(words, vec!["SATISFIED", "CONDITIONAL"], "{painted:?}");
        assert_eq!(painted[0].tone, Tone::Conditional);
        assert_eq!(painted[1].tone, Tone::Conditional);
        assert!(painted[1].detail.contains("TransformResult.conditions = 1"));
        assert!(painted.iter().all(|badge| !badge.claims_completion()));
    }

    /// SYM-006: a satisfied rewrite with no conditions is the one rewrite that may read complete.
    #[test]
    fn a_rewrite_without_conditions_reads_satisfied_only() {
        let projected = record(
            "TransformResult",
            vec![
                ("status", enum_value("TransformStatus", "Satisfied")),
                ("conditions", array("Array", Vec::new())),
            ],
        );
        let painted = badges(Some(&projected), None);
        assert_eq!(painted.len(), 1);
        assert_eq!(painted[0].word, "SATISFIED");
        assert!(!painted[0].claims_completion());
    }

    /// SYM-006: a truncated or inexact rendering says so, and the exactness flags come from the
    /// projection rather than from the pane's assumption.
    #[test]
    fn a_truncated_and_inexact_rendering_carries_both_flags() {
        let projected = Json::obj([
            ("kind", Json::Str("Symbolic".to_string())),
            ("pretty", Json::Str("x^2 + …".to_string())),
            ("truncated", Json::Bool(true)),
            ("truncationReason", Json::Str("node-budget".to_string())),
            ("exact", Json::Bool(false)),
        ]);
        let painted = badges(Some(&projected), None);
        let words: Vec<&str> = painted.iter().map(|badge| badge.word.as_str()).collect();
        assert_eq!(words, vec!["TRUNCATED", "INEXACT"]);
        assert!(painted[0].detail.contains("node-budget"));
        assert!(painted.iter().all(|badge| !badge.claims_completion()));
    }

    /// SYM-006: an exact integer result publishes EXACT, and that is the engine's claim, not the
    /// pane's default.
    #[test]
    fn an_exact_scalar_publishes_exact_because_the_engine_says_so() {
        let projected = Json::obj([
            ("kind", Json::Str("Integer".to_string())),
            ("value", Json::Str("42".to_string())),
            ("exact", Json::Bool(true)),
        ]);
        let painted = badges(Some(&projected), None);
        assert_eq!(painted.len(), 1);
        assert_eq!(painted[0].word, "EXACT");
        assert!(painted[0].claims_completion());
        assert_eq!(painted[0].detail, "Integer.exact = true");
    }

    /// SYM-006: a result that publishes no status field gets no badge at all: the pane shows the
    /// absence instead of inventing a status.
    #[test]
    fn a_result_without_a_status_field_gets_no_badge() {
        let projected = Json::obj([
            ("kind", Json::Str("Symbolic".to_string())),
            ("pretty", Json::Str("x + 1".to_string())),
            ("exact", Json::Bool(true)),
        ]);
        let painted = badges(Some(&projected), None);
        assert_eq!(painted.len(), 1, "only the exactness flag is published");
        assert_eq!(painted[0].word, "EXACT");
        let nothing = badges(None, None);
        assert!(nothing.is_empty());
        assert_eq!(headline(&nothing), "");
    }

    /// SYM-006: a TransformResult with no structured projection still publishes its status in the
    /// inspector subject, so the badge is not lost when the projection is unavailable.
    #[test]
    fn a_subject_detail_supplies_the_status_when_the_projection_is_absent() {
        let inspection = Json::obj([(
            "subject",
            Json::obj([
                ("kind", Json::Str("TransformResult".to_string())),
                ("path", Json::Str("result.expression".to_string())),
                ("detail", Json::Str("BudgetExceeded".to_string())),
            ]),
        )]);
        let painted = badges(None, Some(&inspection));
        assert_eq!(painted.len(), 1);
        assert_eq!(painted[0].word, "INCOMPLETE");
        assert_eq!(painted[0].tone, Tone::Incomplete);
        assert!(painted[0].detail.contains("BudgetExceeded"));
    }
}
