use std::sync::OnceLock;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use mechofly_core::Behavior;
use serde::Deserialize;

use crate::{app::RuntimeSourceIdentity, pet::Skin, runtime::SimulationSession};

const BACKGROUND: Color32 = Color32::from_rgb(7, 8, 13);
const PANEL: Color32 = Color32::from_rgb(14, 14, 23);
const PANEL_RAISED: Color32 = Color32::from_rgb(22, 20, 34);
const GRID: Color32 = Color32::from_rgb(49, 43, 68);
const TEXT: Color32 = Color32::from_rgb(238, 232, 246);
const MUTED: Color32 = Color32::from_rgb(153, 143, 171);
const CYAN: Color32 = Color32::from_rgb(66, 224, 218);
const GREEN: Color32 = Color32::from_rgb(103, 218, 108);
const YELLOW: Color32 = Color32::from_rgb(247, 221, 62);
const VIOLET: Color32 = Color32::from_rgb(154, 111, 226);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBrainCommand {
    OpenLab,
}

pub struct LiveBrainState {
    pub open: bool,
    paused: bool,
    show_context: bool,
    selected_neuron: usize,
    history_floor: u64,
}

impl LiveBrainState {
    pub fn new(open: bool) -> Self {
        Self {
            open,
            paused: false,
            show_context: true,
            selected_neuron: 0,
            history_floor: 0,
        }
    }

    pub fn selected_neuron(&self) -> usize {
        self.selected_neuron
    }

    pub fn set_selected_neuron(&mut self, index: usize) {
        self.selected_neuron = index;
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        session: &SimulationSession,
        behavior: Behavior,
        skin: Skin,
        source_identity: &RuntimeSourceIdentity,
    ) -> Vec<LiveBrainCommand> {
        style(ui.ctx());
        let mut commands = Vec::new();

        egui::Panel::top("live_brain_toolbar")
            .exact_size(38.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(40, 24, 58))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  —  LIVE BRAIN",
                            skin.label().to_ascii_uppercase()
                        ))
                        .strong()
                        .color(TEXT),
                    );
                    ui.separator();
                    if ui
                        .button(if self.paused {
                            "Resume view"
                        } else {
                            "Pause view"
                        })
                        .clicked()
                    {
                        self.paused = !self.paused;
                    }
                    if ui.button("Clear history").clicked() {
                        self.history_floor = session.last_summary.frame;
                    }
                    if ui.button("Brain Lab").clicked() {
                        commands.push(LiveBrainCommand::OpenLab);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if self.show_context {
                                "Hide context"
                            } else {
                                "Show context"
                            })
                            .clicked()
                        {
                            self.show_context = !self.show_context;
                        }
                        ui.monospace(format!(
                            "{} modeled  ·  {} context  ·  {} classes",
                            session.graph.identity.neuron_count,
                            atlas().points.len(),
                            atlas().classes.len()
                        ));
                    });
                });
            });

        egui::Panel::bottom("live_brain_raster")
            .exact_size(172.0)
            .frame(panel_frame())
            .show(ui, |ui| {
                draw_raster(ui, session, self.history_floor);
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.colored_label(VIOLET, "FIXED FLYWIRE X–Y CONTEXT PROJECTION");
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} · model overlay is not anatomical registration",
                            session.graph.identity.structure_claim,
                            source_identity
                                .short_commit()
                                .unwrap_or("unreceipted build")
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
            });

        egui::Panel::right("live_brain_activity")
            .exact_size(294.0)
            .resizable(false)
            .frame(panel_frame())
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        draw_pathway_summary(ui, session);
                        ui.separator();
                        draw_activity_scale(ui);
                        ui.separator();
                        draw_population_bars(ui, session);
                        ui.separator();
                        draw_top_neurons(ui, session, &mut self.selected_neuron);
                    });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:?}", behavior).to_ascii_uppercase())
                            .strong()
                            .size(18.0)
                            .color(behavior_color(behavior)),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "frame {:08}  ·  {} spikes  ·  age {} frames",
                            session.last_summary.frame,
                            session.last_summary.spike_count,
                            session.engine.state.behavior_age_frames
                        ))
                        .monospace()
                        .color(MUTED),
                    );
                    if self.paused {
                        ui.colored_label(
                            YELLOW,
                            "VIEW PAUSED — MODEL CONTINUES; NO LIVE AUTHORITY",
                        );
                    }
                });
                ui.add_space(4.0);
                draw_brain(ui, session, self.show_context, &mut self.selected_neuron);
            });

        commands
    }
}

#[derive(Deserialize)]
struct BrainAtlas {
    classes: Vec<String>,
    points: Vec<[f32; 4]>,
}

fn atlas() -> &'static BrainAtlas {
    static ATLAS: OnceLock<BrainAtlas> = OnceLock::new();
    ATLAS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/brain_points.json"))
            .expect("embedded FlyWire context points must parse")
    })
}

fn draw_brain(
    ui: &mut egui::Ui,
    session: &SimulationSession,
    show_context: bool,
    selected: &mut usize,
) {
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, Color32::from_rgb(5, 5, 10));
    painter.rect_stroke(rect, 5.0, Stroke::new(1.0, GRID), StrokeKind::Inside);
    painter.text(
        rect.left_top() + Vec2::new(9.0, 8.0),
        Align2::LEFT_TOP,
        "L",
        FontId::monospace(13.0),
        MUTED,
    );
    painter.text(
        rect.right_top() + Vec2::new(-9.0, 8.0),
        Align2::RIGHT_TOP,
        "R",
        FontId::monospace(13.0),
        MUTED,
    );

    let brain_rect = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width() * 0.88, rect.height() * 0.86),
    );
    if show_context {
        let context_colors = [
            Color32::from_rgba_unmultiplied(77, 149, 183, 36),
            Color32::from_rgba_unmultiplied(98, 91, 180, 34),
            Color32::from_rgba_unmultiplied(66, 188, 174, 32),
            Color32::from_rgba_unmultiplied(126, 87, 187, 34),
            Color32::from_rgba_unmultiplied(57, 138, 176, 32),
            Color32::from_rgba_unmultiplied(80, 196, 149, 38),
            Color32::from_rgba_unmultiplied(182, 121, 72, 34),
            Color32::from_rgba_unmultiplied(201, 170, 74, 34),
            Color32::from_rgba_unmultiplied(152, 83, 157, 32),
        ];
        for point in atlas().points.iter().step_by(2) {
            let position = Pos2::new(
                brain_rect.center().x + point[0] / 8.4 * brain_rect.width() * 0.48,
                brain_rect.center().y + point[1] / 4.5 * brain_rect.height() * 0.47,
            );
            let class = point[3] as usize % context_colors.len();
            painter.circle_filled(position, 1.0, context_colors[class]);
        }
    }

    let count = session.graph.positions.len();
    let stride = (count / 4_200).max(1);
    let state = &session.engine.state;
    let mut selected_position = None;
    for index in (0..count).step_by(stride) {
        let [x, y] = session.graph.positions[index];
        let position = Pos2::new(
            brain_rect.center().x + x * brain_rect.width() * 0.43,
            brain_rect.center().y + y * brain_rect.height() * 0.42,
        );
        let activation = state.activation[index];
        let spiked = state.spikes[index] != 0;
        let color = if spiked { YELLOW } else { viridis(activation) };
        painter.circle_filled(position, if spiked { 2.3 } else { 1.25 }, color);
        if index == *selected {
            selected_position = Some(position);
        }
    }
    if let Some(position) = selected_position {
        painter.circle_stroke(position, 7.0, Stroke::new(1.5, Color32::WHITE));
    }
    for index in (2..count.min(200)).step_by(45) {
        let [x, y] = session.graph.positions[index];
        let position = Pos2::new(
            brain_rect.center().x + x * brain_rect.width() * 0.43,
            brain_rect.center().y + y * brain_rect.height() * 0.42,
        );
        painter.circle_stroke(position, 5.0, Stroke::new(1.1, CYAN));
    }

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let mut nearest = (*selected, f32::INFINITY);
        for index in (0..count).step_by(stride) {
            let [x, y] = session.graph.positions[index];
            let position = Pos2::new(
                brain_rect.center().x + x * brain_rect.width() * 0.43,
                brain_rect.center().y + y * brain_rect.height() * 0.42,
            );
            let distance = position.distance_sq(pointer);
            if distance < nearest.1 {
                nearest = (index, distance);
            }
        }
        *selected = nearest.0;
    }
}

fn draw_pathway_summary(ui: &mut egui::Ui, session: &SimulationSession) {
    heading(ui, "MODEL PATHWAY SUMMARY");
    key_value(ui, "Session", session.short_session_id());
    key_value(ui, "State", &session.live_digest()[..12]);
    key_value(ui, "Product", &session.graph.identity.product);
    key_value(ui, "Dataset", &session.graph.identity.dataset);
    key_value(ui, "Snapshot", &session.graph.identity.snapshot);
    key_value(
        ui,
        "Graph",
        &format!(
            "{} neurons / {} edges",
            session.graph.identity.neuron_count, session.graph.identity.edge_count
        ),
    );
    key_value(ui, "Step", &format!("{:.2} ms", session.last_step_ms));
    key_value(ui, "Backend", session.assessment.selected.label());
}

fn draw_activity_scale(ui: &mut egui::Ui) {
    heading(ui, "RELATIVE SPIKE ACTIVITY");
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
    let painter = ui.painter();
    for index in 0..80 {
        let t = index as f32 / 79.0;
        let cell = Rect::from_min_max(
            Pos2::new(rect.left() + rect.width() * t, rect.top()),
            Pos2::new(
                rect.left() + rect.width() * (index as f32 + 1.0) / 80.0,
                rect.bottom(),
            ),
        );
        painter.rect_filled(cell, 0.0, viridis(((t * 2.0 - 1.0) * 32_767.0) as i32));
    }
    ui.horizontal(|ui| {
        ui.small("quiet");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small("spike");
        });
    });
}

const POPULATIONS: [&str; 9] = [
    "LC4 loom",
    "LPLC2 loom",
    "GF escape",
    "DNA steer",
    "MDN reverse",
    "DNP09 walk",
    "DNG11 groom",
    "ESCw wing",
    "Landing",
];

fn draw_population_bars(ui: &mut egui::Ui, session: &SimulationSession) {
    heading(ui, "FUNCTIONAL POPULATIONS");
    let state = &session.engine.state;
    for (group, label) in POPULATIONS.iter().enumerate() {
        let mut total = 0_i64;
        let mut count = 0_i64;
        for (index, value) in state.activation.iter().enumerate() {
            if index % POPULATIONS.len() == group {
                total += (*value).max(0) as i64;
                count += 1;
            }
        }
        let fraction = (total as f32 / count.max(1) as f32 / 16_000.0).clamp(0.0, 1.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [82.0, 16.0],
                egui::Label::new(egui::RichText::new(*label).small().color(MUTED)),
            );
            ui.add(
                egui::ProgressBar::new(fraction)
                    .desired_width(ui.available_width())
                    .fill(if group >= 6 { VIOLET } else { GREEN })
                    .show_percentage(),
            );
        });
    }
}

fn draw_top_neurons(ui: &mut egui::Ui, session: &SimulationSession, selected: &mut usize) {
    heading(ui, "TOP ACTIVE NEURONS");
    let mut active: Vec<_> = session
        .engine
        .state
        .activation
        .iter()
        .enumerate()
        .map(|(index, activation)| (index, *activation))
        .collect();
    active.sort_unstable_by_key(|(_, activation)| std::cmp::Reverse(*activation));
    for (rank, (index, activation)) in active.into_iter().take(8).enumerate() {
        if ui
            .selectable_label(
                *selected == index,
                egui::RichText::new(format!(
                    "{:>2}. {:<8} #{:<7} {:+6}",
                    rank + 1,
                    POPULATIONS[index % POPULATIONS.len()],
                    session.graph.neuron_ids[index],
                    activation
                ))
                .monospace(),
            )
            .clicked()
        {
            *selected = index;
        }
    }
    ui.add_space(5.0);
    heading(ui, "SELECTED NEURON");
    let index = (*selected).min(session.graph.neuron_ids.len().saturating_sub(1));
    key_value(ui, "Index", &index.to_string());
    key_value(ui, "Root ID", &session.graph.neuron_ids[index].to_string());
    key_value(ui, "Role", POPULATIONS[index % POPULATIONS.len()]);
    key_value(
        ui,
        "Activation",
        &session.engine.state.activation[index].to_string(),
    );
    key_value(
        ui,
        "Spiked",
        if session.engine.state.spikes[index] != 0 {
            "yes"
        } else {
            "no"
        },
    );
}

fn draw_raster(ui: &mut egui::Ui, session: &SimulationSession, history_floor: u64) {
    heading(ui, "SPIKE RASTER  ·  LAST 5 SECONDS");
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ui.available_height() - 24.0),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(5, 5, 10));
    let frames: Vec<_> = session
        .replay
        .frames()
        .filter(|frame| frame.state.frame >= history_floor)
        .collect();
    if frames.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "history cleared — waiting for retained frames",
            FontId::monospace(11.0),
            MUTED,
        );
        return;
    }
    for row in 0_usize..54 {
        let y = rect.top() + (row as f32 + 0.5) / 54.0 * rect.height();
        if row.is_multiple_of(6) {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(0.5, GRID),
            );
        }
    }
    let frame_count = frames.len();
    for (column, frame) in frames.into_iter().enumerate() {
        let x = rect.left() + (column as f32 + 0.5) / frame_count as f32 * rect.width();
        let stride = (frame.state.spikes.len() / 1_300).max(1);
        for index in (0..frame.state.spikes.len()).step_by(stride) {
            if frame.state.spikes[index] == 0 {
                continue;
            }
            let row = (index * 37 + index / 11) % 54;
            let y = rect.top() + (row as f32 + 0.5) / 54.0 * rect.height();
            painter.circle_filled(Pos2::new(x, y), 0.9, viridis(30_000));
        }
    }
}

fn viridis(activation: i32) -> Color32 {
    let t = ((activation as f32 / 32_768.0) * 0.5 + 0.5).clamp(0.0, 1.0);
    let stops = [
        (68.0, 1.0, 84.0),
        (59.0, 82.0, 139.0),
        (33.0, 145.0, 140.0),
        (94.0, 201.0, 98.0),
        (253.0, 231.0, 37.0),
    ];
    let scaled = t * (stops.len() - 1) as f32;
    let index = scaled.floor() as usize;
    let next = (index + 1).min(stops.len() - 1);
    let fraction = scaled - index as f32;
    let (r0, g0, b0) = stops[index];
    let (r1, g1, b1) = stops[next];
    Color32::from_rgb(
        (r0 + (r1 - r0) * fraction) as u8,
        (g0 + (g1 - g0) * fraction) as u8,
        (b0 + (b1 - b0) * fraction) as u8,
    )
}

fn behavior_color(behavior: Behavior) -> Color32 {
    match behavior {
        Behavior::PreEscape | Behavior::Flight => Color32::from_rgb(255, 105, 72),
        Behavior::Groom => VIOLET,
        Behavior::Landing => GREEN,
        Behavior::Walk | Behavior::Reverse => YELLOW,
        _ => CYAN,
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(10.5).color(CYAN));
}

fn key_value(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).small().color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).small().color(TEXT));
        });
    });
}

fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, GRID))
        .inner_margin(egui::Margin::same(8))
}

fn style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL_RAISED;
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(39, 32, 53);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 44, 77);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(78, 53, 101);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    ctx.set_style_of(egui::Theme::Dark, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_context_is_dense_and_labeled() {
        assert!(atlas().points.len() >= 23_000);
        assert!(atlas().classes.len() >= 8);
    }

    #[test]
    fn every_modeled_population_has_a_visible_lane() {
        assert_eq!(POPULATIONS.len(), 9);
        assert!(POPULATIONS.contains(&"GF escape"));
        assert!(POPULATIONS.contains(&"DNG11 groom"));
        assert!(POPULATIONS.contains(&"Landing"));
    }
}
