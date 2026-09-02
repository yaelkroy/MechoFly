#!/usr/bin/env python3
"""Fail-closed verification for the additive N4.1-D exploratory-flight correction."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


EXPECTED_PARAMETERS = {
    "n4-engineering-v1.json": "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b",
    "n4.1-soft-fatigue-a-responsive-v1.json": "94350dcaa0755fce9fca2d8c3d429eb54c0b4aa370c7cf56bfc4236bb7339615",
    "n4.1-soft-fatigue-b-balanced-v1.json": "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138",
    "n4.1-soft-fatigue-b-natural-bouts-v2.json": "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c",
    "n4.1-soft-fatigue-b-natural-flight-v3.json": "cb3cd2654dcd4fa9def34fb0145645f5d61b59c96c407669cf1e9dd4f12628ef",
    "n4.1-soft-fatigue-c-conservative-v1.json": "b1296cd9640a39852dfa5d8cba2387798fbe681869dc53b8fd24224225f0a18d",
}


def require(text: str, marker: str, label: str) -> None:
    if marker not in text:
        raise ValueError(f"exploratory-flight marker missing: {label}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)

    parameter_root = root / "crates/mechofly-core/parameters"
    observed_parameters = {}
    for name, expected in EXPECTED_PARAMETERS.items():
        observed = hashlib.sha256((parameter_root / name).read_bytes()).hexdigest()
        if observed != expected:
            raise ValueError(f"frozen parameter identity changed: {name}")
        observed_parameters[name] = observed

    parameters = (root / "crates/mechofly-core/src/behavior_parameters.rs").read_text(
        encoding="utf-8"
    )
    pet = (root / "crates/mechofly-app/src/pet.rs").read_text(encoding="utf-8")
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8")
    evidence = (root / "crates/mechofly-app/src/review_evidence.rs").read_text(
        encoding="utf-8"
    )
    collector = (root / "tools/Invoke-N41VisualReview.ps1").read_text(
        encoding="utf-8"
    )
    regression = (root / "tools/Test-N41VisualReviewMetrics.ps1").read_text(
        encoding="utf-8"
    )

    markers = (
        (
            parameters,
            "n4.1-literature-shaped-walk-flight-bouts-uncued-exploration-prior-v2",
            "versioned exploratory motor identity",
        ),
        (
            parameters,
            "not a food-search or territory-coverage model",
            "biological claim boundary",
        ),
        (pet, "uncued_exploratory_heading", "position-independent course selector"),
        (pet, "uncued_course_selection_saccade", "course-selection saccade"),
        (pet, "FLIGHT_COURSE_SELECTION_MAX_RADIANS", "bounded course selection"),
        (pet, "flight_edge_avoidance_turn_rate", "local edge avoidance"),
        (pet, "profile.saccade_active(age_seconds)", "edge response paused during saccades"),
        (
            pet,
            "uncued_course_selection_is_deterministic_and_position_independent",
            "position-independence regression",
        ),
        (
            pet,
            "repeated_cursor_escape_heading_does_not_create_a_one_side_motor_lock",
            "repeated-heading regression",
        ),
        (pet, "natural_flight_motion: false", "canonical motor default"),
        (app, "screen_origin,\n                screen_size,", "trace-bound propagation"),
        (evidence, "movement_left", "normalized movement left bound"),
        (evidence, "movement_right", "normalized movement right bound"),
        (collector, "flight_observed_horizontal_span_fraction", "span measurement"),
        (collector, "flight_horizontal_tertiles_visited", "tertile measurement"),
        (collector, "$flightHorizontalTertilesVisited -ge 2", "anti-confinement gate"),
        (collector, "$flightLeftwardBouts -ge 1", "leftward bout gate"),
        (collector, "$flightRightwardBouts -ge 1", "rightward bout gate"),
        (collector, "no screen-coverage target or threshold", "coverage-goal denial"),
        (
            regression,
            "confined_flight_detected_by_anti_confinement_metrics",
            "confined trace regression",
        ),
        (
            regression,
            "exploratory_flight_directional_metrics_passed",
            "directional trace regression",
        ),
    )
    for text, marker, label in markers:
        require(text, marker, label)

    for forbidden in (
        "flight_exploration_target",
        "FLIGHT_EXPLORATION_GUIDANCE",
        "$flightHorizontalCoverage -ge",
        "$flightHorizontalTertilesVisited -eq 3",
    ):
        if forbidden in pet or forbidden in collector:
            raise ValueError(f"forbidden territory-coverage mechanism present: {forbidden}")

    if "natural_flight_motion = true" in pet:
        raise ValueError("exploratory flight was enabled in the canonical motion default")
    for forbidden in (
        "$env:LOCALAPPDATA",
        "[System.IO.Path]::GetTempPath()",
        "[System.IO.Path]::GetTempFileName()",
        "CopyFromScreen",
        "Stop-MechoFly.ps1",
    ):
        if forbidden in collector or forbidden in regression:
            raise ValueError(f"review artifact contains forbidden boundary: {forbidden}")

    for relative in (
        "docs/N4.1-Uncued-Exploratory-Flight.md",
        ".github/workflows/n4-1-exploratory-flight.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required exploratory-flight artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "additive_feature_gated_n4_1_d_uncued_exploratory_flight",
        "parent_candidate": "n4_1_c_natural_flight_r6_r2_rejected_by_owner",
        "canonical_application_default_profile": "n4",
        "accepted_r5_profile_frozen": "n41-b-natural",
        "candidate_profile": "n41-b-natural-flight",
        "flight_motor": {
            "course_selection": "position_independent_deterministic_uncued_heading",
            "turn_motif": "bounded_first_saccade",
            "edge_response": "local_inward_avoidance_only",
            "screen_region_target": False,
            "food_search_claim": False,
            "territory_coverage_objective": False,
        },
        "anti_confinement_gate": {
            "minimum_complete_flights": 10,
            "minimum_horizontal_tertiles": 2,
            "minimum_leftward_bouts": 1,
            "minimum_rightward_bouts": 1,
            "minimum_screen_coverage": None,
        },
        "parameter_sha256": observed_parameters,
        "parameter_files_changed": False,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(
        "N41_EXPLORATORY_FLIGHT=PASS canonical=n4 accepted_r5=frozen "
        "coverage_objective=false deploy=false"
    )


if __name__ == "__main__":
    main()
