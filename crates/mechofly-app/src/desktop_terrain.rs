use std::cmp::Ordering as CmpOrdering;

use eframe::egui::{Pos2, Vec2};
use serde::{Deserialize, Serialize};

const SAMPLE_INTERVAL_MS: u32 = 1_000;
const MIN_WINDOW_EXTENT_PIXELS: i32 = 64;
const TERRAIN_CLAIM: &str = "LOCAL_TOP_LEVEL_GEOMETRY_ONLY_NO_CONTENT_CAPTURE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NormalizedRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl NormalizedRect {
    fn width(self) -> f32 {
        (self.max[0] - self.min[0]).max(0.0)
    }

    fn height(self) -> f32 {
        (self.max[1] - self.min[1]).max(0.0)
    }

    fn area(self) -> f32 {
        self.width() * self.height()
    }

    fn center(self) -> [f32; 2] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
        ]
    }

    fn contains(self, point: [f32; 2]) -> bool {
        point[0] >= self.min[0]
            && point[0] <= self.max[0]
            && point[1] >= self.min[1]
            && point[1] <= self.max[1]
    }

    fn inset(self, amount: f32) -> Self {
        let x = amount.min(self.width() * 0.45);
        let y = amount.min(self.height() * 0.45);
        Self {
            min: [self.min[0] + x, self.min[1] + y],
            max: [self.max[0] - x, self.max[1] - y],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopTerrainSnapshot {
    pub sample_epoch: u64,
    pub visible_surface_count: usize,
    pub stable_surface_count: usize,
    pub edge_segment_count: usize,
    pub focused_window_fraction: f32,
    pub preferred_refuge_normalized: [f32; 2],
    pub preferred_explore_normalized: Option<[f32; 2]>,
    pub human_risk_at_fly: f32,
    pub content_capture: bool,
    pub semantic_resource_inference: bool,
    pub claim: &'static str,
}

impl Default for DesktopTerrainSnapshot {
    fn default() -> Self {
        Self {
            sample_epoch: 0,
            visible_surface_count: 0,
            stable_surface_count: 0,
            edge_segment_count: 0,
            focused_window_fraction: 0.0,
            preferred_refuge_normalized: [0.92, 0.86],
            preferred_explore_normalized: None,
            human_risk_at_fly: 0.0,
            content_capture: false,
            semantic_resource_inference: false,
            claim: TERRAIN_CLAIM,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SurfaceObservation {
    id: isize,
    rect: NormalizedRect,
    focused: bool,
    stable_ticks: u32,
}

pub struct DesktopTerrainCensus {
    accumulator_ms: u32,
    sample_epoch: u64,
    snapshot: DesktopTerrainSnapshot,
    platform: PlatformCensus,
}

impl Default for DesktopTerrainCensus {
    fn default() -> Self {
        Self {
            accumulator_ms: SAMPLE_INTERVAL_MS,
            sample_epoch: 0,
            snapshot: DesktopTerrainSnapshot::default(),
            platform: PlatformCensus::default(),
        }
    }
}

impl DesktopTerrainCensus {
    pub fn update(
        &mut self,
        delta_ms: u32,
        screen_origin: Pos2,
        screen_size: Vec2,
        fly_center: Pos2,
        cursor_position: Option<Pos2>,
    ) -> DesktopTerrainSnapshot {
        self.accumulator_ms = self.accumulator_ms.saturating_add(delta_ms.min(250));
        if self.accumulator_ms < SAMPLE_INTERVAL_MS {
            return self.snapshot.clone();
        }
        self.accumulator_ms %= SAMPLE_INTERVAL_MS;
        self.sample_epoch = self.sample_epoch.saturating_add(1);
        let observations = self.platform.sample(screen_origin, screen_size);
        self.snapshot = summarize_surfaces(
            self.sample_epoch,
            &observations,
            normalize_point(fly_center, screen_origin, screen_size),
            cursor_position.map(|cursor| normalize_point(cursor, screen_origin, screen_size)),
        );
        self.snapshot.clone()
    }
}

fn summarize_surfaces(
    sample_epoch: u64,
    observations: &[SurfaceObservation],
    fly_normalized: [f32; 2],
    cursor_normalized: Option<[f32; 2]>,
) -> DesktopTerrainSnapshot {
    let focused = observations.iter().find(|surface| surface.focused).copied();
    let stable: Vec<SurfaceObservation> = observations
        .iter()
        .copied()
        .filter(|surface| surface.stable_ticks >= 2)
        .collect();

    let mut candidates = vec![[0.06, 0.08], [0.94, 0.08], [0.06, 0.92], [0.94, 0.92]];
    for surface in &stable {
        let rect = surface.rect;
        let center = rect.center();
        candidates.extend([
            [rect.min[0], center[1]],
            [rect.max[0], center[1]],
            [center[0], rect.min[1]],
            [center[0], rect.max[1]],
        ]);
    }
    for candidate in &mut candidates {
        candidate[0] = candidate[0].clamp(0.035, 0.965);
        candidate[1] = candidate[1].clamp(0.045, 0.955);
    }
    candidates.sort_by(|left, right| {
        left[0]
            .total_cmp(&right[0])
            .then_with(|| left[1].total_cmp(&right[1]))
    });
    candidates.dedup_by(|left, right| squared_distance(*left, *right) < 0.000_025);

    let risk = |point: [f32; 2]| attention_risk(point, focused, cursor_normalized);
    let refuge = candidates
        .iter()
        .copied()
        .min_by(|left, right| compare_refuge(*left, *right, &risk))
        .unwrap_or([0.92, 0.86]);

    let explore = stable
        .iter()
        .flat_map(|surface| {
            let rect = surface.rect;
            let center = rect.center();
            [
                [rect.min[0], center[1]],
                [rect.max[0], center[1]],
                [center[0], rect.min[1]],
                [center[0], rect.max[1]],
            ]
        })
        .filter(|candidate| risk(*candidate) < 0.72)
        .max_by(|left, right| {
            let left_score = squared_distance(*left, fly_normalized)
                + cursor_normalized
                    .map(|cursor| squared_distance(*left, cursor))
                    .unwrap_or(0.25);
            let right_score = squared_distance(*right, fly_normalized)
                + cursor_normalized
                    .map(|cursor| squared_distance(*right, cursor))
                    .unwrap_or(0.25);
            left_score.total_cmp(&right_score)
        });

    DesktopTerrainSnapshot {
        sample_epoch,
        visible_surface_count: observations.len(),
        stable_surface_count: stable.len(),
        edge_segment_count: stable.len().saturating_mul(4),
        focused_window_fraction: focused.map(|surface| surface.rect.area()).unwrap_or(0.0),
        preferred_refuge_normalized: refuge,
        preferred_explore_normalized: explore,
        human_risk_at_fly: risk(fly_normalized),
        content_capture: false,
        semantic_resource_inference: false,
        claim: TERRAIN_CLAIM,
    }
}

fn compare_refuge(
    left: [f32; 2],
    right: [f32; 2],
    risk: &impl Fn([f32; 2]) -> f32,
) -> CmpOrdering {
    risk(left)
        .total_cmp(&risk(right))
        .then_with(|| peripheral_cost(left).total_cmp(&peripheral_cost(right)))
        .then_with(|| left[0].total_cmp(&right[0]))
        .then_with(|| left[1].total_cmp(&right[1]))
}

fn attention_risk(
    point: [f32; 2],
    focused: Option<SurfaceObservation>,
    cursor: Option<[f32; 2]>,
) -> f32 {
    let mut risk: f32 = 0.0;
    if let Some(surface) = focused {
        if surface.rect.inset(0.055).contains(point) {
            risk = risk.max(0.72);
        }
        let title_band = NormalizedRect {
            min: surface.rect.min,
            max: [surface.rect.max[0], (surface.rect.min[1] + 0.055).min(surface.rect.max[1])],
        };
        if title_band.contains(point) {
            risk = risk.max(0.88);
        }
    }
    if let Some(cursor) = cursor {
        let distance = squared_distance(point, cursor).sqrt();
        risk = risk.max(((0.18 - distance) / 0.18).clamp(0.0, 1.0));
    }
    risk.clamp(0.0, 1.0)
}

fn peripheral_cost(point: [f32; 2]) -> f32 {
    point[0]
        .min(1.0 - point[0])
        .min(point[1].min(1.0 - point[1]))
}

fn normalize_point(point: Pos2, origin: Pos2, size: Vec2) -> [f32; 2] {
    [
        ((point.x - origin.x) / size.x.max(1.0)).clamp(0.0, 1.0),
        ((point.y - origin.y) / size.y.max(1.0)).clamp(0.0, 1.0),
    ]
}

fn normalize_rect(rect: [i32; 4], origin: Pos2, size: Vec2) -> NormalizedRect {
    let left = (rect[0] as f32 - origin.x) / size.x.max(1.0);
    let top = (rect[1] as f32 - origin.y) / size.y.max(1.0);
    let right = (rect[2] as f32 - origin.x) / size.x.max(1.0);
    let bottom = (rect[3] as f32 - origin.y) / size.y.max(1.0);
    NormalizedRect {
        min: [left.clamp(0.0, 1.0), top.clamp(0.0, 1.0)],
        max: [right.clamp(0.0, 1.0), bottom.clamp(0.0, 1.0)],
    }
}

fn squared_distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    let x = left[0] - right[0];
    let y = left[1] - right[1];
    x * x + y * y
}

#[cfg(not(windows))]
#[derive(Default)]
struct PlatformCensus;

#[cfg(not(windows))]
impl PlatformCensus {
    fn sample(&mut self, _origin: Pos2, _size: Vec2) -> Vec<SurfaceObservation> {
        Vec::new()
    }
}

#[cfg(windows)]
mod windows_platform {
    use std::{collections::HashMap, mem::zeroed};

    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::{
            EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible,
        },
    };

    use super::{
        MIN_WINDOW_EXTENT_PIXELS, NormalizedRect, Pos2, SurfaceObservation, Vec2, normalize_rect,
    };

    #[derive(Clone, Copy)]
    struct TrackedWindow {
        rect: [i32; 4],
        stable_ticks: u32,
    }

    #[derive(Clone, Copy)]
    struct RawWindow {
        id: isize,
        rect: [i32; 4],
        focused: bool,
    }

    struct EnumerationContext {
        current_process_id: u32,
        foreground: HWND,
        windows: Vec<RawWindow>,
    }

    #[derive(Default)]
    pub(super) struct PlatformCensus {
        tracked: HashMap<isize, TrackedWindow>,
    }

    impl PlatformCensus {
        pub(super) fn sample(
            &mut self,
            origin: Pos2,
            size: Vec2,
        ) -> Vec<SurfaceObservation> {
            let foreground = unsafe { GetForegroundWindow() };
            let mut context = EnumerationContext {
                current_process_id: std::process::id(),
                foreground,
                windows: Vec::new(),
            };
            unsafe {
                EnumWindows(
                    Some(enumerate_window),
                    (&mut context as *mut EnumerationContext) as LPARAM,
                );
            }

            let mut next = HashMap::with_capacity(context.windows.len());
            let observations = context
                .windows
                .into_iter()
                .map(|window| {
                    let stable_ticks = self
                        .tracked
                        .get(&window.id)
                        .filter(|previous| previous.rect == window.rect)
                        .map(|previous| previous.stable_ticks.saturating_add(1))
                        .unwrap_or(0);
                    next.insert(
                        window.id,
                        TrackedWindow {
                            rect: window.rect,
                            stable_ticks,
                        },
                    );
                    SurfaceObservation {
                        id: window.id,
                        rect: normalize_rect(window.rect, origin, size),
                        focused: window.focused,
                        stable_ticks,
                    }
                })
                .filter(|surface| surface.rect.width() > 0.0 && surface.rect.height() > 0.0)
                .collect();
            self.tracked = next;
            observations
        }
    }

    unsafe extern "system" fn enumerate_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let context = unsafe { &mut *(lparam as *mut EnumerationContext) };
        if unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
            return 1;
        }
        let mut process_id = 0_u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process_id);
        }
        if process_id == context.current_process_id {
            return 1;
        }
        let mut rect: RECT = unsafe { zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return 1;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < MIN_WINDOW_EXTENT_PIXELS || height < MIN_WINDOW_EXTENT_PIXELS {
            return 1;
        }
        context.windows.push(RawWindow {
            id: hwnd as isize,
            rect: [rect.left, rect.top, rect.right, rect.bottom],
            focused: hwnd == context.foreground,
        });
        1
    }
}

#[cfg(windows)]
use windows_platform::PlatformCensus;

#[derive(Clone, Debug)]
pub struct DesktopTerrainSelfTest {
    pub passed: bool,
    pub deterministic_summary: bool,
    pub stable_edges_available: bool,
    pub peripheral_refuge_selected: bool,
    pub no_content_capture: bool,
    pub no_semantic_resource_inference: bool,
}

pub fn run_desktop_terrain_self_test() -> DesktopTerrainSelfTest {
    let observations = vec![
        SurfaceObservation {
            id: 1,
            rect: NormalizedRect {
                min: [0.0, 0.0],
                max: [1.0, 0.96],
            },
            focused: true,
            stable_ticks: 5,
        },
        SurfaceObservation {
            id: 2,
            rect: NormalizedRect {
                min: [0.72, 0.18],
                max: [0.96, 0.80],
            },
            focused: false,
            stable_ticks: 4,
        },
    ];
    let first = summarize_surfaces(7, &observations, [0.50, 0.50], Some([0.52, 0.50]));
    let second = summarize_surfaces(7, &observations, [0.50, 0.50], Some([0.52, 0.50]));
    let deterministic_summary = first == second;
    let stable_edges_available = first.stable_surface_count == 2 && first.edge_segment_count == 8;
    let refuge = first.preferred_refuge_normalized;
    let peripheral_refuge_selected = peripheral_cost(refuge) <= 0.08;
    let no_content_capture = !first.content_capture && first.claim == TERRAIN_CLAIM;
    let no_semantic_resource_inference = !first.semantic_resource_inference;
    DesktopTerrainSelfTest {
        passed: deterministic_summary
            && stable_edges_available
            && peripheral_refuge_selected
            && no_content_capture
            && no_semantic_resource_inference,
        deterministic_summary,
        stable_edges_available,
        peripheral_refuge_selected,
        no_content_capture,
        no_semantic_resource_inference,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_desktop_summary_is_deterministic_and_private() {
        let result = run_desktop_terrain_self_test();
        assert!(result.deterministic_summary);
        assert!(result.no_content_capture);
        assert!(result.no_semantic_resource_inference);
    }

    #[test]
    fn stable_window_geometry_produces_edges_and_peripheral_refuge() {
        let result = run_desktop_terrain_self_test();
        assert!(result.stable_edges_available);
        assert!(result.peripheral_refuge_selected);
    }
}
