#!/usr/bin/env python3
"""Fail-closed source verification for the N4.1-B natural-motion correction."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


EXPECTED_N41_B = "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138"
EXPECTED_N41_B_NATURAL = "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c"


def require(text: str, marker: str, label: str) -> None:
    if marker not in text:
        raise ValueError(f"natural-motion marker missing: {label}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)

    parameter = root / "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-balanced-v1.json"
    observed_parameter = hashlib.sha256(parameter.read_bytes()).hexdigest()
    if observed_parameter != EXPECTED_N41_B:
        raise ValueError("N4.1-B controller parameter artifact changed")
    natural_parameter = root / "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-natural-bouts-v2.json"
    observed_natural_parameter = hashlib.sha256(natural_parameter.read_bytes()).hexdigest()
    if observed_natural_parameter != EXPECTED_N41_B_NATURAL:
        raise ValueError("N4.1-B natural-bout parameter artifact changed")

    parameters = (root / "crates/mechofly-core/src/behavior_parameters.rs").read_text(
        encoding="utf-8"
    )
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8")
    pet = (root / "crates/mechofly-app/src/pet.rs").read_text(encoding="utf-8")
    grooming = (root / "crates/mechofly-core/src/grooming_program.rs").read_text(
        encoding="utf-8"
    )
    evidence = (root / "crates/mechofly-app/src/review_evidence.rs").read_text(
        encoding="utf-8"
    )
    collector = (root / "tools/Invoke-N41VisualReview.ps1").read_text(
        encoding="utf-8"
    )
    measure = (root / "tools/Measure-N41NaturalMotionEvidence.ps1").read_text(
        encoding="utf-8"
    )
    metrics_regression = (root / "tools/Test-N41VisualReviewMetrics.ps1").read_text(
        encoding="utf-8"
    )
    probe = (root / "crates/mechofly-core/examples/n41_live_profile.rs").read_text(
        encoding="utf-8"
    )

    required = (
        (app, "synchronize_native_drag(events.dragging, events.position)", "drag-only native synchronization"),
        (pet, "high_refresh_native_round_trip_translation_pixels", "240 Hz rounding regression"),
        (pet, "GroundPathMotif", "straight/curve/sharp-turn path motifs"),
        (pet, "GroundSpeedProfile", "event-keyed bout speed profile"),
        (pet, "WALK_SPEED_ONSET_SECONDS", "translated turn onset"),
        (parameters, "walk_duration_quantiles_frames", "versioned walk quantiles"),
        (parameters, "N41_NATURAL_BOUT_DYNAMICS_VERSION", "natural-bout dynamics identity"),
        (grooming, "HeadSweep", "head grooming"),
        (grooming, "ForelegRub", "foreleg rubbing"),
        (grooming, "AbdomenBrush", "posterior grooming"),
        (grooming, "WingClean", "wing grooming"),
        (evidence, "No screen API is used", "screen-independent evidence"),
        (collector, "Get-NaturalMotionMetrics", "objective live gate"),
        (collector, "walk_duration_distance_r_squared", "clockwork correlation gate"),
        (collector, "[AllowEmptyCollection()]", "empty-collection binding repair"),
        (collector, "Stop-ReviewCandidate -Process $process", "trace writer close"),
        (measure, "trace_sha256", "offline evidence identity"),
        (measure, "Get-NaturalMotionMetrics", "offline metrics replay"),
        (metrics_regression, "zero-bout", "zero-bout metrics regression"),
        (metrics_regression, "first-bout", "first-bout metrics regression"),
        (probe, "APP_MODEL_SEED", "exact application seed probe"),
        (probe, '"extended-65536"', "exact likely Windows tier probe"),
    )
    for text, marker, label in required:
        require(text, marker, label)
    if collector.count("[AllowEmptyCollection()]") < 5:
        raise ValueError("natural-motion collector does not protect supported empty collections")

    forbidden = (
        (collector, "CopyFromScreen", "desktop screen capture"),
        (collector, "$env:LOCALAPPDATA", "AppData mutation"),
        (measure, "$env:LOCALAPPDATA", "offline AppData mutation"),
        (metrics_regression, "$env:LOCALAPPDATA", "regression AppData mutation"),
        (app, "self.pet.screen_position = events.position", "integer round-trip overwrite"),
    )
    for text, marker, label in forbidden:
        if marker in text:
            raise ValueError(f"forbidden natural-motion boundary present: {label}")
    stop_index = collector.find("Stop-ReviewCandidate -Process $process")
    metrics_index = collector.find("$objectiveMetrics = Get-NaturalMotionMetrics")
    if stop_index < 0 or metrics_index < 0 or stop_index > metrics_index:
        raise ValueError("owned candidate is not stopped before trace analysis")

    for relative in (
        "docs/N4.1-Natural-Motion-Correction.md",
        ".github/workflows/n4-1-natural-motion.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required natural-motion artifact is missing: {relative}")

    receipt = {
        "schema_version": 2,
        "status": "PASS",
        "classification": "n4_1_b_natural_bout_controller_presentation_and_evidence_correction",
        "frozen_n41_b_parameter_sha256": observed_parameter,
        "natural_bout_parameter_sha256": observed_natural_parameter,
        "walk_duration_selector": "event_keyed_128_quantile_table",
        "walk_speed_selector": "event_keyed_bout_profile_within_bout_texture",
        "native_position_synchronization": "drag_only",
        "ground_path_motifs": ["straight", "curve_left", "curve_right", "sharp_turn_left", "sharp_turn_right"],
        "grooming_motor_substates": ["prepare", "head_sweep", "foreleg_rub", "abdomen_brush", "wing_clean", "reset"],
        "screen_capture_used": False,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("N41_NATURAL_MOTION=PASS frozen_parameter_unchanged=true natural_profile=versioned deploy=false")


if __name__ == "__main__":
    main()
