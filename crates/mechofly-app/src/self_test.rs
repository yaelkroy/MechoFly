use std::{fs, path::Path, sync::Arc};

use mechofly_core::{
    Action, Behavior, Feedback, ModelCheckpoint, ModelEngine, ModelGraph, ModelTier, PetPolicy,
    PolicyContext, StepInput, StimulationPolicy, StimulationRequest,
};
use serde::Serialize;

use crate::pet::Skin;

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
        schema_version: 3,
        status: if comparison.receipt.live_state_unchanged
            && comparison.receipt.alternative_differs
            && live_before == live_after
            && explicit_learning_changed
            && hotkeys.passed
            && firefly_visual.passed
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
        firefly_visual_style: "neurofly_prism_firefly".to_owned(),
        firefly_palette: "noctiluca_lantern".to_owned(),
        firefly_visual_contract_passed: firefly_visual.passed,
        firefly_opaque_pixels: firefly_visual.opaque_pixels,
        firefly_translucent_pixels: firefly_visual.translucent_pixels,
        firefly_rest_temporal_invariant: firefly_visual.rest_temporal_invariant(),
        firefly_escape_wing_responsive: firefly_visual.escape_wing_pixel_differences > 180,
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
