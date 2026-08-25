use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use eframe::egui::{self, Color32, Sense, Vec2, ViewportCommand};
use mechofly_core::{
    Action, Behavior, ConnectomeImport, Feedback, PetPolicy, PolicyContext,
};

use crate::{
    brain_lab::{BrainLabState, LabCommand},
    compute::ComputePreference,
    pet::{PetMotion, Skin, draw_pet, transparent_frame},
    runtime::SimulationSession,
    tray::{TrayAction, TrayController},
};

const MODEL_INTERVAL: Duration = Duration::from_millis(mechofly_core::MODEL_STEP_MS as u64);

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub skin: Skin,
    pub compute: ComputePreference,
    pub open_brain_lab: bool,
    pub reduced_motion: bool,
}

pub struct MechoFlyApp {
    render_state: Option<eframe::egui_wgpu::RenderState>,
    pub session: SimulationSession,
    pub lab: BrainLabState,
    pub policy: PetPolicy,
    pub skin: Skin,
    pet: PetMotion,
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
        let render_state = cc.wgpu_render_state.clone();
        let seed = 0x4D45_4348_4F46_4C59;
        let now = unix_millis();
        let session = SimulationSession::calibrated(
            render_state.as_ref(),
            config.compute,
            seed,
            now,
        );
        let policy = load_policy().unwrap_or_default();
        let tray_result = TrayController::new();
        let (tray, tray_warning) = match tray_result {
            Ok(tray) => (Some(tray), None),
            Err(error) => (None, Some(error)),
        };
        let pet = PetMotion {
            reduced_motion: config.reduced_motion,
            ..PetMotion::default()
        };
        Self {
            render_state,
            session,
            lab: BrainLabState::new(config.open_brain_lab, config.compute),
            policy,
            skin: config.skin,
            pet,
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
        }
    }

    fn process_tray(&mut self) {
        let actions = self
            .tray
            .as_ref()
            .map(TrayController::poll)
            .unwrap_or_default();
        for action in actions {
            match action {
                TrayAction::OpenBrainLab => self.lab.open = true,
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
    }

    fn display_behavior(&self) -> Behavior {
        match self.session.engine.state.behavior {
            Behavior::Alert | Behavior::PreEscape | Behavior::Flight | Behavior::Landing => {
                self.session.engine.state.behavior
            }
            _ => match self.current_action {
                Action::Pause => Behavior::Rest,
                Action::Explore => Behavior::Walk,
                Action::Inspect => Behavior::Alert,
                Action::Groom => Behavior::Groom,
            },
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
        let imported = match ConnectomeImport::fafb_v783(
            trimmed,
            format!("unix-millis:{}", unix_millis()),
        ) {
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
                entry.sequence,
                entry.action,
                entry.feedback,
                entry.value_before,
                entry.value_after
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
        let elapsed = now.duration_since(self.last_wall).min(Duration::from_millis(250));
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
            self.session.runtime_warning = Some(
                "Model fell behind the wall clock; catch-up was bounded and excess elapsed time was dropped."
                    .to_owned(),
            );
        }
        let monitor_size = ctx.input(|input| {
            input
                .viewport()
                .monitor_size
                .unwrap_or(Vec2::new(1_920.0, 1_080.0))
        });
        let hovered = ctx.input(|input| input.pointer.hover_pos().is_some());
        self.pet.advance(
            elapsed.as_secs_f32(),
            self.display_behavior(),
            monitor_size,
            hovered,
        );
        ctx.request_repaint_after(Duration::from_millis(if self.pet.reduced_motion { 50 } else { 16 }));
        if self.exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.send_viewport_cmd(ViewportCommand::OuterPosition(self.pet.screen_position));
        let display_behavior = self.display_behavior();
        let mut open_lab = false;
        let mut switch_skin = false;
        let mut toggle_pause = false;
        let mut request_exit = false;

        egui::CentralPanel::default()
            .frame(transparent_frame())
            .show(ui, |ui| {
                let (pet_rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 118.0),
                    Sense::click_and_drag(),
                );
                draw_pet(
                    ui.painter(),
                    pet_rect.shrink2(Vec2::new(18.0, 5.0)),
                    self.skin,
                    display_behavior,
                    self.pet.animation_seconds,
                    self.pet.facing,
                    self.pet.reduced_motion,
                );
                let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.dragged() {
                    self.pet.screen_position += response.drag_delta();
                    self.last_interaction = Some(Instant::now());
                }
                if response.double_clicked() || response.secondary_clicked() {
                    open_lab = true;
                    self.last_interaction = Some(Instant::now());
                } else if response.clicked() {
                    self.last_interaction = Some(Instant::now());
                    self.select_policy_action();
                }

                if response.hovered() || self.lab.open {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_premultiplied(255, 253, 248, 236))
                        .corner_radius(5)
                        .inner_margin(egui::Margin::symmetric(5, 3))
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                if ui.small_button("Brain Lab").clicked() {
                                    open_lab = true;
                                }
                                if ui.small_button("Switch skin").clicked() {
                                    switch_skin = true;
                                }
                                if ui
                                    .small_button(if self.pet.paused { "Resume pet" } else { "Pause pet" })
                                    .clicked()
                                {
                                    toggle_pause = true;
                                }
                                if ui.small_button("Exit").clicked() {
                                    request_exit = true;
                                }
                            });
                        });
                }
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{:?} · {:?} · {}",
                            display_behavior,
                            self.current_action,
                            self.session.assessment.selected.label()
                        ))
                        .size(10.0)
                        .color(Color32::from_rgba_premultiplied(23, 33, 43, 210)),
                    );
                });
            });

        if open_lab {
            self.lab.open = true;
        }
        if switch_skin {
            self.skin = match self.skin {
                Skin::Drosophila => Skin::Firefly,
                Skin::Firefly => Skin::Drosophila,
            };
        }
        if toggle_pause {
            self.pet.paused = !self.pet.paused;
        }
        if request_exit {
            self.exit_requested = true;
        }

        if self.lab.open {
            let commands = {
                let lab = &mut self.lab;
                let session = &self.session;
                let policy = &self.policy;
                let skin = self.skin;
                ui.ctx().show_viewport_immediate(
                    egui::ViewportId::from_hash_of("mechofly-brain-lab-v2"),
                    egui::ViewportBuilder::default()
                        .with_title("MechoFly Brain Lab — field notebook")
                        .with_inner_size([1_420.0, 900.0])
                        .with_min_inner_size([1_150.0, 720.0])
                        .with_resizable(true)
                        .with_transparent(false)
                        .with_taskbar(true),
                    |lab_ui, _class| {
                        if lab_ui.input(|input| input.viewport().close_requested()) {
                            lab.open = false;
                        }
                        lab.draw(lab_ui, session, policy, skin)
                    },
                )
            };
            self.handle_lab_commands(commands);
        }

        if let Some(warning) = self.tray_warning.take() {
            self.lab.message = format!("System tray unavailable; pet controls remain available: {warning}");
        }
    }
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
