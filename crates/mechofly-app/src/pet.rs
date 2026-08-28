use std::{f32::consts::PI, str::FromStr};

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use mechofly_core::Behavior;
use serde::{Deserialize, Serialize};

pub const PET_WIDTH: usize = 420;
pub const PET_HEIGHT: usize = 280;
const CENTER: [f32; 2] = [PET_WIDTH as f32 * 0.5, PET_HEIGHT as f32 * 0.5];
const RASTER_SCALE: usize = 2;
const REFERENCE_TICK_SECONDS: f32 = 0.033;
const WALK_SPEED_PIXELS_PER_SECOND: f32 = 1.85 / REFERENCE_TICK_SECONDS;
const REVERSE_SPEED_PIXELS_PER_SECOND: f32 = -2.2 / REFERENCE_TICK_SECONDS;
const ESCAPE_SPEED_PIXELS_PER_SECOND: f32 = 18.0 / REFERENCE_TICK_SECONDS;
const FLIGHT_SPEED_PIXELS_PER_SECOND: f32 = 8.4 / REFERENCE_TICK_SECONDS;
const LANDING_SPEED_PIXELS_PER_SECOND: f32 = 3.8 / REFERENCE_TICK_SECONDS;
const LANDING_CONTACT_SPEED_PIXELS_PER_SECOND: f32 = 0.8 / REFERENCE_TICK_SECONDS;
const NERVOUS_SPEED_PIXELS_PER_SECOND: f32 = 3.0 / REFERENCE_TICK_SECONDS;
const LANDING_COMPLETION_SECONDS: f32 = 0.495;

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
            Self::Firefly => "MechoFly Prism",
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
            // Preserve the historical CLI value while restoring Prism.
            "firefly" | "prism" => Ok(Self::Firefly),
            other => Err(format!("unknown skin {other:?}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PetMotion {
    pub screen_position: Pos2,
    /// Direction in screen coordinates. Zero points right; positive turns down.
    pub heading_radians: f32,
    speed_pixels_per_second: f32,
    pub animation_seconds: f32,
    pub behavior_age_seconds: f32,
    last_behavior: Behavior,
    pub paused: bool,
    pub reduced_motion: bool,
}

impl Default for PetMotion {
    fn default() -> Self {
        Self {
            screen_position: Pos2::new(96.0, 640.0),
            heading_radians: 0.0,
            speed_pixels_per_second: 0.0,
            animation_seconds: 0.0,
            behavior_age_seconds: 0.0,
            last_behavior: Behavior::Rest,
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
        cursor_position: Option<Pos2>,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        if behavior != self.last_behavior {
            self.last_behavior = behavior;
            self.behavior_age_seconds = 0.0;
        }
        if self.paused || held {
            return;
        }
        self.animation_seconds += dt;
        self.behavior_age_seconds += dt;

        let width = screen_size.x.max(480.0);
        let height = screen_size.y.max(320.0);
        let left = screen_origin.x + 8.0;
        let top = screen_origin.y + 8.0;
        let right = (screen_origin.x + width - PET_WIDTH as f32 - 8.0).max(left);
        let bottom = (screen_origin.y + height - PET_HEIGHT as f32 - 8.0).max(top);
        let center = self.screen_position + Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * 0.5;
        if behavior == Behavior::PreEscape
            && let Some(cursor) = cursor_position
        {
            let away = center - cursor;
            if away.length_sq() > 1.0 {
                self.heading_radians = wrapped_angle(away.y.atan2(away.x));
            }
        }

        self.speed_pixels_per_second = match behavior {
            Behavior::Walk => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 0.53).sin()
                            * (0.025 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                WALK_SPEED_PIXELS_PER_SECOND
            }
            Behavior::Reverse => REVERSE_SPEED_PIXELS_PER_SECOND,
            Behavior::PreEscape => ESCAPE_SPEED_PIXELS_PER_SECOND,
            Behavior::Flight => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 1.7).sin()
                            * (0.065 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                FLIGHT_SPEED_PIXELS_PER_SECOND
            }
            Behavior::Landing => {
                let descent = (bottom - self.screen_position.y).max(0.0);
                if descent > 12.0 {
                    self.heading_radians = PI * 0.5;
                    LANDING_SPEED_PIXELS_PER_SECOND
                } else {
                    LANDING_CONTACT_SPEED_PIXELS_PER_SECOND
                }
            }
            Behavior::Alert => {
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 19.0).sin()
                            * (0.18 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                NERVOUS_SPEED_PIXELS_PER_SECOND
            }
            Behavior::Rest | Behavior::Quiet | Behavior::Groom => 0.0,
        };

        self.screen_position +=
            Vec2::angled(self.heading_radians) * self.speed_pixels_per_second * dt;
        if behavior == Behavior::Landing && self.behavior_age_seconds >= LANDING_COMPLETION_SECONDS
        {
            self.screen_position.y = bottom;
            self.heading_radians = PI * 0.5;
            self.speed_pixels_per_second = 0.0;
        }

        let mut bounced_x = false;
        let mut bounced_y = false;
        if self.screen_position.x < left {
            self.screen_position.x = left;
            bounced_x = true;
        } else if self.screen_position.x > right {
            self.screen_position.x = right;
            bounced_x = true;
        }
        if self.screen_position.y < top {
            self.screen_position.y = top;
            bounced_y = true;
        } else if self.screen_position.y > bottom {
            self.screen_position.y = bottom;
            bounced_y = true;
        }
        if bounced_x {
            self.heading_radians = wrapped_angle(PI - self.heading_radians);
        }
        if bounced_y {
            if behavior == Behavior::Landing && self.screen_position.y >= bottom {
                self.heading_radians = PI * 0.5;
                self.speed_pixels_per_second = 0.0;
            } else {
                self.heading_radians = wrapped_angle(-self.heading_radians);
            }
        }
    }
}

fn wrapped_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}

#[derive(Clone, Debug)]
pub struct MotionSelfTest {
    pub passed: bool,
    pub walking_translation_pixels: f32,
    pub escape_translation_pixels: f32,
    pub flight_path_pixels: f32,
    pub flight_horizontal_pixels: f32,
    pub flight_vertical_pixels: f32,
    pub landing_descent_pixels: f32,
    pub landing_reached_surface: bool,
}

pub fn run_motion_self_test() -> MotionSelfTest {
    let origin = Pos2::ZERO;
    let screen = Vec2::new(1_920.0, 1_080.0);
    let bottom = screen.y - PET_HEIGHT as f32 - 8.0;

    let mut walking = PetMotion {
        screen_position: Pos2::new(220.0, bottom),
        ..PetMotion::default()
    };
    let walking_start = walking.screen_position;
    for _ in 0..120 {
        walking.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
    }
    let walking_translation_pixels = walking.screen_position.distance(walking_start);

    let mut airborne = PetMotion {
        screen_position: Pos2::new(760.0, 610.0),
        ..PetMotion::default()
    };
    let cursor = Pos2::new(1_030.0, 900.0);
    let escape_start = airborne.screen_position;
    for _ in 0..12 {
        airborne.advance(
            1.0 / 60.0,
            Behavior::PreEscape,
            origin,
            screen,
            false,
            Some(cursor),
        );
    }
    let escape_translation_pixels = airborne.screen_position.distance(escape_start);

    let flight_start = airborne.screen_position;
    let mut flight_path_pixels = 0.0;
    for _ in 0..72 {
        let before = airborne.screen_position;
        airborne.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
        flight_path_pixels += airborne.screen_position.distance(before);
    }
    let flight_horizontal_pixels = (airborne.screen_position.x - flight_start.x).abs();
    let flight_vertical_pixels = (airborne.screen_position.y - flight_start.y).abs();

    airborne.screen_position.y = bottom - 48.0;
    let landing_start_y = airborne.screen_position.y;
    for _ in 0..30 {
        airborne.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
    }
    let landing_descent_pixels = airborne.screen_position.y - landing_start_y;
    let landing_reached_surface = (airborne.screen_position.y - bottom).abs() < 0.01;

    MotionSelfTest {
        passed: walking_translation_pixels > 100.0
            && escape_translation_pixels > 90.0
            && flight_path_pixels > 250.0
            && flight_horizontal_pixels > 40.0
            && flight_vertical_pixels > 20.0
            && landing_descent_pixels > 45.0
            && landing_reached_surface,
        walking_translation_pixels,
        escape_translation_pixels,
        flight_path_pixels,
        flight_horizontal_pixels,
        flight_vertical_pixels,
        landing_descent_pixels,
        landing_reached_surface,
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

type CubicSegment = ([f32; 2], [f32; 2], [f32; 2], [f32; 2]);

struct SceneBuilder {
    rotation: f32,
    screen_offset_y: f32,
    primitives: Vec<Primitive>,
}

impl SceneBuilder {
    fn new(heading: f32, screen_offset_y: f32) -> Self {
        Self {
            // The local head is at negative X, hence the half turn.
            rotation: heading + PI,
            screen_offset_y,
            primitives: Vec::with_capacity(220),
        }
    }

    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        let cosine = self.rotation.cos();
        let sine = self.rotation.sin();
        [
            CENTER[0] + x * cosine - y * sine,
            CENTER[1] + x * sine + y * cosine + self.screen_offset_y,
        ]
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
            angle: angle + self.rotation,
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

#[derive(Clone, Copy)]
struct Palette {
    outline: Rgba,
    highlight: Rgba,
    leg: Rgba,
    wing_fill: Rgba,
    wing_edge: Rgba,
    head_core: Rgba,
    head_rim: Rgba,
    eye_core: Rgba,
    eye_rim: Rgba,
    thorax_core: Rgba,
    thorax_edge: Rgba,
}

fn palette(skin: Skin) -> Palette {
    match skin {
        Skin::Firefly => Palette {
            outline: Rgba(26, 67, 45, 245),
            highlight: Rgba(185, 218, 67, 225),
            leg: Rgba(41, 91, 49, 248),
            wing_fill: Rgba(82, 96, 179, 82),
            wing_edge: Rgba(139, 99, 211, 205),
            head_core: Rgba::rgb(23, 74, 56),
            head_rim: Rgba::rgb(163, 177, 56),
            eye_core: Rgba::rgb(202, 75, 31),
            eye_rim: Rgba::rgb(240, 155, 48),
            thorax_core: Rgba::rgb(207, 89, 36),
            thorax_edge: Rgba::rgb(241, 169, 54),
        },
        Skin::Drosophila => Palette {
            outline: Rgba(63, 35, 25, 240),
            highlight: Rgba(239, 178, 93, 210),
            leg: Rgba(73, 45, 34, 240),
            wing_fill: Rgba(186, 219, 230, 70),
            wing_edge: Rgba(114, 157, 169, 178),
            head_core: Rgba::rgb(126, 76, 43),
            head_rim: Rgba::rgb(202, 133, 66),
            eye_core: Rgba::rgb(190, 42, 40),
            eye_rim: Rgba::rgb(247, 138, 70),
            thorax_core: Rgba::rgb(151, 93, 42),
            thorax_edge: Rgba::rgb(217, 148, 64),
        },
    }
}

pub fn draw_pet(
    painter: &Painter,
    rect: Rect,
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    heading: f32,
    reduced_motion: bool,
) {
    let scene = pet_scene(skin, behavior, phase, heading, reduced_motion);
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

/// Render into the premultiplied BGRA format required by UpdateLayeredWindow.
pub fn render_pet_bgra(
    output: &mut [u8],
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    heading: f32,
    reduced_motion: bool,
) {
    assert_eq!(output.len(), PET_WIDTH * PET_HEIGHT * 4);
    let scene = pet_scene(skin, behavior, phase, heading, reduced_motion);
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
    heading: f32,
    reduced_motion: bool,
) -> Vec<Primitive> {
    // Rest is deliberately frozen: no bob, wing phase, aura, or shimmer.
    let time = if behavior == Behavior::Rest || reduced_motion {
        0.0
    } else {
        phase
    };
    let gait = if matches!(behavior, Behavior::Walk | Behavior::Reverse) {
        (time * 9.0).sin()
    } else {
        0.0
    };
    let screen_offset = match behavior {
        Behavior::Walk | Behavior::Reverse => -(time * 9.0).sin().abs() * 2.0,
        Behavior::PreEscape => {
            if (time * 12.0).sin() > 0.0 {
                -2.5
            } else {
                1.5
            }
        }
        Behavior::Flight => -8.0 + (time * 5.2).sin() * 3.5,
        Behavior::Landing => 3.0 + (time * 8.0).sin().abs() * 1.5,
        _ => 0.0,
    };
    let colors = palette(skin);
    let mut scene = SceneBuilder::new(heading, screen_offset);
    draw_behavior_field(&mut scene, behavior, time, skin);
    draw_contact_shadow(&mut scene, behavior);
    draw_legs(&mut scene, behavior, gait, time, true, colors);
    draw_wings(&mut scene, behavior, time, colors);
    draw_abdomen(&mut scene, skin, behavior, time, colors);
    draw_thorax(&mut scene, skin, time, colors);
    draw_head(&mut scene, skin, behavior, time, colors);
    draw_legs(&mut scene, behavior, gait, time, false, colors);
    draw_antennae(&mut scene, behavior, time, colors);
    scene.primitives
}

fn behavior_accent(behavior: Behavior, skin: Skin) -> Rgba {
    if skin == Skin::Drosophila {
        match behavior {
            Behavior::PreEscape | Behavior::Flight => Rgba(245, 93, 55, 210),
            Behavior::Groom => Rgba(220, 144, 64, 190),
            Behavior::Landing => Rgba(104, 181, 158, 190),
            _ => Rgba(164, 117, 64, 160),
        }
    } else {
        match behavior {
            Behavior::PreEscape | Behavior::Flight => Rgba(255, 105, 72, 220),
            Behavior::Groom => Rgba(178, 108, 226, 200),
            Behavior::Landing => Rgba(63, 226, 170, 200),
            Behavior::Walk | Behavior::Reverse => Rgba(244, 190, 62, 185),
            _ => Rgba(89, 238, 226, 160),
        }
    }
}

fn draw_behavior_field(scene: &mut SceneBuilder, behavior: Behavior, time: f32, skin: Skin) {
    if !matches!(
        behavior,
        Behavior::Alert | Behavior::PreEscape | Behavior::Flight | Behavior::Landing
    ) {
        return;
    }
    let accent = behavior_accent(behavior, skin);
    let pulse = 0.5 + 0.5 * (time * 3.4).sin();
    scene.ellipse(
        [0.0, 0.0],
        [122.0 + pulse * 6.0, 91.0 + pulse * 5.0],
        0.0,
        Rgba(0, 0, 0, 0),
        Some((Rgba(accent.0, accent.1, accent.2, 42), 1.8)),
    );
    for index in 0..7 {
        let angle = time * (0.62 + index as f32 * 0.07) + index as f32 * 0.91;
        let point = [
            angle.cos() * (98.0 + index as f32 * 2.1),
            angle.sin() * (70.0 + index as f32 * 1.7),
        ];
        let size = 1.5 + (index % 3) as f32 * 0.65;
        scene.ellipse(point, [size, size], 0.0, accent, None);
    }
}

fn draw_contact_shadow(scene: &mut SceneBuilder, behavior: Behavior) {
    let alpha = match behavior {
        Behavior::Flight => 12,
        Behavior::PreEscape => 22,
        _ => 55,
    };
    scene.ellipse([10.0, 58.0], [88.0, 9.0], 0.0, Rgba(5, 13, 22, alpha), None);
}

fn draw_wings(scene: &mut SceneBuilder, behavior: Behavior, time: f32, colors: Palette) {
    if !matches!(
        behavior,
        Behavior::PreEscape | Behavior::Flight | Behavior::Landing
    ) {
        return;
    }
    let intensity = match behavior {
        Behavior::PreEscape => 0.78,
        Behavior::Flight => 1.0,
        Behavior::Landing => 0.48,
        _ => 0.0,
    };
    let frequency = if behavior == Behavior::Landing {
        18.0
    } else {
        28.0
    };
    let phase = time * frequency;
    let beat = phase.sin() * intensity;
    draw_wing(scene, -1.0, beat + 0.12 * intensity, colors, time, 0);
    draw_wing(scene, 1.0, beat - 0.12 * intensity, colors, time, 1);
}

fn draw_wing(
    scene: &mut SceneBuilder,
    side: f32,
    lift: f32,
    colors: Palette,
    time: f32,
    wing_index: usize,
) {
    let root = [-10.0, side * 9.0];
    let tip = [8.0 + lift * 15.0, side * (72.0 - lift.abs() * 5.0)];
    let outer = [46.0 + lift * 8.0, side * (52.0 + lift * 4.0)];
    let trailing = [34.0, side * 23.0];
    let segments = [
        (
            root,
            [-7.0, side * 31.0],
            [-2.0 + lift * 9.0, side * 63.0],
            tip,
        ),
        (
            tip,
            [24.0 + lift * 12.0, side * 77.0],
            [47.0 + lift * 9.0, side * 67.0],
            outer,
        ),
        (
            outer,
            [56.0 + lift * 6.0, side * 43.0],
            [48.0, side * 28.0],
            trailing,
        ),
        (trailing, [21.0, side * 20.0], [0.0, side * 12.0], root),
    ];
    let outline = cubic_loop(&segments, 7);
    let shimmer = 0.5 + 0.5 * (time * 2.7 + wing_index as f32 * 1.17).sin();
    scene.polygon(
        &outline,
        Rgba(
            colors.wing_fill.0.saturating_add((shimmer * 28.0) as u8),
            colors.wing_fill.1,
            colors.wing_fill.2,
            colors.wing_fill.3,
        ),
        Some((colors.wing_edge, 1.25)),
    );
    for fraction in [0.28_f32, 0.52, 0.76] {
        scene.line(
            root,
            mix(tip, outer, fraction),
            0.72,
            Rgba(93, 178, 145, 105),
        );
    }
    scene.line(root, trailing, 0.78, Rgba(204, 181, 74, 125));
    scene.line(
        mix(root, tip, 0.53),
        mix(trailing, outer, 0.55),
        0.65,
        Rgba(129, 104, 196, 105),
    );
    let stigma = mix(tip, outer, 0.27);
    scene.ellipse(
        stigma,
        [5.2, 1.8],
        side * 0.24,
        Rgba(225, 177, 56, 195),
        None,
    );
}

fn cubic_loop(segments: &[CubicSegment], steps: usize) -> Vec<[f32; 2]> {
    let mut points = Vec::with_capacity(segments.len() * steps);
    for &(p0, p1, p2, p3) in segments {
        for step in 0..steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            points.push([
                u.powi(3) * p0[0]
                    + 3.0 * u.powi(2) * t * p1[0]
                    + 3.0 * u * t.powi(2) * p2[0]
                    + t.powi(3) * p3[0],
                u.powi(3) * p0[1]
                    + 3.0 * u.powi(2) * t * p1[1]
                    + 3.0 * u * t.powi(2) * p2[1]
                    + t.powi(3) * p3[1],
            ]);
        }
    }
    points
}

fn mix(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn draw_abdomen(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    time: f32,
    colors: Palette,
) {
    if skin == Skin::Firefly {
        draw_prism_abdomen(scene, behavior, time, colors);
        return;
    }
    let natural: [(u8, u8, u8); 10] = [
        (178, 113, 45),
        (164, 96, 39),
        (146, 82, 34),
        (130, 72, 31),
        (115, 64, 29),
        (103, 57, 27),
        (92, 51, 26),
        (81, 46, 25),
        (70, 40, 24),
        (60, 35, 23),
    ];
    let pulse = if behavior == Behavior::Rest {
        0.0
    } else {
        0.5 + 0.5 * (time * 3.1).sin()
    };
    for (segment, &(r, g, b)) in natural.iter().enumerate() {
        let x = -5.0 + segment as f32 * 10.7;
        let width = if segment < 2 {
            13.0
        } else {
            12.0 - segment as f32 * 0.45
        };
        let height = (16.0 - segment as f32 * 1.05).max(5.2);
        scene.ellipse(
            [x + width * 0.5, 0.0],
            [width * 0.58, height * 0.5],
            0.0,
            Rgba(r, g.saturating_add((pulse * 18.0) as u8), b, 248),
            Some((colors.outline, 0.9)),
        );
        scene.line(
            [x + 2.0, -height * 0.14],
            [x + width - 2.0, height * 0.22],
            0.62,
            Rgba(
                colors.highlight.0,
                colors.highlight.1,
                colors.highlight.2,
                95,
            ),
        );
    }
    scene.polygon(
        &[[101.0, -3.2], [115.0, 0.0], [101.0, 3.2]],
        Rgba(68, 38, 24, 248),
        Some((colors.highlight, 0.8)),
    );
}

fn draw_prism_abdomen(scene: &mut SceneBuilder, behavior: Behavior, time: f32, colors: Palette) {
    let pulse = if behavior == Behavior::Rest {
        0
    } else {
        ((0.5 + 0.5 * (time * 3.1).sin()) * 16.0) as u8
    };
    scene.ellipse(
        [39.0, 0.0],
        [44.0, 18.0],
        0.0,
        Rgba(31, 112_u8.saturating_add(pulse), 50, 250),
        Some((colors.outline, 1.35)),
    );
    for side in [-1.0_f32, 1.0] {
        scene.ellipse(
            [38.0, side * 7.0],
            [37.0, 8.2],
            0.0,
            Rgba(53, 158_u8.saturating_add(pulse), 66, 228),
            None,
        );
        scene.line(
            [7.0, side * 5.5],
            [72.0, side * 5.5],
            1.0,
            Rgba(161, 222, 77, 170),
        );
    }
    scene.line([0.0, 0.0], [81.0, 0.0], 1.45, Rgba(204, 219, 77, 210));
    for (x, half_height) in [(14.0, 15.5), (34.0, 17.5), (55.0, 15.0), (72.0, 10.0)] {
        scene.line(
            [x, -half_height],
            [x, half_height],
            0.72,
            Rgba(21, 92, 47, 175),
        );
    }
    scene.ellipse([78.0, 0.0], [5.0, 8.5], 0.0, Rgba(87, 189, 69, 150), None);
}

fn draw_thorax(scene: &mut SceneBuilder, skin: Skin, time: f32, colors: Palette) {
    if skin == Skin::Firefly {
        let glow = if time == 0.0 {
            0
        } else {
            ((time * 3.0).sin().abs() * 14.0) as u8
        };
        scene.ellipse(
            [-31.0, 0.0],
            [20.0, 17.0],
            0.0,
            colors.thorax_core,
            Some((colors.thorax_edge, 1.5)),
        );
        scene.ellipse(
            [-36.0, 0.0],
            [6.5, 7.5],
            0.0,
            Rgba(67, 47, 32, 230),
            Some((Rgba(239, 158_u8.saturating_add(glow), 51, 210), 0.7)),
        );
        scene.ellipse(
            [-7.0, 0.0],
            [15.5, 15.5],
            0.0,
            Rgba(38, 121_u8.saturating_add(glow), 60, 250),
            Some((colors.highlight, 1.1)),
        );
        scene.line([-16.0, -9.0], [0.0, 9.0], 0.8, Rgba(176, 215, 69, 135));
        return;
    }
    let shell = cubic_loop(
        &[
            ([-45.0, 0.0], [-42.0, -20.0], [-15.0, -24.0], [3.0, -12.0]),
            ([3.0, -12.0], [10.0, -5.0], [10.0, 5.0], [3.0, 12.0]),
            ([3.0, 12.0], [-15.0, 24.0], [-42.0, 20.0], [-45.0, 0.0]),
        ],
        9,
    );
    scene.polygon(&shell, colors.thorax_core, Some((colors.thorax_edge, 1.6)));
    scene.ellipse(
        [-21.0, -4.0],
        [22.0, 13.0],
        -0.18,
        if skin == Skin::Firefly {
            Rgba(87, 106, 218, 92)
        } else {
            Rgba(91, 48, 27, 92)
        },
        None,
    );
    for (a, b) in [
        ([-39.0, -6.0], [-7.0, 13.0]),
        ([-35.0, 13.0], [-9.0, -15.0]),
        ([-23.0, -20.0], [-17.0, 20.0]),
    ] {
        scene.line(a, b, 0.85, Rgba(245, 203, 78, 135));
    }
    let glow = if time == 0.0 {
        0
    } else {
        ((time * 3.0).sin().abs() * 18.0) as u8
    };
    for y in [-13.5_f32, 13.5] {
        scene.ellipse(
            [-22.0, y],
            [5.0, 4.0],
            0.0,
            Rgba(237, 184_u8.saturating_add(glow), 69, 238),
            Some((Rgba(221, 248, 255, 210), 0.65)),
        );
    }
}

fn draw_head(scene: &mut SceneBuilder, skin: Skin, behavior: Behavior, time: f32, colors: Palette) {
    if skin == Skin::Firefly {
        scene.ellipse(
            [-58.0, 0.0],
            [13.0, 12.5],
            0.0,
            colors.head_core,
            Some((colors.head_rim, 1.25)),
        );
        for side in [-1.0_f32, 1.0] {
            let pulse = if behavior == Behavior::Rest {
                0
            } else {
                ((time * 4.1 + side).sin().abs() * 18.0) as u8
            };
            scene.ellipse(
                [-62.0, side * 7.0],
                [6.2, 5.8],
                side * 0.08,
                Rgba(202, 75_u8.saturating_add(pulse), 31, 255),
                Some((colors.eye_rim, 0.95)),
            );
            scene.ellipse(
                [-64.0, side * 8.0],
                [1.5, 1.2],
                0.0,
                Rgba(255, 203, 92, 225),
                None,
            );
        }
        scene.ellipse([-69.0, 0.0], [3.0, 2.2], 0.0, Rgba(215, 139, 40, 235), None);
        return;
    }
    scene.ellipse(
        [-60.0, 0.0],
        [18.0, 18.0],
        0.0,
        colors.head_core,
        Some((colors.head_rim, 1.5)),
    );
    scene.ellipse([-63.0, -3.0], [12.5, 12.5], 0.0, Rgba(9, 30, 54, 70), None);
    for side in [-1.0_f32, 1.0] {
        let pulse = if behavior == Behavior::Rest {
            0.0
        } else {
            (time * 4.1 + side).sin().abs() * 24.0
        };
        scene.ellipse(
            [-62.0, side * 9.0],
            [12.5, 9.5],
            side * 0.08,
            Rgba(
                colors.eye_core.0,
                colors.eye_core.1.saturating_add(pulse as u8),
                colors.eye_core.2,
                255,
            ),
            Some((colors.eye_rim, 1.2)),
        );
        for index in -2..=2 {
            let offset = index as f32 * 3.1;
            scene.line(
                [-70.0, side * 9.0 + offset],
                [-54.0, side * 9.0 + offset + side],
                0.48,
                Rgba(252, 220, 157, 92),
            );
        }
    }
    scene.ellipse([-69.0, 0.0], [12.5, 4.0], 0.0, Rgba(27, 89, 103, 210), None);
    for y in [-4.0_f32, 0.0, 4.0] {
        scene.ellipse([-78.0, y], [1.7, 1.7], 0.0, Rgba(244, 205, 78, 245), None);
    }
}

fn draw_legs(
    scene: &mut SceneBuilder,
    behavior: Behavior,
    gait: f32,
    time: f32,
    rear_layer: bool,
    colors: Palette,
) {
    let locomoting = matches!(behavior, Behavior::Walk | Behavior::Reverse);
    let grooming = behavior == Behavior::Groom;
    let landing = behavior == Behavior::Landing;
    for side_index in 0..2 {
        let side = if side_index == 0 { -1.0 } else { 1.0 };
        for leg_index in 0..3 {
            if rear_layer != (leg_index == 2) {
                continue;
            }
            let phase_offset = leg_index as f32 * 2.07 + if side_index == 0 { 0.0 } else { PI };
            let swing = if locomoting {
                (gait.asin() + phase_offset).sin()
            } else {
                0.0
            };
            let groom_motion = if grooming && leg_index == 0 {
                (time * 12.0 + side_index as f32).sin() * 7.0
            } else {
                0.0
            };
            let hip_x = -31.0 + leg_index as f32 * 13.0;
            let hip_y = side * (8.0 + leg_index as f32 * 2.0);
            let mut reach_x = match leg_index {
                0 => -68.0 + groom_motion,
                1 => -22.0 + swing * 5.0,
                _ => 32.0 + swing * 7.0,
            };
            let mut reach_y = side
                * match leg_index {
                    0 => 34.0,
                    1 => 44.0,
                    _ => 48.0,
                };
            if grooming && leg_index == 0 {
                reach_x = -68.0 + groom_motion;
                reach_y = side * (13.0 + (time * 12.0 + side_index as f32).cos() * 8.0);
            } else if landing {
                reach_y *= 1.13;
                reach_x += if leg_index == 2 { 8.0 } else { -3.0 };
            }
            let knee = [
                (hip_x + reach_x) * 0.5 + swing * 3.0,
                side * (22.0 + leg_index as f32 * 5.0),
            ];
            let ankle = [reach_x, reach_y];
            let toe = [
                reach_x + if leg_index == 2 { 12.0 } else { -7.0 },
                reach_y + side * 3.0,
            ];
            let alpha = if rear_layer { 178 } else { 238 };
            let dark = Rgba(colors.leg.0, colors.leg.1, colors.leg.2, alpha);
            scene.line(
                [hip_x, hip_y],
                knee,
                if rear_layer { 2.0 } else { 2.4 },
                dark,
            );
            scene.line(knee, ankle, if rear_layer { 1.8 } else { 2.2 }, dark);
            scene.line(ankle, toe, 1.45, dark);
            scene.line([hip_x, hip_y], knee, 0.65, Rgba(87, 237, 218, 105));
            scene.ellipse(knee, [1.5, 1.5], 0.0, Rgba(235, 184, 65, 220), None);
        }
    }
}

fn draw_antennae(scene: &mut SceneBuilder, behavior: Behavior, time: f32, colors: Palette) {
    let alert = matches!(
        behavior,
        Behavior::Alert | Behavior::PreEscape | Behavior::Flight
    );
    let sway = if alert { (time * 5.2).sin() * 5.0 } else { 0.0 };
    for side in [-1.0_f32, 1.0] {
        let root = [-72.0, side * 8.0];
        let joint = [-87.0, side * (18.0 + sway)];
        let tip = [-101.0, side * (25.0 + sway * 1.3)];
        scene.line(root, joint, 1.45, colors.leg);
        scene.line(
            joint,
            tip,
            1.05,
            Rgba(
                colors.highlight.0,
                colors.highlight.1,
                colors.highlight.2,
                205,
            ),
        );
        scene.ellipse(tip, [1.8, 1.8], 0.0, Rgba(242, 183, 63, 235), None);
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
                    .map(|p| [p[0] * scale, p[1] * scale])
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
                if (px - nearest_x).powi(2) + (py - nearest_y).powi(2) <= radius * radius {
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn polygon(&mut self, points: &[[f32; 2]], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|p| p[1])
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
    pub flight_pixel_differences: usize,
    pub landing_pixel_differences: usize,
    pub walking_pixel_differences: usize,
    pub wing_state_contract_passed: bool,
}

impl FireflyVisualSelfTest {
    pub fn rest_temporal_invariant(&self) -> bool {
        self.rest_pixel_differences == 0
    }
}

pub fn run_firefly_visual_self_test() -> FireflyVisualSelfTest {
    let render = |behavior, phase| {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, behavior, phase, 0.0, false);
        pixels
    };
    let rest = render(Behavior::Rest, 0.0);
    let rest_later = render(Behavior::Rest, 2.0);
    let escape_first = render(Behavior::PreEscape, 0.04);
    let escape_later = render(Behavior::PreEscape, 0.11);
    let flight_first = render(Behavior::Flight, 0.08);
    let flight_later = render(Behavior::Flight, 0.17);
    let landing = render(Behavior::Landing, 0.35);
    let groom = render(Behavior::Groom, 0.35);
    let walk = render(Behavior::Walk, 0.35);
    let ground_wings_stowed = [
        Behavior::Rest,
        Behavior::Walk,
        Behavior::Reverse,
        Behavior::Groom,
    ]
    .into_iter()
    .all(|behavior| wing_panel_count(&pet_scene(Skin::Firefly, behavior, 0.2, 0.0, false)) == 0);
    let air_wings_deployed = [Behavior::PreEscape, Behavior::Flight, Behavior::Landing]
        .into_iter()
        .all(|behavior| {
            wing_panel_count(&pet_scene(Skin::Firefly, behavior, 0.2, 0.0, false)) == 2
        });

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
        flight_pixel_differences: pixel_difference(&flight_first, &flight_later),
        landing_pixel_differences: pixel_difference(&rest, &landing),
        walking_pixel_differences: pixel_difference(&rest, &walk),
        wing_state_contract_passed: ground_wings_stowed && air_wings_deployed,
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
        if green > 130 && green > red.saturating_add(35) && green > blue.saturating_add(25) {
            result.lantern_pixels += 1;
        }
        if alpha >= 190 && (green > 105 || blue > 130) {
            result.elytron_pixels += 1;
        }
    }
    result.passed = result.opaque_pixels > 1_000
        && result.translucent_pixels > 650
        && result.red_eye_pixels > 70
        && result.lantern_pixels > 45
        && result.elytron_pixels > 450
        && result.rest_temporal_invariant()
        && result.escape_wing_pixel_differences > 180
        && result.flight_pixel_differences > 180
        && result.rest_escape_pixel_differences > 1_000
        && result.grooming_pixel_differences > 100
        && result.landing_pixel_differences > 500
        && result.walking_pixel_differences > 100
        && result.wing_state_contract_passed;
    result
}

fn wing_panel_count(scene: &[Primitive]) -> usize {
    scene
        .iter()
        .filter(|primitive| {
            matches!(
                primitive,
                Primitive::Polygon { fill, .. } if fill.3 == 82
            )
        })
        .count()
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
    use serde::Serialize;

    #[derive(Serialize)]
    struct TrajectorySample {
        time_seconds: f32,
        behavior: String,
        x: f32,
        y: f32,
        heading_radians: f32,
        phase: f32,
    }

    #[test]
    fn heading_points_the_head_along_the_velocity_vector() {
        let right = pet_scene(Skin::Firefly, Behavior::Rest, 0.0, 0.0, true);
        let left = pet_scene(Skin::Firefly, Behavior::Rest, 0.0, PI, true);
        let eye_x = |scene: &[Primitive]| {
            scene
                .iter()
                .filter_map(|primitive| match primitive {
                    Primitive::Ellipse { center, fill, .. }
                        if fill.0 >= 200 && fill.1 < 100 && fill.2 < 50 && fill.3 == 255 =>
                    {
                        Some(center[0])
                    }
                    _ => None,
                })
                .next()
                .unwrap()
        };
        assert!(eye_x(&right) > CENTER[0]);
        assert!(eye_x(&left) < CENTER[0]);
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
            None,
        );
        assert!(motion.screen_position.x <= 1_920.0 - PET_WIDTH as f32 - 8.0);
        assert!(motion.screen_position.y <= 1_080.0 - PET_HEIGHT as f32 - 8.0);
    }

    #[test]
    fn walking_motion_is_time_based_across_refresh_rates() {
        let mut at_30_hz = PetMotion::default();
        let mut at_120_hz = PetMotion::default();
        let origin = Pos2::ZERO;
        let screen = Vec2::new(3_840.0, 2_160.0);
        for _ in 0..60 {
            at_30_hz.advance(1.0 / 30.0, Behavior::Walk, origin, screen, false, None);
        }
        for _ in 0..240 {
            at_120_hz.advance(1.0 / 120.0, Behavior::Walk, origin, screen, false, None);
        }
        assert!((at_30_hz.screen_position.x - at_120_hz.screen_position.x).abs() < 0.8);
        assert!((at_30_hz.animation_seconds - at_120_hz.animation_seconds).abs() < 0.001);
    }

    #[test]
    fn flight_moves_in_two_dimensions_with_recorded_free_flight_curvature() {
        let mut motion = PetMotion {
            screen_position: Pos2::new(800.0, 500.0),
            ..PetMotion::default()
        };
        let start = motion.screen_position;
        for _ in 0..60 {
            motion.advance(
                1.0 / 60.0,
                Behavior::Flight,
                Pos2::ZERO,
                Vec2::new(1_920.0, 1_080.0),
                false,
                None,
            );
        }
        assert!((motion.screen_position.x - start.x).abs() > 20.0);
        assert!((motion.screen_position.y - start.y).abs() > 15.0);
        assert!(motion.heading_radians.abs() > 0.2);
    }

    #[test]
    fn recorded_motion_contract_covers_run_escape_flight_and_touchdown() {
        let result = run_motion_self_test();
        assert!(result.passed, "{result:#?}");
    }

    #[test]
    fn prism_uses_the_reference_desktop_scale() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, Behavior::Rest, 0.0, 0.0, false);
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
        let visible_width = maximum_x.saturating_sub(minimum_x);
        let visible_height = maximum_y.saturating_sub(minimum_y);
        assert!((150..=230).contains(&visible_width));
        assert!((70..=140).contains(&visible_height));
    }

    #[test]
    fn layered_bitmap_has_real_holes_and_premultiplied_color() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(
            &mut pixels,
            Skin::Firefly,
            Behavior::Flight,
            0.25,
            0.7,
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
        assert!(transparent > PET_WIDTH * PET_HEIGHT / 2);
        assert!(visible > PET_WIDTH * PET_HEIGHT / 25);
    }

    #[test]
    fn prism_visual_contract_is_articulated_and_behavior_responsive() {
        let result = run_firefly_visual_self_test();
        assert!(result.passed, "{result:#?}");
        if let Some(path) = std::env::var_os("MECHOFLY_VISUAL_FIXTURE") {
            let mut rest = vec![0; PET_WIDTH * PET_HEIGHT * 4];
            render_pet_bgra(&mut rest, Skin::Firefly, Behavior::Rest, 0.0, 0.0, false);
            write_pam(std::path::Path::new(&path), &rest);
        }
        if let Some(directory) = std::env::var_os("MECHOFLY_VISUAL_FIXTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).expect("visual fixture directory must be writable");
            for (name, behavior, phase, heading) in [
                ("rest", Behavior::Rest, 0.0, 0.0),
                ("walk-a", Behavior::Walk, 0.10, 0.0),
                ("walk-b", Behavior::Walk, 0.42, 0.0),
                ("groom-a", Behavior::Groom, 0.10, 0.0),
                ("groom-b", Behavior::Groom, 0.42, 0.0),
                ("flight-a", Behavior::Flight, 0.08, 0.55),
                ("flight-b", Behavior::Flight, 0.30, 0.55),
                ("landing", Behavior::Landing, 0.34, 0.18),
            ] {
                let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
                render_pet_bgra(&mut pixels, Skin::Firefly, behavior, phase, heading, false);
                write_pam(&directory.join(format!("prism-{name}.pam")), &pixels);
            }
            write_trajectory_fixtures(&directory);
        }
    }

    fn write_trajectory_fixtures(directory: &std::path::Path) {
        let width = 1_920_usize;
        let height = 1_080_usize;
        let origin = Pos2::ZERO;
        let screen = Vec2::new(width as f32, height as f32);
        let bottom = height as f32 - PET_HEIGHT as f32 - 8.0;
        let mut motion = PetMotion {
            screen_position: Pos2::new(240.0, bottom),
            ..PetMotion::default()
        };
        let mut elapsed = 0.0_f32;
        let mut samples = Vec::new();
        let mut record = |motion: &PetMotion, behavior: Behavior, elapsed: f32| {
            samples.push(TrajectorySample {
                time_seconds: elapsed,
                behavior: format!("{behavior:?}"),
                x: motion.screen_position.x,
                y: motion.screen_position.y,
                heading_radians: motion.heading_radians,
                phase: motion.animation_seconds,
            });
        };

        record(&motion, Behavior::Walk, elapsed);
        for frame in 1_u32..=120 {
            motion.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(60) {
                record(&motion, Behavior::Walk, elapsed);
            }
        }
        let cursor = motion.screen_position + Vec2::new(390.0, 330.0);
        for _ in 0..12 {
            motion.advance(
                1.0 / 60.0,
                Behavior::PreEscape,
                origin,
                screen,
                false,
                Some(cursor),
            );
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::PreEscape, elapsed);
        for frame in 1_u32..=240 {
            motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(48) {
                record(&motion, Behavior::Flight, elapsed);
            }
        }
        for _ in 0..30 {
            motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::Landing, elapsed);
        for _ in 0..90 {
            motion.advance(1.0 / 60.0, Behavior::Groom, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::Groom, elapsed);

        for (name, skin) in [("drosophila", Skin::Drosophila), ("prism", Skin::Firefly)] {
            let mut canvas = desktop_canvas(width, height);
            for (index, sample) in samples.iter().enumerate() {
                let behavior = match sample.behavior.as_str() {
                    "Walk" => Behavior::Walk,
                    "PreEscape" => Behavior::PreEscape,
                    "Flight" => Behavior::Flight,
                    "Landing" => Behavior::Landing,
                    "Groom" => Behavior::Groom,
                    _ => Behavior::Rest,
                };
                let mut pet = vec![0; PET_WIDTH * PET_HEIGHT * 4];
                render_pet_bgra(
                    &mut pet,
                    skin,
                    behavior,
                    sample.phase,
                    sample.heading_radians,
                    false,
                );
                stamp_bgra(
                    &mut canvas,
                    width,
                    height,
                    &pet,
                    sample.x.round() as i32,
                    sample.y.round() as i32,
                    if index + 1 == samples.len() { 255 } else { 176 },
                );
            }
            write_bmp(
                &directory.join(format!("behavior-trajectory-{name}.bmp")),
                width,
                height,
                &canvas,
            );
        }

        let json = serde_json::to_string_pretty(&samples).expect("trajectory must serialize");
        std::fs::write(
            directory.join("behavior-trajectory.json"),
            format!("{json}\n"),
        )
        .expect("trajectory receipt must be writable");
    }

    fn desktop_canvas(width: usize, height: usize) -> Vec<u8> {
        let mut canvas = vec![0_u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 4;
                let glow = ((x as f32 / width as f32) * 22.0) as u8;
                canvas[offset] = 26_u8.saturating_add(glow / 3);
                canvas[offset + 1] = 12_u8.saturating_add(glow / 2);
                canvas[offset + 2] = 13_u8.saturating_add(glow);
                canvas[offset + 3] = 255;
                if y + 10 >= height && y + 8 < height {
                    canvas[offset] = 54;
                    canvas[offset + 1] = 70;
                    canvas[offset + 2] = 92;
                }
            }
        }
        canvas
    }

    #[allow(clippy::too_many_arguments)]
    fn stamp_bgra(
        canvas: &mut [u8],
        canvas_width: usize,
        canvas_height: usize,
        pet: &[u8],
        left: i32,
        top: i32,
        opacity: u8,
    ) {
        for pet_y in 0..PET_HEIGHT {
            let canvas_y = top + pet_y as i32;
            if !(0..canvas_height as i32).contains(&canvas_y) {
                continue;
            }
            for pet_x in 0..PET_WIDTH {
                let canvas_x = left + pet_x as i32;
                if !(0..canvas_width as i32).contains(&canvas_x) {
                    continue;
                }
                let source = (pet_y * PET_WIDTH + pet_x) * 4;
                let source_alpha = pet[source + 3] as u32 * opacity as u32 / 255;
                if source_alpha == 0 {
                    continue;
                }
                let target = (canvas_y as usize * canvas_width + canvas_x as usize) * 4;
                let inverse = 255 - source_alpha;
                for channel in 0..3 {
                    let source_channel = pet[source + channel] as u32 * opacity as u32 / 255;
                    canvas[target + channel] = (source_channel
                        + canvas[target + channel] as u32 * inverse / 255)
                        .min(255) as u8;
                }
            }
        }
    }

    fn write_bmp(path: &std::path::Path, width: usize, height: usize, pixels: &[u8]) {
        let pixel_bytes = width * height * 4;
        let file_size = 14 + 40 + pixel_bytes;
        let mut bytes = Vec::with_capacity(file_size);
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&(file_size as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&54_u32.to_le_bytes());
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(&(width as i32).to_le_bytes());
        bytes.extend_from_slice(&(height as i32).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        bytes.extend_from_slice(&2_835_i32.to_le_bytes());
        bytes.extend_from_slice(&2_835_i32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for row in (0..height).rev() {
            let start = row * width * 4;
            bytes.extend_from_slice(&pixels[start..start + width * 4]);
        }
        std::fs::write(path, bytes).expect("trajectory bitmap must be writable");
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
