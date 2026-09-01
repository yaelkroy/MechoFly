#!/usr/bin/env python3
"""Fail-closed static verification for the N4.1-B live visual-review slice."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


EXPECTED_N4 = "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b"
EXPECTED_N41_B = "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138"
EXPECTED_N41_B_NATURAL = "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c"


def require(text: str, marker: str, label: str) -> None:
    if marker not in text:
        raise ValueError(f"visual-review integration marker missing: {label}")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)

    n4 = root / "crates/mechofly-core/parameters/n4-engineering-v1.json"
    n41_b = root / "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-balanced-v1.json"
    n41_b_natural = root / "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-natural-bouts-v2.json"
    if sha256(n4) != EXPECTED_N4:
        raise ValueError("frozen N4 parameter artifact changed")
    if sha256(n41_b) != EXPECTED_N41_B:
        raise ValueError("winning N4.1-B parameter artifact changed")
    if sha256(n41_b_natural) != EXPECTED_N41_B_NATURAL:
        raise ValueError("N4.1-B natural-bout parameter artifact changed")

    cargo = (root / "crates/mechofly-app/Cargo.toml").read_text(encoding="utf-8")
    main_rs = (root / "crates/mechofly-app/src/main.rs").read_text(encoding="utf-8")
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8")
    runtime = (root / "crates/mechofly-app/src/runtime.rs").read_text(encoding="utf-8")
    diagnostics = (root / "crates/mechofly-app/src/diagnostics.rs").read_text(
        encoding="utf-8"
    )
    storage = (root / "crates/mechofly-app/src/storage.rs").read_text(encoding="utf-8")
    desktop_pet = (root / "crates/mechofly-app/src/desktop_pet.rs").read_text(
        encoding="utf-8"
    )
    collector = (root / "tools/Invoke-N41VisualReview.ps1").read_text(encoding="utf-8")
    measure = (root / "tools/Measure-N41NaturalMotionEvidence.ps1").read_text(
        encoding="utf-8"
    )
    metrics_regression = (root / "tools/Test-N41VisualReviewMetrics.ps1").read_text(
        encoding="utf-8"
    )
    evidence = (root / "crates/mechofly-app/src/review_evidence.rs").read_text(
        encoding="utf-8"
    )

    require(cargo, "default = []", "empty default feature set")
    require(cargo, 'n41-visual-review-b = ["dep:png"]', "opt-in visual-review feature")
    require(
        runtime,
        "ModelEngine::new_duration_aware(Arc::clone(&graph), seed)",
        "canonical N4 constructor",
    )
    require(
        runtime,
        '#[cfg(feature = "n41-visual-review-b")]\n    pub fn calibrated_n41_visual_review_b(',
        "compile-time review gate",
    )
    require(runtime, "BehaviorParameterProfile::N41BNatural", "pinned natural-bout selection")
    require(main_rs, '"--n41-b-visual-review"', "explicit runtime review flag")
    require(
        main_rs,
        '"--n41-visual-review-receipt"',
        "candidate launch receipt",
    )
    require(
        main_rs,
        'canonical_default_profile: "n4"',
        "canonical default attestation",
    )
    require(main_rs, 'active_profile: "n41-b-natural"', "active review attestation")
    require(main_rs, 'promotion_authorized: false', "promotion denial")
    require(main_rs, 'deployment_authorized: false', "deployment denial")
    require(app, "SimulationSession::calibrated(", "ordinary application path")
    require(
        app,
        "SimulationSession::calibrated_n41_visual_review_b(",
        "feature-gated application path",
    )
    require(
        app,
        '"MechoFly N4.1-B visual review pet"',
        "visibly labeled candidate window",
    )
    require(desktop_pet, "pub fn new(position: Pos2, title: &str)", "explicit pet title")
    require(storage, 'var_os("MECHOFLY_DATA_DIR")', "isolated storage override")
    require(diagnostics, "crate::storage::override_directory()", "redirected diagnostics")
    require(main_rs, "storage::override_directory()", "redirected runtime profile")
    require(app, "crate::storage::override_directory()", "redirected application state")
    require(evidence, "review-trace.jsonl", "direct motion trace")
    require(evidence, "review-captures", "direct pet captures")
    require(evidence, "composite_bgra_pixel", "constant-backdrop compositor")

    for marker, label in (
        ("$EarlyBoundarySeconds = 30", "30-second early boundary"),
        ("$LateBoundarySeconds = 300", "five-minute late start"),
        ("$FinalBoundarySeconds = 600", "ten-minute final boundary"),
        ("Save-PrivacySafePetCapture", "privacy-safe capture"),
        ("Get-PhaseRatings", "explicit phase ratings"),
        ("ACCEPTED_FOR_GUARDED_NEXT_STEP", "guarded acceptance decision"),
        ("full_desktop_captured = $false", "desktop privacy denial"),
        ("Get-NaturalMotionMetrics", "objective natural-motion gate"),
        ("Stop-ReviewCandidate -Process $process", "trace-writer close before analysis"),
        ("walk_duration_cv", "duration-distribution evidence"),
        ("walk_mean_speed_cv", "speed-distribution evidence"),
        ("[AllowEmptyCollection()]", "empty-collection parameter binding repair"),
        ("deployment_authorized = $false", "collector deployment denial"),
    ):
        require(collector, marker, label)
    if collector.count("[AllowEmptyCollection()]") < 5:
        raise ValueError("visual collector does not protect every supported empty collection")
    for text, marker, label in (
        (measure, "Parser]::ParseFile", "offline collector-function loading"),
        (measure, "Get-NaturalMotionMetrics", "offline objective measurement"),
        (measure, "trace_sha256", "trace identity"),
        (measure, "deployment_authorized = $false", "offline deployment denial"),
        (metrics_regression, "zero-bout", "zero-bout regression"),
        (metrics_regression, "first-bout", "first-bout regression"),
        (
            metrics_regression,
            "first_bout_added_to_empty_list = $true",
            "empty-list first-bout assertion",
        ),
    ):
        require(text, marker, label)
    forbidden_collector_markers = (
        "$env:LOCALAPPDATA",
        "[System.IO.Path]::GetTempPath()",
        "[System.IO.Path]::GetTempFileName()",
        "Stop-MechoFly.ps1",
        "CopyFromScreen",
    )
    for marker in forbidden_collector_markers:
        if marker in collector or marker in measure or marker in metrics_regression:
            raise ValueError(f"visual collector contains forbidden boundary: {marker}")

    required_files = (
        "docs/N4.1-Visual-Acceptance.md",
        ".github/workflows/n4-1-visual-review.yml",
        "tools/Measure-N41NaturalMotionEvidence.ps1",
        "tools/Test-N41VisualReviewMetrics.ps1",
    )
    for relative in required_files:
        if not (root / relative).is_file():
            raise ValueError(f"required visual-review artifact is missing: {relative}")

    receipt = {
        "schema_version": 3,
        "status": "PASS",
        "classification": "feature_gated_n4_1_b_live_visual_review_boundary",
        "canonical_application_default_profile": "n4",
        "review_profile": "n41-b-natural",
        "review_feature": "n41-visual-review-b",
        "review_flag": "--n41-b-visual-review",
        "n4_parameter_sha256": EXPECTED_N4,
        "n41_b_parameter_sha256": EXPECTED_N41_B,
        "n41_b_natural_parameter_sha256": EXPECTED_N41_B_NATURAL,
        "storage_override": "MECHOFLY_DATA_DIR",
        "early_boundary_seconds": 30,
        "late_window_start_seconds": 300,
        "late_window_end_seconds": 600,
        "full_desktop_capture": False,
        "offline_metrics_replay": True,
        "empty_collection_regression": True,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("N41_VISUAL_REVIEW_INTEGRATION=PASS canonical=n4 review=n41-b-natural deploy=false")


if __name__ == "__main__":
    main()
