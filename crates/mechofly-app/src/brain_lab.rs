use std::{sync::Arc, time::Duration};

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use mechofly_core::{
    ComparisonResult, Feedback, PetPolicy, StimulationPolicy, StimulationRequest,
};

use crate::{
    compute::ComputePreference,
    pet::Skin,
    runtime::SimulationSession,
};

const CANVAS: Color32 = Color32::from_rgb(244, 239, 227);
const SURFACE: Color32 = Color32::from_rgb(255, 253, 248);
const INK: Color32 = Color32::from_rgb(23, 33, 43);
const MUTED: Color32 = Color32::from_rgb(78, 89, 100);
const ACTUAL: Color32 = Color32::from_rgb(0, 90, 169);
const ALTERNATIVE: Color32 = Color32::from_rgb(178, 58, 43);
const POSITIVE: Color32 = Color32::from_rgb(23, 107, 82);
const WARNING: Color32 = Color32::from_rgb(138, 75, 8);
const GRID: Color32 = Color32::from_rgb(199, 189, 174);
const ACTUAL_SOFT: Color32 = Color32::from_rgb(220, 234, 244);
const ALTERNATIVE_SOFT: Color32 = Color32::from_rgb(243, 222, 215);
const POSITIVE_SOFT: Color32 = Color32::from_rgb(220, 235, 227);

#[derive(Clone, Debug)]
pub enum LabCommand {
    Reevaluate(ComputePreference),
    GeneratePreview,
    ImportConnectome(String),
    Feedback(Feedback),
    SetLearningEnabled(bool),
    ResetLearning,
    ExportLearning,
    DeleteLearning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LabView {
    Overview,
    Compare,
    Provenance,
    Learning,
}

pub struct BrainLabState {
    pub open: bool,
    view: LabView,
    pub compute_preference: ComputePreference,
    pub replay_frames_back: usize,
    pub targets: String,
    pub amplitude: f32,
    pub duration_ms: u32,
    pub comparison_frames: u32,
    pub authored_label: String,
    pub comparison: Option<ComparisonResult>,
    pub comparison_cursor: usize,
    pub playing: bool,
    pub import_path: String,
    pub selected_neuron: String,
    pub message: String,
}

impl BrainLabState {
    pub fn new(open: bool, preference: ComputePreference) -> Self {
        Self {
            open,
            view: LabView::Overview,
            compute_preference: preference,
            replay_frames_back: 0,
            targets: "3, 7, 11, 19, 31".to_owned(),
            amplitude: 0.20,
            duration_ms: 330,
            comparison_frames: 90,
            authored_label: "authored exploratory preview".to_owned(),
            comparison: None,
            comparison_cursor: 0,
            playing: false,
            import_path: String::new(),
            selected_neuron: String::new(),
            message: "No preview receipt. Live state cannot be targeted from this UI.".to_owned(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        session: &SimulationSession,
        policy: &PetPolicy,
        skin: Skin,
    ) -> Vec<LabCommand> {
        let mut commands = Vec::new();
        style_context(ui.ctx());

        egui::Panel::top("lab_header")
            .frame(egui::Frame::new().fill(INK).inner_margin(egui::Margin::symmetric(18, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::WHITE, egui::RichText::new("MECHOFLY / FIELD NOTEBOOK").strong().size(19.0));
                    ui.add_space(16.0);
                    ui.colored_label(Color32::from_rgb(204, 218, 226), "modeled dynamics · bounded replay · live state isolated");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(Color32::from_rgb(224, 229, 206), format!("{} neurons · {} rows", session.graph.identity.neuron_count, session.graph.identity.edge_count));
                    });
                });
            });

        egui::Panel::bottom("lab_timeline")
            .resizable(true)
            .default_size(168.0)
            .min_size(130.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                timeline(ui, session);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.message).color(MUTED));
                    if let Some(warning) = &session.runtime_warning {
                        ui.colored_label(WARNING, warning);
                    }
                });
            });

        egui::Panel::left("experiment_rail")
            .resizable(false)
            .exact_size(282.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                ui.heading("Experiment rail");
                ui.label(egui::RichText::new("Session").strong().color(MUTED));
                key_value(ui, "ID", session.short_session_id());
                key_value(ui, "Started (Unix ms)", &session.started_unix_millis.to_string());
                key_value(ui, "Skin", skin.label());
                key_value(ui, "Backend", session.assessment.selected.label());
                key_value(ui, "Tier", session.assessment.tier.label());
                key_value(ui, "Last step", &format!("{:.2} ms", session.last_step_ms));
                ui.add_space(8.0);
                egui::ComboBox::from_id_salt("compute_preference")
                    .selected_text(self.compute_preference.label())
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for mode in ComputePreference::ALL {
                            ui.selectable_value(&mut self.compute_preference, mode, mode.label());
                        }
                    });
                if ui
                    .add(egui::Button::new("Re-evaluate capacity").fill(ACTUAL_SOFT))
                    .on_hover_text("Benchmarks CPU and any compute-capable GPU, then starts a new identified model session.")
                    .clicked()
                {
                    commands.push(LabCommand::Reevaluate(self.compute_preference));
                }
                ui.small("A tier change starts a new session; it never mutates the current replay epoch.");

                ui.separator();
                ui.label(egui::RichText::new("Bounded source frame").strong().color(MUTED));
                let maximum = session.replay.len().saturating_sub(1);
                self.replay_frames_back = self.replay_frames_back.min(maximum);
                ui.add(
                    egui::Slider::new(&mut self.replay_frames_back, 0..=maximum)
                        .text("frames back"),
                );
                ui.small(format!("retained {}/240 frames", session.replay.len()));

                ui.separator();
                ui.label(egui::RichText::new("Authored preview").strong().color(ALTERNATIVE));
                ui.label("Target indices");
                ui.text_edit_singleline(&mut self.targets);
                ui.horizontal(|ui| {
                    ui.label("Amplitude");
                    ui.add(egui::DragValue::new(&mut self.amplitude).range(0.01..=0.25).speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Duration ms");
                    ui.add(egui::DragValue::new(&mut self.duration_ms).range(33..=990).speed(33));
                });
                ui.horizontal(|ui| {
                    ui.label("Frames");
                    ui.add(egui::DragValue::new(&mut self.comparison_frames).range(1..=120));
                });
                ui.label("Authored purpose");
                ui.text_edit_singleline(&mut self.authored_label);
                if ui
                    .add(egui::Button::new("Generate discarded comparison").fill(ALTERNATIVE_SOFT))
                    .clicked()
                {
                    commands.push(LabCommand::GeneratePreview);
                }
                ui.small("≤64 targets · amplitude ≤0.25 · 33–990 ms · dosage ceiling · no apply/commit path");

                ui.separator();
                ui.collapsing("Import a user-downloaded connectome", |ui| {
                    ui.label("CSV or CSV.GZ path");
                    ui.text_edit_singleline(&mut self.import_path);
                    if ui.button("Validate and start imported session").clicked() {
                        commands.push(LabCommand::ImportConnectome(self.import_path.clone()));
                    }
                    ui.small("The file stays local. Import records source, version, columns, SHA-256, transform, and warnings.");
                });
            });

        egui::Panel::right("evidence_inspector")
            .resizable(false)
            .exact_size(300.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                ui.heading("Evidence inspector");
                claim_badge(ui, "DERIVED STRUCTURE", POSITIVE_SOFT, POSITIVE);
                claim_badge(ui, "MODELED DYNAMICS", ACTUAL_SOFT, ACTUAL);
                claim_badge(ui, "MEASURED ACTIVITY: NONE", ALTERNATIVE_SOFT, ALTERNATIVE);
                claim_badge(ui, "LIVE HARDWARE: NONE", Color32::from_rgb(238, 232, 219), INK);
                ui.separator();
                ui.label(egui::RichText::new("Graph identity").strong());
                ui.monospace(&session.graph.identity.graph_id);
                key_value(ui, "Dataset", &session.graph.identity.dataset);
                key_value(ui, "Snapshot", &session.graph.identity.snapshot);
                key_value(ui, "Product", &session.graph.identity.product);
                ui.label("Graph SHA-256");
                ui.monospace(&session.graph.identity.sha256);
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Capacity decision").strong());
                ui.label(&session.assessment.reason);
                if let Some(adapter) = &session.assessment.gpu_adapter {
                    key_value(ui, "Adapter", adapter);
                }
                if let Some(backend) = &session.assessment.gpu_backend {
                    key_value(ui, "API", backend);
                }
                ui.separator();
                ui.label(egui::RichText::new("Neuron lookup").strong());
                ui.text_edit_singleline(&mut self.selected_neuron);
                if let Ok(index) = self.selected_neuron.trim().parse::<usize>() {
                    if index < session.engine.state.activation.len() {
                        key_value(ui, "Index", &index.to_string());
                        key_value(ui, "Root ID", &session.graph.neuron_ids[index].to_string());
                        key_value(ui, "Activation", &session.engine.state.activation[index].to_string());
                        key_value(ui, "Spiked", if session.engine.state.spikes[index] == 0 { "no" } else { "yes" });
                    } else {
                        ui.colored_label(WARNING, "Index is outside this graph.");
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("Learning boundary").strong());
                key_value(ui, "Policy", if policy.enabled { "enabled" } else { "disabled" });
                key_value(ui, "Explicit updates", &policy.ledger.len().to_string());
                ui.monospace(policy.digest());
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    view_tab(ui, &mut self.view, LabView::Overview, "Overview");
                    view_tab(ui, &mut self.view, LabView::Compare, "Actual / alternative");
                    view_tab(ui, &mut self.view, LabView::Provenance, "Provenance");
                    view_tab(ui, &mut self.view, LabView::Learning, "Learning ledger");
                });
                ui.add_space(8.0);
                match self.view {
                    LabView::Overview => overview(ui, session),
                    LabView::Compare => comparison_view(ui, self),
                    LabView::Provenance => provenance_view(ui, session),
                    LabView::Learning => learning_view(ui, policy, &mut commands),
                }
            });

        commands
    }

    pub fn generate_preview(&mut self, session: &SimulationSession) {
        let targets = match parse_targets(&self.targets) {
            Ok(targets) => targets,
            Err(error) => {
                self.message = error;
                return;
            }
        };
        let Some(checkpoint) = session.replay.get_from_newest(self.replay_frames_back) else {
            self.message = "Replay is still empty; wait for at least one modeled frame.".to_owned();
            return;
        };
        let request = StimulationRequest {
            targets,
            amplitude: self.amplitude,
            duration_ms: self.duration_ms,
            comparison_frames: self.comparison_frames,
            authored_label: self.authored_label.clone(),
        };
        match StimulationPolicy::default().compare_from_checkpoint(
            checkpoint,
            || session.live_digest(),
            request,
            Arc::clone(&session.graph),
        ) {
            Ok(result) => {
                let unchanged = result.receipt.live_state_unchanged;
                let differs = result.receipt.alternative_differs;
                self.comparison_cursor = 0;
                self.comparison = Some(result);
                self.view = LabView::Compare;
                self.message = format!(
                    "Preview receipt PASS · live unchanged={unchanged} · alternative differs={differs}"
                );
            }
            Err(error) => self.message = format!("Preview rejected: {error}"),
        }
    }
}

fn style_context(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = ACTUAL_SOFT;
    style.visuals.selection.stroke = Stroke::new(1.5, ACTUAL);
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    ctx.set_style_of(egui::Theme::Light, style);
}

fn surface_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, GRID))
        .inner_margin(egui::Margin::same(12))
}

fn key_value(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(format!("{key}:")).color(MUTED));
        ui.label(value);
    });
}

fn claim_badge(ui: &mut egui::Ui, label: &str, fill: Color32, text: Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.colored_label(text, egui::RichText::new(label).strong().size(11.0));
        });
}

fn view_tab(ui: &mut egui::Ui, selected: &mut LabView, view: LabView, label: &str) {
    if ui.selectable_label(*selected == view, label).clicked() {
        *selected = view;
    }
}

fn overview(ui: &mut egui::Ui, session: &SimulationSession) {
    ui.horizontal(|ui| {
        ui.heading("Population field");
        ui.colored_label(ACTUAL, "● modeled activation");
        ui.colored_label(POSITIVE, "◆ current spikes");
    });
    ui.label("Fixed normalized projection · structural coordinates are presentation context, not measured activity.");
    let available = ui.available_size();
    let height = (available.y - 8.0).max(220.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available.x, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5, SURFACE);
    painter.rect_stroke(rect, 5, Stroke::new(1.0, GRID), StrokeKind::Inside);
    draw_live_graph(&painter, rect.shrink(16.0), session);
    painter.text(
        rect.left_top() + Vec2::new(12.0, 10.0),
        Align2::LEFT_TOP,
        format!(
            "frame {} · {} spikes · {:?}",
            session.last_summary.frame,
            session.last_summary.spike_count,
            session.last_summary.behavior
        ),
        FontId::monospace(12.0),
        MUTED,
    );
}

fn draw_live_graph(painter: &egui::Painter, rect: Rect, session: &SimulationSession) {
    let count = session.graph.positions.len();
    let stride = count.div_ceil(2_400).max(1);
    for index in (0..count).step_by(stride) {
        let position = project(rect, session.graph.positions[index]);
        let activation = session.engine.state.activation[index];
        let normalized = ((activation + 8_192) as f32 / 16_384.0).clamp(0.0, 1.0);
        let color = blend(Color32::from_rgb(204, 211, 205), ACTUAL, normalized);
        let radius = if session.engine.state.spikes[index] != 0 { 3.7 } else { 1.3 + normalized * 1.2 };
        painter.circle_filled(position, radius, if session.engine.state.spikes[index] != 0 { POSITIVE } else { color });
    }
}

fn comparison_view(ui: &mut egui::Ui, state: &mut BrainLabState) {
    if state.comparison.is_none() {
        ui.heading("Actual / alternative filmstrip");
        ui.add_space(16.0);
        egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0, GRID)).inner_margin(20).show(ui, |ui| {
            ui.label(egui::RichText::new("No alternative has been authored.").strong().size(18.0));
            ui.label("Use the experiment rail to select a retained source frame and generate a bounded, discarded comparison.");
        });
        return;
    }
    let frame_count = state
        .comparison
        .as_ref()
        .map(|comparison| comparison.frames.len())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.heading("Aligned multi-frame comparison");
        ui.colored_label(ACTUAL, "● ACTUAL");
        ui.colored_label(ALTERNATIVE, "■ ALTERNATIVE");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if state.playing { "Pause" } else { "Play" };
            if ui.button(label).clicked() {
                state.playing = !state.playing;
            }
        });
    });
    if state.playing && frame_count > 0 {
        state.comparison_cursor = ui.input(|input| (input.time * 12.0) as usize) % frame_count;
        ui.ctx().request_repaint_after(Duration::from_millis(80));
    }
    let comparison = state
        .comparison
        .as_ref()
        .expect("comparison presence was checked above");
    ui.label("Both rows start from the same full checkpoint. The lower strip reports neuron-state divergence.");
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 390.0), Sense::hover());
    draw_filmstrip(ui.painter_at(rect), rect, comparison);
    let max = comparison.frames.len().saturating_sub(1);
    state.comparison_cursor = state.comparison_cursor.min(max);
    ui.add(egui::Slider::new(&mut state.comparison_cursor, 0..=max).text("inspection frame"));
    if let Some(frame) = comparison.frames.get(state.comparison_cursor) {
        ui.horizontal_wrapped(|ui| {
            key_value(ui, "Offset", &frame.offset.to_string());
            key_value(ui, "Actual spikes", &frame.actual.spike_count.to_string());
            key_value(ui, "Alternative spikes", &frame.alternative.spike_count.to_string());
            key_value(ui, "Differing neurons", &frame.differing_neurons.to_string());
        });
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        claim_badge(ui, &comparison.receipt.status, POSITIVE_SOFT, POSITIVE);
        ui.label(format!("source frame {}", comparison.receipt.source_frame));
        ui.label(format!("live unchanged: {}", comparison.receipt.live_state_unchanged));
        ui.label(format!("alternative differs: {}", comparison.receipt.alternative_differs));
    });
}

fn draw_filmstrip(painter: egui::Painter, rect: Rect, comparison: &ComparisonResult) {
    painter.rect_filled(rect, 5, SURFACE);
    painter.rect_stroke(rect, 5, Stroke::new(1.0, GRID), StrokeKind::Inside);
    let count = comparison.frames.len();
    let indices = five_indices(count);
    let gap = 8.0;
    let left = rect.left() + 72.0;
    let width = (rect.width() - 88.0 - gap * 4.0) / 5.0;
    let row_height = 142.0;
    let actual_y = rect.top() + 34.0;
    let alternative_y = actual_y + row_height + 24.0;
    painter.text(Pos2::new(rect.left() + 10.0, actual_y + 55.0), Align2::LEFT_CENTER, "ACTUAL", FontId::proportional(12.0), ACTUAL);
    painter.text(Pos2::new(rect.left() + 10.0, alternative_y + 55.0), Align2::LEFT_CENTER, "ALT", FontId::proportional(12.0), ALTERNATIVE);
    for (column, index) in indices.into_iter().enumerate() {
        let frame = &comparison.frames[index];
        let x = left + column as f32 * (width + gap);
        let actual_rect = Rect::from_min_size(Pos2::new(x, actual_y), Vec2::new(width, 112.0));
        let alt_rect = Rect::from_min_size(Pos2::new(x, alternative_y), Vec2::new(width, 112.0));
        mini_frame(&painter, actual_rect, &frame.actual_sample, ACTUAL, false);
        mini_frame(&painter, alt_rect, &frame.alternative_sample, ALTERNATIVE, true);
        painter.text(Pos2::new(x + width * 0.5, rect.top() + 12.0), Align2::CENTER_TOP, format!("+{}", frame.offset), FontId::monospace(11.0), MUTED);
        let divergence = frame.differing_neurons as f32
            / comparison.frames[0].actual_sample.len().max(1) as f32;
        let bar_rect = Rect::from_min_size(Pos2::new(x, rect.bottom() - 24.0), Vec2::new(width, 8.0));
        painter.rect_filled(bar_rect, 2, Color32::from_rgb(229, 224, 214));
        painter.rect_filled(Rect::from_min_size(bar_rect.min, Vec2::new(width * divergence.clamp(0.0, 1.0), 8.0)), 2, ALTERNATIVE);
    }
    painter.text(Pos2::new(rect.left() + 10.0, rect.bottom() - 20.0), Align2::LEFT_CENTER, "Δ", FontId::proportional(13.0), ALTERNATIVE);
}

fn mini_frame(
    painter: &egui::Painter,
    rect: Rect,
    samples: &[mechofly_core::NeuronSample],
    color: Color32,
    squares: bool,
) {
    painter.rect_filled(rect, 3, Color32::from_rgb(249, 247, 240));
    painter.rect_stroke(rect, 3, Stroke::new(1.2, color), StrokeKind::Inside);
    for sample in samples {
        let normalized_index = sample.index as f32 / samples.last().map(|s| s.index.max(1)).unwrap_or(1) as f32;
        let angle = normalized_index * std::f32::consts::TAU * 7.0;
        let radius = normalized_index.sqrt() * 0.43;
        let center = rect.center();
        let position = center + Vec2::new(angle.cos() * rect.width() * radius, angle.sin() * rect.height() * radius * 0.85);
        let intensity = ((sample.activation_q15 as f32 + 8_192.0) / 16_384.0).clamp(0.15, 1.0);
        let mark = blend(Color32::from_rgb(208, 207, 199), color, intensity);
        let size = if sample.spiked { 3.0 } else { 1.5 };
        if squares {
            painter.rect_filled(Rect::from_center_size(position, Vec2::splat(size * 2.0)), 0, mark);
        } else {
            painter.circle_filled(position, size, mark);
        }
    }
}

fn five_indices(count: usize) -> [usize; 5] {
    if count <= 1 {
        return [0; 5];
    }
    [0, count / 4, count / 2, count * 3 / 4, count - 1]
}

fn provenance_view(ui: &mut egui::Ui, session: &SimulationSession) {
    ui.heading("Provenance ledger");
    ui.label("Every layer below has a narrower claim than the one above it. None is a biological recording.");
    ui.add_space(10.0);
    provenance_card(ui, "1 · DERIVED CONNECTOME STRUCTURE", POSITIVE_SOFT, &session.graph.identity.structure_claim, &session.graph.identity.source_url, &session.graph.identity.sha256);
    provenance_card(ui, "2 · MODELED NEURAL DYNAMICS", ACTUAL_SOFT, "deterministic signed fixed-point software dynamics", mechofly_core::MODEL_VERSION, &session.engine.model_identity());
    provenance_card(ui, "3 · MODELED SOFTWARE LEARNING", Color32::from_rgb(237, 232, 247), "bounded contextual policy changed only by explicit feedback", mechofly_core::learning::LEARNING_RULE_VERSION, "stored separately from neural state");
    provenance_card(ui, "4 · AUTHORED PRESENTATION", ALTERNATIVE_SOFT, "procedural pet skin and field-notebook interface", "presentation-v2", "does not alter graph or dynamics");
}

fn provenance_card(ui: &mut egui::Ui, title: &str, fill: Color32, claim: &str, source: &str, digest: &str) {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, GRID)).corner_radius(5).inner_margin(12).show(ui, |ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.label(claim);
        ui.monospace(source);
        ui.monospace(digest);
    });
    ui.add_space(8.0);
}

fn learning_view(ui: &mut egui::Ui, policy: &PetPolicy, commands: &mut Vec<LabCommand>) {
    ui.heading("Bounded pet-policy ledger");
    ui.label("The connectome is immutable. Only this separate software policy learns, and only from explicit feedback.");
    let mut enabled = policy.enabled;
    if ui.checkbox(&mut enabled, "Allow learning from explicit feedback").changed() {
        commands.push(LabCommand::SetLearningEnabled(enabled));
    }
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new("Encourage current action").fill(POSITIVE_SOFT)).clicked() {
            commands.push(LabCommand::Feedback(Feedback::Encourage));
        }
        if ui.add(egui::Button::new("Discourage current action").fill(ALTERNATIVE_SOFT)).clicked() {
            commands.push(LabCommand::Feedback(Feedback::Discourage));
        }
        if ui.button("Export ledger").clicked() {
            commands.push(LabCommand::ExportLearning);
        }
        if ui.button("Reset values").clicked() {
            commands.push(LabCommand::ResetLearning);
        }
        if ui.button("Delete ledger").clicked() {
            commands.push(LabCommand::DeleteLearning);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for entry in policy.ledger.iter().rev() {
            egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0, GRID)).inner_margin(8).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(format!("#{:04}", entry.sequence));
                    ui.label(format!("{:?} / {:?}", entry.action, entry.feedback));
                    ui.label(format!("{} → {}", entry.value_before, entry.value_after));
                });
                ui.small(&entry.claim);
                ui.monospace(&entry.policy_after_sha256);
            });
            ui.add_space(5.0);
        }
        if policy.ledger.is_empty() {
            ui.label("No learning updates. Watching the pet does not train it.");
        }
    });
}

fn timeline(ui: &mut egui::Ui, session: &SimulationSession) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("ACTIVITY TIMELINE").strong());
        ui.label(format!("last {} retained frames", session.replay.len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!(
                "{:?} persisted {} frames",
                session.engine.state.behavior, session.engine.state.behavior_age_frames
            ));
        });
    });
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 92.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3, Color32::from_rgb(249, 247, 240));
    painter.rect_stroke(rect, 3, Stroke::new(1.0, GRID), StrokeKind::Inside);
    let summaries: Vec<_> = session.replay.summaries().collect();
    if summaries.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, "waiting for modeled frames", FontId::proportional(12.0), MUTED);
        return;
    }
    let max_spikes = summaries.iter().map(|summary| summary.spike_count).max().unwrap_or(1).max(1);
    let bar_width = rect.width() / summaries.len() as f32;
    for (index, summary) in summaries.into_iter().enumerate() {
        let height = summary.spike_count as f32 / max_spikes as f32 * (rect.height() - 22.0);
        let x = rect.left() + index as f32 * bar_width;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x, rect.bottom() - height - 12.0), Pos2::new(x + bar_width.max(1.0), rect.bottom() - 12.0)),
            0,
            ACTUAL,
        );
        let behavior_color = match summary.behavior {
            mechofly_core::Behavior::Rest | mechofly_core::Behavior::Quiet => GRID,
            mechofly_core::Behavior::Walk | mechofly_core::Behavior::Reverse => POSITIVE,
            mechofly_core::Behavior::Groom => WARNING,
            _ => ALTERNATIVE,
        };
        painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.bottom() - 8.0), Vec2::new(bar_width.max(1.0), 5.0)), 0, behavior_color);
    }
}

fn parse_targets(text: &str) -> Result<Vec<usize>, String> {
    text.split(|character: char| character == ',' || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<usize>()
                .map_err(|_| format!("Target {part:?} is not a non-negative neuron index."))
        })
        .collect()
}

fn project(rect: Rect, point: [f32; 2]) -> Pos2 {
    Pos2::new(
        egui::lerp(rect.left()..=rect.right(), (point[0] + 1.0) * 0.5),
        egui::lerp(rect.top()..=rect.bottom(), (point[1] + 1.0) * 0.5),
    )
}

fn blend(from: Color32, to: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    Color32::from_rgb(
        (from.r() as f32 + (to.r() as f32 - from.r() as f32) * amount) as u8,
        (from.g() as f32 + (to.g() as f32 - from.g() as f32) * amount) as u8,
        (from.b() as f32 + (to.b() as f32 - from.b() as f32) * amount) as u8,
    )
}
