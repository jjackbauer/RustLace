//! The native egui workbench window (eframe/egui 0.37) and its panes.
//!
//! The measured prototype in `rust/lovelace-desktop-spike` is the source of the editor,
//! syntax, plot and layout code; this module adapts those pieces to the product's
//! [`crate::workbench::Workbench`] instead of the spike's sample documents, sample variables
//! and sample SVG.
//!
//! A11Y-003/A11Y-014: [`a11y`] owns the window's single keyboard-focus ring and the ASCII/Unicode
//! localization of the frame's `DocumentView`, so both are defined once instead of per pane.
//! A11Y-005: [`typography`] resolves the configurable editor font, size, line height and ligature
//! choice into the one set of metrics every editor decoration derives from.

pub(crate) mod a11y;
mod app;
pub(crate) mod control;
pub(crate) mod editor;
pub(crate) mod empty;
pub(crate) mod file_dialogs;
pub(crate) mod measure;
pub(crate) mod measure_run;
pub(crate) mod plot;
pub(crate) mod semantic;
mod syntax;
pub(crate) mod theme;
pub(crate) mod type_scale;
pub(crate) mod typography;

pub use app::run;

/// `--visual-audit`: the VIS-004/006/007/008 tables, printed from the same constants and
/// constructors the window paints with.
///
/// It is an audit, not a proposal: the type scale is [`type_scale::Role`], the state table is
/// [`control::paint`], the semantic carriers are [`semantic::SemanticRole`] and the empty states
/// are [`empty::catalog`]. A deliberate edit to any of them changes this output, so the artifact
/// cannot drift from the UI.
pub fn visual_audit_cli() -> i32 {
    println!("lovelace-desktop visual audit (VIS-004, VIS-006, VIS-007, VIS-008)");
    println!("\n== type scale (VIS-004) ==");
    println!("role | pt | weight | family | line target | line ratio");
    for role in type_scale::Role::ALL {
        println!(
            "{} | {:.1} | {:?} | {:?} | {:.2} | {:.2}x",
            role.name(),
            role.pt(),
            role.weight(),
            role.font().family,
            role.line_pt(),
            role.line_ratio()
        );
    }
    println!(
        "smallest role: {} at {:.1} pt; minimum body/code: {:.1}/{:.1} pt; prose line ratio: {:.2}x",
        type_scale::smallest_role().name(),
        type_scale::smallest_role().pt(),
        type_scale::BODY_PT,
        type_scale::CODE_PT,
        type_scale::PROSE_LINE_RATIO
    );

    println!("\n== control state matrix (VIS-006) ==");
    println!("kind | state | fill | border | width | dashed | text | cue | non-colour carrier");
    for kind in control::ControlKind::ALL {
        for state in control::ControlState::ALL {
            let paint = control::paint(kind, state);
            println!(
                "{} | {} | #{:02x}{:02x}{:02x} | #{:02x}{:02x}{:02x} | {:.0} | {} | #{:02x}{:02x}{:02x} a={} | {:?} | {}",
                kind.name(),
                state.name(),
                paint.fill.r(),
                paint.fill.g(),
                paint.fill.b(),
                paint.border.r(),
                paint.border.g(),
                paint.border.b(),
                paint.border_width,
                paint.dashed,
                paint.text.r(),
                paint.text.g(),
                paint.text.b(),
                paint.text.a(),
                paint.cue,
                state.carrier()
            );
        }
    }

    println!("\n== semantic colour roles (VIS-007) ==");
    print!("{}", semantic::role_table());

    println!("\n== empty states (VIS-008) ==");
    println!("key | headline | purpose | reason | next | action");
    for state in empty::catalog() {
        println!(
            "{} | {} | {} | {} | {} | {}",
            state.key,
            state.headline,
            state.purpose,
            state.reason,
            state.next,
            state.action.map(|action| action.label()).unwrap_or("-")
        );
    }
    println!(
        "\ncounts: {} type roles, {} control kinds x {} states, {} semantic roles, {} empty states",
        type_scale::Role::ALL.len(),
        control::ControlKind::ALL.len(),
        control::ControlState::ALL.len(),
        semantic::SemanticRole::ALL.len(),
        empty::catalog().len()
    );
    0
}
