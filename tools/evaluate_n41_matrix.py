#!/usr/bin/env python3
"""Evaluate the offline N4.1 matrix without authorizing runtime promotion."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

PROFILES = ("n3", "n4", "n41-a", "n41-b", "n41-c")
CANDIDATES = PROFILES[2:]
MIB = 1024 * 1024


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def occupancy(report: dict, scenario: str, field: str, behaviors: set[str]) -> float:
    numerator = 0
    denominator = 0
    matching = [run["deterministic"] for run in report["runs"] if run["deterministic"]["scenario"] == scenario]
    if not matching:
        raise ValueError(f"missing scenario {scenario} in {report['profile']}")
    for run in matching:
        values = run[field]
        numerator += sum(int(values.get(behavior, 0)) for behavior in behaviors)
        denominator += sum(int(value) for value in values.values())
    if denominator == 0:
        raise ValueError(f"empty occupancy window {scenario}/{field} in {report['profile']}")
    return 100.0 * numerator / denominator


def maximum_unresponsive_seconds(report: dict, scenario: str) -> float:
    frames = max(
        run["deterministic"]["maximum_walk_drive_unresponsive_frames"]
        for run in report["runs"]
        if run["deterministic"]["scenario"] == scenario
    )
    return frames * report["model_step_ms"] / 1000.0


def mean_ns(report: dict, component: str) -> int:
    return int(report["timings"][component]["mean_ns"])


def non_dominated(rows: list[dict]) -> list[str]:
    dimensions = (
        ("target_distance", False),
        ("maximum_unresponsive_seconds", False),
        ("controller_overhead_percent", False),
        ("incremental_memory_mib", False),
        ("quiet_calm_percent", True),
        ("mixed_late_walk_percent", True),
    )
    frontier = []
    for candidate in rows:
        dominated = False
        for other in rows:
            if other is candidate:
                continue
            no_worse = True
            strictly_better = False
            for key, maximize in dimensions:
                left = other[key]
                right = candidate[key]
                if maximize:
                    no_worse &= left >= right
                    strictly_better |= left > right
                else:
                    no_worse &= left <= right
                    strictly_better |= left < right
            if no_worse and strictly_better:
                dominated = True
                break
        if not dominated:
            frontier.append(candidate["profile"])
    return frontier


def evaluate(directory: Path) -> tuple[dict, str]:
    envelope_path = directory / "matrix-envelope.json"
    envelope = load_json(envelope_path)
    if envelope.get("status") != "PASS" or tuple(envelope.get("profiles", ())) != PROFILES:
        raise ValueError("matrix envelope is incomplete or failed")
    authority = envelope["executable_sha256"]
    reports = {}
    memory = {}
    for record in envelope["records"]:
        profile = record["profile"]
        if profile not in PROFILES or profile in reports:
            raise ValueError("duplicate or unknown matrix profile")
        report_path = directory / record["report"]
        if sha256(report_path) != record["report_sha256"]:
            raise ValueError(f"report checksum mismatch for {profile}")
        report = load_json(report_path)
        if (
            report.get("status") != "PASS"
            or report.get("profile") != profile
            or report.get("executable_authority_sha256") != authority
            or not report.get("all_repeat_groups_equal")
            or report.get("invariant_violations") != 0
        ):
            raise ValueError(f"report identity/invariant failure for {profile}")
        reports[profile] = report
        memory[profile] = int(record["peak_working_set_bytes"])
    if tuple(reports) != PROFILES:
        raise ValueError("profile order or completeness changed")

    neural_reference = {
        (
            run["deterministic"]["scenario"],
            run["deterministic"]["seed_index"],
            run["repeat"],
        ): run["deterministic"]["final_neural_sha256"]
        for run in reports["n3"]["runs"]
    }
    for profile in PROFILES[1:]:
        observed = {
            (
                run["deterministic"]["scenario"],
                run["deterministic"]["seed_index"],
                run["repeat"],
            ): run["deterministic"]["final_neural_sha256"]
            for run in reports[profile]["runs"]
        }
        if observed != neural_reference:
            raise ValueError(f"neural arrays changed under matched input for {profile}")

    baseline = reports["n3"]
    baseline_total = mean_ns(baseline, "total")
    if baseline_total <= 0:
        raise ValueError("N3 total timing is zero")
    baseline_memory = memory["n3"]
    n4_late_walk = occupancy(reports["n4"], "walking", "late_5m_occupancy_frames", {"Walk"})
    n4_mixed_late_walk = occupancy(reports["n4"], "mixed", "late_5m_occupancy_frames", {"Walk"})
    rows = []
    for profile in CANDIDATES:
        report = reports[profile]
        late_walk = occupancy(report, "walking", "late_5m_occupancy_frames", {"Walk"})
        early_walk = occupancy(report, "walking", "early_30s_occupancy_frames", {"Walk"})
        mixed_late_walk = occupancy(report, "mixed", "late_5m_occupancy_frames", {"Walk"})
        quiet_calm = occupancy(report, "quiet_rest", "occupancy_frames", {"Quiet", "Rest"})
        pipeline = sum(
            mean_ns(report, component)
            for component in ("intent_build", "controller", "telemetry", "summary_hash")
        )
        baseline_pipeline = sum(
            mean_ns(baseline, component)
            for component in ("intent_build", "controller", "telemetry", "summary_hash")
        )
        overhead = 100.0 * (pipeline - baseline_pipeline) / baseline_total
        incremental_memory = memory[profile] - baseline_memory if baseline_memory else 0
        maximum_unresponsive = maximum_unresponsive_seconds(report, "walking")
        gates = {
            "determinism_and_invariants": True,
            "late_walk_50_to_70_percent": 50.0 <= late_walk <= 70.0,
            "mixed_late_walk_nonzero": mixed_late_walk > 0.0,
            "quiet_session_majority_calm": quiet_calm > 50.0,
            "controller_cpu_overhead_at_most_5_percent": overhead <= 5.0,
            "memory_evidence_available": baseline_memory > 0 and memory[profile] > 0,
            "incremental_memory_at_most_10_mib": baseline_memory > 0
            and incremental_memory <= 10 * MIB,
        }
        rows.append(
            {
                "profile": profile,
                "parameter_set_id": report["parameter_set_id"],
                "parameter_sha256": report["parameter_sha256"],
                "early_30s_walk_percent": early_walk,
                "late_5m_walk_percent": late_walk,
                "mixed_late_walk_percent": mixed_late_walk,
                "quiet_calm_percent": quiet_calm,
                "maximum_unresponsive_seconds": maximum_unresponsive,
                "controller_overhead_percent": overhead,
                "peak_working_set_bytes": memory[profile],
                "incremental_memory_bytes": incremental_memory,
                "incremental_memory_mib": incremental_memory / MIB,
                "target_distance": abs(late_walk - 60.0),
                "gates": gates,
                "all_gates_pass": all(gates.values()),
            }
        )

    frontier = non_dominated(rows)
    passing = [row for row in rows if row["all_gates_pass"]]
    passing.sort(
        key=lambda row: (
            row["target_distance"],
            row["maximum_unresponsive_seconds"],
            row["controller_overhead_percent"],
            row["incremental_memory_bytes"],
            row["profile"],
        )
    )
    recommended = passing[0]["profile"] if passing else None
    status = "WINNER_IDENTIFIED" if recommended else "NO_WINNER"
    result = {
        "schema_version": 1,
        "status": status,
        "classification": "offline_candidate_evaluation_not_promotion",
        "executable_sha256": authority,
        "matrix_envelope_sha256": sha256(envelope_path),
        "n4_baseline": {
            "late_5m_walk_percent": n4_late_walk,
            "mixed_late_walk_percent": n4_mixed_late_walk,
            "peak_working_set_bytes": memory["n4"],
        },
        "n3_baseline": {
            "mean_total_step_ns": baseline_total,
            "peak_working_set_bytes": baseline_memory,
        },
        "pareto_frontier": frontier,
        "neural_arrays_equal_across_profiles": True,
        "recommended_profile_for_visual_review": recommended,
        "candidates": rows,
        "promotion_authorized": False,
        "next_decision": (
            "run early/late visual acceptance on the recommended profile"
            if recommended
            else "retune bounded candidates; do not wire or deploy a fatigue policy"
        ),
        "claim_boundary": "Targets are product-engineering criteria, not biological truth. This evaluation cannot deploy, change shortcuts, overwrite N4, or authorize promotion.",
    }
    lines = [
        "# MechoFly N4.1 offline matrix evaluation",
        "",
        f"Status: **{status}**",
        "",
        f"Frozen N4 late-session walking in this matched matrix: **{n4_late_walk:.2f}%**.",
        f"Frozen N4 mixed late-session walking: **{n4_mixed_late_walk:.2f}%**.",
        "",
        "| Profile | Late walk | Mixed late walk | Quiet/calm | Max unresponsive | CPU overhead | Memory delta | Gates |",
        "|---|---:|---:|---:|---:|---:|---:|:---:|",
    ]
    for row in rows:
        lines.append(
            "| {profile} | {late:.2f}% | {mixed:.2f}% | {quiet:.2f}% | {unresponsive:.2f}s | {cpu:.2f}% | {memory:.2f} MiB | {gates} |".format(
                profile=row["profile"],
                late=row["late_5m_walk_percent"],
                mixed=row["mixed_late_walk_percent"],
                quiet=row["quiet_calm_percent"],
                unresponsive=row["maximum_unresponsive_seconds"],
                cpu=row["controller_overhead_percent"],
                memory=row["incremental_memory_mib"],
                gates="PASS" if row["all_gates_pass"] else "FAIL",
            )
        )
    lines.extend(
        [
            "",
            f"Pareto frontier: **{', '.join(frontier)}**.",
            f"Recommended for visual review: **{recommended or 'none'}**.",
            "",
            "No runtime promotion or deployment is authorized by this evaluation.",
            "",
        ]
    )
    return result, "\n".join(lines)


def atomic_text(path: Path, text: str) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(text, encoding="utf-8")
    temporary.replace(path)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix-dir", required=True, type=Path)
    parser.add_argument("--output-json", required=True, type=Path)
    parser.add_argument("--output-markdown", required=True, type=Path)
    args = parser.parse_args()
    result, markdown = evaluate(args.matrix_dir.resolve())
    atomic_text(args.output_json.resolve(), json.dumps(result, indent=2) + "\n")
    atomic_text(args.output_markdown.resolve(), markdown)
    print(
        "N41_EVALUATION={} recommended_profile={} promotion_authorized=false".format(
            result["status"], result["recommended_profile_for_visual_review"] or "none"
        )
    )


if __name__ == "__main__":
    main()
