#!/usr/bin/env python3
"""Fail-closed static verification for the N4.1-B live visual-review slice."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


EXPECTED_N4 = "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b"
EXPECTED_N41_B = "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138"


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
    if sha256(n4) != EXPECTED_N4:
        raise ValueError("frozen N4 parameter artifact changed")
    if sha256(n41_b) != EXPECTED_N41_B:
        raise ValueError("winning N4.1-B parameter artifact changed")

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

    require(cargo, "default = []", "empty default feature set")
    require(cargo, "n41-visual-review-b = []", "opt-in visual-review feature")
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
    require(runtime, "BehaviorParameterProfile::N41B", "pinned N4.1-B selection")
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
    require(main_rs, 'active_profile: "n41-b"', "active review attestation")
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

    for marker, label in (
        ("$EarlyBoundarySeconds = 30", "30-second early boundary"),
        ("$LateBoundarySeconds = 300", "five-minute late start"),
        ("$FinalBoundarySeconds = 600", "ten-minute final boundary"),
        ("Save-PrivacySafePetCapture", "privacy-safe capture"),
        ("Get-PhaseRatings", "explicit phase ratings"),
        ("ACCEPTED_FOR_GUARDED_NEXT_STEP", "guarded acceptance decision"),
        ("full_desktop_captured = $false", "desktop privacy denial"),
        ("deployment_authorized = $false", "collector deployment denial"),
    ):
        require(collector, marker, label)
    forbidden_collector_markers = (
        "$env:LOCALAPPDATA",
        "[System.IO.Path]::GetTempPath()",
        "[System.IO.Path]::GetTempFileName()",
        "Stop-MechoFly.ps1",
    )
    for marker in forbidden_collector_markers:
        if marker in collector:
            raise ValueError(f"visual collector contains forbidden boundary: {marker}")

    required_files = (
        "docs/N4.1-Visual-Acceptance.md",
        ".github/workflows/n4-1-visual-review.yml",
    )
    for relative in required_files:
        if not (root / relative).is_file():
            raise ValueError(f"required visual-review artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "feature_gated_n4_1_b_live_visual_review_boundary",
        "canonical_application_default_profile": "n4",
        "review_profile": "n41-b",
        "review_feature": "n41-visual-review-b",
        "review_flag": "--n41-b-visual-review",
        "n4_parameter_sha256": EXPECTED_N4,
        "n41_b_parameter_sha256": EXPECTED_N41_B,
        "storage_override": "MECHOFLY_DATA_DIR",
        "early_boundary_seconds": 30,
        "late_window_start_seconds": 300,
        "late_window_end_seconds": 600,
        "full_desktop_capture": False,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("N41_VISUAL_REVIEW_INTEGRATION=PASS canonical=n4 review=n41-b deploy=false")


if __name__ == "__main__":
    main()
