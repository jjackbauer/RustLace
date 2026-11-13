//! VIS-008: the workbench's empty states.
//!
//! Every pane that can have nothing to show explains three things in the pane itself:
//!
//! 1. **what the pane is for** ([`EmptyState::purpose`]),
//! 2. **why it is empty right now** ([`EmptyState::reason`]), and
//! 3. **what to do next** ([`EmptyState::next`]),
//!
//! and, where the next step is a command the window can run, it offers that command as a button
//! ([`EmptyState::action`]). The table in [`catalog`] is the closed list the eight pane states
//! come from; [`paint`] is the one renderer, so a pane cannot invent its own wording or its own
//! action. The tests in this module prove every entry carries all three facts and a distinct
//! action, and that the rendered card is a real label/button tree a UIA capture can read.

use egui::{Align, Layout, Rect, RichText, Ui, UiBuilder};

use super::theme;
use super::type_scale;

/// The command an empty state offers beside its guidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyAction {
    /// Start the active document's run (`Ctrl+Enter`).
    Run,
    /// Clear the workspace filter so the committed rows are shown again.
    ClearFilter,
    /// Put keyboard focus in the editor for the first keystroke.
    FocusEditor,
}

impl EmptyAction {
    /// The button label. Every label is distinct, so a UIA capture can name the pane's action.
    pub fn label(self) -> &'static str {
        match self {
            EmptyAction::Run => "Run to populate",
            EmptyAction::ClearFilter => "Clear the filter",
            EmptyAction::FocusEditor => "Start typing",
        }
    }

    /// The action's stable id, used by tests and the capture driver.
    pub fn id(self) -> &'static str {
        match self {
            EmptyAction::Run => "empty-action-run",
            EmptyAction::ClearFilter => "empty-action-clear-filter",
            EmptyAction::FocusEditor => "empty-action-focus-editor",
        }
    }
}

/// One pane's empty state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyState {
    /// Stable key: `editor`, `plot`, `results`, `workspace`, `functions`, `logs`, `inspector`,
    /// `search`.
    pub key: &'static str,
    /// The short headline ("No plot yet").
    pub headline: String,
    /// What the pane is for.
    pub purpose: String,
    /// Why it is empty right now.
    pub reason: String,
    /// What to do next, including the command's name.
    pub next: String,
    /// The command offered beside the guidance, when one applies.
    pub action: Option<EmptyAction>,
}

impl EmptyState {
    /// Builds one state. The purpose is fixed per pane; the reason depends on the state that made
    /// the pane empty (no run yet, no matches, ...), which is why it is a `String`.
    pub fn new(
        key: &'static str,
        headline: impl Into<String>,
        purpose: impl Into<String>,
        reason: impl Into<String>,
        next: impl Into<String>,
    ) -> EmptyState {
        EmptyState {
            key,
            headline: headline.into(),
            purpose: purpose.into(),
            reason: reason.into(),
            next: next.into(),
            action: None,
        }
    }

    /// Adds the action the card offers.
    pub fn with_action(mut self, action: EmptyAction) -> EmptyState {
        self.action = Some(action);
        self
    }
}

/// The editor's empty state: an empty buffer.
pub fn editor() -> EmptyState {
    EmptyState::new(
        "editor",
        "Empty document",
        "Write and edit this document's Lovelace script.",
        "This document has no text yet.",
        "Type a script, or open an existing .lv file. Ctrl+Enter runs it.",
    )
    .with_action(EmptyAction::FocusEditor)
}

/// The graph's empty state: no engine plot yet.
pub fn plot() -> EmptyState {
    EmptyState::new(
        "plot",
        "No plot yet",
        "Draws the SVG the engine's plot(...) call produced.",
        "No run has produced a plot for this document.",
        "Run a script that calls plot(x, y, \"title\").",
    )
    .with_action(EmptyAction::Run)
}

/// The results section's empty state: no committed result.
pub fn results() -> EmptyState {
    EmptyState::new(
        "results",
        "No result yet",
        "Shows the last committed value and its structure.",
        "No run has published a result for this document.",
        "Press Run (Ctrl+Enter) to evaluate.",
    )
    .with_action(EmptyAction::Run)
}

/// The variables table's empty state: no committed workspace.
pub fn workspace() -> EmptyState {
    EmptyState::new(
        "workspace",
        "No variables yet",
        "Lists the variables the last run committed.",
        "No run has published a workspace for this document.",
        "Run the document to publish them.",
    )
    .with_action(EmptyAction::Run)
}

/// The functions table's empty state: the run published no functions.
pub fn functions() -> EmptyState {
    EmptyState::new(
        "functions",
        "No functions yet",
        "Lists engine builtins, plugins and your func definitions.",
        "The last run published no functions.",
        "Define one with func, then run.",
    )
    .with_action(EmptyAction::Run)
}

/// The log's empty state: the document's log has no lines.
pub fn logs() -> EmptyState {
    EmptyState::new(
        "logs",
        "No log lines yet",
        "Shows run progress, output, results, timings and diagnostics.",
        "This document's log is empty.",
        "Press Run to fill the log.",
    )
    .with_action(EmptyAction::Run)
}

/// The symbolic inspector's empty state: no committed inspection payload.
pub fn inspector() -> EmptyState {
    EmptyState::new(
        "inspector",
        "Nothing to inspect",
        "Shows pretty, canonical, tree, assumptions, trace and MathIR for the last result.",
        "No run has published an inspection payload.",
        "Run a symbolic expression to inspect it.",
    )
    .with_action(EmptyAction::Run)
}

/// The find bar's empty state: the query matched nothing.
pub fn search(query: &str) -> EmptyState {
    let reason = if query.is_empty() {
        "The find bar has no query yet.".to_string()
    } else {
        format!("No match for {query:?}.")
    };
    EmptyState::new(
        "search",
        "No matches",
        "Searches the active document and highlights matches.",
        reason,
        "Change the query, or turn off Aa, W or .*.",
    )
}

/// The eight empty states, exactly as the panes construct them.
///
/// `run` states share the reason "no run has published anything yet" because that is the same
/// fact for every pane; the purpose and the next step differ, which is what makes each pane's
/// guidance actionable rather than a repeated "nothing here".
pub fn catalog() -> Vec<EmptyState> {
    vec![
        editor(),
        plot(),
        results(),
        workspace(),
        functions(),
        logs(),
        inspector(),
        search("alpha"),
    ]
}

/// The filtered workspace states: the rows exist, the filter hides them.
pub fn filtered(key: &'static str, purpose: &str, reason: &str) -> EmptyState {
    EmptyState::new(
        key,
        "Nothing matches the filter",
        purpose,
        reason,
        "Clear the filter to show every committed row again.",
    )
    .with_action(EmptyAction::ClearFilter)
}

/// Paints `state` inside `rect` and returns the action the user pressed, if any.
///
/// The card is centred horizontally, at most 440 pt wide, and keeps its three facts (purpose,
/// reason, next) in the type scale: the headline is a pane title, the facts are captions, and the
/// action is a body-size button. In a very short pane (under 72 pt) the headline and purpose are
/// dropped so the card never paints outside its clip; the reason and next step always remain.
pub fn paint(ui: &mut Ui, rect: Rect, state: &EmptyState) -> Option<EmptyAction> {
    if rect.width() < 80.0 || rect.height() < 24.0 {
        return None;
    }
    let width = (rect.width() - 16.0).clamp(80.0, 440.0);
    let height = rect.height().min(150.0);
    let card = Rect::from_center_size(rect.center(), egui::vec2(width, height));
    let mut action = None;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(card)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_max_width(width);
            let compact = height < 72.0;
            if !compact {
                ui.add(
                    egui::Label::new(
                        RichText::new(&state.headline)
                            .font(type_scale::Role::PaneTitle.font())
                            .color(theme::text()),
                    )
                    .wrap(),
                );
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&state.purpose)
                            .font(type_scale::Role::Caption.font())
                            .color(theme::text())
                            .line_height(Some(type_scale::Role::Caption.prose_line_pt())),
                    )
                    .wrap(),
                );
            }
            ui.add(
                egui::Label::new(
                    RichText::new(&state.reason)
                        .font(type_scale::Role::Caption.font())
                        .color(theme::dim())
                        .line_height(Some(type_scale::Role::Caption.prose_line_pt())),
                )
                .wrap(),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(&state.next)
                        .font(type_scale::Role::Caption.font())
                        .color(theme::accent())
                        .line_height(Some(type_scale::Role::Caption.prose_line_pt())),
                )
                .wrap(),
            );
            if let Some(offered) = state.action {
                if height >= 48.0 {
                    ui.add_space(2.0);
                    // The stable action id makes the button's egui identity independent of the
                    // pane and of the order in which empty states paint.
                    let button = ui
                        .push_id(offered.id(), |ui| {
                            ui.add(
                                egui::Button::new(
                                    RichText::new(offered.label())
                                        .font(type_scale::Role::Body.font()),
                                )
                                .min_size(egui::vec2(0.0, 22.0)),
                            )
                        })
                        .inner;
                    button.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, offered.label())
                    });
                    if button.clicked() {
                        action = Some(offered);
                    }
                }
            }
        },
    );
    action
}

/// The visible facts of an empty state, in paint order, for tests and capture assertions.
#[cfg(test)]
pub fn facts(state: &EmptyState) -> [&str; 4] {
    [
        state.headline.as_str(),
        state.purpose.as_str(),
        state.reason.as_str(),
        state.next.as_str(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_pane_has_an_empty_state_that_explains_what_why_and_next() {
        let catalog = catalog();
        let keys: Vec<&str> = catalog.iter().map(|state| state.key).collect();
        for expected in [
            "editor",
            "plot",
            "results",
            "workspace",
            "functions",
            "logs",
            "inspector",
            "search",
        ] {
            assert!(keys.contains(&expected), "missing empty state for {expected}: {keys:?}");
        }
        let mut headlines = BTreeSet::new();
        for state in &catalog {
            assert!(!state.headline.is_empty(), "{}: no headline", state.key);
            assert!(!state.purpose.is_empty(), "{}: no purpose", state.key);
            assert!(!state.reason.is_empty(), "{}: no reason", state.key);
            assert!(!state.next.is_empty(), "{}: no next step", state.key);
            assert_ne!(state.purpose, state.reason, "{}: purpose is not the reason", state.key);
            assert_ne!(state.reason, state.next, "{}: next is not the reason", state.key);
            assert!(
                state.next.split_whitespace().count() >= 4,
                "{}: the next step must name an action, got {:?}",
                state.key,
                state.next
            );
            assert!(headlines.insert(state.headline.clone()), "duplicate headline {}", state.headline);
        }
        // Every run-fed pane offers the one command that fills it.
        for key in ["plot", "results", "workspace", "functions", "logs", "inspector"] {
            let state = catalog.iter().find(|s| s.key == key).unwrap();
            assert_eq!(state.action, Some(EmptyAction::Run), "{key} must offer Run");
        }
        assert_eq!(
            catalog.iter().find(|s| s.key == "editor").unwrap().action,
            Some(EmptyAction::FocusEditor)
        );
        let filtered = filtered("workspace", "purpose", "reason");
        assert_eq!(filtered.action, Some(EmptyAction::ClearFilter));
    }

    #[test]
    fn every_action_has_a_distinct_label_and_id() {
        let mut labels = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for action in [
            EmptyAction::Run,
            EmptyAction::ClearFilter,
            EmptyAction::FocusEditor,
        ] {
            assert!(labels.insert(action.label()), "duplicate label {}", action.label());
            assert!(ids.insert(action.id()), "duplicate id {}", action.id());
            assert!(!action.label().is_empty());
        }
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn the_card_paints_its_facts_and_offers_its_action_in_a_real_frame() {
        use egui::Context;
        let ctx = Context::default();
        let state = catalog().into_iter().find(|s| s.key == "plot").unwrap();
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                let rect =
                    Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(400.0, 260.0));
                action = paint(ui, rect, &state);
            },
        );
        output.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut painted = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut painted);
        }
        for fact in facts(&state) {
            assert!(
                painted.iter().any(|text| text.contains(fact)),
                "the card must paint {fact:?}; painted: {painted:?}"
            );
        }
        assert!(
            painted.iter().any(|text| text == EmptyAction::Run.label()),
            "the card must paint its action; painted: {painted:?}"
        );
        assert!(action.is_none(), "no pointer in a headless pass, so nothing was pressed");
    }

    #[test]
    fn a_short_pane_keeps_the_reason_and_next_step_and_drops_the_headline() {
        use egui::Context;
        let ctx = Context::default();
        let state = catalog().into_iter().find(|s| s.key == "logs").unwrap();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                // 60 pt is below the compact threshold; the card must remain readable without
                // the headline and purpose.
                let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(300.0, 60.0));
                let _ = paint(ui, rect, &state);
            },
        );
        output.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut painted = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut painted);
        }
        assert!(painted.iter().any(|t| t.contains(&state.reason)), "{painted:?}");
        assert!(painted.iter().any(|t| t.contains(&state.next)), "{painted:?}");
        assert!(!painted.iter().any(|t| t.contains(&state.headline)), "{painted:?}");
    }
}
