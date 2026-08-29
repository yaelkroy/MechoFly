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
replace_once(main, "mod diagnostics;\n", "mod desktop_terrain;\nmod diagnostics;\n")

ecotope = "crates/mechofly-app/src/ecotope.rs"
replace_once(
    ecotope,
    "use serde::{Deserialize, Serialize};\n\nconst BELIEF_WIDTH: usize = 32;\n",
    "use serde::{Deserialize, Serialize};\n\nuse crate::desktop_terrain::DesktopTerrainSnapshot;\n\nconst BELIEF_WIDTH: usize = 32;\n",
)
replace_once(
    ecotope,
    '''    pub transition_reason: &'static str,
}
''',
    '''    pub transition_reason: &'static str,
    pub desktop_sample_epoch: u64,
    pub desktop_visible_surface_count: usize,
    pub desktop_stable_surface_count: usize,
    pub desktop_edge_segment_count: usize,
    pub desktop_focused_window_fraction: f32,
    pub desktop_human_risk_at_fly: f32,
    pub desktop_geometry_claim: &'static str,
    pub desktop_content_capture: bool,
    pub desktop_semantic_resource_inference: bool,
}
''',
)
replace_once(
    ecotope,
    '''            transition_reason: "initial quiet wake",
        }
''',
    '''            transition_reason: "initial quiet wake",
            desktop_sample_epoch: 0,
            desktop_visible_surface_count: 0,
            desktop_stable_surface_count: 0,
            desktop_edge_segment_count: 0,
            desktop_focused_window_fraction: 0.0,
            desktop_human_risk_at_fly: 0.0,
            desktop_geometry_claim: "LOCAL_TOP_LEVEL_GEOMETRY_ONLY_NO_CONTENT_CAPTURE",
            desktop_content_capture: false,
            desktop_semantic_resource_inference: false,
        }
''',
)
replace_once(
    ecotope,
    '''        screen_size: Vec2,
        behavior: Behavior,
        cursor_threat: f32,
''',
    '''        screen_size: Vec2,
        terrain: &DesktopTerrainSnapshot,
        behavior: Behavior,
        cursor_threat: f32,
''',
)
replace_once(
    ecotope,
    '''        self.advance_world(delta_ms);
        self.advance_motivation(delta_ms, behavior);
''',
    '''        self.advance_world(delta_ms);
        self.advance_motivation(delta_ms, behavior);
        self.refuge_normalized = terrain.preferred_refuge_normalized;
''',
)
replace_once(
    ecotope,
    '''            fly_normalized,
        );
''',
    '''            fly_normalized,
            terrain,
        );
''',
)
replace_once(
    ecotope,
    '''                transition_reason: self.transition_reason,
            },
''',
    '''                transition_reason: self.transition_reason,
                desktop_sample_epoch: terrain.sample_epoch,
                desktop_visible_surface_count: terrain.visible_surface_count,
                desktop_stable_surface_count: terrain.stable_surface_count,
                desktop_edge_segment_count: terrain.edge_segment_count,
                desktop_focused_window_fraction: terrain.focused_window_fraction,
                desktop_human_risk_at_fly: terrain.human_risk_at_fly,
                desktop_geometry_claim: terrain.claim,
                desktop_content_capture: terrain.content_capture,
                desktop_semantic_resource_inference: terrain.semantic_resource_inference,
            },
''',
)
replace_once(
    ecotope,
    '''        fly_normalized: [f32; 2],
    ) -> (EcologicalMode, Option<[f32; 2]>, &'static str) {
''',
    '''        fly_normalized: [f32; 2],
        terrain: &DesktopTerrainSnapshot,
    ) -> (EcologicalMode, Option<[f32; 2]>, &'static str) {
''',
)
replace_once(
    ecotope,
    '''        let exploration_target = [
            0.08 + unit_interval(hash_x) * 0.84,
            0.10 + unit_interval(hash_y) * 0.76,
        ];
''',
    '''        let exploration_target = terrain.preferred_explore_normalized.unwrap_or([
            0.08 + unit_interval(hash_x) * 0.84,
            0.10 + unit_interval(hash_y) * 0.76,
        ]);
''',
)
replace_once(
    ecotope,
    "    let source_before = first.source_normalized;\n",
    "    let terrain = DesktopTerrainSnapshot::default();\n    let source_before = first.source_normalized;\n",
)
replace_once(
    ecotope,
    "            .step(100, fly, origin, screen, Behavior::Quiet, 0.0)\n",
    "            .step(100, fly, origin, screen, &terrain, Behavior::Quiet, 0.0)\n",
)
replace_once(
    ecotope,
    "            .step(100, fly, origin, screen, Behavior::Quiet, 0.0)\n",
    "            .step(100, fly, origin, screen, &terrain, Behavior::Quiet, 0.0)\n",
)

app = "crates/mechofly-app/src/app.rs"
replace_once(
    app,
    '''    diagnostics,
    ecotope::{CursorThreatEstimator, EcotopeMapSnapshot, EcotopeSnapshot, ScreenEcotope},
''',
    '''    desktop_terrain::{DesktopTerrainCensus, DesktopTerrainSnapshot},
    diagnostics,
    ecotope::{CursorThreatEstimator, EcotopeMapSnapshot, EcotopeSnapshot, ScreenEcotope},
''',
)
replace_once(
    app,
    '''    ecotope: ScreenEcotope,
    ecotope_snapshot: EcotopeSnapshot,
''',
    '''    desktop_terrain: DesktopTerrainCensus,
    desktop_terrain_snapshot: DesktopTerrainSnapshot,
    ecotope: ScreenEcotope,
    ecotope_snapshot: EcotopeSnapshot,
''',
)
replace_once(
    app,
    '''            policy,
            ecotope: ScreenEcotope::new(seed ^ 0x5343_5245_454E_4543),
''',
    '''            policy,
            desktop_terrain: DesktopTerrainCensus::default(),
            desktop_terrain_snapshot: DesktopTerrainSnapshot::default(),
            ecotope: ScreenEcotope::new(seed ^ 0x5343_5245_454E_4543),
''',
)
replace_once(
    app,
    "        let cursor_threat = self.cursor_threat.update(\n",
    '''        self.desktop_terrain_snapshot = self.desktop_terrain.update(
            presentation_elapsed.as_millis().min(250) as u32,
            screen_origin,
            screen_size,
            pet_center,
            cursor_position,
        );
        let cursor_threat = self.cursor_threat.update(
''',
)
replace_once(
    app,
    '''                screen_size,
                self.session.engine.state.behavior,
''',
    '''                screen_size,
                &self.desktop_terrain_snapshot,
                self.session.engine.state.behavior,
''',
)

brain = "crates/mechofly-app/src/brain_lab.rs"
replace_once(
    brain,
    '''                    ui.monospace(format!(
                        "VIRTUAL CUE {:.2}  ·  HUNGER {:.2}  ·  BELIEF {:.2} ± {:.2}  ·  EPOCH {}",
                        ecotope.cue_strength,
                        ecotope.hunger,
                        ecotope.best_expected_reward,
                        ecotope.best_uncertainty,
                        ecotope.source_epoch
                    ));
''',
    '''                    ui.monospace(format!(
                        "VIRTUAL CUE {:.2}  ·  HUNGER {:.2}  ·  BELIEF {:.2} ± {:.2}  ·  EPOCH {}",
                        ecotope.cue_strength,
                        ecotope.hunger,
                        ecotope.best_expected_reward,
                        ecotope.best_uncertainty,
                        ecotope.source_epoch
                    ));
                    ui.monospace(format!(
                        "DESKTOP SURFACES {}  ·  STABLE {}  ·  EDGES {}  ·  HUMAN RISK {:.2}",
                        ecotope.desktop_visible_surface_count,
                        ecotope.desktop_stable_surface_count,
                        ecotope.desktop_edge_segment_count,
                        ecotope.desktop_human_risk_at_fly
                    ));
''',
)

shelf = "crates/mechofly-app/src/habitat_shelf.rs"
replace_once(
    shelf,
    '''                        metric(ui, "Resource semantics", snapshot.resource_semantics);
''',
    '''                        metric(ui, "Resource semantics", snapshot.resource_semantics);
                        metric(
                            ui,
                            "Desktop surfaces",
                            &format!(
                                "{} visible · {} stable · {} edges",
                                snapshot.desktop_visible_surface_count,
                                snapshot.desktop_stable_surface_count,
                                snapshot.desktop_edge_segment_count
                            ),
                        );
                        metric(
                            ui,
                            "Human-risk proxy",
                            &format!("{:.3}", snapshot.desktop_human_risk_at_fly),
                        );
                        metric(ui, "Desktop evidence", snapshot.desktop_geometry_claim);
''',
)

self_test = "crates/mechofly-app/src/self_test.rs"
replace_once(
    self_test,
    '''    ecotope::run_ecotope_self_test,
    habitat_shelf::run_habitat_shelf_self_test,
''',
    '''    desktop_terrain::run_desktop_terrain_self_test,
    ecotope::run_ecotope_self_test,
    habitat_shelf::run_habitat_shelf_self_test,
''',
)
replace_once(
    self_test,
    '''    observe_hotkey_contract: bool,
    firefly_visual_style: String,
''',
    '''    observe_hotkey_contract: bool,
    desktop_terrain_enabled: bool,
    desktop_terrain_deterministic_summary: bool,
    desktop_terrain_stable_edges_available: bool,
    desktop_terrain_peripheral_refuge_selected: bool,
    desktop_terrain_no_content_capture: bool,
    desktop_terrain_no_semantic_resource_inference: bool,
    desktop_terrain_claim: String,
    firefly_visual_style: String,
''',
)
replace_once(
    self_test,
    '''    let ecotope = run_ecotope_self_test();
    let habitat_shelf = run_habitat_shelf_self_test();
''',
    '''    let desktop_terrain = run_desktop_terrain_self_test();
    let ecotope = run_ecotope_self_test();
    let habitat_shelf = run_habitat_shelf_self_test();
''',
)
replace_once(
    self_test,
    '''            && hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E")
''',
    '''            && hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E")
            && desktop_terrain.passed
''',
)
replace_once(self_test, "        schema_version: 7,\n", "        schema_version: 8,\n")
replace_once(
    self_test,
    '''        observe_hotkey_contract: hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E"),
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
''',
    '''        observe_hotkey_contract: hotkeys.labels.iter().any(|label| label == "Ctrl+Alt+E"),
        desktop_terrain_enabled: true,
        desktop_terrain_deterministic_summary: desktop_terrain.deterministic_summary,
        desktop_terrain_stable_edges_available: desktop_terrain.stable_edges_available,
        desktop_terrain_peripheral_refuge_selected:
            desktop_terrain.peripheral_refuge_selected,
        desktop_terrain_no_content_capture: desktop_terrain.no_content_capture,
        desktop_terrain_no_semantic_resource_inference:
            desktop_terrain.no_semantic_resource_inference,
        desktop_terrain_claim: "LOCAL_TOP_LEVEL_GEOMETRY_ONLY_NO_CONTENT_CAPTURE"
            .to_owned(),
        firefly_visual_style: "recorded_legacy_prism_port_v6".to_owned(),
''',
)

workflow = ".github/workflows/windows.yml"
replace_once(
    workflow,
    "          if ($receipt.schema_version -ne 7) { throw 'Self-test receipt schema is stale.' }\n",
    "          if ($receipt.schema_version -ne 8) { throw 'Self-test receipt schema is stale.' }\n",
)
replace_once(
    workflow,
    "          if (-not $receipt.observe_hotkey_contract) { throw 'Ctrl+Alt+E Observe Mode contract is absent.' }\n",
    '''          if (-not $receipt.observe_hotkey_contract) { throw 'Ctrl+Alt+E Observe Mode contract is absent.' }
          if (-not $receipt.desktop_terrain_enabled) { throw 'Desktop terrain census is absent.' }
          if (-not $receipt.desktop_terrain_deterministic_summary) { throw 'Desktop terrain summary is not deterministic.' }
          if (-not $receipt.desktop_terrain_stable_edges_available) { throw 'Stable desktop edges are unavailable.' }
          if (-not $receipt.desktop_terrain_peripheral_refuge_selected) { throw 'Desktop refuge selection is not peripheral.' }
          if (-not $receipt.desktop_terrain_no_content_capture) { throw 'Desktop terrain captures content.' }
          if (-not $receipt.desktop_terrain_no_semantic_resource_inference) { throw 'Desktop UI content can become a resource.' }
''',
)

docs = "docs/SCREEN_ECOTOPE.md"
text = read(docs)
addition = '''

## Local structural desktop terrain census

The first desktop census samples only visible top-level window rectangles, focus identity, surface persistence, and virtual-screen geometry. It does not request window titles, control text, screenshots, OCR, URLs, file names, clipboard data, or typed content.

Stable rectangle edges become possible landmarks for exploration, and low-risk peripheral geometry becomes a refuge prior. This is a geometric analogy only. Ordinary windows, icons, images, and colors never become food, water, or shelter through semantic inference. Focus and cursor proximity contribute to a human-risk proxy shown in Brain Lab and the Habitat Shelf; this increment does not yet scan focused controls through UI Automation or enforce control-level no-go regions.
'''
if addition.strip() in text:
    raise SystemExit("docs: desktop terrain section already present")
write(docs, text + addition)

for path in [main, ecotope, app, brain, shelf, self_test, workflow, docs]:
    text = read(path)
    for prohibited in ["ChatGPT", "OpenAI", "Codex"]:
        if prohibited in text:
            raise SystemExit(f"{path}: prohibited attribution marker {prohibited}")
