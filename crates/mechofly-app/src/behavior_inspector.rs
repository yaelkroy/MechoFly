//! Compact N4 diagnostics. Full explanatory timelines remain the N7 workstream.
use crate::runtime::SimulationSession;
use eframe::egui;

pub fn draw(ui: &mut egui::Ui, session: &SimulationSession) {
    let Some(state) = &session.engine.state.behavior_dynamics else {
        return;
    };
    egui::Panel::bottom("n4-controller-state")
        .exact_size(76.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!(
                "N4 CONTROLLER  |  {:?} / {:?}  |  elapsed {} ms  |  minimum {} ms  |  target {} ms",
                state.current_macro_state, state.current_substate,
                u64::from(state.elapsed_frames) * 33,
                u64::from(state.minimum_dwell_frames) * 33,
                u64::from(state.target_duration_frames) * 33,
            )).strong().size(12.0));
            ui.label(egui::RichText::new(format!(
                "Reason: {}  |  sequence {}  |  draw {:016x}  |  parameters {}",
                state.last_transition_reason.as_str(), state.transition_sequence,
                state.deterministic_duration_draw, &state.parameter_sha256[..12],
            )).size(11.0));
            ui.label(egui::RichText::new(format!(
                "MODELED / ENGINEERING PRIOR (not biological measurements): arousal {}  fatigue {}  contamination {}  exploration {} [Q15]",
                state.context.arousal_q15, state.context.fatigue_q15,
                state.context.contamination_q15, state.context.exploration_q15,
            )).size(10.0));
        });
}
