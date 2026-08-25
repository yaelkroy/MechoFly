use std::str::FromStr;

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use mechofly_core::Behavior;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skin {
    #[default]
    Drosophila,
    Firefly,
}

impl Skin {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Drosophila => "Drosophila Natural",
            Self::Firefly => "Firefly Field",
        }
    }

    pub const fn cli(self) -> &'static str {
        match self {
            Self::Drosophila => "drosophila",
            Self::Firefly => "firefly",
        }
    }
}

impl FromStr for Skin {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "drosophila" => Ok(Self::Drosophila),
            "firefly" => Ok(Self::Firefly),
            other => Err(format!("unknown skin {other:?}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PetMotion {
    pub screen_position: Pos2,
    pub facing: f32,
    pub animation_seconds: f32,
    pub paused: bool,
    pub reduced_motion: bool,
}

impl Default for PetMotion {
    fn default() -> Self {
        Self {
            screen_position: Pos2::new(96.0, 640.0),
            facing: 1.0,
            animation_seconds: 0.0,
            paused: false,
            reduced_motion: false,
        }
    }
}

impl PetMotion {
    pub fn advance(&mut self, dt: f32, behavior: Behavior, monitor_size: Vec2, hovered: bool) {
        let dt = dt.clamp(0.0, 0.1);
        if self.paused || hovered {
            return;
        }
        self.animation_seconds += dt;
        let speed = match behavior {
            Behavior::Walk => 54.0,
            Behavior::Reverse => -32.0,
            Behavior::Flight => 128.0,
            Behavior::Landing => 26.0,
            _ => 0.0,
        };
        self.screen_position.x += speed * self.facing * dt;
        if behavior == Behavior::Flight && !self.reduced_motion {
            self.screen_position.y += (self.animation_seconds * 8.0).sin() * 42.0 * dt;
        }
        let width = monitor_size.x.max(480.0);
        let height = monitor_size.y.max(320.0);
        if self.screen_position.x < 8.0 {
            self.screen_position.x = 8.0;
            self.facing = 1.0;
        } else if self.screen_position.x > width - 248.0 {
            self.screen_position.x = width - 248.0;
            self.facing = -1.0;
        }
        self.screen_position.y = self
            .screen_position
            .y
            .clamp(24.0, (height - 180.0).max(24.0));
    }
}

pub fn draw_pet(
    painter: &Painter,
    rect: Rect,
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    facing: f32,
    reduced_motion: bool,
) {
    let center = rect.center() + Vec2::new(0.0, -8.0);
    let scale = rect.width().min(rect.height() * 1.35) / 188.0;
    let transform = |point: [f32; 2]| {
        Pos2::new(
            center.x + point[0] * scale * facing,
            center.y + point[1] * scale,
        )
    };
    let gait = if reduced_motion { 0.0 } else { (phase * 9.0).sin() };
    let grooming = behavior == Behavior::Groom;
    let flying = matches!(behavior, Behavior::Flight | Behavior::PreEscape);

    let leg_color = match skin {
        Skin::Drosophila => Color32::from_rgb(70, 44, 34),
        Skin::Firefly => Color32::from_rgb(40, 48, 42),
    };
    for side in [-1.0_f32, 1.0] {
        for (index, anchor_x) in [-30.0_f32, 0.0, 30.0].into_iter().enumerate() {
            let swing = gait
                * (if index.is_multiple_of(2) {
                    side
                } else {
                    -side
                })
                * 7.0;
            let lift = if grooming && index == 0 { -26.0 } else { 0.0 };
            let a = transform([anchor_x, 13.0 * side]);
            let b = transform([anchor_x + swing, 35.0 * side + lift]);
            let c = transform([anchor_x + swing + 22.0, 48.0 * side + lift]);
            painter.line_segment([a, b], Stroke::new(3.0 * scale, leg_color));
            painter.line_segment([b, c], Stroke::new(2.3 * scale, leg_color));
        }
    }

    if skin == Skin::Drosophila || flying {
        let wing_alpha = if flying { 122 } else { 64 };
        let wing_lift = if flying && !reduced_motion {
            (phase * 28.0).sin() * 18.0
        } else {
            0.0
        };
        let (outer_x, outer_y) = if flying { (68.0, 42.0) } else { (54.0, 20.0) };
        for side in [-1.0_f32, 1.0] {
            let points = vec![
                transform([-18.0, side * 4.0]),
                transform([15.0, side * (26.0 + wing_lift)]),
                transform([outer_x, side * (outer_y + wing_lift)]),
                transform([36.0, side * (8.0 + wing_lift * 0.25)]),
            ];
            painter.add(Shape::convex_polygon(
                points,
                Color32::from_rgba_premultiplied(174, 210, 214, wing_alpha),
                Stroke::new(
                    1.1 * scale,
                    Color32::from_rgba_premultiplied(70, 103, 103, 120),
                ),
            ));
        }
    }

    match skin {
        Skin::Drosophila => draw_drosophila(painter, &transform, scale),
        Skin::Firefly => draw_firefly(painter, &transform, scale),
    }

    let antenna_sway = if reduced_motion { 0.0 } else { (phase * 2.3).sin() * 4.0 };
    for side in [-1.0_f32, 1.0] {
        painter.line_segment(
            [
                transform([-61.0, side * 8.0]),
                transform([-84.0, side * (19.0 + antenna_sway)]),
            ],
            Stroke::new(1.8 * scale, leg_color),
        );
    }
}

fn draw_drosophila(
    painter: &Painter,
    transform: &impl Fn([f32; 2]) -> Pos2,
    scale: f32,
) {
    painter.add(ellipse(
        transform([27.0, 0.0]),
        Vec2::new(94.0 * scale, 48.0 * scale),
        Color32::from_rgb(167, 112, 53),
        Stroke::new(2.0 * scale, Color32::from_rgb(75, 44, 26)),
    ));
    for x in [5.0_f32, 22.0, 39.0, 55.0] {
        painter.line_segment(
            [transform([x, -21.0]), transform([x, 21.0])],
            Stroke::new(1.3 * scale, Color32::from_rgb(92, 58, 30)),
        );
    }
    painter.circle_filled(transform([-24.0, 0.0]), 27.0 * scale, Color32::from_rgb(111, 69, 37));
    painter.circle_filled(transform([-57.0, 0.0]), 24.0 * scale, Color32::from_rgb(75, 47, 34));
    for side in [-1.0_f32, 1.0] {
        painter.circle_filled(
            transform([-62.0, side * 13.0]),
            10.5 * scale,
            Color32::from_rgb(166, 46, 38),
        );
        painter.circle_filled(
            transform([-65.0, side * 15.0]),
            2.0 * scale,
            Color32::from_rgb(255, 205, 112),
        );
    }
}

fn draw_firefly(
    painter: &Painter,
    transform: &impl Fn([f32; 2]) -> Pos2,
    scale: f32,
) {
    painter.add(ellipse(
        transform([23.0, 0.0]),
        Vec2::new(98.0 * scale, 52.0 * scale),
        Color32::from_rgb(40, 103, 69),
        Stroke::new(2.0 * scale, Color32::from_rgb(24, 55, 39)),
    ));
    painter.line_segment(
        [transform([-2.0, 0.0]), transform([69.0, 0.0])],
        Stroke::new(1.5 * scale, Color32::from_rgb(127, 190, 120)),
    );
    painter.add(ellipse(
        transform([65.0, 0.0]),
        Vec2::new(35.0 * scale, 46.0 * scale),
        Color32::from_rgb(196, 224, 70),
        Stroke::new(1.8 * scale, Color32::from_rgb(101, 129, 35)),
    ));
    painter.circle_filled(transform([-28.0, 0.0]), 28.0 * scale, Color32::from_rgb(194, 132, 45));
    painter.circle_filled(transform([-60.0, 0.0]), 23.0 * scale, Color32::from_rgb(50, 55, 45));
    for side in [-1.0_f32, 1.0] {
        painter.circle_filled(
            transform([-63.0, side * 13.0]),
            8.0 * scale,
            Color32::from_rgb(215, 76, 43),
        );
    }
}

fn ellipse(center: Pos2, size: Vec2, fill: Color32, stroke: Stroke) -> Shape {
    let points: Vec<Pos2> = (0..40)
        .map(|index| {
            let angle = index as f32 / 40.0 * std::f32::consts::TAU;
            center + Vec2::new(angle.cos() * size.x * 0.5, angle.sin() * size.y * 0.5)
        })
        .collect();
    Shape::convex_polygon(points, fill, stroke)
}

pub fn transparent_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin::same(0))
}
