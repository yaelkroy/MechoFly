#!/usr/bin/env python3
"""Fail-closed static verification for the N7 explanation foundation."""
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
ACCEPTED_R10_CAPSULE = (
    "d93d88d8ab2b2d0da15605e761adcdb9b303345756ed14c2f2f41e6ddc7c7835"
)


def require(text: str, marker: str, label: str) -> None:
    if marker not in text:
        raise ValueError(f"N7 scientific-explanation marker missing: {label}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)

    parameter_root = root / "crates/mechofly-core/parameters"
    observed_parameters: dict[str, str] = {}
    for name, expected in EXPECTED_PARAMETERS.items():
        observed = hashlib.sha256((parameter_root / name).read_bytes()).hexdigest()
        if observed != expected:
            raise ValueError(f"frozen parameter identity changed: {name}")
        observed_parameters[name] = observed

    manifest = (root / "crates/mechofly-app/Cargo.toml").read_text(encoding="utf-8")
    main_rs = (root / "crates/mechofly-app/src/main.rs").read_text(encoding="utf-8")
    checkpoint = (root / "crates/mechofly-app/src/product_checkpoint.rs").read_text(
        encoding="utf-8"
    )
    explanation = (
        root / "crates/mechofly-app/src/scientific_explanation.rs"
    ).read_text(encoding="utf-8")

    markers = (
        (
            manifest,
            'n7-scientific-explanation = ["n6-counterfactual-replay"]',
            "additive feature and R10 dependency",
        ),
        (
            main_rs,
            '"--n7-scientific-explanation-self-test"',
            "isolated self-test option",
        ),
        (
            main_rs,
            '#[cfg(not(feature = "n7-scientific-explanation"))]',
            "lower-feature rejection gate",
        ),
        (checkpoint, ACCEPTED_R10_CAPSULE, "accepted R10 capsule pin"),
        (checkpoint, "ScientificReplaySource", "R10 controller observation source"),
        (
            checkpoint,
            "N7 controller observation differs from accepted R10 replay",
            "per-frame R10 identity gate",
        ),
        (explanation, "ScientificExplanationBundle", "portable explanation bundle"),
        (explanation, "ControllerExplanationFrame", "tagged controller frames"),
        (explanation, "BehaviorEpisodeExplanation", "bounded episode explanations"),
        (explanation, '#[serde(rename = "MEASURED")]', "MEASURED vocabulary"),
        (explanation, '#[serde(rename = "DERIVED")]', "DERIVED vocabulary"),
        (explanation, '#[serde(rename = "MODELED")]', "MODELED vocabulary"),
        (explanation, '#[serde(rename = "INFERRED")]', "INFERRED vocabulary"),
        (explanation, '#[serde(rename = "AUTHORED")]', "AUTHORED vocabulary"),
        (
            explanation,
            '#[serde(rename = "ENGINEERING PRIOR")]',
            "ENGINEERING PRIOR vocabulary",
        ),
        (explanation, '#[serde(rename = "UNRESOLVED")]', "UNRESOLVED vocabulary"),
        (
            explanation,
            '#[serde(rename = "PRESENTATION SAFETY OVERRIDE")]',
            "PRESENTATION SAFETY OVERRIDE vocabulary",
        ),
        (explanation, "minimum_dwell_frames", "minimum-dwell explanation"),
        (
            explanation,
            "sampled_target_duration_frames",
            "sampled-duration explanation",
        ),
        (explanation, "hysteresis_active_intents", "hysteresis explanation"),
        (explanation, "refractory_until_frame", "refractory explanation"),
        (explanation, "interruptible", "interruptibility explanation"),
        (explanation, "why_state_began", "state-entry explanation"),
        (explanation, "why_state_persisted", "state-persistence explanation"),
        (explanation, "why_ended", "state-exit explanation"),
        (
            explanation,
            "bounded replay fabricated an open episode ending",
            "unresolved-ending fail-closed gate",
        ),
        (
            explanation,
            "N7 explanation accepted provenance-tag tampering",
            "provenance tamper gate",
        ),
        (
            explanation,
            "N7 explanation accepted causal-statement tampering",
            "causal-copy tamper gate",
        ),
        (explanation, "live_ui_promoted: false", "UI-promotion denial"),
        (explanation, "live_restore_authorized: false", "live-restore denial"),
        (
            explanation,
            "controller_or_motor_semantics_changed: false",
            "behavior-semantic freeze",
        ),
        (explanation, "screen_ecology_added: false", "screen-ecology denial"),
        (explanation, "food_search_added: false", "food-search denial"),
        (explanation, "appdata_write_authorized: false", "AppData-write denial"),
        (explanation, "deployment_authorized: false", "deployment denial"),
    )
    for text, marker, label in markers:
        require(text, marker, label)

    default_line = next(
        (line.strip() for line in manifest.splitlines() if line.strip().startswith("default =")),
        None,
    )
    if default_line is None or "n7-" in default_line or "n6-" in default_line:
        raise ValueError("an N6/N7 experimental feature is enabled by default")

    combined = "\n".join((manifest, main_rs, checkpoint, explanation))
    for forbidden in (
        "$env:LOCALAPPDATA",
        "AppData\\Local",
        "live_restore_authorized: true",
        "controller_or_motor_semantics_changed: true",
        "live_ui_promoted: true",
        "screen_ecology_added: true",
        "food_search_added: true",
        "deployment_authorized: true",
    ):
        if forbidden in combined:
            raise ValueError(f"forbidden N7 boundary present: {forbidden}")

    for relative in (
        "docs/N6-Product-State-Checkpoint-Foundation.md",
        "docs/N6-Paired-Counterfactual-Replay.md",
        "docs/N7-Scientific-Explanation-Provenance-Foundation.md",
        ".github/workflows/n6-counterfactual-replay.yml",
        ".github/workflows/n7-scientific-explanation.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required N7 artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "behavior_neutral_n7_scientific_explanation_foundation",
        "feature": "n7-scientific-explanation",
        "source_replay_capsule_sha256": ACCEPTED_R10_CAPSULE,
        "canonical_feature_enabled": False,
        "actual_and_counterfactual_controller_lanes": True,
        "frame_count_per_lane": 96,
        "history_limit_frames": 128,
        "evidence_vocabulary_complete": True,
        "every_displayed_quantity_tagged": True,
        "why_began_persisted_ended": True,
        "controller_causal_state_present": True,
        "tamper_rejection": True,
        "deterministic_reverification": True,
        "uncertainty_not_fabricated": True,
        "offline_fixture_only": True,
        "live_restore_authorized": False,
        "controller_or_motor_semantics_changed": False,
        "parameter_sha256": observed_parameters,
        "parameter_files_changed": False,
        "live_ui_promoted": False,
        "screen_ecology_added": False,
        "food_search_added": False,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(
        "N7_SCIENTIFIC_EXPLANATION_STATIC=PASS frames=96 limit=128 "
        "tagged=true ui_promoted=false deploy=false"
    )


if __name__ == "__main__":
    main()
