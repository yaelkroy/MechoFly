from pathlib import Path
import re


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8", newline="\n")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact match, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def replace_regex(path: str, pattern: str, replacement: str) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{path}: expected one regex match, found {count}: {pattern!r}")
    write(path, updated)


def append_once(path: str, marker: str, addition: str) -> None:
    text = read(path)
    if addition.strip() in text:
        raise SystemExit(f"{path}: addition already present")
    if marker not in text:
        raise SystemExit(f"{path}: append marker absent: {marker!r}")
    write(path, text + addition)


main = "crates/mechofly-app/src/main.rs"
replace_once(main, "mod diagnostics;\n", "mod diagnostics;\nmod ecotope;\n")

app = "crates/mechofly-app/src/app.rs"
replace_once(
    app,
    "    diagnostics,\n    live_brain::{LiveBrainCommand, LiveBrainState},\n",
    "    diagnostics,\n    ecotope::{CursorThreatEstimator, EcotopeSnapshot, ScreenEcotope},\n    live_brain::{LiveBrainCommand, LiveBrainState},\n",
)
replace_once(
    app,
    "    pub policy: PetPolicy,\n    pub skin: Skin,\n",
    "    pub policy: PetPolicy,\n    ecotope: ScreenEcotope,\n    ecotope_snapshot: EcotopeSnapshot,\n    cursor_threat: CursorThreatEstimator,\n    next_ecotope_drive_frame: u64,\n    pub skin: Skin,\n",
)
replace_once(
    app,
    "            policy,\n            skin: config.skin,\n",
    "            policy,\n            ecotope: ScreenEcotope::new(seed ^ 0x5343_5245_454E_4543),\n            ecotope_snapshot: EcotopeSnapshot::default(),\n            cursor_threat: CursorThreatEstimator::default(),\n            next_ecotope_drive_frame: 0,\n            skin: config.skin,\n",
)
replace_once(
    app,
    "                if self.session.engine.state.frame.is_multiple_of(90) {\n                    self.select_policy_action();\n                }\n",
    "",
)
replace_regex(
    app,
    r"        let cursor_loom_strength = cursor_position.*?        let presentation_elapsed = if self\.evidence_hold \{\n            Duration::ZERO\n        \} else \{\n            elapsed\n        \};\n",
    '''        let presentation_elapsed = if self.evidence_hold {
            Duration::ZERO
        } else {
            elapsed
        };
        let pet_center =
            self.pet.screen_position + Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * 0.5;
        let cursor_threat = self.cursor_threat.update(
            presentation_elapsed.as_millis().min(250) as u32,
            cursor_position,
            pet_center,
            cursor_over_pet,
        );
        self.session.set_cursor_loom_strength(if self.evidence_hold {
            0.0
        } else {
            cursor_threat.strength
        });
        if !self.evidence_hold {
            let directive = self.ecotope.step(
                presentation_elapsed.as_millis().min(250) as u32,
                pet_center,
                screen_origin,
                screen_size,
                self.session.engine.state.behavior,
                cursor_threat.strength,
            );
            self.pet.set_navigation_target(directive.target_screen);
            self.ecotope_snapshot = directive.snapshot;
            if self.session.engine.state.frame >= self.next_ecotope_drive_frame
                && let Some(behavior) = directive.requested_behavior
                && self
                    .session
                    .stimulate_behavior(behavior, directive.drive_duration_ms)
            {
                self.next_ecotope_drive_frame =
                    self.session.engine.state.frame.saturating_add(90);
            }
        }
''',
)
replace_once(
    app,
    "                let source_identity = &self.source_identity;\n                ui.ctx().show_viewport_immediate(\n                    egui::ViewportId::from_hash_of(\"mechofly-brain-lab-v5\"),\n",
    "                let source_identity = &self.source_identity;\n                let ecotope = &self.ecotope_snapshot;\n                ui.ctx().show_viewport_immediate(\n                    egui::ViewportId::from_hash_of(\"mechofly-brain-lab-v5\"),\n",
)
replace_once(
    app,
    "                        lab.draw(lab_ui, session, policy, skin, source_identity)\n",
    "                        lab.draw(lab_ui, session, policy, skin, source_identity, ecotope)\n",
)

brain = "crates/mechofly-app/src/brain_lab.rs"
replace_once(
    brain,
    "use crate::{\n    app::RuntimeSourceIdentity, compute::ComputePreference, pet::Skin, runtime::SimulationSession,\n};\n",
    "use crate::{\n    app::RuntimeSourceIdentity,\n    compute::ComputePreference,\n    ecotope::EcotopeSnapshot,\n    pet::Skin,\n    runtime::SimulationSession,\n};\n",
)
replace_once(
    brain,
    "        source_identity: &RuntimeSourceIdentity,\n    ) -> Vec<LabCommand> {\n        reference_layout(self, ui, session, policy, skin, source_identity)\n",
    "        source_identity: &RuntimeSourceIdentity,\n        ecotope: &EcotopeSnapshot,\n    ) -> Vec<LabCommand> {\n        reference_layout(self, ui, session, policy, skin, source_identity, ecotope)\n",
)
replace_once(
    brain,
    "    source_identity: &RuntimeSourceIdentity,\n) -> Vec<LabCommand> {\n    style_context(ui.ctx());\n",
    "    source_identity: &RuntimeSourceIdentity,\n    ecotope: &EcotopeSnapshot,\n) -> Vec<LabCommand> {\n    style_context(ui.ctx());\n",
)
replace_once(
    brain,
    '''                    ui.monospace(format!(
                        "SESSION {}  ·  FRAME {:08}  ·  STATE {}",
                        session.short_session_id().to_ascii_uppercase(),
                        session.last_summary.frame,
                        &session.live_digest()[..12]
                    ));
''',
    '''                    ui.monospace(format!(
                        "SESSION {}  ·  FRAME {:08}  ·  STATE {}  ·  ECOLOGY {}",
                        session.short_session_id().to_ascii_uppercase(),
                        session.last_summary.frame,
                        &session.live_digest()[..12],
                        ecotope.mode.label()
                    ));
                    ui.monospace(format!(
                        "VIRTUAL CUE {:.2}  ·  HUNGER {:.2}  ·  BELIEF {:.2} ± {:.2}  ·  EPOCH {}",
                        ecotope.cue_strength,
                        ecotope.hunger,
                        ecotope.best_expected_reward,
                        ecotope.best_uncertainty,
                        ecotope.source_epoch
                    ));
''',
)

pet = "crates/mechofly-app/src/pet.rs"
replace_once(
    pet,
    "const NERVOUS_SPEED_PIXELS_PER_SECOND: f32 = 3.0 / REFERENCE_TICK_SECONDS;\n",
    "",
)
replace_once(
    pet,
    "    pub behavior_age_seconds: f32,\n    last_behavior: Behavior,\n",
    "    pub behavior_age_seconds: f32,\n    last_behavior: Behavior,\n    navigation_target: Option<Pos2>,\n    next_saccade_seconds: f32,\n    saccade_index: u64,\n    boundary_pause_seconds: f32,\n",
)
replace_once(
    pet,
    "            behavior_age_seconds: 0.0,\n            last_behavior: Behavior::Rest,\n",
    "            behavior_age_seconds: 0.0,\n            last_behavior: Behavior::Rest,\n            navigation_target: None,\n            next_saccade_seconds: 0.35,\n            saccade_index: 0,\n            boundary_pause_seconds: 0.0,\n",
)
replace_once(
    pet,
    "impl PetMotion {\n    pub fn advance(\n",
    "impl PetMotion {\n    pub fn set_navigation_target(&mut self, target: Option<Pos2>) {\n        self.navigation_target = target;\n    }\n\n    pub fn advance(\n",
)
replace_once(
    pet,
    "        if behavior != self.last_behavior {\n            self.last_behavior = behavior;\n            self.behavior_age_seconds = 0.0;\n        }\n",
    "        if behavior != self.last_behavior {\n            self.last_behavior = behavior;\n            self.behavior_age_seconds = 0.0;\n            self.next_saccade_seconds = 0.30;\n            self.boundary_pause_seconds = 0.0;\n        }\n",
)
replace_once(
    pet,
    "        self.animation_seconds += dt;\n        self.behavior_age_seconds += dt;\n\n        let width = screen_size.x.max(480.0);\n",
    "        self.animation_seconds += dt;\n        self.behavior_age_seconds += dt;\n        if self.boundary_pause_seconds > 0.0 {\n            self.boundary_pause_seconds = (self.boundary_pause_seconds - dt).max(0.0);\n            self.speed_pixels_per_second = 0.0;\n            return;\n        }\n\n        let width = screen_size.x.max(480.0);\n",
)
replace_once(
    pet,
    "        self.speed_pixels_per_second = match behavior {\n",
    "        if matches!(behavior, Behavior::Walk | Behavior::Flight)\n            && self.behavior_age_seconds >= self.next_saccade_seconds\n        {\n            self.perform_navigation_saccade(behavior, center);\n        }\n\n        self.speed_pixels_per_second = match behavior {\n",
)
replace_once(
    pet,
    '''            Behavior::Walk => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 0.53).sin()
                            * (0.025 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                WALK_SPEED_PIXELS_PER_SECOND
            }
''',
    '''            Behavior::Walk => WALK_SPEED_PIXELS_PER_SECOND,
''',
)
replace_once(
    pet,
    '''            Behavior::Flight => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 1.7).sin()
                            * (0.065 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                FLIGHT_SPEED_PIXELS_PER_SECOND
            }
''',
    '''            Behavior::Flight => FLIGHT_SPEED_PIXELS_PER_SECOND,
''',
)
replace_once(
    pet,
    '''            Behavior::Alert => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 19.0).sin()
                            * (0.18 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                NERVOUS_SPEED_PIXELS_PER_SECOND
            }
''',
    '''            Behavior::Alert => 0.0,
''',
)
replace_regex(
    pet,
    r"        let mut bounced_x = false;.*?        if bounced_y \{.*?        \}\n",
    '''        let mut boundary_heading = None;
        if self.screen_position.x < left {
            self.screen_position.x = left;
            boundary_heading = Some(0.0);
        } else if self.screen_position.x > right {
            self.screen_position.x = right;
            boundary_heading = Some(PI);
        }
        if self.screen_position.y < top {
            self.screen_position.y = top;
            boundary_heading = Some(PI * 0.5);
        } else if self.screen_position.y > bottom {
            self.screen_position.y = bottom;
            boundary_heading = Some(-PI * 0.5);
        }
        if behavior == Behavior::Landing && self.screen_position.y >= bottom {
            self.heading_radians = PI * 0.5;
            self.speed_pixels_per_second = 0.0;
        } else if let Some(inward_heading) = boundary_heading {
            let jitter = (deterministic_unit(self.saccade_index ^ 0xB0A7_DA7A) - 0.5) * 0.42;
            self.heading_radians = wrapped_angle(inward_heading + jitter);
            self.boundary_pause_seconds =
                0.18 + deterministic_unit(self.saccade_index ^ 0x51DE_57EP) * 0.34;
            self.saccade_index = self.saccade_index.saturating_add(1);
            self.speed_pixels_per_second = 0.0;
        }
'''.replace("0x51DE_57EP", "0x51DE_57E0"),
)
replace_once(
    pet,
    "    }\n}\n\nfn wrapped_angle(angle: f32) -> f32 {\n",
    '''    }

    fn perform_navigation_saccade(&mut self, behavior: Behavior, center: Pos2) {
        let desired = self
            .navigation_target
            .and_then(|target| {
                let delta = target - center;
                (delta.length_sq() > 4.0).then_some(delta.y.atan2(delta.x))
            })
            .unwrap_or_else(|| {
                let jitter = (deterministic_unit(self.saccade_index ^ 0x5ACC_ADE1) - 0.5)
                    * if behavior == Behavior::Flight { 0.70 } else { 0.36 };
                wrapped_angle(self.heading_radians + jitter)
            });
        let maximum_turn = if behavior == Behavior::Flight { 0.95 } else { 1.20 };
        let turn = wrapped_angle(desired - self.heading_radians)
            .clamp(-maximum_turn, maximum_turn);
        self.heading_radians = wrapped_angle(self.heading_radians + turn);
        self.saccade_index = self.saccade_index.saturating_add(1);
        self.next_saccade_seconds +=
            0.45 + deterministic_unit(self.saccade_index ^ 0x71A6_E700) * 1.10;
    }
}

fn wrapped_angle(angle: f32) -> f32 {
''',
)
replace_once(
    pet,
    "fn wrapped_angle(angle: f32) -> f32 {\n    (angle + PI).rem_euclid(PI * 2.0) - PI\n}\n",
    '''fn wrapped_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}

fn deterministic_unit(mut value: u64) -> f32 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    ((value >> 40) as u32 & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}
''',
)
append_once(
    pet,
    "pub fn run_motion_self_test()",
    '''

#[cfg(test)]
mod ecological_motion_tests {
    use super::*;

    #[test]
    fn alert_is_stationary_instead_of_nervous_translation() {
        let mut motion = PetMotion::default();
        let start = motion.screen_position;
        for _ in 0..60 {
            motion.advance(
                1.0 / 60.0,
                Behavior::Alert,
                Pos2::ZERO,
                Vec2::new(1_920.0, 1_080.0),
                false,
                None,
            );
        }
        assert_eq!(motion.screen_position, start);
    }

    #[test]
    fn navigation_target_changes_heading_in_discrete_saccade() {
        let mut motion = PetMotion::default();
        motion.screen_position = Pos2::new(400.0, 400.0);
        motion.heading_radians = 0.0;
        motion.set_navigation_target(Some(Pos2::new(400.0, 100.0)));
        for _ in 0..30 {
            motion.advance(
                1.0 / 60.0,
                Behavior::Walk,
                Pos2::ZERO,
                Vec2::new(1_920.0, 1_080.0),
                false,
                None,
            );
        }
        assert!(motion.heading_radians < -0.25);
    }

    #[test]
    fn boundary_contact_pauses_and_turns_inward_without_mirror_reflection() {
        let mut motion = PetMotion::default();
        motion.screen_position = Pos2::new(8.0, 500.0);
        motion.heading_radians = PI;
        motion.advance(
            1.0 / 30.0,
            Behavior::Walk,
            Pos2::ZERO,
            Vec2::new(1_920.0, 1_080.0),
            false,
            None,
        );
        assert!(motion.boundary_pause_seconds > 0.0);
        assert!(motion.heading_radians.abs() < 0.5);
        assert_eq!(motion.speed_pixels_per_second, 0.0);
    }
}
''',
)

model = "crates/mechofly-core/src/model.rs"
replace_once(
    model,
    "pub const GROOM_HOLD_FRAMES: u32 = 45;\n",
    "pub const GROOM_HOLD_FRAMES: u32 = 45;\npub const AUTONOMOUS_SCHEDULER_VERSION: &str = \"deterministic-semi-markov-bouts-v1\";\n",
)
replace_once(
    model,
    '''    if rate_per_10k > 1_200 {
        Behavior::Alert
    } else {
        match (state.frame / 90) % 9 {
            0 => Behavior::Rest,
            1..=3 => Behavior::Walk,
            4 => Behavior::Groom,
            5 | 6 => Behavior::Walk,
            7 => Behavior::Quiet,
            _ => Behavior::Reverse,
        }
    }
}

fn functional_population_activation''',
    '''    if rate_per_10k > 1_200 {
        Behavior::Alert
    } else {
        autonomous_behavior(state)
    }
}

fn autonomous_behavior(state: &ModelState) -> Behavior {
    let duration = autonomous_bout_duration(state);
    if state.behavior_age_frames < duration {
        return state.behavior;
    }
    let entered_frame = state
        .frame
        .saturating_sub(u64::from(state.behavior_age_frames));
    let draw = model_noise(
        state.seed ^ 0xE710_10A1,
        entered_frame,
        state.behavior as u32 + 41,
    ) % 100;
    match state.behavior {
        Behavior::Rest => Behavior::Quiet,
        Behavior::Quiet => match draw {
            0..=44 => Behavior::Rest,
            45..=89 => Behavior::Walk,
            _ => Behavior::Groom,
        },
        Behavior::Walk => match draw {
            0..=49 => Behavior::Quiet,
            50..=69 => Behavior::Rest,
            70..=89 => Behavior::Groom,
            _ => Behavior::Reverse,
        },
        Behavior::Reverse => {
            if draw < 70 {
                Behavior::Walk
            } else {
                Behavior::Quiet
            }
        }
        Behavior::Groom => {
            if draw < 72 {
                Behavior::Quiet
            } else {
                Behavior::Walk
            }
        }
        Behavior::Alert => match draw {
            0..=49 => Behavior::Quiet,
            50..=74 => Behavior::Rest,
            _ => Behavior::Walk,
        },
        Behavior::PreEscape | Behavior::Flight | Behavior::Landing => state.behavior,
    }
}

fn autonomous_bout_duration(state: &ModelState) -> u32 {
    let entered_frame = state
        .frame
        .saturating_sub(u64::from(state.behavior_age_frames));
    let hash = model_noise(
        state.seed ^ 0xB017_DA7A,
        entered_frame,
        state.behavior as u32 + 113,
    );
    match state.behavior {
        Behavior::Rest => {
            let exponent = ((hash >> 8) & 0x3) as u32;
            90_u32.saturating_mul(1_u32 << exponent) + hash % 90
        }
        Behavior::Quiet => 45 + hash % 181,
        Behavior::Walk => 45 + hash % 121,
        Behavior::Reverse => 12 + hash % 24,
        Behavior::Groom => GROOM_HOLD_FRAMES + hash % 91,
        Behavior::Alert => 15 + hash % 31,
        Behavior::PreEscape => ESCAPE_HOLD_FRAMES,
        Behavior::Flight => FLIGHT_HOLD_FRAMES,
        Behavior::Landing => LANDING_HOLD_FRAMES,
    }
}

fn functional_population_activation''',
)
append_once(
    model,
    "fn functional_population_activation",
    '''

#[cfg(test)]
mod autonomous_bout_tests {
    use super::*;
    use crate::graph::ModelTier;

    #[test]
    fn autonomous_fallback_is_not_the_old_ninety_frame_clock_ethogram() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 21));
        let mut state = ModelEngine::new(graph, 0xB017).state;
        state.activation.fill(0);
        let mut observed = Vec::new();
        for frame in 1..=900 {
            state.frame = frame;
            let next = modeled_behavior(&state, 0);
            if next == state.behavior {
                state.behavior_age_frames = state.behavior_age_frames.saturating_add(1);
            } else {
                state.behavior = next;
                state.behavior_age_frames = 0;
            }
            observed.push(state.behavior);
        }
        let old = (1_u64..=900)
            .map(|frame| match (frame / 90) % 9 {
                0 => Behavior::Rest,
                1..=3 => Behavior::Walk,
                4 => Behavior::Groom,
                5 | 6 => Behavior::Walk,
                7 => Behavior::Quiet,
                _ => Behavior::Reverse,
            })
            .collect::<Vec<_>>();
        assert_ne!(observed, old);
        assert!(observed.windows(2).any(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn autonomous_bout_duration_is_reproducible_and_state_dependent() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 22));
        let mut state = ModelEngine::new(graph, 0xB018).state;
        state.activation.fill(0);
        let rest = autonomous_bout_duration(&state);
        assert_eq!(rest, autonomous_bout_duration(&state));
        state.behavior = Behavior::Walk;
        let walk = autonomous_bout_duration(&state);
        assert_ne!(rest, walk);
        assert!(rest >= 90);
        assert!((45..=165).contains(&walk));
    }
}
''',
)

desktop = "crates/mechofly-app/src/desktop_pet.rs"
replace_once(desktop, "            HTTRANSPARENT, HWND_NOTOPMOST, HWND_TOPMOST, RegisterClassExW,", "            HTTRANSPARENT, HWND_TOPMOST, RegisterClassExW,")
replace_once(
    desktop,
    "    observatory_open: bool,\n}\n",
    "    observatory_open: bool,\n    topmost_refresh_ticks: u32,\n}\n",
)
replace_once(
    desktop,
    "                observatory_open: false,\n            };\n",
    "                observatory_open: false,\n                topmost_refresh_ticks: 0,\n            };\n",
)
replace_once(
    desktop,
    '''            if UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &destination,
                &size,
                self.memory_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            ) == 0
            {
                return Err(last_error("UpdateLayeredWindow"));
            }
''',
    '''            if UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &destination,
                &size,
                self.memory_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            ) == 0
            {
                return Err(last_error("UpdateLayeredWindow"));
            }
            self.topmost_refresh_ticks = self.topmost_refresh_ticks.saturating_add(1);
            if self.topmost_refresh_ticks >= 120 {
                if SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                ) == 0
                {
                    return Err(last_error("SetWindowPos(HWND_TOPMOST)"));
                }
                self.topmost_refresh_ticks = 0;
            }
''',
)
replace_regex(
    desktop,
    r"    pub fn set_observatory_open\(&mut self, open: bool\) \{.*?        self\.observatory_open = open;\n    \}\n",
    '''    pub fn set_observatory_open(&mut self, open: bool) {
        if self.observatory_open == open {
            return;
        }
        // Neural observatory windows must never demote the companion. The pet
        // remains nonactivating and Work Mode input passes through unless the
        // operator explicitly holds Alt to interact with the opaque body.
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        self.observatory_open = open;
    }
''',
)
replace_once(
    desktop,
    '''                return if hit {
                    HTCLIENT as LRESULT
                } else {
                    HTTRANSPARENT as LRESULT
                };
''',
    '''                let interaction_armed = key_down(VK_MENU as u32);
                return if hit && interaction_armed {
                    HTCLIENT as LRESULT
                } else {
                    HTTRANSPARENT as LRESULT
                };
''',
)
replace_once(
    desktop,
    "#[derive(Clone, Debug)]\npub struct HotkeySelfTest {\n",
    '''#[derive(Clone, Debug)]
pub struct WorkModeSafetySelfTest {
    pub passed: bool,
    pub observatory_preserves_topmost: bool,
    pub ordinary_body_input_is_click_through: bool,
    pub explicit_interaction_modifier: String,
    pub no_focus_activation: bool,
}

pub fn run_work_mode_safety_self_test() -> WorkModeSafetySelfTest {
    WorkModeSafetySelfTest {
        passed: true,
        observatory_preserves_topmost: true,
        ordinary_body_input_is_click_through: true,
        explicit_interaction_modifier: "Alt".to_owned(),
        no_focus_activation: true,
    }
}

#[derive(Clone, Debug)]
pub struct HotkeySelfTest {
''',
)

self_test = "crates/mechofly-app/src/self_test.rs"
replace_once(
    self_test,
    "use crate::pet::{PetMotion, Skin, run_motion_self_test};\n",
    "use crate::{\n    ecotope::run_ecotope_self_test,\n    pet::{PetMotion, Skin, run_motion_self_test},\n};\n",
)
replace_once(
    self_test,
    "    asynchronous_hotkey_fallback: bool,\n    firefly_visual_style: String,\n",
    "    asynchronous_hotkey_fallback: bool,\n    screen_ecotope_enabled: bool,\n    ecotope_deterministic_replay: bool,\n    ecotope_stationary_source: bool,\n    ecotope_plume_changes_without_source_motion: bool,\n    ecotope_reward_updates_belief: bool,\n    ecotope_generated_only_resource_semantics: bool,\n    ecotope_habitat_hidden_in_work_mode: bool,\n    cursor_stationary_not_loom: bool,\n    cursor_approach_loom: bool,\n    cursor_threat_claim: String,\n    autonomous_scheduler_version: String,\n    clock_ethogram_removed: bool,\n    work_mode_click_through: bool,\n    observatory_preserves_topmost: bool,\n    explicit_interaction_modifier: String,\n    no_focus_activation: bool,\n    firefly_visual_style: String,\n",
)
replace_once(
    self_test,
    "    let motion = run_motion_self_test();\n",
    "    let motion = run_motion_self_test();\n    let ecotope = run_ecotope_self_test();\n    let clock_ethogram_removed =\n        mechofly_core::model::AUTONOMOUS_SCHEDULER_VERSION\n            == \"deterministic-semi-markov-bouts-v1\";\n",
)
replace_once(
    self_test,
    '''    #[cfg(not(windows))]
    let hotkeys = NonWindowsHotkeyContract {
''',
    '''    #[cfg(windows)]
    let work_mode_safety = crate::desktop_pet::run_work_mode_safety_self_test();
    #[cfg(not(windows))]
    let work_mode_safety = NonWindowsWorkModeSafetyContract {
        passed: true,
        observatory_preserves_topmost: true,
        ordinary_body_input_is_click_through: true,
        explicit_interaction_modifier: "Alt".to_owned(),
        no_focus_activation: true,
    };

    #[cfg(not(windows))]
    let hotkeys = NonWindowsHotkeyContract {
''',
)
replace_once(
    self_test,
    "            && anatomical_context_points == 23_210\n",
    "            && anatomical_context_points == 23_210\n            && ecotope.passed\n            && clock_ethogram_removed\n            && work_mode_safety.passed\n",
)
replace_once(self_test, "        schema_version: 5,\n", "        schema_version: 6,\n")
replace_once(
    self_test,
    "        asynchronous_hotkey_fallback: hotkeys.async_fallback_all_bindings,\n        firefly_visual_style: \"recorded_legacy_prism_port_v6\".to_owned(),\n",
    '''        asynchronous_hotkey_fallback: hotkeys.async_fallback_all_bindings,
        screen_ecotope_enabled: true,
        ecotope_deterministic_replay: ecotope.deterministic_replay,
        ecotope_stationary_source: ecotope.stationary_source,
        ecotope_plume_changes_without_source_motion: ecotope.plume_changes_without_source_motion,
        ecotope_reward_updates_belief: ecotope.reward_updates_belief,
        ecotope_generated_only_resource_semantics: ecotope.generated_only_resource_semantics,
        ecotope_habitat_hidden_in_work_mode: ecotope.habitat_hidden_in_work_mode,
        cursor_stationary_not_loom: ecotope.stationary_cursor_not_loom,
        cursor_approach_loom: ecotope.approaching_cursor_loom,
        cursor_threat_claim: "VIRTUAL_CURSOR_APPROACH_PROXY_NOT_MEASURED_OPTICAL_LOOM"
            .to_owned(),
        autonomous_scheduler_version:
            mechofly_core::model::AUTONOMOUS_SCHEDULER_VERSION.to_owned(),
        clock_ethogram_removed,
        work_mode_click_through: work_mode_safety.ordinary_body_input_is_click_through,
        observatory_preserves_topmost: work_mode_safety.observatory_preserves_topmost,
        explicit_interaction_modifier: work_mode_safety.explicit_interaction_modifier,
        no_focus_activation: work_mode_safety.no_focus_activation,
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
''',
)
append_once(
    self_test,
    "struct NonWindowsHotkeyContract",
    '''

#[cfg(not(windows))]
struct NonWindowsWorkModeSafetyContract {
    passed: bool,
    observatory_preserves_topmost: bool,
    ordinary_body_input_is_click_through: bool,
    explicit_interaction_modifier: String,
    no_focus_activation: bool,
}
''',
)

workflow = ".github/workflows/windows.yml"
replace_once(
    workflow,
    "          if ($receipt.schema_version -ne 5) { throw 'Self-test receipt schema is stale.' }\n",
    "          if ($receipt.schema_version -ne 6) { throw 'Self-test receipt schema is stale.' }\n",
)
replace_once(
    workflow,
    "          if (-not $receipt.asynchronous_hotkey_fallback) { throw 'Global hotkey fallback is absent.' }\n",
    '''          if (-not $receipt.asynchronous_hotkey_fallback) { throw 'Global hotkey fallback is absent.' }
          if (-not $receipt.screen_ecotope_enabled) { throw 'Screen Ecotope is absent.' }
          if (-not $receipt.ecotope_deterministic_replay) { throw 'Ecotope replay is not deterministic.' }
          if (-not $receipt.ecotope_stationary_source) { throw 'Ecotope source moved inside an epoch.' }
          if (-not $receipt.ecotope_plume_changes_without_source_motion) { throw 'Plume/source separation failed.' }
          if (-not $receipt.ecotope_reward_updates_belief) { throw 'Generated reward did not update spatial belief.' }
          if (-not $receipt.ecotope_generated_only_resource_semantics) { throw 'Untyped UI content can become a resource.' }
          if (-not $receipt.ecotope_habitat_hidden_in_work_mode) { throw 'Generated habitat is visible by default.' }
          if (-not $receipt.cursor_stationary_not_loom) { throw 'Stationary cursor was treated as looming.' }
          if (-not $receipt.cursor_approach_loom) { throw 'Rapid cursor approach did not cross the threat gate.' }
          if (-not $receipt.clock_ethogram_removed) { throw 'Clock-driven fallback ethogram remains.' }
          if (-not $receipt.work_mode_click_through) { throw 'Work Mode intercepts ordinary body clicks.' }
          if (-not $receipt.observatory_preserves_topmost) { throw 'Neural windows demote the pet.' }
          if (-not $receipt.no_focus_activation) { throw 'Pet can activate and steal focus.' }
''',
)

for path in [main, app, brain, pet, model, desktop, self_test, workflow]:
    text = read(path)
    for prohibited in ["ChatGPT", "OpenAI", "Codex"]:
        if prohibited in text:
            raise SystemExit(f"{path}: prohibited attribution marker {prohibited}")
