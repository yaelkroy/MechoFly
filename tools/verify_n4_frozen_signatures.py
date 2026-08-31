#!/usr/bin/env python3
"""Prove the new experiment framework did not change default N4 semantics."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

EXPECTED_PARAMETER_SHA256 = "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b"
EXPECTED = {
    ("grooming", 0): "b4caded9987739cdac361484c64d82977da5e1169652ceea2c752bcfe295560a",
    ("grooming", 1): "bdc16748dd1a41e0f64f7e76778cffccd1ac815792e218da253398ff2dbade24",
    ("mixed", 0): "0cd3daa81816cdd8f040bbe64eaa59ebc4f55fa205471651eee3f3b80050c85a",
    ("mixed", 1): "5787911d98f467a5ac76c3986d32f17481bbf67581be3dcffe5723a18db5b0fd",
    ("quiet_rest", 0): "c5d2bb7a8d9236cec313e6b3d064bacbc5743efe987169502656192a7b116bbc",
    ("quiet_rest", 1): "bd75c2e05c6f0aaa8e0d7239a725179a834ede85bb6b02e14a9efcbe16f2c0c0",
    ("repeated_loom", 0): "8acf8be9e913111a1bc4c60f88494ede5bd813ce3a6f9926136cfdc5f0793b9a",
    ("repeated_loom", 1): "ea6f694c4e3010ef52a5419b2cff27f30c31e35a09f8b13e1b52166f66417a96",
    ("walking", 0): "136490866747ce32f07ddef5862e472545800c94e3a841fa63d6a25a89a23e22",
    ("walking", 1): "1e2a4019774b4af391ccf432ea46c95c981c33841504457f8b5ec709bf20e88e",
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--campaign-dir", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    directory = args.campaign_dir.resolve(strict=True)
    campaign = json.loads((directory / "campaign.json").read_text(encoding="utf-8"))
    if (
        campaign.get("controller") != "n4-explicit-duration-engineering-v1"
        or campaign.get("parameter_sha256") != EXPECTED_PARAMETER_SHA256
        or campaign.get("seeds") != 2
        or campaign.get("repeats") != 2
        or campaign.get("seconds") != 30
        or len(campaign.get("runs", [])) != 20
    ):
        raise ValueError("default N4 smoke identity or matrix changed")
    observed: dict[tuple[str, int], set[str]] = {}
    for run in campaign["runs"]:
        deterministic = run["deterministic"]
        key = (deterministic["scenario"], deterministic["seed_index"])
        observed.setdefault(key, set()).add(run["deterministic_signature_sha256"])
    if set(observed) != set(EXPECTED):
        raise ValueError("default N4 scenario/seed groups changed")
    mismatches = {
        f"{scenario}/{seed}": sorted(observed[(scenario, seed)])
        for (scenario, seed), expected in EXPECTED.items()
        if observed[(scenario, seed)] != {expected}
    }
    if mismatches:
        raise ValueError(f"default N4 deterministic signatures drifted: {mismatches}")
    receipt = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "frozen_n4_semantic_parity",
        "validated_commit": "6b67b6f8678151f1e568ebfb84ac773f679fa583",
        "validated_tree": "7a4d47d5189d628b586f8765f9e6d1aafbb08aac",
        "parameter_sha256": EXPECTED_PARAMETER_SHA256,
        "scenario_seed_groups": len(EXPECTED),
        "repeat_runs": 20,
        "all_signatures_exact": True,
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("N4_FROZEN_SEMANTIC_PARITY=PASS groups=10 runs=20")


if __name__ == "__main__":
    main()
