use std::{sync::Arc, time::Duration};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use mechofly_core::{ComparisonResult, Feedback, PetPolicy, StimulationPolicy, StimulationRequest};

use crate::{
    app::RuntimeSourceIdentity, compute::ComputePreference, pet::Skin,
    runtime::SimulationSession,
};

const CANVAS: Color32 = Color32::from_rgb(5, 9, 15);
const SURFACE: Color32 = Color32::from_rgb(10, 17, 27);
const SURFACE_RAISED: Color32 = Color32::from_rgb(15, 25, 39);
const INK: Color32 = Color32::from_rgb(228, 238, 247);
const MUTED: Color32 = Color32::from_rgb(133, 151, 169);
const ACTUAL: Color32 = Color32::from_rgb(56, 220, 202);
const ALTERNATIVE: Color32 = Color32::from_rgb(255, 181, 82);
const POSITIVE: Color32 = Color32::from_rgb(185, 241, 98);
const WARNING: Color32 = Color32::from_rgb(255, 116, 112);
const GRID: Color32 = Color32::from_rgb(34, 51, 70);
const ACTUAL_SOFT: Color32 = Color32::from_rgb(16, 58, 61);
const ALTERNATIVE_SOFT: Color32 = Color32::from_rgb(69, 49, 26);
const POSITIVE_SOFT: Color32 = Color32::from_rgb(38, 64, 37);
const VIOLET: Color32 = Color32::from_rgb(164, 131, 255);

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
        source_identity: &RuntimeSourceIdentity,
    ) -> Vec<LabCommand> {
        let mut commands = Vec::new();
        style_context(ui.ctx());

        egui::Panel::top("lab_header")
            .exact_size(82.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(7, 13, 22))
                    .stroke(Stroke::new(1.0, GRID))
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("MECHOFLY  //  NEURAL OBSERVATORY")
                                .strong()
                                .size(20.0)
                                .color(INK),
                        );
                        ui.label(
                            egui::RichText::new(
                                "connectome-grounded model · retained replay · authored alternatives",
                            )
                            .size(11.5)
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        status_chip(
                            ui,
                            "LIVE AUTHORITY  NONE",
                            Color32::from_rgb(45, 33, 38),
                            WARNING,
                        );
                        status_chip(ui, "MODEL RUNNING", ACTUAL_SOFT, ACTUAL);
                        if let Some(commit) = source_identity.short_commit() {
                            status_chip(
                                ui,
                                &format!("BUILD {commit}"),
                                Color32::from_rgb(38, 31, 59),
                                VIOLET,
                            );
                        }
                    });
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.monospace(format!(
                        "SESSION {}   FRAME {:08}   {:>7} NEURONS   {:>9} EDGES",
                        session.short_session_id().to_ascii_uppercase(),
                        session.last_summary.frame,
                        session.graph.identity.neuron_count,
                        session.graph.identity.edge_count
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(
                            POSITIVE,
                            format!(
                                "{} · {:.2} ms",
                                session.assessment.selected.label(),
                                session.last_step_ms
                            ),
                        );
                    });
                });
            });

        egui::Panel::bottom("lab_timeline")
            .resizable(false)
            .exact_size(142.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                timeline(ui, session);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("EVENT").strong().color(VIOLET));
                    ui.label(egui::RichText::new(&self.message).color(MUTED).size(11.0));
                    if let Some(warning) = &session.runtime_warning {
                        ui.colored_label(WARNING, warning);
                    }
                });
            });

        egui::Panel::left("experiment_rail")
            .resizable(false)
            .exact_size(258.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section_title(ui, "MODEL CONTROL", ACTUAL);
                        ui.label(
                            egui::RichText::new("Active session")
                                .strong()
                                .color(INK),
                        );
                        key_value(ui, "ID", session.short_session_id());
                        key_value(
                            ui,
                            "Started (Unix ms)",
                            &session.started_unix_millis.to_string(),
                        );
                        key_value(ui, "Skin", skin.label());
                        key_value(ui, "Compute", session.assessment.selected.label());
                        key_value(ui, "Capacity", session.assessment.tier.label());
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Compute preference").color(MUTED));
                        egui::ComboBox::from_id_salt("compute_preference")
                            .selected_text(self.compute_preference.label())
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for mode in ComputePreference::ALL {
                                    ui.selectable_value(
                                        &mut self.compute_preference,
                                        mode,
                                        mode.label(),
                                    );
                                }
                            });
                        if ui
                            .add_sized(
                                [ui.available_width(), 30.0],
                                egui::Button::new("Re-evaluate system capacity")
                                    .fill(ACTUAL_SOFT),
                            )
                            .on_hover_text(
                                "Benchmarks CPU and any compute-capable GPU, then starts a new identified model session.",
                            )
                            .clicked()
                        {
                            commands.push(LabCommand::Reevaluate(self.compute_preference));
                        }
                        ui.small(
                            "A capacity change creates a new session. The current replay epoch is never rewritten.",
                        );

                        ui.separator();
                        section_title(ui, "REPLAY SOURCE", VIOLET);
                        let maximum = session.replay.len().saturating_sub(1);
                        self.replay_frames_back = self.replay_frames_back.min(maximum);
                        ui.add(
                            egui::Slider::new(&mut self.replay_frames_back, 0..=maximum)
                                .text("frames back"),
                        );
                        ui.monospace(format!(
                            "{:03}/240 RETAINED",
                            session.replay.len()
                        ));

                        ui.separator();
                        ui.collapsing("LOCAL CONNECTOME IMPORT", |ui| {
                            ui.label("FlyWire Codex connection-table path");
                            ui.text_edit_singleline(&mut self.import_path);
                            if ui
                                .add_sized(
                                    [ui.available_width(), 28.0],
                                    egui::Button::new("Validate and start session"),
                                )
                                .clicked()
                            {
                                commands.push(LabCommand::ImportConnectome(
                                    self.import_path.clone(),
                                ));
                            }
                            ui.small(
                                "Local-only import records dataset version, source URL, columns, SHA-256, transforms, and warnings.",
                            );
                        });
                        ui.add_space(8.0);
                        status_chip(
                            ui,
                            "NO APPLY / COMMIT PATH",
                            Color32::from_rgb(45, 33, 38),
                            WARNING,
                        );
                    });
            });

        egui::Panel::right("evidence_inspector")
            .resizable(false)
            .exact_size(296.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section_title(ui, "TRUST LAYER", POSITIVE);
                        claim_badge(ui, "DERIVED CONNECTOME STRUCTURE", POSITIVE_SOFT, POSITIVE);
                        claim_badge(ui, "MODELED DYNAMICS", ACTUAL_SOFT, ACTUAL);
                        claim_badge(
                            ui,
                            "MEASURED ACTIVITY  NONE",
                            Color32::from_rgb(47, 36, 54),
                            VIOLET,
                        );
                        claim_badge(
                            ui,
                            "LIVE HARDWARE AUTHORITY  NONE",
                            Color32::from_rgb(45, 33, 38),
                            WARNING,
                        );
                        ui.separator();
                        ui.label(egui::RichText::new("Graph identity").strong());
                        ui.monospace(&session.graph.identity.graph_id);
                        key_value(ui, "Dataset", &session.graph.identity.dataset);
                        key_value(ui, "Snapshot", &session.graph.identity.snapshot);
                        key_value(ui, "Product", &session.graph.identity.product);
                        ui.label(egui::RichText::new("Graph SHA-256").color(MUTED));
                        ui.monospace(&session.graph.identity.sha256);
                        ui.separator();
                        ui.label(egui::RichText::new("Software build identity").strong());
                        if source_identity.is_complete() {
                            key_value(
                                ui,
                                "Branch",
                                source_identity.branch.as_deref().unwrap_or_default(),
                            );
                            ui.label(egui::RichText::new("Commit").color(MUTED));
                            ui.monospace(
                                source_identity.commit.as_deref().unwrap_or_default(),
                            );
                            ui.label(egui::RichText::new("Git tree").color(MUTED));
                            ui.monospace(
                                source_identity.tree.as_deref().unwrap_or_default(),
                            );
                            ui.label(egui::RichText::new("Executable SHA-256").color(MUTED));
                            ui.monospace(
                                source_identity
                                    .executable_sha256
                                    .as_deref()
                                    .unwrap_or_default(),
                            );
                        } else {
                            claim_badge(
                                ui,
                                "UNRECORDED DEVELOPMENT BUILD",
                                Color32::from_rgb(45, 33, 38),
                                WARNING,
                            );
                        }
                        ui.separator();
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
                                key_value(
                                    ui,
                                    "Root ID",
                                    &session.graph.neuron_ids[index].to_string(),
                                );
                                key_value(
                                    ui,
                                    "Activation",
                                    &session.engine.state.activation[index].to_string(),
                                );
                                key_value(
                                    ui,
                                    "Spiked",
                                    if session.engine.state.spikes[index] == 0 {
                                        "no"
                                    } else {
                                        "yes"
                                    },
                                );
                            } else {
                                ui.colored_label(WARNING, "Index is outside this graph.");
                            }
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Learning boundary").strong());
                        key_value(
                            ui,
                            "Policy",
                            if policy.enabled { "enabled" } else { "disabled" },
                        );
                        key_value(ui, "Explicit updates", &policy.ledger.len().to_string());
                        ui.monospace(policy.digest());
                    });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    view_tab(ui, &mut self.view, LabView::Overview, "POPULATION");
                    view_tab(ui, &mut self.view, LabView::Compare, "COUNTERFACTUAL");
                    view_tab(ui, &mut self.view, LabView::Provenance, "PROVENANCE");
                    view_tab(ui, &mut self.view, LabView::Learning, "LEARNING");
                });
                ui.add_space(12.0);
                match self.view {
                    LabView::Overview => overview(ui, session),
                    LabView::Compare => comparison_view(ui, self, &mut commands),
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
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = ACTUAL_SOFT;
    style.visuals.selection.stroke = Stroke::new(1.5, ACTUAL);
    style.visuals.extreme_bg_color = Color32::from_rgb(3, 7, 12);
    style.visuals.faint_bg_color = SURFACE_RAISED;
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, GRID);
    style.visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, GRID);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(23, 41, 55);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACTUAL);
    style.visuals.widgets.active.bg_fill = ACTUAL_SOFT;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACTUAL);
    style.visuals.hyperlink_color = ACTUAL;
    style.visuals.warn_fg_color = WARNING;
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    ctx.set_style_of(egui::Theme::Dark, style);
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

fn section_title(ui: &mut egui::Ui, label: &str, accent: Color32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 17.0), Sense::hover());
        ui.painter().rect_filled(rect, 2, accent);
        ui.label(
            egui::RichText::new(label)
                .strong()
                .size(12.0)
                .color(INK),
        );
    });
    ui.add_space(4.0);
}

fn status_chip(ui: &mut egui::Ui, label: &str, fill: Color32, text: Color32) {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, text.gamma_multiply(0.55)))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).strong().size(10.0).color(text));
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
    let active = *selected == view;
    let button = egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .size(11.0)
            .color(if active { CANVAS } else { MUTED }),
    )
    .fill(if active { ACTUAL } else { SURFACE_RAISED })
    .stroke(Stroke::new(1.0, if active { ACTUAL } else { GRID }));
    if ui.add(button).clicked() {
        *selected = view;
    }
}

fn overview(ui: &mut egui::Ui, session: &SimulationSession) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Modeled population field");
            ui.label(
                egui::RichText::new(
                    "Fixed structural projection · presentation context, never measured activity",
                )
                .color(MUTED),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            metric_card(
                ui,
                "BEHAVIOR",
                &format!("{:?}", session.last_summary.behavior).to_ascii_uppercase(),
                VIOLET,
            );
            metric_card(
                ui,
                "SPIKES",
                &session.last_summary.spike_count.to_string(),
                POSITIVE,
            );
            metric_card(
                ui,
                "FRAME",
                &session.last_summary.frame.to_string(),
                ACTUAL,
            );
        });
    });
    ui.add_space(10.0);
    let available = ui.available_size();
    let height = (available.y - 8.0).max(220.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available.x, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8, Color32::from_rgb(4, 10, 17));
    painter.rect_stroke(rect, 8, Stroke::new(1.0, GRID), StrokeKind::Inside);
    draw_field_grid(&painter, rect.shrink(14.0));
    draw_live_graph(&painter, rect.shrink2(Vec2::new(30.0, 26.0)), session);
    painter.text(
        rect.left_top() + Vec2::new(14.0, 12.0),
        Align2::LEFT_TOP,
        "L  ·  BILATERAL STRUCTURAL PROJECTION  ·  R",
        FontId::monospace(12.0),
        MUTED,
    );
    painter.text(
        rect.right_bottom() - Vec2::new(14.0, 12.0),
        Align2::RIGHT_BOTTOM,
        "MODELED ACTIVATION  ●     CURRENT SPIKES  ◆",
        FontId::monospace(10.5),
        ACTUAL,
    );
}

fn draw_live_graph(painter: &egui::Painter, rect: Rect, session: &SimulationSession) {
    let count = session.graph.positions.len();
    // An even stride sampled only even synthetic indices, which selected one
    // modeled hemisphere and produced the old triangular wedge. An odd stride
    // alternates parity and preserves the bilateral presentation.
    let stride = display_stride(count);
    for index in (0..count).step_by(stride) {
        let position = project(rect, session.graph.positions[index]);
        let activation = session.engine.state.activation[index];
        let normalized = ((activation + 8_192) as f32 / 16_384.0).clamp(0.0, 1.0);
        let color = blend(Color32::from_rgb(35, 51, 67), ACTUAL, normalized);
        let radius = if session.engine.state.spikes[index] != 0 {
            3.5
        } else {
            1.15 + normalized * 1.4
        };
        if session.engine.state.spikes[index] != 0 {
            painter.circle_filled(position, 8.5, POSITIVE.gamma_multiply(0.08));
            painter.circle_filled(position, 5.5, POSITIVE.gamma_multiply(0.16));
        } else if normalized > 0.78 {
            painter.circle_filled(position, radius + 2.5, ACTUAL.gamma_multiply(0.07));
        }
        painter.circle_filled(
            position,
            radius,
            if session.engine.state.spikes[index] != 0 {
                POSITIVE
            } else {
                color
            },
        );
    }
}

fn display_stride(count: usize) -> usize {
    count.div_ceil(3_200).max(1) | 1
}

fn draw_field_grid(painter: &egui::Painter, rect: Rect) {
    for index in 0..=8 {
        let amount = index as f32 / 8.0;
        let x = egui::lerp(rect.left()..=rect.right(), amount);
        let y = egui::lerp(rect.top()..=rect.bottom(), amount);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.7, GRID.gamma_multiply(0.42)),
        );
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.7, GRID.gamma_multiply(0.42)),
        );
    }
    painter.line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, VIOLET.gamma_multiply(0.35)),
    );
}

fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, accent: Color32) {
    egui::Frame::new()
        .fill(SURFACE_RAISED)
        .stroke(Stroke::new(1.0, GRID))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).size(9.0).color(MUTED));
                ui.label(egui::RichText::new(value).strong().color(accent));
            });
        });
}

fn comparison_view(
    ui: &mut egui::Ui,
    state: &mut BrainLabState,
    commands: &mut Vec<LabCommand>,
) {
    preview_composer(ui, state, commands);
    ui.add_space(10.0);
    if state.comparison.is_none() {
        egui::Frame::new()
            .fill(SURFACE_RAISED)
            .stroke(Stroke::new(1.0, GRID))
            .corner_radius(7)
            .inner_margin(20)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("No counterfactual has been authored")
                        .strong()
                        .size(18.0),
                );
                ui.label(
                    egui::RichText::new(
                        "Choose a bounded replay source at left, author the limits above, and generate a discarded comparison.",
                    )
                    .color(MUTED),
                );
            });
        return;
    }
    let frame_count = state
        .comparison
        .as_ref()
        .map(|comparison| comparison.frames.len())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.heading("Aligned multi-frame counterfactual");
        status_chip(ui, "● ACTUAL", ACTUAL_SOFT, ACTUAL);
        status_chip(ui, "■ AUTHORED ALTERNATIVE", ALTERNATIVE_SOFT, ALTERNATIVE);
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
            key_value(
                ui,
                "Alternative spikes",
                &frame.alternative.spike_count.to_string(),
            );
            key_value(
                ui,
                "Differing neurons",
                &frame.differing_neurons.to_string(),
            );
        });
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        claim_badge(ui, &comparison.receipt.status, POSITIVE_SOFT, POSITIVE);
        ui.label(format!("source frame {}", comparison.receipt.source_frame));
        ui.label(format!(
            "live unchanged: {}",
            comparison.receipt.live_state_unchanged
        ));
        ui.label(format!(
            "alternative differs: {}",
            comparison.receipt.alternative_differs
        ));
    });
}

fn preview_composer(
    ui: &mut egui::Ui,
    state: &mut BrainLabState,
    commands: &mut Vec<LabCommand>,
) {
    egui::Frame::new()
        .fill(Color32::from_rgb(19, 18, 25))
        .stroke(Stroke::new(1.0, ALTERNATIVE.gamma_multiply(0.55)))
        .corner_radius(7)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                section_title(ui, "AUTHORED STIMULATION PREVIEW", ALTERNATIVE);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_chip(
                        ui,
                        "DISCARDED · LIVE STATE ISOLATED",
                        Color32::from_rgb(45, 33, 38),
                        WARNING,
                    );
                });
            });
            ui.columns(2, |columns| {
                columns[0].label(egui::RichText::new("Target neuron indices").color(MUTED));
                columns[0].text_edit_singleline(&mut state.targets);
                columns[0].label(egui::RichText::new("Authored purpose").color(MUTED));
                columns[0].text_edit_singleline(&mut state.authored_label);

                columns[1].horizontal(|ui| {
                    ui.label("Amplitude");
                    ui.add(
                        egui::DragValue::new(&mut state.amplitude)
                            .range(0.01..=0.25)
                            .speed(0.01),
                    );
                    ui.label("Duration ms");
                    ui.add(
                        egui::DragValue::new(&mut state.duration_ms)
                            .range(33..=990)
                            .speed(33),
                    );
                    ui.label("Frames");
                    ui.add(egui::DragValue::new(&mut state.comparison_frames).range(1..=120));
                });
                if columns[1]
                    .add_sized(
                        [columns[1].available_width(), 31.0],
                        egui::Button::new("Generate isolated comparison")
                            .fill(ALTERNATIVE_SOFT),
                    )
                    .clicked()
                {
                    commands.push(LabCommand::GeneratePreview);
                }
                columns[1].small(
                    "≤64 targets · amplitude ≤0.25 · 33–990 ms · dosage ceiling · no apply path",
                );
            });
        });
}

fn draw_filmstrip(painter: egui::Painter, rect: Rect, comparison: &ComparisonResult) {
    painter.rect_filled(rect, 7, Color32::from_rgb(4, 10, 17));
    painter.rect_stroke(rect, 7, Stroke::new(1.0, GRID), StrokeKind::Inside);
    let count = comparison.frames.len();
    let indices = five_indices(count);
    let gap = 8.0;
    let left = rect.left() + 72.0;
    let width = (rect.width() - 88.0 - gap * 4.0) / 5.0;
    let row_height = 142.0;
    let actual_y = rect.top() + 34.0;
    let alternative_y = actual_y + row_height + 24.0;
    painter.text(
        Pos2::new(rect.left() + 10.0, actual_y + 55.0),
        Align2::LEFT_CENTER,
        "ACTUAL",
        FontId::proportional(12.0),
        ACTUAL,
    );
    painter.text(
        Pos2::new(rect.left() + 10.0, alternative_y + 55.0),
        Align2::LEFT_CENTER,
        "ALT",
        FontId::proportional(12.0),
        ALTERNATIVE,
    );
    for (column, index) in indices.into_iter().enumerate() {
        let frame = &comparison.frames[index];
        let x = left + column as f32 * (width + gap);
        let actual_rect = Rect::from_min_size(Pos2::new(x, actual_y), Vec2::new(width, 112.0));
        let alt_rect = Rect::from_min_size(Pos2::new(x, alternative_y), Vec2::new(width, 112.0));
        mini_frame(&painter, actual_rect, &frame.actual_sample, ACTUAL, false);
        mini_frame(
            &painter,
            alt_rect,
            &frame.alternative_sample,
            ALTERNATIVE,
            true,
        );
        painter.text(
            Pos2::new(x + width * 0.5, rect.top() + 12.0),
            Align2::CENTER_TOP,
            format!("+{}", frame.offset),
            FontId::monospace(11.0),
            MUTED,
        );
        let divergence =
            frame.differing_neurons as f32 / comparison.frames[0].actual_sample.len().max(1) as f32;
        let bar_rect =
            Rect::from_min_size(Pos2::new(x, rect.bottom() - 24.0), Vec2::new(width, 8.0));
        painter.rect_filled(bar_rect, 2, Color32::from_rgb(28, 38, 50));
        painter.rect_filled(
            Rect::from_min_size(
                bar_rect.min,
                Vec2::new(width * divergence.clamp(0.0, 1.0), 8.0),
            ),
            2,
            ALTERNATIVE,
        );
    }
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.bottom() - 20.0),
        Align2::LEFT_CENTER,
        "Δ",
        FontId::proportional(13.0),
        ALTERNATIVE,
    );
}

fn mini_frame(
    painter: &egui::Painter,
    rect: Rect,
    samples: &[mechofly_core::NeuronSample],
    color: Color32,
    squares: bool,
) {
    painter.rect_filled(rect, 3, Color32::from_rgb(7, 14, 23));
    painter.rect_stroke(rect, 3, Stroke::new(1.2, color), StrokeKind::Inside);
    for sample in samples {
        let normalized_index =
            sample.index as f32 / samples.last().map(|s| s.index.max(1)).unwrap_or(1) as f32;
        let angle = normalized_index * std::f32::consts::TAU * 7.0;
        let radius = normalized_index.sqrt() * 0.43;
        let center = rect.center();
        let position = center
            + Vec2::new(
                angle.cos() * rect.width() * radius,
                angle.sin() * rect.height() * radius * 0.85,
            );
        let intensity = ((sample.activation_q15 as f32 + 8_192.0) / 16_384.0).clamp(0.15, 1.0);
        let mark = blend(Color32::from_rgb(40, 53, 66), color, intensity);
        let size = if sample.spiked { 3.0 } else { 1.5 };
        if squares {
            painter.rect_filled(
                Rect::from_center_size(position, Vec2::splat(size * 2.0)),
                0,
                mark,
            );
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
    ui.heading("Provenance stack");
    ui.label(
        egui::RichText::new(
            "Claims narrow at every layer. No layer below represents a biological recording or live intervention.",
        )
        .color(MUTED),
    );
    ui.add_space(10.0);
    provenance_card(
        ui,
        "1 · DERIVED CONNECTOME STRUCTURE",
        POSITIVE_SOFT,
        &session.graph.identity.structure_claim,
        &session.graph.identity.source_url,
        &session.graph.identity.sha256,
    );
    provenance_card(
        ui,
        "2 · MODELED NEURAL DYNAMICS",
        ACTUAL_SOFT,
        "deterministic signed fixed-point software dynamics",
        mechofly_core::MODEL_VERSION,
        &session.engine.model_identity(),
    );
    provenance_card(
        ui,
        "3 · MODELED SOFTWARE LEARNING",
        Color32::from_rgb(38, 31, 59),
        "bounded contextual policy changed only by explicit feedback",
        mechofly_core::learning::LEARNING_RULE_VERSION,
        "stored separately from neural state",
    );
    provenance_card(
        ui,
        "4 · AUTHORED PRESENTATION",
        ALTERNATIVE_SOFT,
        "independently authored procedural pet skin and neural-observatory interface",
        "presentation-v3",
        "does not alter graph or dynamics",
    );
}

fn provenance_card(
    ui: &mut egui::Ui,
    title: &str,
    fill: Color32,
    claim: &str,
    source: &str,
    digest: &str,
) {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, GRID))
        .corner_radius(5)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(claim);
            ui.monospace(source);
            ui.monospace(digest);
        });
    ui.add_space(8.0);
}

fn learning_view(ui: &mut egui::Ui, policy: &PetPolicy, commands: &mut Vec<LabCommand>) {
    ui.heading("Bounded software-policy ledger");
    ui.label(
        egui::RichText::new(
            "The connectome is immutable. Only the separate companion policy learns, and only after explicit feedback.",
        )
        .color(MUTED),
    );
    let mut enabled = policy.enabled;
    if ui
        .checkbox(&mut enabled, "Allow learning from explicit feedback")
        .changed()
    {
        commands.push(LabCommand::SetLearningEnabled(enabled));
    }
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new("Encourage current action").fill(POSITIVE_SOFT))
            .clicked()
        {
            commands.push(LabCommand::Feedback(Feedback::Encourage));
        }
        if ui
            .add(egui::Button::new("Discourage current action").fill(ALTERNATIVE_SOFT))
            .clicked()
        {
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
            egui::Frame::new()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, GRID))
                .inner_margin(8)
                .show(ui, |ui| {
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 76.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4, Color32::from_rgb(4, 10, 17));
    painter.rect_stroke(rect, 3, Stroke::new(1.0, GRID), StrokeKind::Inside);
    let summaries: Vec<_> = session.replay.summaries().collect();
    if summaries.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "waiting for modeled frames",
            FontId::proportional(12.0),
            MUTED,
        );
        return;
    }
    let max_spikes = summaries
        .iter()
        .map(|summary| summary.spike_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_width = rect.width() / summaries.len() as f32;
    for (index, summary) in summaries.into_iter().enumerate() {
        let height = summary.spike_count as f32 / max_spikes as f32 * (rect.height() - 22.0);
        let x = rect.left() + index as f32 * bar_width;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(x, rect.bottom() - height - 12.0),
                Pos2::new(x + bar_width.max(1.0), rect.bottom() - 12.0),
            ),
            0,
            ACTUAL,
        );
        let behavior_color = match summary.behavior {
            mechofly_core::Behavior::Rest | mechofly_core::Behavior::Quiet => GRID,
            mechofly_core::Behavior::Walk | mechofly_core::Behavior::Reverse => POSITIVE,
            mechofly_core::Behavior::Groom => WARNING,
            _ => ALTERNATIVE,
        };
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(x, rect.bottom() - 8.0),
                Vec2::new(bar_width.max(1.0), 5.0),
            ),
            0,
            behavior_color,
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_sampling_preserves_alternating_hemispheres() {
        for count in [4_096_usize, 12_615, 65_536, 139_255] {
            assert!(!display_stride(count).is_multiple_of(2));
        }
    }

    #[test]
    fn preview_targets_accept_commas_and_whitespace() {
        assert_eq!(parse_targets("3, 7  11\n19"), Ok(vec![3, 7, 11, 19]));
    }
}
