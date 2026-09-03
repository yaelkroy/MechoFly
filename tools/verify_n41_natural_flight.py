#!/usr/bin/env python3
"""Fail-closed source verification for the additive N4.1-C flight slice."""
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
        raise ValueError(f"natural-flight marker missing: {label}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    parameter_root = root / "crates/mechofly-core/parameters"

    observed = {}
    for name, expected in EXPECTED_PARAMETERS.items():
        value = hashlib.sha256((parameter_root / name).read_bytes()).hexdigest()
        if value != expected:
            raise ValueError(f"parameter identity changed: {name}")
        observed[name] = value

    parameters = (root / "crates/mechofly-core/src/behavior_parameters.rs").read_text(
        encoding="utf-8"
    )
    runtime = (root / "crates/mechofly-app/src/runtime.rs").read_text(encoding="utf-8")
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8")
    pet = (root / "crates/mechofly-app/src/pet.rs").read_text(encoding="utf-8")
    evidence = (root / "crates/mechofly-app/src/review_evidence.rs").read_text(
        encoding="utf-8"
    )
    collector = (root / "tools/Invoke-N41VisualReview.ps1").read_text(
        encoding="utf-8"
    )

    for text, marker, label in (
        (parameters, "N41BNaturalFlight", "additive profile enum"),
        (parameters, '"n41-b-natural-flight"', "explicit profile CLI"),
        (parameters, "flight_duration_quantiles_frames", "event-keyed flight quantiles"),
        (parameters, "N41_NATURAL_FLIGHT_DYNAMICS_VERSION", "flight dynamics identity"),
        (runtime, "BehaviorParameterProfile::N41BNaturalFlight", "review-only profile selection"),
        (app, "pet.natural_flight_motion = config.n41_visual_review_b", "runtime feature gate"),
        (pet, "natural_flight_motion: false", "canonical motor default"),
        (pet, "FlightPathProfile", "deterministic flight motor profile"),
        (pet, "FlightSaccade", "rapid flight turns"),
        (pet, "flight_maneuver_active", "direct maneuver-state evidence"),
        (pet, "hermite_position", "velocity-continuous landing path"),
        (pet, "hermite_velocity", "landing velocity evidence"),
        (pet, "FLIGHT_SPEED_MULTIPLIERS", "between-bout speed variation"),
        (pet, "uncued_exploratory_heading", "position-independent course selector"),
        (pet, "flight_edge_avoidance_turn_rate", "local edge avoidance"),
        (evidence, "movement_left", "normalized movement bounds"),
        (evidence, '"flight-takeoff.png"', "takeoff capture"),
        (evidence, '"flight-maneuver.png"', "maneuver capture"),
        (evidence, '"flight-touchdown.png"', "touchdown capture"),
        (collector, "flight_duration_cv", "flight-duration gate"),
        (collector, "flight_mean_speed_cv", "flight-speed gate"),
        (collector, "flight_saccades", "rapid-turn gate"),
        (collector, "flight_observed_horizontal_span_fraction", "horizontal span evidence"),
        (collector, "natural_flight_motion_all_bouts", "motor-gate attestation"),
        (collector, "Ctrl+Alt+L", "bounded owner-trigger protocol"),
        (collector, "deployment_authorized = $false", "deployment denial"),
    ):
        require(text, marker, label)

    canonical_constructor = "ModelEngine::new_duration_aware(Arc::clone(&graph), seed)"
    require(runtime, canonical_constructor, "canonical N4 constructor")
    if "natural_flight_motion = true" in pet:
        raise ValueError("natural flight motor is enabled inside the canonical motion default")
    for forbidden in (
        "$env:LOCALAPPDATA",
        "[System.IO.Path]::GetTempPath()",
        "CopyFromScreen",
        "Stop-MechoFly.ps1",
    ):
        if forbidden in collector:
            raise ValueError(f"review collector contains forbidden boundary: {forbidden}")

    for relative in (
        "crates/mechofly-core/tests/n41_natural_flight.rs",
        "docs/N4.1-Natural-Flight.md",
        ".github/workflows/n4-1-natural-flight.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required natural-flight artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "additive_feature_gated_n4_1_c_natural_flight_boundary",
        "canonical_application_default_profile": "n4",
        "accepted_r5_profile_frozen": "n41-b-natural",
        "candidate_profile": "n41-b-natural-flight",
        "flight_duration_selector": "event_keyed_128_quantile_table",
        "flight_speed_selector": "event_keyed_bout_profile_with_within_bout_texture",
        "flight_path_motifs": [
            "straight_segment",
            "rapid_saccade",
            "low_drift",
            "position_independent_uncued_course_selection",
            "local_edge_avoidance",
        ],
        "landing_path": "incoming_velocity_to_zero_velocity_cubic_hermite",
        "zero_input_escape_added": False,
        "parameter_sha256": observed,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(
        "N41_NATURAL_FLIGHT=PASS canonical=n4 accepted_r5=frozen "
        "candidate=n41-b-natural-flight deploy=false"
    )


if __name__ == "__main__":
    main()
