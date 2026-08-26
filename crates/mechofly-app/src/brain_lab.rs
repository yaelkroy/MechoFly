use std::{sync::Arc, time::Duration};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use mechofly_core::{ComparisonResult, Feedback, PetPolicy, StimulationPolicy, StimulationRequest};
use serde::Deserialize;

use crate::{
    app::RuntimeSourceIdentity, compute::ComputePreference, pet::Skin, runtime::SimulationSession,
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

#[derive(Deserialize)]
struct AtlasAsset {
    classes: Vec<String>,
    points: Vec<[f32; 4]>,
    source: String,
}

struct AtlasPoint {
    position: [f32; 2],
    class: usize,
    depth: f32,
}

struct AnatomicalAtlas {
    classes: Vec<String>,
    points: Vec<AtlasPoint>,
    source: String,
}

impl AnatomicalAtlas {
    fn load() -> Self {
        let asset: AtlasAsset = serde_json::from_str(include_str!("../assets/brain_points.json"))
            .expect("embedded FlyWire anatomical context must be valid JSON");
        let points = asset
            .points
            .into_iter()
            .map(|point| AtlasPoint {
                // Fixed dorsal three-quarter presentation. These points are
                // immutable anatomical context, never modeled neurons.
                position: [
                    ((point[0] + point[2] * 0.22) / 10.75).clamp(-1.0, 1.0),
                    (-(point[1] - point[2] * 0.16) / 5.05).clamp(-1.0, 1.0),
                ],
                class: point[3].max(0.0) as usize,
                depth: ((point[2] + 3.5) / 7.0).clamp(0.0, 1.0),
            })
            .collect();
        Self {
            classes: asset.classes,
            points,
            source: asset.source,
        }
    }
}

#[derive(Clone, Copy)]
struct Connection {
    index: usize,
    weight: i32,
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
    atlas: AnatomicalAtlas,
    neighborhood_for: Option<usize>,
    neighborhood_graph_sha256: Option<String>,
    inbound: Vec<Connection>,
    outbound: Vec<Connection>,
    inbound_count: usize,
    outbound_count: usize,
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
            atlas: AnatomicalAtlas::load(),
            neighborhood_for: None,
            neighborhood_graph_sha256: None,
            inbound: Vec::new(),
            outbound: Vec::new(),
            inbound_count: 0,
            outbound_count: 0,
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
            .exact_size(194.0)
            .frame(surface_frame())
            .show(ui, |ui| {
                timeline(
                    ui,
                    session,
                    self.replay_frames_back,
                    resolve_selected_index(&self.selected_neuron, session),
                );
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
                            ui.monospace(source_identity.commit.as_deref().unwrap_or_default());
                            ui.label(egui::RichText::new("Git tree").color(MUTED));
                            ui.monospace(source_identity.tree.as_deref().unwrap_or_default());
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
                        ui.label(egui::RichText::new("Circuit explorer").strong());
                        ui.label(
                            egui::RichText::new(
                                "Enter a model index or graph root ID, or click a modeled point.",
                            )
                            .size(10.5)
                            .color(MUTED),
                        );
                        ui.text_edit_singleline(&mut self.selected_neuron);
                        if let Some(index) = resolve_selected_index(&self.selected_neuron, session)
                        {
                            self.ensure_neighborhood(session, index);
                            key_value(ui, "Model index", &index.to_string());
                            key_value(
                                ui,
                                "Graph root ID",
                                &session.graph.neuron_ids[index].to_string(),
                            );
                            key_value(
                                ui,
                                "Activation q15",
                                &session.engine.state.activation[index].to_string(),
                            );
                            key_value(
                                ui,
                                "Current spike",
                                if session.engine.state.spikes[index] == 0 {
                                    "no"
                                } else {
                                    "YES"
                                },
                            );
                            connection_list(
                                ui,
                                "STRONGEST INBOUND",
                                self.inbound_count,
                                &self.inbound,
                                session,
                                ACTUAL,
                            );
                            connection_list(
                                ui,
                                "STRONGEST OUTBOUND",
                                self.outbound_count,
                                &self.outbound,
                                session,
                                ALTERNATIVE,
                            );
                        } else if !self.selected_neuron.trim().is_empty() {
                            ui.colored_label(WARNING, "No model index or root ID matched.");
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Learning boundary").strong());
                        key_value(
                            ui,
                            "Policy",
                            if policy.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            },
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
                    LabView::Overview => overview(ui, session, self),
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

    fn ensure_neighborhood(&mut self, session: &SimulationSession, index: usize) {
        if self.neighborhood_for == Some(index)
            && self.neighborhood_graph_sha256.as_deref()
                == Some(session.graph.identity.sha256.as_str())
        {
            return;
        }
        let offsets = &session.graph.incoming_offsets;
        let sources = &session.graph.incoming_sources;
        let weights = &session.graph.modeled_weights;
        let start = offsets[index] as usize;
        let end = offsets[index + 1] as usize;
        self.inbound = (start..end)
            .map(|edge| Connection {
                index: sources[edge] as usize,
                weight: weights[edge],
            })
            .collect();
        self.inbound_count = self.inbound.len();
        self.outbound.clear();
        for target in 0..session.graph.neuron_ids.len() {
            let target_start = offsets[target] as usize;
            let target_end = offsets[target + 1] as usize;
            for edge in target_start..target_end {
                if sources[edge] as usize == index {
                    self.outbound.push(Connection {
                        index: target,
                        weight: weights[edge],
                    });
                }
            }
        }
        self.outbound_count = self.outbound.len();
        self.inbound
            .sort_by_key(|connection| std::cmp::Reverse(connection.weight.unsigned_abs()));
        self.outbound
            .sort_by_key(|connection| std::cmp::Reverse(connection.weight.unsigned_abs()));
        self.inbound.truncate(24);
        self.outbound.truncate(24);
        self.neighborhood_for = Some(index);
        self.neighborhood_graph_sha256 = Some(session.graph.identity.sha256.clone());
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
        ui.label(egui::RichText::new(label).strong().size(12.0).color(INK));
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

fn overview(ui: &mut egui::Ui, session: &SimulationSession, state: &mut BrainLabState) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Anatomical activity observatory");
            ui.label(
                egui::RichText::new(
                    "FlyWire-derived anatomical context + real modeled frame state · never measured activity",
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
            metric_card(ui, "FRAME", &session.last_summary.frame.to_string(), ACTUAL);
        });
    });
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        causal_stage(ui, "01", "CONTEXT", "environment + policy", VIOLET);
        ui.colored_label(MUTED, "→");
        causal_stage(ui, "02", "MODEL", "fixed-point dynamics", ACTUAL);
        ui.colored_label(MUTED, "→");
        causal_stage(ui, "03", "SPIKES", "current modeled frame", POSITIVE);
        ui.colored_label(MUTED, "→");
        causal_stage(ui, "04", "BEHAVIOR", "bounded state machine", ALTERNATIVE);
        ui.colored_label(MUTED, "→");
        causal_stage(ui, "05", "PET POSE", "authored presentation", WARNING);
    });
    ui.add_space(7.0);
    let available = ui.available_size();
    let height = (available.y - 8.0).max(220.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(available.x, height), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8, Color32::from_rgb(4, 10, 17));
    painter.rect_stroke(rect, 8, Stroke::new(1.0, GRID), StrokeKind::Inside);
    draw_field_grid(&painter, rect.shrink(14.0));
    let field = rect.shrink2(Vec2::new(30.0, 30.0));
    draw_anatomical_context(&painter, field, &state.atlas);
    let selected = resolve_selected_index(&state.selected_neuron, session);
    draw_live_graph(
        &painter,
        field,
        session,
        &state.atlas,
        selected,
        &state.inbound,
        &state.outbound,
    );
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let hovered = pointer
        .filter(|position| rect.contains(*position))
        .and_then(|position| {
            nearest_displayed_neuron(position, field, session, &state.atlas, 18.0)
        });
    if let (Some(position), Some(index)) = (pointer, hovered) {
        let box_size = Vec2::new(238.0, 43.0);
        let box_min = Pos2::new(
            (position.x + 13.0).min(rect.right() - box_size.x - 8.0),
            (position.y + 13.0).min(rect.bottom() - box_size.y - 8.0),
        );
        let tooltip = Rect::from_min_size(box_min, box_size);
        painter.rect_filled(tooltip, 4, Color32::from_rgba_unmultiplied(5, 12, 20, 238));
        painter.rect_stroke(tooltip, 4, Stroke::new(1.0, ACTUAL), StrokeKind::Inside);
        painter.text(
            tooltip.left_top() + Vec2::new(8.0, 6.0),
            Align2::LEFT_TOP,
            format!(
                "INDEX {index}  ·  ROOT {}\nactivation {}  ·  spike {}  ·  click to inspect",
                session.graph.neuron_ids[index],
                session.engine.state.activation[index],
                if session.engine.state.spikes[index] == 0 {
                    "no"
                } else {
                    "YES"
                }
            ),
            FontId::monospace(10.0),
            INK,
        );
    }
    if response.clicked()
        && let Some(position) = response.interact_pointer_pos()
        && let Some(index) = nearest_displayed_neuron(position, field, session, &state.atlas, 22.0)
    {
        state.selected_neuron = index.to_string();
        state.neighborhood_for = None;
        state.message = format!(
            "Selected modeled neuron {index} (root {}). Circuit neighborhood is synchronized.",
            session.graph.neuron_ids[index]
        );
    }
    painter.text(
        rect.left_top() + Vec2::new(14.0, 12.0),
        Align2::LEFT_TOP,
        format!(
            "FAFB ANATOMICAL CONTEXT  ·  {} SOMA POINTS  ·  NOT SIMULATED",
            state.atlas.points.len()
        ),
        FontId::monospace(12.0),
        MUTED,
    );
    painter.text(
        rect.right_bottom() - Vec2::new(14.0, 12.0),
        Align2::RIGHT_BOTTOM,
        "MODELED ACTIVITY  ●   CURRENT SPIKES  ◆   ORDINAL DISPLAY REGISTRATION — NO IDENTITY MAPPING",
        FontId::monospace(10.5),
        ACTUAL,
    );
    painter.text(
        rect.left_bottom() + Vec2::new(14.0, -12.0),
        Align2::LEFT_BOTTOM,
        format!(
            "{}  ·  {} classes",
            state.atlas.source,
            state.atlas.classes.len()
        ),
        FontId::monospace(9.5),
        MUTED,
    );
}

fn causal_stage(ui: &mut egui::Ui, number: &str, label: &str, detail: &str, accent: Color32) {
    egui::Frame::new()
        .fill(SURFACE_RAISED)
        .stroke(Stroke::new(1.0, accent.gamma_multiply(0.55)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(accent, egui::RichText::new(number).strong().monospace());
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(label).strong().size(9.5).color(INK));
                    ui.label(egui::RichText::new(detail).size(8.5).color(MUTED));
                });
            });
        });
}

fn draw_anatomical_context(painter: &egui::Painter, rect: Rect, atlas: &AnatomicalAtlas) {
    for point in atlas.points.iter().step_by(2) {
        let position = project(rect, point.position);
        let color = atlas_color(point.class, 18 + (point.depth * 20.0) as u8);
        painter.circle_filled(position, 0.75 + point.depth * 0.45, color);
    }
}

fn atlas_color(class: usize, alpha: u8) -> Color32 {
    let (red, green, blue) = match class % 9 {
        0 => (55, 112, 132),
        1 => (70, 112, 101),
        2 => (98, 95, 132),
        3 => (67, 123, 125),
        4 => (112, 86, 122),
        5 => (126, 99, 73),
        6 => (77, 104, 139),
        7 => (111, 116, 73),
        _ => (112, 78, 94),
    };
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn draw_live_graph(
    painter: &egui::Painter,
    rect: Rect,
    session: &SimulationSession,
    atlas: &AnatomicalAtlas,
    selected: Option<usize>,
    inbound: &[Connection],
    outbound: &[Connection],
) {
    if let Some(index) = selected {
        let center = displayed_model_position(rect, session, atlas, index);
        for connection in inbound.iter().take(12) {
            painter.line_segment(
                [
                    displayed_model_position(rect, session, atlas, connection.index),
                    center,
                ],
                Stroke::new(0.8, ACTUAL.gamma_multiply(0.52)),
            );
        }
        for connection in outbound.iter().take(12) {
            painter.line_segment(
                [
                    center,
                    displayed_model_position(rect, session, atlas, connection.index),
                ],
                Stroke::new(0.8, ALTERNATIVE.gamma_multiply(0.48)),
            );
        }
    }
    let count = session.graph.positions.len();
    let stride = display_stride(count);
    for index in (0..count).step_by(stride) {
        let position = displayed_model_position(rect, session, atlas, index);
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
    if let Some(index) = selected {
        let position = displayed_model_position(rect, session, atlas, index);
        painter.circle_filled(position, 10.0, VIOLET.gamma_multiply(0.14));
        painter.circle_stroke(position, 7.0, Stroke::new(1.8, VIOLET));
        painter.circle_filled(
            position,
            3.2,
            if session.engine.state.spikes[index] == 0 {
                INK
            } else {
                POSITIVE
            },
        );
    }
}

fn displayed_model_position(
    rect: Rect,
    session: &SimulationSession,
    atlas: &AnatomicalAtlas,
    index: usize,
) -> Pos2 {
    if atlas.points.is_empty() {
        return project(rect, session.graph.positions[index]);
    }
    let root = session.graph.neuron_ids[index];
    let atlas_index = presentation_hash(root ^ index as u64) as usize % atlas.points.len();
    project(rect, atlas.points[atlas_index].position)
}

fn presentation_hash(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn nearest_displayed_neuron(
    pointer: Pos2,
    rect: Rect,
    session: &SimulationSession,
    atlas: &AnatomicalAtlas,
    maximum_distance: f32,
) -> Option<usize> {
    let mut nearest = None;
    let mut nearest_squared = maximum_distance * maximum_distance;
    for index in
        (0..session.graph.positions.len()).step_by(display_stride(session.graph.positions.len()))
    {
        let position = displayed_model_position(rect, session, atlas, index);
        let distance = position.distance_sq(pointer);
        if distance < nearest_squared {
            nearest_squared = distance;
            nearest = Some(index);
        }
    }
    nearest
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

fn comparison_view(ui: &mut egui::Ui, state: &mut BrainLabState, commands: &mut Vec<LabCommand>) {
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

fn preview_composer(ui: &mut egui::Ui, state: &mut BrainLabState, commands: &mut Vec<LabCommand>) {
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
                        egui::Button::new("Generate isolated comparison").fill(ALTERNATIVE_SOFT),
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

fn resolve_selected_index(text: &str, session: &SimulationSession) -> Option<usize> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let explicit_root = trimmed
        .strip_prefix("root:")
        .or_else(|| trimmed.strip_prefix("id:"));
    let value = explicit_root
        .unwrap_or(trimmed)
        .trim()
        .parse::<u64>()
        .ok()?;
    if explicit_root.is_none() && (value as usize) < session.graph.neuron_ids.len() {
        return Some(value as usize);
    }
    session
        .graph
        .neuron_ids
        .iter()
        .position(|root_id| *root_id == value)
}

fn connection_list(
    ui: &mut egui::Ui,
    label: &str,
    total: usize,
    connections: &[Connection],
    session: &SimulationSession,
    accent: Color32,
) {
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.colored_label(accent, egui::RichText::new(label).strong().size(9.5));
        ui.label(
            egui::RichText::new(format!("{total} total"))
                .size(9.5)
                .color(MUTED),
        );
    });
    for connection in connections.iter().take(6) {
        let sign = if connection.weight >= 0 { "+" } else { "" };
        ui.monospace(format!(
            "#{:<6} root {:<12}  {sign}{}",
            connection.index, session.graph.neuron_ids[connection.index], connection.weight
        ));
    }
    if total == 0 {
        ui.label(egui::RichText::new("none").size(10.0).color(MUTED));
    }
}

fn timeline(
    ui: &mut egui::Ui,
    session: &SimulationSession,
    replay_frames_back: usize,
    selected_neuron: Option<usize>,
) {
    let current_fraction = session.last_summary.spike_count as f64
        / session.graph.identity.neuron_count.max(1) as f64
        * 100.0;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("ACTIVITY TIMELINE").strong());
        ui.label(format!("last {} retained frames", session.replay.len()));
        ui.colored_label(
            POSITIVE,
            format!(
                "current {} spikes ({current_fraction:.3}%/frame)",
                session.last_summary.spike_count
            ),
        );
        ui.colored_label(
            VIOLET,
            format!("mean q15 {}", session.last_summary.mean_activation_q15),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!(
                "{:?} persisted {} frames",
                session.engine.state.behavior, session.engine.state.behavior_age_frames
            ));
        });
    });
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 126.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4, Color32::from_rgb(4, 10, 17));
    painter.rect_stroke(rect, 3, Stroke::new(1.0, GRID), StrokeKind::Inside);
    let frames: Vec<_> = session.replay.frames().collect();
    if frames.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "waiting for modeled frames",
            FontId::proportional(12.0),
            MUTED,
        );
        return;
    }
    let max_spikes = frames
        .iter()
        .map(|frame| frame.summary.spike_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let plot = Rect::from_min_max(
        rect.left_top() + Vec2::new(58.0, 8.0),
        rect.right_bottom() - Vec2::new(8.0, 8.0),
    );
    let spike_rect =
        Rect::from_min_max(plot.left_top(), Pos2::new(plot.right(), plot.top() + 62.0));
    let activation_rect = Rect::from_min_max(
        Pos2::new(plot.left(), spike_rect.bottom() + 4.0),
        Pos2::new(plot.right(), spike_rect.bottom() + 30.0),
    );
    let behavior_y = plot.bottom() - 6.0;
    painter.text(
        Pos2::new(rect.left() + 7.0, spike_rect.center().y),
        Align2::LEFT_CENTER,
        "SPIKES",
        FontId::monospace(9.5),
        ACTUAL,
    );
    painter.text(
        Pos2::new(rect.left() + 7.0, activation_rect.center().y),
        Align2::LEFT_CENTER,
        "MEAN",
        FontId::monospace(9.5),
        VIOLET,
    );
    painter.text(
        Pos2::new(rect.left() + 7.0, behavior_y),
        Align2::LEFT_CENTER,
        "STATE",
        FontId::monospace(9.5),
        MUTED,
    );
    for fraction in [0.0_f32, 0.5, 1.0] {
        let y = egui::lerp(spike_rect.bottom()..=spike_rect.top(), fraction);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(0.6, GRID.gamma_multiply(0.5)),
        );
    }
    painter.text(
        spike_rect.right_top(),
        Align2::RIGHT_TOP,
        format!("scale 0–{max_spikes}"),
        FontId::monospace(8.5),
        MUTED,
    );
    let zero_y = egui::lerp(activation_rect.bottom()..=activation_rect.top(), 0.5);
    painter.line_segment(
        [
            Pos2::new(activation_rect.left(), zero_y),
            Pos2::new(activation_rect.right(), zero_y),
        ],
        Stroke::new(0.7, GRID),
    );
    let bar_width = plot.width() / frames.len() as f32;
    let mut activation_points = Vec::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        let summary = &frame.summary;
        let height = summary.spike_count as f32 / max_spikes as f32 * spike_rect.height();
        let x = plot.left() + index as f32 * bar_width;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(x, spike_rect.bottom() - height),
                Pos2::new(x + bar_width.max(1.0), spike_rect.bottom()),
            ),
            0,
            ACTUAL.gamma_multiply(0.76),
        );
        let mean = ((summary.mean_activation_q15 as f32 + 32_768.0) / 65_535.0).clamp(0.0, 1.0);
        activation_points.push(Pos2::new(
            x + bar_width * 0.5,
            egui::lerp(activation_rect.bottom()..=activation_rect.top(), mean),
        ));
        let behavior_color = match summary.behavior {
            mechofly_core::Behavior::Rest | mechofly_core::Behavior::Quiet => GRID,
            mechofly_core::Behavior::Walk | mechofly_core::Behavior::Reverse => POSITIVE,
            mechofly_core::Behavior::Groom => WARNING,
            _ => ALTERNATIVE,
        };
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(x, behavior_y - 3.0),
                Vec2::new(bar_width.max(1.0), 6.0),
            ),
            0,
            behavior_color,
        );
        if let Some(neuron) = selected_neuron
            && frame.state.spikes.get(neuron).copied().unwrap_or_default() != 0
        {
            painter.circle_filled(
                Pos2::new(x + bar_width * 0.5, activation_rect.bottom() - 2.5),
                1.8,
                POSITIVE,
            );
        }
    }
    painter.add(egui::Shape::line(
        activation_points,
        Stroke::new(1.3, VIOLET),
    ));
    let replay_index = frames
        .len()
        .saturating_sub(1)
        .saturating_sub(replay_frames_back.min(frames.len() - 1));
    let replay_x = plot.left() + (replay_index as f32 + 0.5) * bar_width;
    painter.line_segment(
        [
            Pos2::new(replay_x, plot.top()),
            Pos2::new(replay_x, plot.bottom()),
        ],
        Stroke::new(1.4, ALTERNATIVE),
    );
    painter.text(
        Pos2::new(replay_x + 4.0, plot.top() + 3.0),
        Align2::LEFT_TOP,
        format!("replay −{replay_frames_back}"),
        FontId::monospace(8.5),
        ALTERNATIVE,
    );
    if response.hovered()
        && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        && plot.contains(pointer)
    {
        let index =
            (((pointer.x - plot.left()) / bar_width).floor() as usize).min(frames.len() - 1);
        let frame = frames[index];
        let x = plot.left() + (index as f32 + 0.5) * bar_width;
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, INK.gamma_multiply(0.55)),
        );
        painter.text(
            Pos2::new(plot.right() - 6.0, plot.top() + 5.0),
            Align2::RIGHT_TOP,
            format!(
                "FRAME {}  ·  {} spikes  ·  mean {}  ·  {:?}",
                frame.summary.frame,
                frame.summary.spike_count,
                frame.summary.mean_activation_q15,
                frame.summary.behavior
            ),
            FontId::monospace(9.5),
            INK,
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

    #[test]
    fn embedded_anatomical_context_has_expected_scope() {
        let atlas = AnatomicalAtlas::load();
        assert_eq!(atlas.points.len(), 23_210);
        assert_eq!(atlas.classes.len(), 9);
        assert!(atlas.source.contains("FlyWire"));
        assert!(atlas.points.iter().all(|point| {
            (-1.0..=1.0).contains(&point.position[0]) && (-1.0..=1.0).contains(&point.position[1])
        }));
    }
}
