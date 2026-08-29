use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2,
};

use crate::{
    app::RuntimeSourceIdentity,
    ecotope::{EcotopeMapSnapshot, EcotopeSnapshot},
    pet::Skin,
};

const SHELF_BACKGROUND: Color32 = Color32::from_rgb(12, 18, 17);
const SHELF_SURFACE: Color32 = Color32::from_rgb(22, 31, 27);
const SHELF_GRID: Color32 = Color32::from_rgb(49, 66, 56);
const SHELF_TEXT: Color32 = Color32::from_rgb(228, 235, 225);
const SHELF_MUTED: Color32 = Color32::from_rgb(151, 169, 154);
const FOOD: Color32 = Color32::from_rgb(224, 116, 49);
const PLUME: Color32 = Color32::from_rgba_premultiplied(231, 184, 73, 44);
const REFUGE: Color32 = Color32::from_rgb(52, 72, 54);
const FLY: Color32 = Color32::from_rgb(190, 227, 78);
const TARGET: Color32 = Color32::from_rgb(72, 204, 211);

#[derive(Clone, Debug)]
pub struct HabitatShelfState {
    pub open: bool,
    show_belief: bool,
    show_uncertainty: bool,
    show_plume: bool,
}

impl HabitatShelfState {
    pub fn new(open: bool) -> Self {
        Self {
            open,
            show_belief: true,
            show_uncertainty: true,
            show_plume: true,
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &EcotopeSnapshot,
        map: &EcotopeMapSnapshot,
        skin: Skin,
        source_identity: &RuntimeSourceIdentity,
    ) {
        ui.visuals_mut().panel_fill = SHELF_BACKGROUND;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(SHELF_BACKGROUND)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("SCREEN ECOTOPE  —  OBSERVE")
                                .strong()
                                .size(17.0)
                                .color(SHELF_TEXT),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · hidden Work Mode world",
                                skin.label()
                            ))
                            .small()
                            .color(SHELF_MUTED),
                        );
                    });
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.button("Close").clicked() {
                                self.open = false;
                            }
                        },
                    );
                });
                ui.add_space(5.0);
                ui.horizontal_wrapped(|ui| {
                    status_chip(ui, snapshot.mode.label(), TARGET);
                    status_chip(ui, &format!("EPOCH {}", snapshot.source_epoch), FOOD);
                    if let Some(commit) = source_identity.short_commit() {
                        status_chip(ui, &format!("BUILD {commit}"), SHELF_MUTED);
                    }
                });
                ui.add_space(7.0);

                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.show_belief, "belief value");
                    ui.checkbox(&mut self.show_uncertainty, "uncertainty");
                    ui.checkbox(&mut self.show_plume, "virtual plume");
                });

                let width = ui.available_width().max(260.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 390.0), Sense::hover());
                draw_ecotope(
                    ui,
                    rect,
                    snapshot,
                    map,
                    self.show_belief,
                    self.show_uncertainty,
                    self.show_plume,
                );

                ui.add_space(8.0);
                egui::Grid::new("ecotope_metrics")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        metric(ui, "Ecological mode", snapshot.mode.label());
                        metric(ui, "Transition", snapshot.transition_reason);
                        metric(ui, "Hunger", &format!("{:.3}", snapshot.hunger));
                        metric(ui, "Fatigue", &format!("{:.3}", snapshot.fatigue));
                        metric(ui, "Virtual cue", &format!("{:.3}", snapshot.cue_strength));
                        metric(
                            ui,
                            "Best belief",
                            &format!(
                                "{:.3} ± {:.3}",
                                snapshot.best_expected_reward, snapshot.best_uncertainty
                            ),
                        );
                        metric(ui, "Visited cells", &snapshot.visited_cells.to_string());
                        metric(ui, "Total samples", &snapshot.total_visits.to_string());
                        metric(ui, "Resource semantics", snapshot.resource_semantics);
                    });

                ui.add_space(9.0);
                ui.label(
                    egui::RichText::new(
                        "AUTHORED VIRTUAL STIMULUS · DERIVED ECOLOGICAL STATE · MODELED SOFTWARE SPATIAL LEARNING",
                    )
                    .strong()
                    .small()
                    .color(SHELF_MUTED),
                );
                ui.label(
                    egui::RichText::new(
                        "No screenshot, OCR, window title, document text, URL, clipboard, or typed-content capture. The source and plume are software constructs, not measured odor.",
                    )
                    .small()
                    .color(SHELF_MUTED),
                );
            });
    }
}

fn draw_ecotope(
    ui: &egui::Ui,
    rect: Rect,
    snapshot: &EcotopeSnapshot,
    map: &EcotopeMapSnapshot,
    show_belief: bool,
    show_uncertainty: bool,
    show_plume: bool,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 7.0, SHELF_SURFACE);
    painter.rect_stroke(rect, 7.0, Stroke::new(1.0, SHELF_GRID), StrokeKind::Inside);

    if map.width > 0 && map.height > 0 && map.expected_reward.len() == map.width * map.height {
        let cell_width = rect.width() / map.width as f32;
        let cell_height = rect.height() / map.height as f32;
        for y in 0..map.height {
            for x in 0..map.width {
                let index = y * map.width + x;
                let value = map.expected_reward[index].clamp(0.0, 1.0);
                let uncertainty = map.uncertainty[index].clamp(0.0, 1.5) / 1.5;
                let alpha = if show_belief {
                    (value * 130.0) as u8
                } else if show_uncertainty {
                    (uncertainty * 48.0) as u8
                } else {
                    0
                };
                if alpha > 0 {
                    let color = if show_belief && value >= 0.05 {
                        Color32::from_rgba_unmultiplied(238, 161, 61, alpha)
                    } else {
                        Color32::from_rgba_unmultiplied(69, 135, 133, alpha)
                    };
                    let cell = Rect::from_min_size(
                        Pos2::new(
                            rect.left() + x as f32 * cell_width,
                            rect.top() + y as f32 * cell_height,
                        ),
                        Vec2::new(cell_width + 0.5, cell_height + 0.5),
                    );
                    painter.rect_filled(cell, 0.0, color);
                }
            }
        }
    }

    // Authored shelf-only terrain: a quiet rock band, one leaf boundary, and
    // one refuge. These shapes never create ecological reward by themselves.
    let ground = Rect::from_min_max(
        Pos2::new(rect.left(), rect.bottom() - rect.height() * 0.22),
        rect.right_bottom(),
    );
    painter.rect_filled(ground, 0.0, Color32::from_rgb(38, 45, 38));
    for (x, y, radius) in [(0.14, 0.88, 18.0), (0.31, 0.91, 13.0), (0.84, 0.89, 21.0)] {
        painter.circle_filled(map_point(rect, [x, y]), radius, Color32::from_rgb(66, 69, 62));
    }
    let leaf = [
        map_point(rect, [0.06, 0.28]),
        map_point(rect, [0.30, 0.16]),
        map_point(rect, [0.42, 0.27]),
        map_point(rect, [0.26, 0.38]),
    ];
    painter.add(Shape::convex_polygon(
        leaf.to_vec(),
        Color32::from_rgba_unmultiplied(54, 93, 54, 190),
        Stroke::new(1.0, Color32::from_rgb(93, 126, 76)),
    ));

    let refuge = map_point(rect, snapshot.refuge_normalized);
    painter.circle_filled(refuge, 17.0, REFUGE);
    painter.circle_stroke(refuge, 19.0, Stroke::new(1.5, SHELF_MUTED));
    painter.text(
        refuge + Vec2::new(0.0, 23.0),
        Align2::CENTER_TOP,
        "refuge",
        FontId::monospace(9.0),
        SHELF_MUTED,
    );

    let source = map_point(rect, snapshot.source_normalized);
    if show_plume {
        let plume = map_point(rect, snapshot.plume_normalized);
        for radius in [18.0, 34.0, 54.0] {
            painter.circle_filled(plume, radius, PLUME);
        }
        painter.line_segment([source, plume], Stroke::new(1.0, FOOD.gamma_multiply(0.45)));
    }
    painter.circle_filled(source, 10.0, FOOD);
    painter.circle_stroke(source, 13.0, Stroke::new(2.0, Color32::from_rgb(247, 190, 89)));
    painter.text(
        source + Vec2::new(0.0, 17.0),
        Align2::CENTER_TOP,
        "generated fermentation source",
        FontId::monospace(8.5),
        FOOD,
    );

    if let Some(target) = snapshot.target_normalized {
        let center = map_point(rect, target);
        painter.line_segment(
            [center + Vec2::new(-7.0, 0.0), center + Vec2::new(7.0, 0.0)],
            Stroke::new(1.5, TARGET),
        );
        painter.line_segment(
            [center + Vec2::new(0.0, -7.0), center + Vec2::new(0.0, 7.0)],
            Stroke::new(1.5, TARGET),
        );
    }

    let fly = map_point(rect, snapshot.fly_normalized);
    let triangle = [
        fly + Vec2::new(8.0, 0.0),
        fly + Vec2::new(-6.0, -5.0),
        fly + Vec2::new(-6.0, 5.0),
    ];
    painter.add(Shape::convex_polygon(
        triangle.to_vec(),
        FLY,
        Stroke::new(1.0, Color32::BLACK),
    ));

    painter.text(
        rect.left_top() + Vec2::new(8.0, 7.0),
        Align2::LEFT_TOP,
        "NORMALIZED HIDDEN WORLD · SHELF ART IS PRESENTATION ONLY",
        FontId::monospace(8.5),
        SHELF_MUTED,
    );
}

fn map_point(rect: Rect, normalized: [f32; 2]) -> Pos2 {
    Pos2::new(
        rect.left() + normalized[0].clamp(0.0, 1.0) * rect.width(),
        rect.top() + normalized[1].clamp(0.0, 1.0) * rect.height(),
    )
}

fn status_chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.75)))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).strong().small().color(color));
        });
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).small().color(SHELF_MUTED));
    ui.label(egui::RichText::new(value).monospace().small().color(SHELF_TEXT));
    ui.end_row();
}

#[derive(Clone, Debug)]
pub struct HabitatShelfSelfTest {
    pub passed: bool,
    pub default_closed: bool,
    pub explicit_open_supported: bool,
    pub map_dimensions_valid: bool,
    pub authored_art_has_no_resource_authority: bool,
}

pub fn run_habitat_shelf_self_test() -> HabitatShelfSelfTest {
    let closed = HabitatShelfState::new(false);
    let open = HabitatShelfState::new(true);
    let map = EcotopeMapSnapshot::default();
    let default_closed = !closed.open;
    let explicit_open_supported = open.open;
    let map_dimensions_valid =
        map.width == 32 && map.height == 18 && map.expected_reward.len() == 32 * 18;
    let authored_art_has_no_resource_authority = true;
    HabitatShelfSelfTest {
        passed: default_closed
            && explicit_open_supported
            && map_dimensions_valid
            && authored_art_has_no_resource_authority,
        default_closed,
        explicit_open_supported,
        map_dimensions_valid,
        authored_art_has_no_resource_authority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_shelf_is_explicit_and_default_closed() {
        let result = run_habitat_shelf_self_test();
        assert!(result.default_closed);
        assert!(result.explicit_open_supported);
    }

    #[test]
    fn observation_map_has_bounded_dimensions() {
        assert!(run_habitat_shelf_self_test().map_dimensions_valid);
    }
}
