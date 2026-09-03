#!/usr/bin/env python3
"""Fail-closed static verification for N6 paired counterfactual replay."""
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
        raise ValueError(f"N6 counterfactual-replay marker missing: {label}")


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
    product = (root / "crates/mechofly-app/src/product_checkpoint.rs").read_text(
        encoding="utf-8"
    )

    markers = (
        (
            manifest,
            'n6-counterfactual-replay = ["n6-product-checkpoint"]',
            "additive replay feature and checkpoint dependency",
        ),
        (
            main_rs,
            '"--n6-counterfactual-replay-self-test"',
            "isolated replay self-test option",
        ),
        (
            main_rs,
            '#[cfg(not(feature = "n6-counterfactual-replay"))]',
            "canonical and checkpoint-only rejection gate",
        ),
        (product, "CounterfactualReplayCapsule", "portable replay capsule"),
        (product, "source_checkpoint: ProductCheckpoint", "embedded source checkpoint"),
        (product, "CounterfactualReplayLane", "paired serialized lanes"),
        (product, "COUNTERFACTUAL_REPLAY_FRAMES: usize = 96", "fixed replay length"),
        (
            product,
            "COUNTERFACTUAL_REPLAY_HISTORY_LIMIT: usize = 128",
            "bounded history ceiling",
        ),
        (product, "first_divergence_model_frame", "first-divergence proof"),
        (product, "common_event_keyed_randomness", "common-randomness contract"),
        (product, "counterfactual replay diverged before intervention", "pre-intervention identity gate"),
        (product, "counterfactual replay did not re-verify exactly", "exact re-verification gate"),
        (product, "counterfactual replay accepted intervention tampering", "intervention tamper gate"),
        (product, "counterfactual replay accepted frame tampering", "frame tamper gate"),
        (product, "counterfactual replay mutated live product state", "live-state isolation gate"),
        (product, "live_restore_authorized: false", "live-restore denial"),
        (product, "controller_or_motor_semantics_changed: false", "behavior-semantic freeze"),
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
    if default_line is None or "n6-" in default_line:
        raise ValueError("an N6 experimental feature is enabled by default")

    combined = "\n".join((manifest, main_rs, product))
    for forbidden in (
        "$env:LOCALAPPDATA",
        "AppData\\Local",
        "live_restore_authorized: true",
        "controller_or_motor_semantics_changed: true",
        "screen_ecology_added: true",
        "food_search_added: true",
        "deployment_authorized: true",
    ):
        if forbidden in combined:
            raise ValueError(f"forbidden N6 replay boundary present: {forbidden}")

    for relative in (
        "docs/N6-Product-State-Checkpoint-Foundation.md",
        "docs/N6-Paired-Counterfactual-Replay.md",
        ".github/workflows/n6-counterfactual-replay.yml",
    ):
        if not (root / relative).is_file():
            raise ValueError(f"required N6 replay artifact is missing: {relative}")

    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "behavior_neutral_n6_paired_counterfactual_replay",
        "feature": "n6-counterfactual-replay",
        "canonical_feature_enabled": False,
        "source_checkpoint_embedded": True,
        "actual_and_alternative_lanes": True,
        "history_frames": 96,
        "history_limit_frames": 128,
        "first_divergence_proof": True,
        "tamper_rejection": True,
        "deterministic_reverification": True,
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
        "N6_COUNTERFACTUAL_REPLAY_STATIC=PASS frames=96 limit=128 "
        "discarded_only=true live_restore=false deploy=false"
    )


if __name__ == "__main__":
    main()
