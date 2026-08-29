from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8", newline="\n")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one exact match, found {count}: {old[:100]!r}"
        )
    write(path, text.replace(old, new, 1))


main = "crates/mechofly-app/src/main.rs"
replace_once(main, "mod ecotope;\n", "mod ecotope;\nmod habitat_shelf;\n")
replace_once(
    main,
    '''            open_brain_lab: args.iter().any(|arg| arg == "--brain-lab"),
            reduced_motion: profile.reduced_motion.unwrap_or(false)
''',
    '''            open_brain_lab: args.iter().any(|arg| arg == "--brain-lab"),
            open_ecotope_shelf: args.iter().any(|arg| arg == "--observe-mode"),
            reduced_motion: profile.reduced_motion.unwrap_or(false)
''',
)

ecotope = "crates/mechofly-app/src/ecotope.rs"
replace_once(
    ecotope,
    '''    pub source_normalized: [f32; 2],
    pub plume_normalized: [f32; 2],
    pub target_normalized: Option<[f32; 2]>,
''',
    '''    pub source_normalized: [f32; 2],
    pub plume_normalized: [f32; 2],
    pub refuge_normalized: [f32; 2],
    pub fly_normalized: [f32; 2],
    pub target_normalized: Option<[f32; 2]>,
    pub visited_cells: usize,
    pub total_visits: u64,
''',
)
replace_once(
    ecotope,
    '''            source_normalized: [0.72, 0.68],
            plume_normalized: [0.72, 0.68],
            target_normalized: None,
''',
    '''            source_normalized: [0.72, 0.68],
            plume_normalized: [0.72, 0.68],
            refuge_normalized: [0.12, 0.84],
            fly_normalized: [0.05, 0.60],
            target_normalized: None,
            visited_cells: 0,
            total_visits: 0,
''',
)
replace_once(
    ecotope,
    "#[derive(Clone, Debug)]\npub struct EcotopeDirective {\n",
    '''#[derive(Clone, Debug, PartialEq)]
pub struct EcotopeMapSnapshot {
    pub width: usize,
    pub height: usize,
    pub expected_reward: Vec<f32>,
    pub uncertainty: Vec<f32>,
    pub visits: Vec<u32>,
}

impl Default for EcotopeMapSnapshot {
    fn default() -> Self {
        Self {
            width: BELIEF_WIDTH,
            height: BELIEF_HEIGHT,
            expected_reward: vec![0.0; BELIEF_CELL_COUNT],
            uncertainty: vec![1.0; BELIEF_CELL_COUNT],
            visits: vec![0; BELIEF_CELL_COUNT],
        }
    }
}

#[derive(Clone, Debug)]
pub struct EcotopeDirective {
''',
)
replace_once(
    ecotope,
    '''                source_normalized: self.source_normalized,
                plume_normalized,
                target_normalized,
''',
    '''                source_normalized: self.source_normalized,
                plume_normalized,
                refuge_normalized: self.refuge_normalized,
                fly_normalized,
                target_normalized,
                visited_cells: self.belief.iter().filter(|cell| cell.visit_count > 0).count(),
                total_visits: self
                    .belief
                    .iter()
                    .map(|cell| u64::from(cell.visit_count))
                    .sum(),
''',
)
replace_once(
    ecotope,
    "    fn advance_world(&mut self, delta_ms: u32) {\n",
    '''    pub fn observation_map(&self) -> EcotopeMapSnapshot {
        EcotopeMapSnapshot {
            width: BELIEF_WIDTH,
            height: BELIEF_HEIGHT,
            expected_reward: self
                .belief
                .iter()
                .map(|cell| cell.expected_reward)
                .collect(),
            uncertainty: self
                .belief
                .iter()
                .map(|cell| cell.reward_variance.sqrt())
                .collect(),
            visits: self.belief.iter().map(|cell| cell.visit_count).collect(),
        }
    }

    fn advance_world(&mut self, delta_ms: u32) {
''',
)

app = "crates/mechofly-app/src/app.rs"
replace_once(
    app,
    '''    ecotope::{CursorThreatEstimator, EcotopeSnapshot, ScreenEcotope},
    live_brain::{LiveBrainCommand, LiveBrainState},
''',
    '''    ecotope::{CursorThreatEstimator, EcotopeMapSnapshot, EcotopeSnapshot, ScreenEcotope},
    habitat_shelf::HabitatShelfState,
    live_brain::{LiveBrainCommand, LiveBrainState},
''',
)
replace_once(
    app,
    '''    pub open_brain_lab: bool,
    pub reduced_motion: bool,
''',
    '''    pub open_brain_lab: bool,
    pub open_ecotope_shelf: bool,
    pub reduced_motion: bool,
''',
)
replace_once(
    app,
    '''    ecotope: ScreenEcotope,
    ecotope_snapshot: EcotopeSnapshot,
    cursor_threat: CursorThreatEstimator,
''',
    '''    ecotope: ScreenEcotope,
    ecotope_snapshot: EcotopeSnapshot,
    ecotope_map: EcotopeMapSnapshot,
    habitat_shelf: HabitatShelfState,
    cursor_threat: CursorThreatEstimator,
''',
)
replace_once(
    app,
    '''    evidence_hold: bool,
    seed: u64,
''',
    '''    evidence_hold: bool,
    last_screen_origin: Pos2,
    last_screen_size: Vec2,
    seed: u64,
''',
)
replace_once(
    app,
    '''            ecotope: ScreenEcotope::new(seed ^ 0x5343_5245_454E_4543),
            ecotope_snapshot: EcotopeSnapshot::default(),
            cursor_threat: CursorThreatEstimator::default(),
''',
    '''            ecotope: ScreenEcotope::new(seed ^ 0x5343_5245_454E_4543),
            ecotope_snapshot: EcotopeSnapshot::default(),
            ecotope_map: EcotopeMapSnapshot::default(),
            habitat_shelf: HabitatShelfState::new(config.open_ecotope_shelf),
            cursor_threat: CursorThreatEstimator::default(),
''',
)
replace_once(
    app,
    '''            evidence_hold: false,
            seed,
''',
    '''            evidence_hold: false,
            last_screen_origin: Pos2::ZERO,
            last_screen_size: Vec2::new(1_920.0, 1_080.0),
            seed,
''',
)
replace_once(
    app,
    '''        if events.hotkey(HotkeyAction::BrainLab) {
            self.live_brain.open = !self.live_brain.open;
            self.lab.message = "Global hotkey Ctrl+Alt+N: Live Brain toggled.".to_owned();
        }
''',
    '''        if events.hotkey(HotkeyAction::BrainLab) {
            self.live_brain.open = !self.live_brain.open;
            self.lab.message = "Global hotkey Ctrl+Alt+N: Live Brain toggled.".to_owned();
        }
        if events.hotkey(HotkeyAction::EcotopeShelf) {
            self.habitat_shelf.open = !self.habitat_shelf.open;
            self.lab.message = format!(
                "Global hotkey Ctrl+Alt+E: Screen Ecotope observation shelf {}.",
                if self.habitat_shelf.open { "opened" } else { "closed" }
            );
        }
''',
)
replace_once(
    app,
    "        let presentation_elapsed = if self.evidence_hold {\n",
    '''        self.last_screen_origin = screen_origin;
        self.last_screen_size = screen_size;
        let presentation_elapsed = if self.evidence_hold {
''',
)
replace_once(
    app,
    '''            self.pet.set_navigation_target(directive.target_screen);
            self.ecotope_snapshot = directive.snapshot;
''',
    '''            self.pet.set_navigation_target(directive.target_screen);
            self.ecotope_snapshot = directive.snapshot;
            self.ecotope_map = self.ecotope.observation_map();
''',
)
replace_once(
    app,
    "        if let Some(warning) = self.tray_warning.take() {\n",
    '''        if self.habitat_shelf.open {
            let skin = self.skin;
            let title = format!("{} — Screen Ecotope", skin.label());
            let snapshot = &self.ecotope_snapshot;
            let map = &self.ecotope_map;
            let source_identity = &self.source_identity;
            let shelf = &mut self.habitat_shelf;
            let shelf_position = [
                self.last_screen_origin.x + self.last_screen_size.x - 408.0,
                self.last_screen_origin.y + 48.0,
            ];
            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("mechofly-screen-ecotope-shelf-v1"),
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([380.0, 720.0])
                    .with_min_inner_size([320.0, 560.0])
                    .with_position(shelf_position)
                    .with_resizable(true)
                    .with_taskbar(true)
                    .with_window_level(egui::WindowLevel::AlwaysOnTop),
                |shelf_ui, _class| {
                    shelf_ui.ctx().request_repaint_after(Duration::from_millis(100));
                    if shelf_ui.input(|input| input.viewport().close_requested()) {
                        shelf.open = false;
                    }
                    shelf.draw(shelf_ui, snapshot, map, skin, source_identity);
                },
            );
        }

        if let Some(warning) = self.tray_warning.take() {
''',
)

desktop = "crates/mechofly-app/src/desktop_pet.rs"
replace_once(
    desktop,
    "const HOTKEY_BRAIN_LAB: i32 = 0x4D08;\n",
    "const HOTKEY_BRAIN_LAB: i32 = 0x4D08;\nconst HOTKEY_ECOTOPE_SHELF: i32 = 0x4D09;\n",
)
replace_once(desktop, "    BrainLab,\n}\n", "    BrainLab,\n    EcotopeShelf,\n}\n")
replace_once(
    desktop,
    "            Self::BrainLab => 1 << 7,\n",
    "            Self::BrainLab => 1 << 7,\n            Self::EcotopeShelf => 1 << 8,\n",
)
replace_once(
    desktop,
    "const HOTKEY_BINDINGS: [HotkeyBinding; 8] = [\n",
    "const HOTKEY_BINDINGS: [HotkeyBinding; 9] = [\n",
)
replace_once(
    desktop,
    '''    HotkeyBinding {
        id: HOTKEY_BRAIN_LAB,
        key: b'N' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::BrainLab,
        label: "Ctrl+Alt+N",
    },
];
''',
    '''    HotkeyBinding {
        id: HOTKEY_BRAIN_LAB,
        key: b'N' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::BrainLab,
        label: "Ctrl+Alt+N",
    },
    HotkeyBinding {
        id: HOTKEY_ECOTOPE_SHELF,
        key: b'E' as u32,
        modifiers: MOD_CONTROL | MOD_ALT,
        action: HotkeyAction::EcotopeShelf,
        label: "Ctrl+Alt+E",
    },
];
''',
)

self_test = "crates/mechofly-app/src/self_test.rs"
replace_once(
    self_test,
    '''    ecotope::run_ecotope_self_test,
    pet::{PetMotion, Skin, run_motion_self_test},
''',
    '''    ecotope::run_ecotope_self_test,
    habitat_shelf::run_habitat_shelf_self_test,
    pet::{PetMotion, Skin, run_motion_self_test},
''',
)
replace_once(
    self_test,
    '''    no_focus_activation: bool,
    firefly_visual_style: String,
''',
    '''    no_focus_activation: bool,
    habitat_shelf_available: bool,
    habitat_shelf_default_closed: bool,
    habitat_shelf_explicit_open_supported: bool,
    habitat_shelf_map_dimensions_valid: bool,
    habitat_shelf_art_has_no_resource_authority: bool,
    observe_hotkey_contract: bool,
    firefly_visual_style: String,
''',
)
replace_once(
    self_test,
    '''    let ecotope = run_ecotope_self_test();
    let clock_ethogram_removed =
''',
    '''    let ecotope = run_ecotope_self_test();
    let habitat_shelf = run_habitat_shelf_self_test();
    let clock_ethogram_removed =
''',
)
replace_once(
    self_test,
    "            && work_mode_safety.passed\n",
    '''            && work_mode_safety.passed
            && habitat_shelf.passed
            && hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E")
''',
)
replace_once(self_test, "        schema_version: 6,\n", "        schema_version: 7,\n")
replace_once(
    self_test,
    '''        no_focus_activation: work_mode_safety.no_focus_activation,
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
''',
    '''        no_focus_activation: work_mode_safety.no_focus_activation,
        habitat_shelf_available: true,
        habitat_shelf_default_closed: habitat_shelf.default_closed,
        habitat_shelf_explicit_open_supported: habitat_shelf.explicit_open_supported,
        habitat_shelf_map_dimensions_valid: habitat_shelf.map_dimensions_valid,
        habitat_shelf_art_has_no_resource_authority:
            habitat_shelf.authored_art_has_no_resource_authority,
        observe_hotkey_contract: hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E"),
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
''',
)
replace_once(
    self_test,
    '''            "Ctrl+Alt+N",
        ]
''',
    '''            "Ctrl+Alt+N",
            "Ctrl+Alt+E",
        ]
''',
)

workflow = ".github/workflows/windows.yml"
replace_once(
    workflow,
    "          if ($receipt.schema_version -ne 6) { throw 'Self-test receipt schema is stale.' }\n",
    "          if ($receipt.schema_version -ne 7) { throw 'Self-test receipt schema is stale.' }\n",
)
replace_once(
    workflow,
    "          if ($receipt.global_hotkey_count -ne 8) { throw 'Expected eight global hotkeys.' }\n",
    "          if ($receipt.global_hotkey_count -ne 9) { throw 'Expected nine global hotkeys.' }\n",
)
replace_once(
    workflow,
    "          if (-not $receipt.no_focus_activation) { throw 'Pet can activate and steal focus.' }\n",
    '''          if (-not $receipt.no_focus_activation) { throw 'Pet can activate and steal focus.' }
          if (-not $receipt.habitat_shelf_available) { throw 'Observe Mode Habitat Shelf is absent.' }
          if (-not $receipt.habitat_shelf_default_closed) { throw 'Habitat Shelf is not default closed.' }
          if (-not $receipt.habitat_shelf_explicit_open_supported) { throw 'Habitat Shelf cannot be explicitly opened.' }
          if (-not $receipt.habitat_shelf_map_dimensions_valid) { throw 'Habitat Shelf map dimensions are invalid.' }
          if (-not $receipt.habitat_shelf_art_has_no_resource_authority) { throw 'Shelf art can create ecological reward.' }
          if (-not $receipt.observe_hotkey_contract) { throw 'Ctrl+Alt+E Observe Mode contract is absent.' }
''',
)

start = "host-windows/Start-MechoFly.ps1"
replace_once(
    start,
    '''    [switch] $BrainLab,

    [switch] $ReducedMotion
''',
    '''    [switch] $BrainLab,

    [switch] $ObserveMode,

    [switch] $ReducedMotion
''',
)
replace_once(
    start,
    '''if ($BrainLab) {
    $Arguments += '--brain-lab'
}
''',
    '''if ($BrainLab) {
    $Arguments += '--brain-lab'
}
if ($ObserveMode) {
    $Arguments += '--observe-mode'
}
''',
)

docs = "docs/SCREEN_ECOTOPE.md"
text = read(docs)
addition = '''

## Observe Mode Habitat Shelf

The optional Habitat Shelf is default closed and opens only through the explicit `Ctrl+Alt+E` command or the `--observe-mode` launch option. It renders a peripheral, authored microhabitat view of the hidden Screen Ecotope: generated source, virtual plume, refuge, current target, fly location, spatial belief value, uncertainty, and visit counts.

Shelf rocks, leaf shapes, substrate, and other scenery are presentation only. They never create resource authority. The generated fermentation source remains the sole food stimulus in this milestone. Closing the Shelf does not reset or pause the hidden world or its deterministic learning state.
'''
if addition.strip() in text:
    raise SystemExit("docs: Observe Mode section already present")
write(docs, text + addition)

for path in [main, ecotope, app, desktop, self_test, workflow, start, docs]:
    text = read(path)
    for prohibited in ["ChatGPT", "OpenAI", "Codex"]:
        if prohibited in text:
            raise SystemExit(f"{path}: prohibited attribution marker {prohibited}")
