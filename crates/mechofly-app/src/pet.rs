use std::str::FromStr;

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use mechofly_core::Behavior;
use serde::{Deserialize, Serialize};

pub const PET_WIDTH: usize = 340;
pub const PET_HEIGHT: usize = 220;
const SCENE_CENTER_X: f32 = PET_WIDTH as f32 * 0.5;
const RASTER_SCALE: usize = 2;

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
            Self::Firefly => "Firefly Lantern",
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
    pub fn advance(
        &mut self,
        dt: f32,
        behavior: Behavior,
        screen_origin: Pos2,
        screen_size: Vec2,
        held: bool,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        if self.paused || held {
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
        let width = screen_size.x.max(480.0);
        let height = screen_size.y.max(320.0);
        let left = screen_origin.x + 8.0;
        let right = screen_origin.x + width - PET_WIDTH as f32 - 8.0;
        if self.screen_position.x < left {
            self.screen_position.x = left;
            self.facing = 1.0;
        } else if self.screen_position.x > right {
            self.screen_position.x = right;
            self.facing = -1.0;
        }
        self.screen_position.y = self.screen_position.y.clamp(
            screen_origin.y + 8.0,
            (screen_origin.y + height - PET_HEIGHT as f32 - 8.0).max(screen_origin.y + 8.0),
        );
    }
}

#[derive(Clone, Copy)]
struct Rgba(u8, u8, u8, u8);

impl Rgba {
    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(red, green, blue, 255)
    }

    fn egui(self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.0, self.1, self.2, self.3)
    }
}

enum Primitive {
    Ellipse {
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: Rgba,
    },
    Polygon {
        points: Vec<[f32; 2]>,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
}

struct SceneBuilder {
    facing: f32,
    offset_y: f32,
    primitives: Vec<Primitive>,
}

impl SceneBuilder {
    fn new(facing: f32, offset_y: f32) -> Self {
        Self {
            facing: if facing < 0.0 { -1.0 } else { 1.0 },
            offset_y,
            primitives: Vec::with_capacity(96),
        }
    }

    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        let x = if self.facing < 0.0 {
            SCENE_CENTER_X * 2.0 - x
        } else {
            x
        };
        [x, y + self.offset_y]
    }

    fn ellipse(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    ) {
        self.primitives.push(Primitive::Ellipse {
            center: self.point(center[0], center[1]),
            radii,
            angle: angle * self.facing,
            fill,
            stroke,
        });
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: Rgba) {
        self.primitives.push(Primitive::Line {
            from: self.point(from[0], from[1]),
            to: self.point(to[0], to[1]),
            width,
            color,
        });
    }

    fn polygon(&mut self, points: &[[f32; 2]], fill: Rgba, stroke: Option<(Rgba, f32)>) {
        let points = points
            .iter()
            .map(|point| self.point(point[0], point[1]))
            .collect();
        self.primitives.push(Primitive::Polygon {
            points,
            fill,
            stroke,
        });
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
    let scene = pet_scene(skin, behavior, phase, facing, reduced_motion);
    let scale = (rect.width() / PET_WIDTH as f32).min(rect.height() / PET_HEIGHT as f32);
    let origin = rect.center() - Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * scale * 0.5;
    let position = |point: [f32; 2]| origin + Vec2::new(point[0], point[1]) * scale;

    for primitive in scene {
        match primitive {
            Primitive::Ellipse {
                center,
                radii,
                angle,
                fill,
                stroke,
            } => {
                let points = ellipse_points(center, radii, angle, 48)
                    .into_iter()
                    .map(position)
                    .collect();
                painter.add(Shape::convex_polygon(
                    points,
                    fill.egui(),
                    stroke
                        .map(|(color, width)| Stroke::new(width * scale, color.egui()))
                        .unwrap_or(Stroke::NONE),
                ));
            }
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => {
                painter.line_segment(
                    [position(from), position(to)],
                    Stroke::new(width * scale, color.egui()),
                );
                painter.circle_filled(position(from), width * scale * 0.5, color.egui());
                painter.circle_filled(position(to), width * scale * 0.5, color.egui());
            }
            Primitive::Polygon {
                points,
                fill,
                stroke,
            } => {
                painter.add(Shape::convex_polygon(
                    points.into_iter().map(position).collect(),
                    fill.egui(),
                    stroke
                        .map(|(color, width)| Stroke::new(width * scale, color.egui()))
                        .unwrap_or(Stroke::NONE),
                ));
            }
        }
    }
}

/// Render the pet into the premultiplied BGRA format required by
/// `UpdateLayeredWindow` with `AC_SRC_ALPHA`.
pub fn render_pet_bgra(
    output: &mut [u8],
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    facing: f32,
    reduced_motion: bool,
) {
    assert_eq!(output.len(), PET_WIDTH * PET_HEIGHT * 4);
    let scene = pet_scene(skin, behavior, phase, facing, reduced_motion);
    let mut canvas = RasterCanvas::new(PET_WIDTH * RASTER_SCALE, PET_HEIGHT * RASTER_SCALE);
    for primitive in &scene {
        canvas.draw(primitive, RASTER_SCALE as f32);
    }
    canvas.downsample(output, RASTER_SCALE);
}

fn pet_scene(
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    facing: f32,
    reduced_motion: bool,
) -> Vec<Primitive> {
    let animated = !reduced_motion;
    let gait = if animated && matches!(behavior, Behavior::Walk | Behavior::Reverse) {
        (phase * 9.0).sin()
    } else {
        0.0
    };
    let offset_y = match behavior {
        Behavior::Walk | Behavior::Reverse if animated => (phase * 9.0).sin().abs() * -2.2,
        Behavior::Flight | Behavior::PreEscape if animated => -12.0 + (phase * 5.0).sin() * 4.0,
        Behavior::Landing => 3.0,
        _ => 0.0,
    };
    let mut scene = SceneBuilder::new(facing, offset_y);

    scene.ellipse(
        [165.0, 187.0],
        [105.0, 8.0],
        0.0,
        Rgba(
            5,
            12,
            16,
            if matches!(behavior, Behavior::Flight) {
                10
            } else {
                38
            },
        ),
        None,
    );

    let leg = match skin {
        Skin::Drosophila => Rgba::rgb(83, 55, 42),
        Skin::Firefly => Rgba::rgb(31, 50, 43),
    };
    draw_legs(&mut scene, behavior, gait, leg);
    draw_wings(&mut scene, skin, behavior, phase, animated);

    match skin {
        Skin::Drosophila => draw_drosophila_scene(&mut scene),
        Skin::Firefly => draw_firefly_scene(&mut scene, behavior),
    }

    draw_front_details(&mut scene, skin, behavior, phase, animated, leg);
    scene.primitives
}

fn draw_legs(scene: &mut SceneBuilder, behavior: Behavior, gait: f32, color: Rgba) {
    let grooming = behavior == Behavior::Groom;
    let pairs = [
        (152.0, -gait * 10.0),
        (188.0, gait * 8.0),
        (216.0, -gait * 9.0),
    ];
    for (index, (anchor, swing)) in pairs.into_iter().enumerate() {
        let far = Rgba(color.0, color.1, color.2, 150);
        scene.line([anchor, 122.0], [anchor - 20.0 + swing, 157.0], 3.0, far);
        scene.line(
            [anchor - 20.0 + swing, 157.0],
            [anchor - 43.0 + swing, 178.0],
            2.2,
            far,
        );

        let front_lift = if grooming && index == 2 { -52.0 } else { 0.0 };
        scene.line(
            [anchor + 2.0, 127.0],
            [anchor + 22.0 - swing, 158.0 + front_lift],
            3.2,
            color,
        );
        scene.line(
            [anchor + 22.0 - swing, 158.0 + front_lift],
            [anchor + 44.0 - swing, 181.0 + front_lift],
            2.3,
            color,
        );
    }
}

fn draw_wings(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    animated: bool,
) {
    let flying = matches!(behavior, Behavior::Flight | Behavior::PreEscape);
    let flutter = if flying && animated {
        (phase * 24.0).sin() * 16.0
    } else {
        0.0
    };
    let (fill, edge) = match skin {
        Skin::Drosophila => (Rgba(176, 214, 224, 88), Rgba(103, 145, 157, 150)),
        Skin::Firefly => (Rgba(94, 197, 190, 62), Rgba(53, 132, 124, 145)),
    };
    if flying {
        scene.polygon(
            &[
                [194.0, 103.0],
                [161.0, 43.0 - flutter],
                [74.0, 24.0 - flutter * 0.6],
                [100.0, 91.0],
            ],
            Rgba(fill.0, fill.1, fill.2, 118),
            Some((edge, 2.0)),
        );
        scene.polygon(
            &[
                [196.0, 125.0],
                [156.0, 164.0 + flutter],
                [84.0, 193.0 + flutter * 0.4],
                [112.0, 135.0],
            ],
            Rgba(fill.0, fill.1, fill.2, 104),
            Some((edge, 1.8)),
        );
        for y in [77.0_f32, 108.0, 139.0] {
            scene.line([42.0, y], [15.0, y + 3.0], 2.0, Rgba(95, 189, 181, 105));
        }
    } else {
        scene.polygon(
            &[[197.0, 104.0], [163.0, 70.0], [86.0, 62.0], [118.0, 106.0]],
            fill,
            Some((edge, 1.7)),
        );
        scene.polygon(
            &[
                [196.0, 120.0],
                [154.0, 143.0],
                [96.0, 151.0],
                [125.0, 126.0],
            ],
            Rgba(fill.0, fill.1, fill.2, fill.3.saturating_sub(18)),
            Some((edge, 1.5)),
        );
    }
}

fn draw_firefly_scene(scene: &mut SceneBuilder, behavior: Behavior) {
    let glow_alpha = if matches!(behavior, Behavior::Alert | Behavior::PreEscape) {
        76
    } else {
        56
    };
    scene.ellipse(
        [62.0, 116.0],
        [52.0, 48.0],
        0.0,
        Rgba(204, 255, 74, glow_alpha / 4),
        None,
    );
    scene.ellipse(
        [68.0, 116.0],
        [39.0, 37.0],
        0.0,
        Rgba(211, 249, 62, glow_alpha),
        None,
    );
    scene.ellipse(
        [123.0, 116.0],
        [72.0, 34.0],
        -0.04,
        Rgba::rgb(28, 103, 66),
        Some((Rgba::rgb(13, 48, 34), 3.0)),
    );
    scene.ellipse(
        [66.0, 116.0],
        [28.0, 31.0],
        0.0,
        Rgba::rgb(199, 235, 55),
        Some((Rgba::rgb(95, 132, 25), 2.4)),
    );
    scene.ellipse(
        [190.0, 116.0],
        [39.0, 38.0],
        0.02,
        Rgba::rgb(41, 66, 48),
        Some((Rgba::rgb(14, 29, 23), 3.0)),
    );
    scene.ellipse(
        [226.0, 116.0],
        [31.0, 30.0],
        0.0,
        Rgba::rgb(198, 126, 35),
        Some((Rgba::rgb(94, 58, 20), 2.4)),
    );
    scene.ellipse(
        [243.0, 116.0],
        [18.0, 24.0],
        0.0,
        Rgba::rgb(37, 47, 38),
        Some((Rgba::rgb(13, 20, 17), 2.0)),
    );
    for x in [91.0_f32, 112.0, 133.0, 154.0] {
        scene.line([x, 88.0], [x + 1.0, 144.0], 2.0, Rgba(127, 197, 103, 190));
    }
    scene.line([87.0, 116.0], [164.0, 116.0], 1.6, Rgba(135, 208, 121, 190));
    for (x, y, size) in [
        (101.0, 102.0, 2.5),
        (116.0, 129.0, 2.0),
        (132.0, 98.0, 1.8),
        (149.0, 132.0, 2.2),
        (164.0, 106.0, 1.8),
    ] {
        scene.ellipse([x, y], [size, size], 0.0, Rgba::rgb(184, 226, 112), None);
    }
    scene.ellipse(
        [250.0, 105.0],
        [8.5, 10.0],
        -0.2,
        Rgba::rgb(224, 69, 36),
        Some((Rgba::rgb(94, 30, 20), 1.2)),
    );
    scene.ellipse(
        [250.0, 127.0],
        [8.5, 10.0],
        0.2,
        Rgba::rgb(224, 69, 36),
        Some((Rgba::rgb(94, 30, 20), 1.2)),
    );

    if behavior == Behavior::Landing {
        for (radius, alpha) in [(30.0_f32, 110_u8), (48.0, 66), (67.0, 36)] {
            scene.ellipse(
                [174.0, 187.0],
                [radius, radius * 0.16],
                0.0,
                Rgba(0, 0, 0, 0),
                Some((Rgba(194, 235, 65, alpha), 2.0)),
            );
        }
    }
}

fn draw_drosophila_scene(scene: &mut SceneBuilder) {
    scene.ellipse(
        [105.0, 116.0],
        [71.0, 33.0],
        -0.05,
        Rgba::rgb(176, 113, 45),
        Some((Rgba::rgb(82, 45, 24), 3.0)),
    );
    scene.ellipse(
        [184.0, 116.0],
        [42.0, 39.0],
        0.02,
        Rgba::rgb(92, 61, 37),
        Some((Rgba::rgb(45, 29, 22), 3.0)),
    );
    scene.ellipse(
        [229.0, 116.0],
        [33.0, 31.0],
        0.0,
        Rgba::rgb(143, 86, 43),
        Some((Rgba::rgb(72, 39, 25), 2.5)),
    );
    for x in [65.0_f32, 86.0, 107.0, 128.0, 149.0] {
        scene.line([x, 89.0], [x + 2.0, 143.0], 2.5, Rgba(87, 48, 25, 210));
    }
    scene.line([53.0, 116.0], [157.0, 116.0], 1.5, Rgba(234, 166, 75, 130));
    scene.ellipse(
        [248.0, 104.0],
        [13.0, 15.0],
        -0.16,
        Rgba::rgb(183, 43, 42),
        Some((Rgba::rgb(77, 23, 23), 1.8)),
    );
    scene.ellipse(
        [248.0, 128.0],
        [13.0, 15.0],
        0.16,
        Rgba::rgb(183, 43, 42),
        Some((Rgba::rgb(77, 23, 23), 1.8)),
    );
    for (x, y) in [
        (244.0_f32, 99.0_f32),
        (251.0, 104.0),
        (245.0, 124.0),
        (251.0, 130.0),
    ] {
        scene.ellipse([x, y], [1.7, 1.7], 0.0, Rgba::rgb(255, 177, 95), None);
    }
}

fn draw_front_details(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    animated: bool,
    leg: Rgba,
) {
    let alert = matches!(behavior, Behavior::Alert | Behavior::PreEscape);
    let sway = if animated {
        (phase * 2.4).sin() * 4.0
    } else {
        0.0
    };
    let lift = if alert { -11.0 } else { 0.0 };
    scene.line([248.0, 103.0], [279.0, 82.0 + lift + sway], 2.0, leg);
    scene.line(
        [279.0, 82.0 + lift + sway],
        [305.0, 78.0 + lift + sway],
        1.5,
        leg,
    );
    scene.line([248.0, 129.0], [280.0, 146.0 - lift - sway], 2.0, leg);
    scene.line(
        [280.0, 146.0 - lift - sway],
        [305.0, 150.0 - lift - sway],
        1.5,
        leg,
    );

    if behavior == Behavior::PreEscape {
        let cue = match skin {
            Skin::Drosophila => Rgba(255, 126, 80, 120),
            Skin::Firefly => Rgba(214, 247, 74, 120),
        };
        scene.ellipse(
            [184.0, 116.0],
            [126.0, 73.0],
            0.0,
            Rgba(0, 0, 0, 0),
            Some((cue, 2.5)),
        );
    }
}

fn ellipse_points(center: [f32; 2], radii: [f32; 2], angle: f32, count: usize) -> Vec<[f32; 2]> {
    let cosine = angle.cos();
    let sine = angle.sin();
    (0..count)
        .map(|index| {
            let theta = index as f32 / count as f32 * std::f32::consts::TAU;
            let x = theta.cos() * radii[0];
            let y = theta.sin() * radii[1];
            [
                center[0] + x * cosine - y * sine,
                center[1] + x * sine + y * cosine,
            ]
        })
        .collect()
}

struct RasterCanvas {
    width: usize,
    height: usize,
    premultiplied_bgra: Vec<u8>,
}

impl RasterCanvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            premultiplied_bgra: vec![0; width * height * 4],
        }
    }

    fn draw(&mut self, primitive: &Primitive, scale: f32) {
        match primitive {
            Primitive::Ellipse {
                center,
                radii,
                angle,
                fill,
                stroke,
            } => self.ellipse(
                [center[0] * scale, center[1] * scale],
                [radii[0] * scale, radii[1] * scale],
                *angle,
                *fill,
                stroke.map(|(color, width)| (color, width * scale)),
            ),
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => self.line(
                [from[0] * scale, from[1] * scale],
                [to[0] * scale, to[1] * scale],
                width * scale,
                *color,
            ),
            Primitive::Polygon {
                points,
                fill,
                stroke,
            } => {
                let points: Vec<_> = points
                    .iter()
                    .map(|point| [point[0] * scale, point[1] * scale])
                    .collect();
                self.polygon(&points, *fill);
                if let Some((color, width)) = stroke {
                    for index in 0..points.len() {
                        self.line(
                            points[index],
                            points[(index + 1) % points.len()],
                            width * scale,
                            *color,
                        );
                    }
                }
            }
        }
    }

    fn ellipse(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    ) {
        let extent = radii[0].max(radii[1]) + stroke.map_or(0.0, |(_, width)| width);
        let bounds = self.bounds(
            center[0] - extent,
            center[1] - extent,
            center[0] + extent,
            center[1] + extent,
        );
        let cosine = angle.cos();
        let sine = angle.sin();
        let inner =
            stroke.map(|(_, width)| (1.0 - width / radii[0].min(radii[1]).max(1.0)).max(0.0));
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                let dx = x as f32 + 0.5 - center[0];
                let dy = y as f32 + 0.5 - center[1];
                let local_x = dx * cosine + dy * sine;
                let local_y = -dx * sine + dy * cosine;
                let distance = (local_x / radii[0]).powi(2) + (local_y / radii[1]).powi(2);
                if distance <= 1.0 {
                    let color = match (stroke, inner) {
                        (Some((stroke_color, _)), Some(inner_radius))
                            if distance.sqrt() >= inner_radius =>
                        {
                            stroke_color
                        }
                        _ => fill,
                    };
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: Rgba) {
        let radius = width * 0.5;
        let bounds = self.bounds(
            from[0].min(to[0]) - radius,
            from[1].min(to[1]) - radius,
            from[0].max(to[0]) + radius,
            from[1].max(to[1]) + radius,
        );
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let length_squared = (dx * dx + dy * dy).max(0.0001);
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let t =
                    (((px - from[0]) * dx + (py - from[1]) * dy) / length_squared).clamp(0.0, 1.0);
                let nearest_x = from[0] + t * dx;
                let nearest_y = from[1] + t * dy;
                let distance_squared = (px - nearest_x).powi(2) + (py - nearest_y).powi(2);
                if distance_squared <= radius * radius {
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn polygon(&mut self, points: &[[f32; 2]], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        let min_x = points
            .iter()
            .map(|point| point[0])
            .fold(f32::INFINITY, f32::min);
        let min_y = points
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let bounds = self.bounds(min_x, min_y, max_x, max_y);
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                if point_in_polygon([x as f32 + 0.5, y as f32 + 0.5], points) {
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn bounds(&self, left: f32, top: f32, right: f32, bottom: f32) -> (usize, usize, usize, usize) {
        let x0 = left.floor().max(0.0) as usize;
        let y0 = top.floor().max(0.0) as usize;
        let x1 = right.ceil().min((self.width - 1) as f32).max(0.0) as usize;
        let y1 = bottom.ceil().min((self.height - 1) as f32).max(0.0) as usize;
        (x0.min(x1), y0.min(y1), x1, y1)
    }

    fn blend(&mut self, x: usize, y: usize, color: Rgba) {
        let index = (y * self.width + x) * 4;
        let source_alpha = color.3 as u32;
        let inverse = 255 - source_alpha;
        let source = [color.2, color.1, color.0];
        for (channel, value) in source.into_iter().enumerate() {
            let source_premultiplied = (value as u32 * source_alpha + 127) / 255;
            self.premultiplied_bgra[index + channel] = (source_premultiplied
                + self.premultiplied_bgra[index + channel] as u32 * inverse / 255)
                .min(255) as u8;
        }
        self.premultiplied_bgra[index + 3] = (source_alpha
            + self.premultiplied_bgra[index + 3] as u32 * inverse / 255)
            .min(255) as u8;
    }

    fn downsample(&self, output: &mut [u8], scale: usize) {
        for y in 0..PET_HEIGHT {
            for x in 0..PET_WIDTH {
                let output_index = (y * PET_WIDTH + x) * 4;
                for channel in 0..4 {
                    let mut total = 0_u32;
                    for sample_y in 0..scale {
                        for sample_x in 0..scale {
                            let source_x = x * scale + sample_x;
                            let source_y = y * scale + sample_y;
                            total += self.premultiplied_bgra
                                [(source_y * self.width + source_x) * 4 + channel]
                                as u32;
                        }
                    }
                    output[output_index + channel] =
                        (total / (scale * scale) as u32).min(255) as u8;
                }
            }
        }
    }
}

fn point_in_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let a = polygon[current];
        let b = polygon[previous];
        let denominator = b[1] - a[1];
        let crosses = (a[1] > point[1]) != (b[1] > point[1])
            && point[0]
                < (b[0] - a[0]) * (point[1] - a[1])
                    / if denominator.abs() < f32::EPSILON {
                        f32::EPSILON
                    } else {
                        denominator
                    }
                    + a[0];
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[derive(Clone, Debug)]
pub struct FireflyVisualSelfTest {
    pub passed: bool,
    pub opaque_pixels: usize,
    pub translucent_pixels: usize,
    pub red_eye_pixels: usize,
    pub lantern_pixels: usize,
    pub elytron_pixels: usize,
    pub rest_pixel_differences: usize,
    pub escape_wing_pixel_differences: usize,
    pub rest_escape_pixel_differences: usize,
    pub grooming_pixel_differences: usize,
}

impl FireflyVisualSelfTest {
    pub fn rest_temporal_invariant(&self) -> bool {
        self.rest_pixel_differences > 0
            && self.rest_pixel_differences < 1_200
            && self.rest_pixel_differences.saturating_mul(6) < self.escape_wing_pixel_differences
    }
}

pub fn run_firefly_visual_self_test() -> FireflyVisualSelfTest {
    let render = |behavior, phase| {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, behavior, phase, 1.0, false);
        pixels
    };
    let rest = render(Behavior::Rest, 0.0);
    let rest_later = render(Behavior::Rest, 2.0);
    let escape_first = render(Behavior::PreEscape, 0.04);
    let escape_later = render(Behavior::PreEscape, 0.11);
    let groom = render(Behavior::Groom, 0.35);

    let mut result = FireflyVisualSelfTest {
        passed: false,
        opaque_pixels: 0,
        translucent_pixels: 0,
        red_eye_pixels: 0,
        lantern_pixels: 0,
        elytron_pixels: 0,
        rest_pixel_differences: pixel_difference(&rest, &rest_later),
        escape_wing_pixel_differences: pixel_difference(&escape_first, &escape_later),
        rest_escape_pixel_differences: pixel_difference(&rest, &escape_first),
        grooming_pixel_differences: pixel_difference(&rest, &groom),
    };
    for pixel in rest.as_chunks::<4>().0 {
        let alpha = pixel[3] as u32;
        if alpha == 0 {
            continue;
        }
        if alpha >= 220 {
            result.opaque_pixels += 1;
        } else {
            result.translucent_pixels += 1;
        }
        let unpremultiply = |channel: u8| (channel as u32 * 255 / alpha).min(255) as u8;
        let red = unpremultiply(pixel[2]);
        let green = unpremultiply(pixel[1]);
        let blue = unpremultiply(pixel[0]);
        if red > 145 && red > green.saturating_add(35) && red > blue.saturating_add(20) {
            result.red_eye_pixels += 1;
        }
        if green > 145 && red > 105 && green > blue.saturating_add(30) {
            result.lantern_pixels += 1;
        }
        if alpha >= 190 && green > red.saturating_add(18) && green > 55 && blue < 150 {
            result.elytron_pixels += 1;
        }
    }
    result.passed = result.opaque_pixels > 1_200
        && result.translucent_pixels > 180
        && result.red_eye_pixels > 20
        && result.lantern_pixels > 45
        && result.elytron_pixels > 280
        && result.rest_temporal_invariant()
        && result.escape_wing_pixel_differences > 180
        && result.rest_escape_pixel_differences > 900
        && result.grooming_pixel_differences > 90;
    result
}

fn pixel_difference(first: &[u8], second: &[u8]) -> usize {
    first
        .as_chunks::<4>()
        .0
        .iter()
        .zip(second.as_chunks::<4>().0)
        .filter(|(first, second)| *first != *second)
        .count()
}

pub fn transparent_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin::same(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_facing_places_head_ahead_of_lantern() {
        let scene = pet_scene(Skin::Firefly, Behavior::Rest, 0.0, 1.0, true);
        let centers: Vec<_> = scene
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Ellipse { center, .. } => Some(center[0]),
                _ => None,
            })
            .collect();
        assert!(centers.iter().any(|x| *x < SCENE_CENTER_X - 90.0));
        assert!(centers.iter().any(|x| *x > SCENE_CENTER_X + 70.0));
    }

    #[test]
    fn motion_keeps_pet_inside_virtual_screen() {
        let mut motion = PetMotion {
            screen_position: Pos2::new(9_999.0, 9_999.0),
            ..PetMotion::default()
        };
        motion.advance(
            0.1,
            Behavior::Walk,
            Pos2::new(-1_920.0, 0.0),
            Vec2::new(3_840.0, 1_080.0),
            false,
        );
        assert!(motion.screen_position.x <= 1_920.0 - PET_WIDTH as f32 - 8.0);
        assert!(motion.screen_position.y <= 1_080.0 - PET_HEIGHT as f32 - 8.0);
    }

    #[test]
    fn walking_motion_is_time_based_across_refresh_rates() {
        let mut at_30_hz = PetMotion::default();
        let mut at_120_hz = PetMotion::default();
        let start_x = at_30_hz.screen_position.x;
        let origin = Pos2::ZERO;
        let screen = Vec2::new(3_840.0, 2_160.0);

        for _ in 0..60 {
            at_30_hz.advance(1.0 / 30.0, Behavior::Walk, origin, screen, false);
        }
        for _ in 0..240 {
            at_120_hz.advance(1.0 / 120.0, Behavior::Walk, origin, screen, false);
        }

        assert!((at_30_hz.screen_position.x - at_120_hz.screen_position.x).abs() < 0.1);
        assert!((at_30_hz.animation_seconds - at_120_hz.animation_seconds).abs() < 0.001);
        assert!((at_30_hz.screen_position.x - start_x - 108.0).abs() < 0.1);
    }

    #[test]
    fn restored_firefly_uses_the_accepted_desktop_scale() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, Behavior::Rest, 0.0, 1.0, false);

        let mut minimum_x = PET_WIDTH;
        let mut maximum_x = 0;
        let mut minimum_y = PET_HEIGHT;
        let mut maximum_y = 0;
        for y in 0..PET_HEIGHT {
            for x in 0..PET_WIDTH {
                if pixels[(y * PET_WIDTH + x) * 4 + 3] > 12 {
                    minimum_x = minimum_x.min(x);
                    maximum_x = maximum_x.max(x);
                    minimum_y = minimum_y.min(y);
                    maximum_y = maximum_y.max(y);
                }
            }
        }

        assert!(maximum_x.saturating_sub(minimum_x) >= 220);
        assert!(maximum_y.saturating_sub(minimum_y) >= 105);
    }

    #[test]
    fn layered_bitmap_has_real_holes_and_premultiplied_color() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(
            &mut pixels,
            Skin::Firefly,
            Behavior::Flight,
            0.25,
            1.0,
            false,
        );
        let mut transparent = 0;
        let mut visible = 0;
        for pixel in pixels.as_chunks::<4>().0 {
            let alpha = pixel[3];
            if alpha == 0 {
                transparent += 1;
            } else {
                visible += 1;
                assert!(pixel[0] <= alpha && pixel[1] <= alpha && pixel[2] <= alpha);
            }
        }
        assert!(transparent > PET_WIDTH * PET_HEIGHT / 3);
        assert!(visible > PET_WIDTH * PET_HEIGHT / 20);
    }

    #[test]
    fn firefly_prism_visual_contract_is_colorful_and_behavior_responsive() {
        let result = run_firefly_visual_self_test();
        assert!(result.passed, "{result:#?}");

        if let Some(path) = std::env::var_os("MECHOFLY_VISUAL_FIXTURE") {
            let mut rest = vec![0; PET_WIDTH * PET_HEIGHT * 4];
            render_pet_bgra(&mut rest, Skin::Firefly, Behavior::Rest, 0.0, 1.0, false);
            write_pam(std::path::Path::new(&path), &rest);
        }
    }

    fn write_pam(path: &std::path::Path, pixels: &[u8]) {
        let mut bytes = format!(
            "P7\nWIDTH {PET_WIDTH}\nHEIGHT {PET_HEIGHT}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
        )
        .into_bytes();
        for pixel in pixels.as_chunks::<4>().0 {
            let alpha = pixel[3] as u32;
            if alpha == 0 {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let unpremultiply = |channel: u8| (channel as u32 * 255 / alpha).min(255) as u8;
                bytes.extend_from_slice(&[
                    unpremultiply(pixel[2]),
                    unpremultiply(pixel[1]),
                    unpremultiply(pixel[0]),
                    pixel[3],
                ]);
            }
        }
        std::fs::write(path, bytes).expect("visual fixture must be writable");
    }
}
