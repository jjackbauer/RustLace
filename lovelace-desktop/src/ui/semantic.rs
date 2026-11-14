//! VIS-007: the semantic colour roles and the non-colour carrier every role must carry.
//!
//! A status is never painted as a colour alone. [`SemanticRole`] pairs the theme token that
//! carries the meaning with two carriers a reader can use without colour vision:
//!
//! - a bracketed **text marker** ([`SemanticRole::marker`]) that is part of the rendered string
//!   (the status strip, the logs pane), and
//! - an ASCII **shape** ([`SemanticRole::glyph`]) documented by the shape name the UI paints or
//!   states in the accessible name (`!` on a square for failure, `>` for progress, `/` for
//!   cancelled, ...).
//!
//! The table is deliberately small and closed: a run state, a log kind and a table origin all
//! resolve to one of these roles, so there is no paint site with a semantic colour and no
//! carrier. [`tests::every_role_has_a_colour_a_marker_and_a_shape`] is the source-level proof.
//!
//! The plot series are the one engine-owned surface: the engine's SVG paints each polyline in
//! its own stroke colour. The pane adds a numbered badge at the first point of each polyline and
//! a legend row that repeats the number and the colour (`plot.rs`), so a series is identified by
//! its number/shape even in greyscale. The exported SVG keeps the engine's bytes unchanged.

use crate::log_kind::LogKind;
use crate::run_state::RunState;

use super::theme;

/// One semantic role: a meaning, a colour token, a text marker and a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticRole {
    /// A run finished, a save succeeded: the positive token.
    Success,
    /// A run failed or a diagnostic was reported: the error token.
    Failure,
    /// A warning, a stale result, a cancelled run: the warning token.
    Warning,
    /// In-flight work: the progress token.
    Progress,
    /// The user cancelled: the warning token, a distinct `/` shape and the word "cancelled".
    Cancelled,
    /// An engine diagnostic line: the error token, the `[diagnostic]` marker.
    Diagnostic,
    /// Readiness/advisory text: the secondary text token, the `[info]` marker.
    Neutral,
}

impl SemanticRole {
    /// Every role, in the order the role table documents them.
    pub const ALL: [SemanticRole; 7] = [
        SemanticRole::Success,
        SemanticRole::Failure,
        SemanticRole::Warning,
        SemanticRole::Progress,
        SemanticRole::Cancelled,
        SemanticRole::Diagnostic,
        SemanticRole::Neutral,
    ];

    /// The role's stable name for the report and tests.
    pub fn name(self) -> &'static str {
        match self {
            SemanticRole::Success => "success",
            SemanticRole::Failure => "failure",
            SemanticRole::Warning => "warning",
            SemanticRole::Progress => "progress",
            SemanticRole::Cancelled => "cancelled",
            SemanticRole::Diagnostic => "diagnostic",
            SemanticRole::Neutral => "neutral",
        }
    }

    /// The theme token that carries the role's colour.
    pub fn color(self) -> egui::Color32 {
        match self {
            SemanticRole::Success => theme::positive(),
            SemanticRole::Failure | SemanticRole::Diagnostic => theme::error(),
            SemanticRole::Warning | SemanticRole::Cancelled => theme::warning(),
            SemanticRole::Progress => theme::progress(),
            SemanticRole::Neutral => theme::dim(),
        }
    }

    /// The text carrier: a bracketed marker rendered beside the value.
    ///
    /// The success and failure markers keep the `[OK]`/`[ERR]` spelling earlier rounds pinned;
    /// every other state gets a word of its own, so "cancelled" can never be mistaken for an
    /// error or a warning by colour alone.
    pub fn marker(self) -> &'static str {
        match self {
            SemanticRole::Success => "[OK]",
            SemanticRole::Failure => "[ERR]",
            SemanticRole::Warning => "[WARN]",
            SemanticRole::Progress => "[RUN]",
            SemanticRole::Cancelled => "[CANCELLED]",
            SemanticRole::Diagnostic => "[diagnostic]",
            SemanticRole::Neutral => "[info]",
        }
    }

    /// The shape carrier: the ASCII glyph the pane paints (or names in its description).
    pub fn glyph(self) -> &'static str {
        match self {
            SemanticRole::Success => "+",
            SemanticRole::Failure | SemanticRole::Diagnostic => "!",
            SemanticRole::Warning => "^",
            SemanticRole::Progress => ">",
            SemanticRole::Cancelled => "/",
            SemanticRole::Neutral => ".",
        }
    }

    /// The shape's human name, used in the role table and the accessible name.
    pub fn shape(self) -> &'static str {
        match self {
            SemanticRole::Success => "check badge",
            SemanticRole::Failure => "filled square with bang",
            SemanticRole::Warning => "triangle",
            SemanticRole::Progress => "moving bar",
            SemanticRole::Cancelled => "slash",
            SemanticRole::Diagnostic => "underlined span with bang",
            SemanticRole::Neutral => "dot",
        }
    }

    /// The one-word state name the marker introduces.
    pub fn label(self) -> &'static str {
        match self {
            SemanticRole::Success => "ok",
            SemanticRole::Failure => "error",
            SemanticRole::Warning => "warning",
            SemanticRole::Progress => "running",
            SemanticRole::Cancelled => "cancelled",
            SemanticRole::Diagnostic => "diagnostic",
            SemanticRole::Neutral => "info",
        }
    }

    /// The role for a run state, or `None` for Idle (which carries no outcome).
    pub fn for_run_state(state: RunState) -> Option<SemanticRole> {
        match state {
            RunState::Idle => None,
            RunState::Queued | RunState::Running | RunState::Cancelling => {
                Some(SemanticRole::Progress)
            }
            RunState::Finished => Some(SemanticRole::Success),
            RunState::Failed => Some(SemanticRole::Failure),
            RunState::Cancelled => Some(SemanticRole::Cancelled),
            RunState::Stale => Some(SemanticRole::Warning),
        }
    }

    /// The role for a log line kind. The log's own `[kind]` marker is the text carrier; this
    /// table adds the semantic colour it paints with and the shape the legend documents.
    pub fn for_log_kind(kind: LogKind) -> SemanticRole {
        match kind {
            LogKind::Echo => SemanticRole::Neutral,
            LogKind::Stdout => SemanticRole::Neutral,
            LogKind::Result => SemanticRole::Success,
            LogKind::Diagnostic => SemanticRole::Diagnostic,
            LogKind::Timing => SemanticRole::Warning,
            LogKind::Progress => SemanticRole::Progress,
            LogKind::Info => SemanticRole::Neutral,
        }
    }
}

/// VIS-007: the role table as a fixed-width report, printed by the visual audit.
pub fn role_table() -> String {
    let mut text = String::from("role | colour role | text carrier | shape carrier | meaning\n");
    for role in SemanticRole::ALL {
        text.push_str(&format!(
            "{} | {} | {} | {} ({}) | {}\n",
            role.name(),
            role.label(),
            role.marker(),
            role.glyph(),
            role.shape(),
            role.color_name(),
        ));
    }
    text
}

impl SemanticRole {
    /// The token name (not its hex value) so the table names the role, not a literal.
    pub fn color_name(self) -> &'static str {
        match self {
            SemanticRole::Success => "positive",
            SemanticRole::Failure | SemanticRole::Diagnostic => "error",
            SemanticRole::Warning | SemanticRole::Cancelled => "warning",
            SemanticRole::Progress => "progress",
            SemanticRole::Neutral => "text_secondary",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_role_has_a_colour_a_marker_and_a_shape() {
        let mut markers = BTreeSet::new();
        let mut glyphs = BTreeSet::new();
        for role in SemanticRole::ALL {
            assert!(!role.name().is_empty());
            assert!(
                !role.marker().is_empty(),
                "{}: no text carrier",
                role.name()
            );
            assert!(
                role.marker().starts_with('[') && role.marker().ends_with(']'),
                "{}: the marker must be a visible bracketed word",
                role.name()
            );
            assert!(!role.glyph().is_empty(), "{}: no shape", role.name());
            assert!(!role.shape().is_empty(), "{}: no shape name", role.name());
            assert!(!role.color_name().is_empty());
            assert!(
                markers.insert(role.marker()),
                "duplicate marker {}",
                role.marker()
            );
            glyphs.insert(role.glyph());
        }
        assert_eq!(markers.len(), SemanticRole::ALL.len());
        // Failure and diagnostic deliberately share the bang; the marker text distinguishes them.
        assert!(
            glyphs.len() >= 5,
            "the shapes must distinguish the roles: {glyphs:?}"
        );
        assert_eq!(SemanticRole::Success.marker(), "[OK]");
        assert_eq!(SemanticRole::Failure.marker(), "[ERR]");
        assert_eq!(SemanticRole::Cancelled.marker(), "[CANCELLED]");
    }

    #[test]
    fn every_run_state_maps_to_a_carrier_and_idle_maps_to_none() {
        use RunState::*;
        let mut seen_states = 0;
        for state in [
            Idle, Queued, Running, Cancelling, Finished, Failed, Cancelled, Stale,
        ] {
            seen_states += 1;
            match SemanticRole::for_run_state(state) {
                Some(role) => {
                    assert!(!role.marker().is_empty());
                    assert!(!role.glyph().is_empty());
                    assert_ne!(
                        role.color_name(),
                        "text_secondary",
                        "{state:?}: an outcome must be semantic"
                    );
                }
                None => assert_eq!(state, Idle, "{state:?} must not be silent"),
            }
        }
        assert_eq!(seen_states, 8, "the table covers every RunState");
        assert_eq!(
            SemanticRole::for_run_state(Finished),
            Some(SemanticRole::Success)
        );
        assert_eq!(
            SemanticRole::for_run_state(Cancelled),
            Some(SemanticRole::Cancelled)
        );
        assert_eq!(
            SemanticRole::for_run_state(Stale),
            Some(SemanticRole::Warning)
        );
        assert_eq!(
            SemanticRole::for_run_state(Running),
            Some(SemanticRole::Progress)
        );
    }

    #[test]
    fn every_log_kind_paints_a_semantic_colour_with_its_text_marker() {
        for kind in LogKind::ALL {
            let role = SemanticRole::for_log_kind(kind);
            assert!(!role.color_name().is_empty(), "{kind:?}");
            assert!(
                !kind.marker().is_empty(),
                "{kind:?}: the log model must carry its marker"
            );
            assert!(role.marker().starts_with('['));
        }
        assert_eq!(
            SemanticRole::for_log_kind(LogKind::Diagnostic),
            SemanticRole::Diagnostic
        );
        assert_eq!(
            SemanticRole::for_log_kind(LogKind::Progress),
            SemanticRole::Progress
        );
    }

    #[test]
    fn the_role_table_lists_every_role_and_its_carriers() {
        let table = role_table();
        for role in SemanticRole::ALL {
            assert!(table.contains(role.name()), "{table}");
            assert!(table.contains(role.marker()), "{table}");
        }
        assert!(
            table.lines().count() == SemanticRole::ALL.len() + 1,
            "{table}"
        );
    }
}
