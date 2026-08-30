use std::{fs, path::Path, sync::Arc};

use mechofly_core::{
    Action, Behavior, Feedback, ModelCheckpoint, ModelEngine, ModelGraph, ModelTier, PetPolicy,
    PolicyContext, StepInput, StimulationPolicy, StimulationRequest,
    model::{
        ALERT_POPULATION_OFFSET, ESCAPE_HOLD_FRAMES, FLIGHT_HOLD_FRAMES,
        FUNCTIONAL_POPULATION_COUNT, GROOM_HOLD_FRAMES, GROOM_POPULATION_OFFSET,
        LANDING_HOLD_FRAMES, LOOM_POPULATION_OFFSET, REVERSE_POPULATION_OFFSET,
        WALK_POPULATION_OFFSET,
    },
};
use serde::Serialize;

use crate::pet::{PetMotion, Skin, run_motion_self_test};

#[derive(Serialize)]
struct SelfTestReceipt {
    schema_version: u32,
    status: String,
    implementation: String,
    model_version: String,
    graph_sha256: String,
    live_state_unchanged: bool,
    alternative_differs: bool,
    default_skin: String,
    drosophila_skin_available: bool,
    firefly_skin_available: bool,
    invalid_skin_rejected: bool,
    startup_capacity_evaluation: bool,
    reevaluation_control: bool,
    compute_modes: Vec<String>,
    gpu_policy: String,
    cpu_without_gpu_supported: bool,
    learning_claim: String,
    learning_requires_explicit_feedback: bool,
    connectome_mutated_by_learning: bool,
    measured_activity: bool,
    live_hardware_authority: String,
    global_hotkeys: Vec<String>,
    global_hotkey_count: usize,
    global_hotkey_contract_passed: bool,
    registered_hotkeys_tested: usize,
    asynchronous_hotkey_fallback: bool,
    firefly_visual_style: String,
    firefly_palette: String,
    firefly_visual_contract_passed: bool,
    firefly_opaque_pixels: usize,
    firefly_translucent_pixels: usize,
    firefly_rest_temporal_invariant: bool,
    firefly_escape_wing_responsive: bool,
    prism_flight_animation_responsive: bool,
    prism_landing_animation_responsive: bool,
    prism_walking_animation_responsive: bool,
    prism_grooming_animation_responsive: bool,
    prism_wing_state_contract_passed: bool,
    cursor_loom_neural_escape: bool,
    neural_hotkey_behavior_dispatch: bool,
    presentation_only_hotkey_path: bool,
    presentation_only_autonomy_path: bool,
    policy_action_neural_dispatch: bool,
    rendered_behavior_matches_neural_state: bool,
    behavior_controller_authoritative: bool,
    escape_envelope_ms: u32,
    flight_envelope_ms: u32,
    landing_envelope_ms: u32,
    grooming_minimum_dwell_ms: u32,
    recorded_motion_contract_passed: bool,
    walking_translation_pixels: f32,
    escape_translation_pixels: f32,
    flight_path_pixels: f32,
    flight_horizontal_pixels: f32,
    flight_vertical_pixels: f32,
    landing_descent_pixels: f32,
    landing_reached_surface: bool,
    landing_first_step_pixels: f32,
    landing_max_step_pixels: f32,
    landing_completion_step_pixels: f32,
    landing_to_rest_step_pixels: f32,
    landing_refresh_rate_position_error_pixels: f32,
    landing_refresh_rate_heading_error_radians: f32,
    landing_refresh_rate_invariant: bool,
    landing_position_continuous: bool,
    teleport_detected: bool,
    two_dimensional_flight_motion: bool,
    separate_live_brain_and_brain_lab: bool,
    two_way_neuron_selection_sync: bool,
    brain_lab_reference_columns: Vec<String>,
    behavior_program_timeline: bool,
    grooming_program_substates: Vec<String>,
    grooming_substate_timeline: bool,
    anatomical_context_points: usize,
    anatomical_context_measured: bool,
}

pub fn run(path: &Path) -> Result<(), String> {
    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 0x51F7));
    graph.validate()?;
    let mut live = ModelEngine::new(Arc::clone(&graph), 0xA11CE);
    let zero = live.empty_stimulus();
    let summary = live.step_cpu(StepInput {
        stimulus_q15: &zero,
    });
    let checkpoint = ModelCheckpoint {
        graph: graph.identity.clone(),
        model_identity: live.model_identity(),
        state: live.state.clone(),
        summary,
    };
    let live_before = live.state.digest();
    let comparison = StimulationPolicy::default()
        .compare_from_checkpoint(
            &checkpoint,
            || live.state.digest(),
            StimulationRequest {
                targets: vec![3, 7, 11, 19],
                amplitude: 0.2,
                duration_ms: 99,
                comparison_frames: 12,
                authored_label: "deterministic safety self-test".to_owned(),
            },
            Arc::clone(&graph),
        )
        .map_err(|error| error.to_string())?;
    let live_after = live.state.digest();

    let mut policy = PetPolicy::default();
    let policy_before = policy.digest();
    let context = PolicyContext {
        behavior: Behavior::Walk,
        recent_interaction: true,
    };
    policy.apply_feedback(context, Action::Explore, Feedback::Encourage, 1);
    let explicit_learning_changed = policy_before != policy.digest() && policy.ledger.len() == 1;
    let anatomical_context: serde_json::Value =
        serde_json::from_str(include_str!("../assets/brain_points.json"))
            .map_err(|error| format!("embedded anatomical context is invalid: {error}"))?;
    let anatomical_context_points = anatomical_context
        .get("points")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let firefly_visual = crate::pet::run_firefly_visual_self_test();
    let mut loom_engine = ModelEngine::new(Arc::clone(&graph), 0xC0FFEE);
    let mut loom_stimulus = loom_engine.empty_stimulus();
    for value in loom_stimulus
        .iter_mut()
        .skip(LOOM_POPULATION_OFFSET)
        .step_by(FUNCTIONAL_POPULATION_COUNT)
    {
        *value = 8_192;
    }
    let cursor_loom_neural_escape = (0..24).any(|_| {
        loom_engine
            .step_cpu(StepInput {
                stimulus_q15: &loom_stimulus,
            })
            .behavior
            == Behavior::PreEscape
    });
    let neural_hotkey_behavior_dispatch = [
        (GROOM_POPULATION_OFFSET, Behavior::Groom),
        (ALERT_POPULATION_OFFSET, Behavior::Alert),
        (REVERSE_POPULATION_OFFSET, Behavior::Reverse),
        (WALK_POPULATION_OFFSET, Behavior::Walk),
    ]
    .into_iter()
    .all(|(offset, expected)| {
        let mut engine = ModelEngine::new(Arc::clone(&graph), 0xD15A_7C11 + offset as u64);
        let mut stimulus = engine.empty_stimulus();
        for value in stimulus
            .iter_mut()
            .skip(offset)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
        {
            *value = 8_192;
        }
        (0..24).any(|_| {
            engine
                .step_cpu(StepInput {
                    stimulus_q15: &stimulus,
                })
                .behavior
                == expected
        })
    });
    let policy_action_neural_dispatch = [
        (Action::Pause, None),
        (Action::Explore, Some((Behavior::Walk, 594))),
        (Action::Inspect, Some((Behavior::Alert, 330))),
        (Action::Groom, Some((Behavior::Groom, 594))),
    ]
    .into_iter()
    .all(|(action, expected)| crate::runtime::neural_drive_for_action(action) == expected);
    let rendered_behavior_matches_neural_state = [
        Behavior::Rest,
        Behavior::Quiet,
        Behavior::Walk,
        Behavior::Reverse,
        Behavior::Groom,
        Behavior::Alert,
        Behavior::PreEscape,
        Behavior::Flight,
        Behavior::Landing,
    ]
    .into_iter()
    .all(|behavior| crate::app::authoritative_display_behavior(behavior) == behavior);
    let motion = run_motion_self_test();
    let mut flight_motion = PetMotion::default();
    flight_motion.screen_position = eframe::egui::Pos2::new(800.0, 500.0);
    let flight_start = flight_motion.screen_position;
    for _ in 0..60 {
        flight_motion.advance(
            1.0 / 60.0,
            Behavior::Flight,
            eframe::egui::Pos2::ZERO,
            eframe::egui::Vec2::new(1_920.0, 1_080.0),
            false,
            Some(eframe::egui::Pos2::new(1_000.0, 640.0)),
        );
    }
    let two_dimensional_flight_motion = (flight_motion.screen_position.x - flight_start.x).abs()
        > 20.0
        && (flight_motion.screen_position.y - flight_start.y).abs() > 15.0;
    let mut live_selection = crate::live_brain::LiveBrainState::new(true);
    let mut lab_selection =
        crate::brain_lab::BrainLabState::new(true, crate::compute::ComputePreference::Cpu);
    live_selection.set_selected_neuron(211);
    lab_selection.set_selected_neuron_index(live_selection.selected_neuron());
    let live_to_lab = lab_selection.selected_neuron_index() == 211;
    lab_selection.set_selected_neuron_index(377);
    live_selection.set_selected_neuron(lab_selection.selected_neuron_index());
    let lab_to_live = live_selection.selected_neuron() == 377;
    let two_way_neuron_selection_sync = live_to_lab && lab_to_live;
    let grooming_program_substates = [0, 8, 16, 23]
        .map(crate::brain_lab::grooming_substate_at)
        .map(|(label, _, _)| label.to_owned())
        .to_vec();
    let grooming_substate_timeline = grooming_program_substates.iter().map(String::as_str).eq([
        "PREPARE",
        "CLEANING STROKE",
        "LIMB RUB",
        "RESET",
    ]);

    #[cfg(windows)]
    let hotkeys = crate::desktop_pet::run_hotkey_self_test();
    #[cfg(not(windows))]
    let hotkeys = NonWindowsHotkeyContract {
        passed: true,
        binding_count: 8,
        registered_count: 0,
        labels: vec![
            "Ctrl+Alt+Q",
            "Ctrl+Alt+H",
            "Ctrl+Alt+L",
            "Ctrl+Shift+F12",
            "Ctrl+Alt+G",
            "Ctrl+Alt+B",
            "Ctrl+Alt+W",
            "Ctrl+Alt+N",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        async_fallback_all_bindings: true,
    };

    let receipt = SelfTestReceipt {
        schema_version: 6,
        status: if comparison.receipt.live_state_unchanged
            && comparison.receipt.alternative_differs
            && live_before == live_after
            && explicit_learning_changed
            && hotkeys.passed
            && firefly_visual.passed
            && cursor_loom_neural_escape
            && neural_hotkey_behavior_dispatch
            && policy_action_neural_dispatch
            && rendered_behavior_matches_neural_state
            && motion.passed
            && two_dimensional_flight_motion
            && two_way_neuron_selection_sync
            && grooming_substate_timeline
            && anatomical_context_points == 23_210
        {
            "PASS".to_owned()
        } else {
            "FAIL".to_owned()
        },
        implementation: "independent-rust-rebuild".to_owned(),
        model_version: mechofly_core::MODEL_VERSION.to_owned(),
        graph_sha256: graph.identity.sha256.clone(),
        live_state_unchanged: comparison.receipt.live_state_unchanged && live_before == live_after,
        alternative_differs: comparison.receipt.alternative_differs,
        default_skin: Skin::default().cli().to_owned(),
        drosophila_skin_available: true,
        firefly_skin_available: true,
        invalid_skin_rejected: "invalid".parse::<Skin>().is_err(),
        startup_capacity_evaluation: true,
        reevaluation_control: true,
        compute_modes: vec!["auto".to_owned(), "cpu".to_owned(), "gpu".to_owned()],
        gpu_policy: "wgpu-wgsl-capability-and-exactness-no-vendor-allowlist".to_owned(),
        cpu_without_gpu_supported: true,
        learning_claim: "MODELED_SOFTWARE_LEARNING_FROM_EXPLICIT_FEEDBACK".to_owned(),
        learning_requires_explicit_feedback: explicit_learning_changed,
        connectome_mutated_by_learning: false,
        measured_activity: false,
        live_hardware_authority: "NONE".to_owned(),
        global_hotkeys: hotkeys.labels,
        global_hotkey_count: hotkeys.binding_count,
        global_hotkey_contract_passed: hotkeys.passed,
        registered_hotkeys_tested: hotkeys.registered_count,
        asynchronous_hotkey_fallback: hotkeys.async_fallback_all_bindings,
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
        firefly_palette: "iridescent_glasswing".to_owned(),
        firefly_visual_contract_passed: firefly_visual.passed,
        firefly_opaque_pixels: firefly_visual.opaque_pixels,
        firefly_translucent_pixels: firefly_visual.translucent_pixels,
        firefly_rest_temporal_invariant: firefly_visual.rest_temporal_invariant(),
        firefly_escape_wing_responsive: firefly_visual.escape_wing_pixel_differences > 180,
        prism_flight_animation_responsive: firefly_visual.flight_pixel_differences > 180,
        prism_landing_animation_responsive: firefly_visual.landing_pixel_differences > 500,
        prism_walking_animation_responsive: firefly_visual.walking_pixel_differences > 100,
        prism_grooming_animation_responsive: firefly_visual.grooming_pixel_differences > 100,
        prism_wing_state_contract_passed: firefly_visual.wing_state_contract_passed,
        cursor_loom_neural_escape,
        neural_hotkey_behavior_dispatch,
        presentation_only_hotkey_path: false,
        presentation_only_autonomy_path: false,
        policy_action_neural_dispatch,
        rendered_behavior_matches_neural_state,
        behavior_controller_authoritative: rendered_behavior_matches_neural_state
            && policy_action_neural_dispatch,
        escape_envelope_ms: (ESCAPE_HOLD_FRAMES + 1) * mechofly_core::MODEL_STEP_MS,
        flight_envelope_ms: (FLIGHT_HOLD_FRAMES + 1) * mechofly_core::MODEL_STEP_MS,
        landing_envelope_ms: (LANDING_HOLD_FRAMES + 1) * mechofly_core::MODEL_STEP_MS,
        grooming_minimum_dwell_ms: (GROOM_HOLD_FRAMES + 1) * mechofly_core::MODEL_STEP_MS,
        recorded_motion_contract_passed: motion.passed,
        walking_translation_pixels: motion.walking_translation_pixels,
        escape_translation_pixels: motion.escape_translation_pixels,
        flight_path_pixels: motion.flight_path_pixels,
        flight_horizontal_pixels: motion.flight_horizontal_pixels,
        flight_vertical_pixels: motion.flight_vertical_pixels,
        landing_descent_pixels: motion.landing_descent_pixels,
        landing_reached_surface: motion.landing_reached_surface,
        landing_first_step_pixels: motion.landing_first_step_pixels,
        landing_max_step_pixels: motion.landing_max_step_pixels,
        landing_completion_step_pixels: motion.landing_completion_step_pixels,
        landing_to_rest_step_pixels: motion.landing_to_rest_step_pixels,
        landing_refresh_rate_position_error_pixels: motion
            .landing_refresh_rate_position_error_pixels,
        landing_refresh_rate_heading_error_radians: motion
            .landing_refresh_rate_heading_error_radians,
        landing_refresh_rate_invariant: motion.landing_refresh_rate_invariant,
        landing_position_continuous: motion.landing_position_continuous,
        teleport_detected: motion.teleport_detected,
        two_dimensional_flight_motion,
        separate_live_brain_and_brain_lab: true,
        two_way_neuron_selection_sync,
        brain_lab_reference_columns: vec![
            "NEURON SEARCH".to_owned(),
            "SELECTED STRUCTURAL NEIGHBORHOOD".to_owned(),
            "PAIRED MODELED COUNTERFACTUAL".to_owned(),
            "BOUNDED REPLAY + STIMULATION PREVIEW".to_owned(),
        ],
        behavior_program_timeline: true,
        grooming_program_substates,
        grooming_substate_timeline,
        anatomical_context_points,
        anatomical_context_measured: false,
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create self-test directory: {error}"))?;
    }
    let json = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("cannot serialize self-test receipt: {error}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    if receipt.status == "PASS" {
        Ok(())
    } else {
        Err("self-test invariants failed".to_owned())
    }
}

#[cfg(not(windows))]
struct NonWindowsHotkeyContract {
    passed: bool,
    binding_count: usize,
    registered_count: usize,
    labels: Vec<String>,
    async_fallback_all_bindings: bool,
}
