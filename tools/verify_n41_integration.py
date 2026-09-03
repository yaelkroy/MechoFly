#!/usr/bin/env python3
"""Static fail-closed checks for the isolated N4.1 experiment/review boundary."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

EXPECTED = {
    "crates/mechofly-core/parameters/n4-engineering-v1.json": "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b",
    "crates/mechofly-core/parameters/n4.1-soft-fatigue-a-responsive-v1.json": "94350dcaa0755fce9fca2d8c3d429eb54c0b4aa370c7cf56bfc4236bb7339615",
    "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-balanced-v1.json": "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138",
    "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-natural-bouts-v2.json": "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c",
    "crates/mechofly-core/parameters/n4.1-soft-fatigue-b-natural-flight-v3.json": "cb3cd2654dcd4fa9def34fb0145645f5d61b59c96c407669cf1e9dd4f12628ef",
    "crates/mechofly-core/parameters/n4.1-soft-fatigue-c-conservative-v1.json": "b1296cd9640a39852dfa5d8cba2387798fbe681869dc53b8fd24224225f0a18d",
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    hashes = {}
    for relative, expected in EXPECTED.items():
        path = root / relative
        observed = hashlib.sha256(path.read_bytes()).hexdigest()
        if observed != expected:
            raise ValueError(f"immutable parameter artifact changed: {relative}")
        hashes[relative] = observed

    model = (root / "crates/mechofly-core/src/model.rs").read_text(encoding="utf-8")
    runtime = (root / "crates/mechofly-app/src/runtime.rs").read_text(encoding="utf-8")
    parameters = (root / "crates/mechofly-core/src/behavior_parameters.rs").read_text(encoding="utf-8")
    dynamics = (root / "crates/mechofly-core/src/behavior_dynamics.rs").read_text(encoding="utf-8")
    required = {
        "default_n4_profile": "Self::new_duration_aware_with_profile(graph, seed, BehaviorParameterProfile::N4)",
        "offline_profile_constructor": "pub fn new_duration_aware_with_profile(",
        "graded_policy": "FatiguePolicy::GradedResponse",
        "nonzero_floor": "return p.fatigue_min_response_q15;",
        "event_keyed_draw": "fatigue_response_draw_q15(",
    }
    combined = "\n".join((model, parameters, dynamics))
    for label, marker in required.items():
        if marker not in combined:
            raise ValueError(f"N4.1 integration marker missing: {label}")
    if "ModelEngine::new_duration_aware(Arc::clone(&graph), seed)" not in runtime:
        raise ValueError("application no longer constructs the frozen-default N4 profile")
    if "new_duration_aware_with_profile" in runtime:
        review_markers = (
            '#[cfg(feature = "n41-visual-review-b")]',
            "pub fn calibrated_n41_visual_review_b(",
            "BehaviorParameterProfile::N41BNaturalFlight",
        )
        for marker in review_markers:
            if marker not in runtime:
                raise ValueError(
                    "experimental application profile is not confined to the visual-review feature"
                )
        cargo = (root / "crates/mechofly-app/Cargo.toml").read_text(encoding="utf-8")
        main = (root / "crates/mechofly-app/src/main.rs").read_text(encoding="utf-8")
        if "default = []" not in cargo or 'n41-visual-review-b = ["dep:png"]' not in cargo:
            raise ValueError("N4.1 visual-review feature is not opt-in")
        if 'const N41_B_VISUAL_REVIEW_FLAG: &str = "--n41-b-visual-review";' not in main:
            raise ValueError("N4.1 visual review lacks its explicit command-line gate")
    for relative in (
        "crates/mechofly-core/examples/n41_experiment.rs",
        "crates/mechofly-core/tests/n41_soft_fatigue.rs",
        "tools/run_n41_matrix.py",
        "tools/evaluate_n41_matrix.py",
        "tools/verify_n4_frozen_signatures.py",
        "docs/N4.1-Soft-Fatigue-Experiment.md",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required N4.1 artifact is missing: {relative}")
    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "isolated_n4_1_experiment_and_feature_gated_visual_review_boundary",
        "application_default_profile": "n4",
        "experimental_profiles_in_canonical_application_runtime": False,
        "experimental_profile_in_opt_in_visual_review_build": "n41-b-natural-flight",
        "visual_review_requires_feature": "n41-visual-review-b",
        "visual_review_requires_flag": "--n41-b-visual-review",
        "parameter_sha256": hashes,
        "promotion_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("N41_RUNTIME_ISOLATION=PASS application_default=n4 promotion_authorized=false")


if __name__ == "__main__":
    main()
