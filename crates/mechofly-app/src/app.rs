use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use eframe::egui::{self, Pos2, Sense, Vec2, ViewportCommand};
use mechofly_core::{Action, Behavior, ConnectomeImport, Feedback, PetPolicy, PolicyContext};

#[cfg(windows)]
use crate::desktop_pet::HotkeyAction;
use crate::{
    brain_lab::{BrainLabState, LabCommand},
    compute::ComputePreference,
    diagnostics,
    live_brain::{LiveBrainCommand, LiveBrainState},
    pet::{PET_HEIGHT, PET_WIDTH, PetMotion, Skin, draw_pet_at_age, transparent_frame},
    runtime::SimulationSession,
    tray::{TrayAction, TrayController},
};

const MODEL_INTERVAL: Duration = Duration::from_millis(mechofly_core::MODEL_STEP_MS as u64);
const CATCHUP_WARNING: &str =
    "Model fell behind the wall clock; catch-up was bounded and excess elapsed time was dropped.";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub skin: Skin,
    pub compute: ComputePreference,
    pub open_brain_lab: bool,
    pub reduced_motion: bool,
    pub source_identity: RuntimeSourceIdentity,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeSourceIdentity {
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub tree: Option<String>,
    pub executable_sha256: Option<String>,
}

impl RuntimeSourceIdentity {
    pub fn short_commit(&self) -> Option<&str> {
        self.commit
            .as_deref()
            .map(|commit| commit.get(..12).unwrap_or(commit))
    }

    pub fn is_complete(&self) -> bool {
        self.branch.is_some()
            && self.commit.is_some()
            && self.tree.is_some()
            && self.executable_sha256.is_some()
    }
}

pub struct MechoFlyApp {
    render_state: Option<eframe::egui_wgpu::RenderState>,
    pub session: SimulationSession,
    pub live_brain: LiveBrainState,
    pub lab: BrainLabState,
    pub policy: PetPolicy,
    pub skin: Skin,
    source_identity: RuntimeSourceIdentity,
    pet: PetMotion,
    #[cfg(windows)]
    desktop_pet: Option<crate::desktop_pet::PetOverlay>,
    tray: Option<TrayController>,
    tray_warning: Option<String>,
    current_action: Action,
    last_context: PolicyContext,
    last_interaction: Option<Instant>,
    last_wall: Instant,
    accumulator: Duration,
    seed: u64,
    exit_requested: bool,
}

impl MechoFlyApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: AppConfig) -> Self {
        diagnostics::mark("eframe application construction started");
        let render_state = cc.wgpu_render_state.clone();
        let seed = 0x4D45_4348_4F46_4C59;
        let now = unix_millis();
        let session =
            SimulationSession::calibrated(render_state.as_ref(), config.compute, seed, now);
        diagnostics::mark("capacity assessment and simulation session initialized");
        let policy = load_policy().unwrap_or_default();
        let tray_result = TrayController::new();
        let (tray, tray_warning) = match tray_result {
            Ok(tray) => (Some(tray), None),
            Err(error) => (None, Some(error)),
        };
        diagnostics::mark("system tray initialization attempted");
        let mut pet = PetMotion::default();
        pet.reduced_motion = config.reduced_motion;
        #[cfg(windows)]
        let (desktop_pet, overlay_warning) = match crate::desktop_pet::PetOverlay::new(
            pet.screen_position,
        ) {
            Ok(overlay) => {
                diagnostics::mark("native per-pixel-alpha desktop pet initialized");
                diagnostics::mark(&format!(
                    "{} of 8 global hotkeys registered; asynchronous fallback covers every binding",
                    overlay.registered_hotkey_count()
                ));
                (Some(overlay), None)
            }
            Err(error) => {
                diagnostics::mark("native desktop pet failed; exposing transparent fallback host");
                cc.egui_ctx
                    .send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
                        PET_WIDTH as f32,
                        PET_HEIGHT as f32,
                    )));
                cc.egui_ctx
                    .send_viewport_cmd(ViewportCommand::OuterPosition(pet.screen_position));
                cc.egui_ctx
                    .send_viewport_cmd(ViewportCommand::Visible(true));
                (
                    None,
                    Some(format!("Native desktop overlay unavailable: {error}")),
                )
            }
        };
        #[cfg(windows)]
        let tray_warning = match (tray_warning, overlay_warning) {
            (Some(tray), Some(overlay)) => Some(format!("{tray}; {overlay}")),
            (Some(warning), None) | (None, Some(warning)) => Some(warning),
            (None, None) => None,
        };
        let app = Self {
            render_state,
            session,
            live_brain: LiveBrainState::new(config.open_brain_lab),
            lab: BrainLabState::new(config.open_brain_lab, config.compute),
            policy,
            skin: config.skin,
            source_identity: config.source_identity,
            pet,
            #[cfg(windows)]
            desktop_pet,
            tray,
            tray_warning,
            current_action: Action::Explore,
            last_context: PolicyContext {
                behavior: Behavior::Rest,
                recent_interaction: false,
            },
            last_interaction: None,
            last_wall: Instant::now(),
            accumulator: Duration::ZERO,
            seed,
            exit_requested: false,
        };
        diagnostics::mark("eframe application construction completed");
        app
    }

    fn process_tray(&mut self) {
        let actions = self
            .tray
            .as_ref()
            .map(TrayController::poll)
            .unwrap_or_default();
        for action in actions {
            match action {
                TrayAction::OpenBrainLab => self.live_brain.open = true,
                TrayAction::DrosophilaSkin => self.skin = Skin::Drosophila,
                TrayAction::FireflySkin => self.skin = Skin::Firefly,
                TrayAction::Reevaluate => {
                    self.reevaluate(self.lab.compute_preference);
                }
                TrayAction::Pause => self.pet.paused = !self.pet.paused,
                TrayAction::Exit => self.exit_requested = true,
            }
        }
    }

    fn select_policy_action(&mut self) {
        let recent_interaction = self
            .last_interaction
            .is_some_and(|time| time.elapsed() < Duration::from_secs(12));
        self.last_context = PolicyContext {
            behavior: self.session.engine.state.behavior,
            recent_interaction,
        };
        self.current_action = self.policy.choose(
            self.last_context,
            self.session.engine.state.frame,
            self.seed,
        );
        self.session.stimulate_action(self.current_action);
    }

    #[cfg(windows)]
    fn handle_hotkeys(&mut self, events: crate::desktop_pet::PetEvents) {
        if events.hotkey(HotkeyAction::Quit) || events.hotkey(HotkeyAction::EmergencyQuit) {
            self.exit_requested = true;
        }
        if events.hotkey(HotkeyAction::ToggleVisibility)
            && let Some(overlay) = &mut self.desktop_pet
        {
            let visible = !overlay.is_visible();
            overlay.set_visible(visible);
            self.lab.message = format!(
                "Global hotkey Ctrl+Alt+H: desktop companion {}.",
                if visible { "shown" } else { "hidden" }
            );
        }
        if events.hotkey(HotkeyAction::BrainLab) {
            self.live_brain.open = !self.live_brain.open;
            self.lab.message = "Global hotkey Ctrl+Alt+N: Live Brain toggled.".to_owned();
        }
        if events.hotkey(HotkeyAction::Loom) {
            let accepted = self.session.stimulate_behavior(Behavior::PreEscape, 594);
            self.lab.message = format!(
                "Global hotkey Ctrl+Alt+L: bounded loom sensory drive {}.",
                if accepted { "accepted" } else { "rejected" }
            );
        }
        for (action, behavior, duration_ms, label) in [
            (HotkeyAction::Groom, Behavior::Groom, 500, "Ctrl+Alt+G"),
            (HotkeyAction::Reverse, Behavior::Reverse, 400, "Ctrl+Alt+B"),
            (HotkeyAction::Walk, Behavior::Walk, 500, "Ctrl+Alt+W"),
        ] {
            if events.hotkey(action) {
                let accepted = self.session.stimulate_behavior(behavior, duration_ms);
                self.lab.message = format!(
                    "Global hotkey {label}: bounded {behavior:?} population drive {}; animation waits for controller selection.",
                    if accepted { "accepted" } else { "rejected" }
                );
            }
        }
    }

    fn handle_lab_commands(&mut self, commands: Vec<LabCommand>) {
        for command in commands {
            match command {
                LabCommand::Reevaluate(preference) => self.reevaluate(preference),
                LabCommand::GeneratePreview => self.lab.generate_preview(&self.session),
                LabCommand::ImportConnectome(path) => self.import_connectome(path),
                LabCommand::Feedback(feedback) => self.apply_feedback(feedback),
                LabCommand::SetLearningEnabled(enabled) => {
                    self.policy.enabled = enabled;
                    self.persist_policy();
                    self.lab.message = format!(
                        "Modeled software learning {}. Neural/connectome state unchanged.",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                LabCommand::ResetLearning => {
                    self.policy.reset();
                    self.persist_policy();
                    self.lab.message =
                        "Policy values and local ledger reset. Neural/connectome state unchanged."
                            .to_owned();
                }
                LabCommand::DeleteLearning => {
                    self.policy.reset();
                    let path = policy_path();
                    let _ = fs::remove_file(&path);
                    self.lab.message = format!(
                        "Deleted local learning state at {}. Neural/connectome state unchanged.",
                        path.display()
                    );
                }
                LabCommand::ExportLearning => self.export_learning(),
            }
        }
    }

    fn reevaluate(&mut self, preference: ComputePreference) {
        let previous = self.session.short_session_id().to_owned();
        self.session = SimulationSession::calibrated(
            self.render_state.as_ref(),
            preference,
            self.seed,
            unix_millis(),
        );
        self.lab.compute_preference = preference;
        self.lab.comparison = None;
        self.lab.replay_frames_back = 0;
        self.lab.message = format!(
            "Capacity re-evaluated. Closed session {previous}; started {} with {}.",
            self.session.short_session_id(),
            self.session.assessment.short_status()
        );
    }

    fn import_connectome(&mut self, path: String) {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            self.lab.message = "Select a local CSV or CSV.GZ file first.".to_owned();
            return;
        }
        let imported =
            match ConnectomeImport::fafb_v783(trimmed, format!("unix-millis:{}", unix_millis())) {
                Ok(imported) => imported,
                Err(error) => {
                    self.lab.message = format!("Connectome import rejected: {error}");
                    return;
                }
            };
        let manifest_digest = imported.manifest_digest();
        if let Err(error) = persist_import_manifest(&imported.manifest, &manifest_digest) {
            self.lab.message = format!("Graph parsed, but manifest persistence failed: {error}");
            return;
        }
        let warning_count = imported.manifest.validation_warnings.len();
        self.session = SimulationSession::with_imported_graph(
            self.render_state.as_ref(),
            self.lab.compute_preference,
            Arc::new(imported.graph),
            self.seed,
            unix_millis(),
        );
        self.lab.comparison = None;
        self.lab.message = format!(
            "Imported graph and started session {} · manifest {} · {} validation warning(s).",
            self.session.short_session_id(),
            &manifest_digest[..12],
            warning_count
        );
    }

    fn apply_feedback(&mut self, feedback: Feedback) {
        self.last_interaction = Some(Instant::now());
        if let Some(entry) = self.policy.apply_feedback(
            self.last_context,
            self.current_action,
            feedback,
            unix_millis(),
        ) {
            self.lab.message = format!(
                "Learning ledger #{}: {:?} {:?}, {} → {}. Connectome unchanged.",
                entry.sequence, entry.action, entry.feedback, entry.value_before, entry.value_after
            );
            self.persist_policy();
        } else {
            self.lab.message = "Learning is disabled; feedback caused no update.".to_owned();
        }
    }

    fn persist_policy(&mut self) {
        match save_policy(&self.policy) {
            Ok(path) => {
                if self.lab.message.is_empty() {
                    self.lab.message = format!("Saved policy to {}", path.display());
                }
            }
            Err(error) => self.lab.message = error,
        }
    }

    fn export_learning(&mut self) {
        let directory = app_data_dir().join("exports");
        let path = directory.join(format!("learning-ledger-{}.json", unix_millis()));
        let result = fs::create_dir_all(&directory)
            .and_then(|_| serde_json::to_vec_pretty(&self.policy).map_err(std::io::Error::other))
            .and_then(|bytes| fs::write(&path, bytes));
        self.lab.message = match result {
            Ok(()) => format!("Exported modeled-learning ledger to {}", path.display()),
            Err(error) => format!("Could not export learning ledger: {error}"),
        };
    }
}

impl eframe::App for MechoFlyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_tray();
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_wall)
            .min(Duration::from_millis(250));
        self.last_wall = now;
        self.accumulator += elapsed;
        let mut steps = 0;
        while self.accumulator >= MODEL_INTERVAL && steps < 5 {
            self.session.step();
            self.accumulator -= MODEL_INTERVAL;
            steps += 1;
            if self.session.engine.state.frame.is_multiple_of(90) {
                self.select_policy_action();
            }
        }
        if steps == 5 && self.accumulator >= MODEL_INTERVAL {
            self.accumulator = Duration::ZERO;
            self.session.runtime_warning = Some(CATCHUP_WARNING.to_owned());
        } else if self.session.runtime_warning.as_deref() == Some(CATCHUP_WARNING) {
            self.session.runtime_warning = None;
        }
        let mut screen_origin = Pos2::ZERO;
        let mut screen_size = ctx.input(|input| {
            input
                .viewport()
                .monitor_size
                .unwrap_or(Vec2::new(1_920.0, 1_080.0))
        });
        let mut held = ctx.input(|input| input.pointer.primary_down());
        let mut cursor_position = None;
        let mut cursor_over_pet = false;
        #[cfg(windows)]
        if let Some(events) = self
            .desktop_pet
            .as_ref()
            .map(crate::desktop_pet::PetOverlay::poll)
        {
            let overlay = self
                .desktop_pet
                .as_ref()
                .expect("overlay existed while its events were polled");
            screen_origin = overlay.screen_origin();
            screen_size = overlay.screen_size();
            held = events.dragging;
            cursor_position = events.cursor_position;
            cursor_over_pet = events.hovered;
            if let Some(position) = events.position {
                self.pet.screen_position = position;
            }
            if events.open_lab {
                self.live_brain.open = true;
            }
            if events.interacted {
                self.last_interaction = Some(Instant::now());
                self.select_policy_action();
            }
            self.handle_hotkeys(events);
        }
        let cursor_loom_strength = cursor_position
            .map(|cursor| {
                let pet_center =
                    self.pet.screen_position + Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * 0.5;
                let distance = pet_center.distance(cursor);
                ((360.0 - distance) / 240.0).clamp(0.0, 1.0)
            })
            .unwrap_or(0.0);
        self.session.set_cursor_loom_strength(if cursor_over_pet {
            1.0
        } else {
            cursor_loom_strength
        });
        self.pet.advance(
            elapsed.as_secs_f32(),
            authoritative_display_behavior(self.session.engine.state.behavior),
            screen_origin,
            screen_size,
            held,
            cursor_position,
        );
        #[cfg(windows)]
        {
            let behavior = authoritative_display_behavior(self.session.engine.state.behavior);
            let update_error = self.desktop_pet.as_mut().and_then(|overlay| {
                overlay.set_observatory_open(self.live_brain.open || self.lab.open);
                overlay
                    .update(
                        self.pet.screen_position,
                        self.skin,
                        behavior,
                        self.pet.animation_seconds,
                        self.pet.behavior_age_seconds,
                        self.pet.heading_radians,
                        self.pet.reduced_motion,
                    )
                    .err()
            });
            if let Some(error) = update_error {
                diagnostics::mark("native desktop pet update failed; enabling fallback host");
                self.desktop_pet = None;
                self.lab.message =
                    format!("Desktop overlay failed and switched to compatibility mode: {error}");
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
                    PET_WIDTH as f32,
                    PET_HEIGHT as f32,
                )));
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(self.pet.screen_position));
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            }
        }
        ctx.request_repaint_after(Duration::from_millis(if self.pet.reduced_motion {
            50
        } else {
            16
        }));
        if self.exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let show_fallback_pet = {
            #[cfg(windows)]
            {
                self.desktop_pet.is_none()
            }
            #[cfg(not(windows))]
            {
                true
            }
        };
        if show_fallback_pet {
            self.draw_fallback_pet(ui);
        } else {
            egui::CentralPanel::default()
                .frame(transparent_frame())
                .show(ui, |_ui| {});
        }

        if self.live_brain.open {
            let behavior = authoritative_display_behavior(self.session.engine.state.behavior);
            let commands = {
                let live_brain = &mut self.live_brain;
                let session = &self.session;
                let source_identity = &self.source_identity;
                ui.ctx().show_viewport_immediate(
                    egui::ViewportId::from_hash_of("mechofly-live-brain-v5"),
                    egui::ViewportBuilder::default()
                        .with_title("MechoFly Prism — Live Brain")
                        .with_inner_size([1_120.0, 760.0])
                        .with_min_inner_size([840.0, 620.0])
                        .with_resizable(true)
                        .with_transparent(false)
                        .with_taskbar(true),
                    |brain_ui, _class| {
                        if brain_ui.input(|input| input.viewport().close_requested()) {
                            live_brain.open = false;
                        }
                        live_brain.draw(brain_ui, session, behavior, source_identity)
                    },
                )
            };
            for command in commands {
                match command {
                    LiveBrainCommand::OpenLab => self.lab.open = true,
                }
            }
            self.lab
                .set_selected_neuron_index(self.live_brain.selected_neuron());
        }

        if self.lab.open {
            let commands = {
                let lab = &mut self.lab;
                let session = &self.session;
                let policy = &self.policy;
                let skin = self.skin;
                let source_identity = &self.source_identity;
                ui.ctx().show_viewport_immediate(
                    egui::ViewportId::from_hash_of("mechofly-brain-lab-v5"),
                    egui::ViewportBuilder::default()
                        .with_title("MechoFly Prism — Brain Lab")
                        .with_inner_size([1_580.0, 820.0])
                        .with_min_inner_size([1_260.0, 720.0])
                        .with_resizable(true)
                        .with_transparent(false)
                        .with_taskbar(true),
                    |lab_ui, _class| {
                        if lab_ui.input(|input| input.viewport().close_requested()) {
                            lab.open = false;
                        }
                        lab.draw(lab_ui, session, policy, skin, source_identity)
                    },
                )
            };
            self.handle_lab_commands(commands);
            self.live_brain
                .set_selected_neuron(self.lab.selected_neuron_index());
        }

        if let Some(warning) = self.tray_warning.take() {
            self.lab.message =
                format!("Desktop host warning: {warning}. Right-click the pet to open Brain Lab.");
        }
    }
}

impl MechoFlyApp {
    fn draw_fallback_pet(&mut self, ui: &mut egui::Ui) {
        ui.send_viewport_cmd(ViewportCommand::OuterPosition(self.pet.screen_position));
        let behavior = authoritative_display_behavior(self.session.engine.state.behavior);
        egui::CentralPanel::default()
            .frame(transparent_frame())
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32),
                    Sense::click_and_drag(),
                );
                draw_pet_at_age(
                    ui.painter(),
                    rect,
                    self.skin,
                    behavior,
                    self.pet.animation_seconds,
                    self.pet.behavior_age_seconds,
                    self.pet.heading_radians,
                    self.pet.reduced_motion,
                );
                let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.dragged() {
                    self.pet.screen_position += response.drag_delta();
                    self.last_interaction = Some(Instant::now());
                }
                if response.double_clicked() || response.secondary_clicked() {
                    self.live_brain.open = true;
                    self.last_interaction = Some(Instant::now());
                } else if response.clicked() {
                    self.last_interaction = Some(Instant::now());
                    self.select_policy_action();
                }
            });
    }
}

pub(crate) const fn authoritative_display_behavior(neural_behavior: Behavior) -> Behavior {
    neural_behavior
}

fn app_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".mechofly"))
        })
        .join("MechoFly")
}

fn policy_path() -> PathBuf {
    app_data_dir().join("learning-policy.json")
}

fn load_policy() -> Option<PetPolicy> {
    let bytes = fs::read(policy_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_policy(policy: &PetPolicy) -> Result<PathBuf, String> {
    let path = policy_path();
    let parent = path.parent().expect("policy path has a parent");
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(policy)
        .map_err(|error| format!("cannot serialize policy: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(path)
}

fn persist_import_manifest(
    manifest: &mechofly_core::ImportManifest,
    digest: &str,
) -> Result<PathBuf, String> {
    let directory = app_data_dir().join("imports");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let path = directory.join(format!("{}.manifest.json", &digest[..16]));
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("cannot serialize import manifest: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(path)
}

pub fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
