#!/usr/bin/env python3
"""Fail-closed static verification for the N6 product-checkpoint foundation."""
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
        raise ValueError(f"N6 product-checkpoint marker missing: {label}")


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
    runtime = (root / "crates/mechofly-app/src/runtime.rs").read_text(encoding="utf-8")
    pet = (root / "crates/mechofly-app/src/pet.rs").read_text(encoding="utf-8")
    product = (root / "crates/mechofly-app/src/product_checkpoint.rs").read_text(
        encoding="utf-8"
    )

    markers = (
        (manifest, "n6-product-checkpoint = []", "additive compile feature"),
        (
            main_rs,
            '"--n6-product-checkpoint-self-test"',
            "feature self-test option",
        ),
        (
            main_rs,
            '#[cfg(not(feature = "n6-product-checkpoint"))]',
            "canonical rejection gate",
        ),
        (runtime, "SimulationRuntimeCheckpoint", "runtime state checkpoint"),
        (runtime, "ModelEngine::from_state", "exact model-state restoration"),
        (runtime, "DiscardedSimulationBranch", "discarded runtime branch"),
        (runtime, "last_behavior_intent", "intent-state preservation"),
        (runtime, "sensory_stimulus", "sensory-state preservation"),
        (runtime, "authored_drive", "authored-drive preservation"),
        (pet, "PetMotionCheckpoint", "visible motor checkpoint"),
        (pet, "ground_bout_sequence", "walk RNG contract state"),
        (pet, "flight_bout_sequence", "flight RNG contract state"),
        (pet, "landing_target_position", "landing-state preservation"),
        (product, "#[serde(deny_unknown_fields)]", "unknown-field rejection"),
        (product, "ProductCheckpoint", "complete product checkpoint"),
        (product, "PetPolicy", "policy-state preservation"),
        (product, "grooming_program", "grooming-program preservation"),
        (product, "wall_clock_excluded: true", "wall-clock exclusion"),
        (product, "live_restore_authorized: false", "live-restore denial"),
        (
            product,
            "zero-intervention product branches diverged",
            "zero-intervention identity gate",
        ),
        (
            product,
            "changed-input product branch did not diverge",
            "counterfactual divergence gate",
        ),
        (
            product,
            "discarded product branches mutated live state",
            "live-state isolation gate",
        ),
        (product, "screen_ecology_added: false", "screen-ecology denial"),
        (product, "food_search_added: false", "food-search denial"),
        (product, "appdata_write_authorized: false", "AppData-write denial"),
        (product, "deployment_authorized: false", "deployment denial"),
    )
    for text, marker, label in markers:
        require(text, marker, label)

    default_line = next(
        (line.strip() for line in manifest.splitlines() if line.strip().startswith("default =")),
        None,
    )
    if default_line is None or "n6-product-checkpoint" in default_line:
        raise ValueError("N6 product checkpoint is enabled in the canonical default")

    for forbidden in (
        "$env:LOCALAPPDATA",
        "AppData\\Local",
        "live_restore_authorized: true",
        "screen_ecology_added: true",
        "food_search_added: true",
        "deployment_authorized: true",
    ):
        if forbidden in product or forbidden in runtime or forbidden in main_rs:
            raise ValueError(f"forbidden N6 boundary present: {forbidden}")

    for relative in (
        "docs/N6-Product-State-Checkpoint-Foundation.md",
        ".github/workflows/n6-product-checkpoint.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required N6 artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "behavior_neutral_n6_product_checkpoint_foundation",
        "feature": "n6-product-checkpoint",
        "canonical_feature_enabled": False,
        "checkpoint_scope": {
            "model_state": True,
            "intent_state": True,
            "sensory_and_authored_drive_state": True,
            "policy_state": True,
            "visible_motor_state": True,
            "grooming_program_state": True,
            "event_keyed_rng_state": True,
            "wall_clock": False,
        },
        "discarded_counterfactual_only": True,
        "live_restore_authorized": False,
        "controller_or_motor_semantics_changed": False,
        "parameter_sha256": observed_parameters,
        "parameter_files_changed": False,
        "screen_ecology_added": False,
        "food_search_added": False,
        "appdata_write_authorized": False,
        "promotion_authorized": False,
        "deployment_authorized": False,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(
        "N6_PRODUCT_CHECKPOINT_STATIC=PASS canonical=false "
        "discarded_only=true live_restore=false deploy=false"
    )


if __name__ == "__main__":
    main()
