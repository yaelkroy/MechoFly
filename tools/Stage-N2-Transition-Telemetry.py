#!/usr/bin/env python3
"""Apply the N2 observational transition-telemetry patch to the N0 tree.

This is a temporary staging tool. It extracts the already-reviewed source
payload from the staging workflow, replaces format-sensitive patch fragments
with whitespace-tolerant equivalents, and executes the resulting patch.
"""

from __future__ import annotations

import re
from pathlib import Path


BASE_COMMIT = "c80d827a0d7eae9636bbf02c2916b5946f42319e"
BASE_TREE = "c4c59970af6dbe0449a02e916580dedf06efad1d"
PAYLOAD_WORKFLOW = Path(".github/workflows/apply-n2-transition-telemetry.yml")


def extract_payload() -> str:
    text = PAYLOAD_WORKFLOW.read_text(encoding="utf-8")
    start_marker = "          python3 - <<'PY'\n"
    end_marker = "\n          PY\n      - name: Format and inspect"
    start = text.index(start_marker) + len(start_marker)
    end = text.index(end_marker, start)
    lines: list[str] = []
    for line in text[start:end].splitlines():
        lines.append(line[10:] if line.startswith("          ") else line)
    return "\n".join(lines) + "\n"


def replace_one(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one block, found {count}")
    return source.replace(old, new, 1)


def replace_patcher_block(
    source: str,
    start_marker: str,
    end_marker: str,
    replacement: str,
    label: str,
) -> str:
    try:
        start = source.index(start_marker)
        end = source.index(end_marker, start)
    except ValueError as error:
        raise SystemExit(f"{label}: staging block marker is absent") from error
    return source[:start] + replacement + source[end:]


def main() -> None:
    script = extract_payload()

    old_helper = '''def replace_regex_once(path: str, pattern: str, replacement: str) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{path}: expected one regex match, found {count}: {pattern!r}")
    write(path, updated)
'''
    new_helper = '''def replace_regex_once(path: str, pattern: str, replacement: str) -> None:
    text = read(path)
    regex = re.compile(pattern, flags=re.S)
    matches = list(regex.finditer(text))
    if len(matches) != 1:
        raise SystemExit(
            f"{path}: expected one regex match, found {len(matches)}: {pattern!r}"
        )
    match = matches[0]
    expanded = re.sub(
        r"\\\\([1-9])",
        lambda token: match.group(int(token.group(1))) or "",
        replacement,
    )
    updated = text[: match.start()] + expanded + text[match.end() :]
    write(path, updated)
'''
    script = replace_one(
        script,
        old_helper,
        new_helper,
        "regex replacement helper",
    )

    triple = '"' * 3
    grooming_start = (
        "replace_once(\n    self_test,\n    "
        + triple
        + "    let grooming_substate_timeline"
    )
    receipt_start = (
        "replace_once(\n    self_test,\n    "
        + triple
        + "    let receipt = SelfTestReceipt"
    )
    pass_start = (
        "replace_once(\n    self_test,\n    "
        + triple
        + "            && two_way_neuron_selection_sync"
    )
    fields_start = (
        "replace_once(\n    self_test,\n    "
        + triple
        + "        behavior_controller_authoritative:"
    )

    script = replace_patcher_block(
        script,
        grooming_start,
        receipt_start,
        '''replace_regex_once(
    self_test,
    r'(    let grooming_substate_timeline = grooming_program_substates\\.iter\\(\\)\\.map\\(String::as_str\\)\\.eq\\(\\[.*?    \\]\\);\\n)(\\n    #\\[cfg\\(windows\\)\\])',
    r'\\1    let behavior_telemetry = run_behavior_telemetry_self_test(Arc::clone(&graph));\\n\\2',
)

''',
        "grooming telemetry hook",
    )
    script = replace_patcher_block(
        script,
        receipt_start,
        pass_start,
        '''replace_regex_once(
    self_test,
    r'(    let receipt = SelfTestReceipt \\{\\n\\s*)schema_version: 6,',
    r'\\1schema_version: 7,',
)

''',
        "self-test schema update",
    )
    script = replace_patcher_block(
        script,
        pass_start,
        fields_start,
        '''replace_regex_once(
    self_test,
    r'(\\s+&& two_way_neuron_selection_sync\\n\\s+&& grooming_substate_timeline\\n)(\\s+&& anatomical_context_points == 23_210)',
    r'\\1            && behavior_telemetry.passed\\n\\2',
)

''',
        "self-test telemetry pass gate",
    )

    receipt_fields = '''replace_regex_once(
    self_test,
    r'(        behavior_controller_authoritative: rendered_behavior_matches_neural_state\\n            && policy_action_neural_dispatch,\\n)(        escape_envelope_ms:)',
    (
        r'\\1'
        '        behavior_telemetry_schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,\\n'
        '        behavior_telemetry_controller: BEHAVIOR_TELEMETRY_CONTROLLER.to_owned(),\\n'
        '        behavior_telemetry_claim_boundary: BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY.to_owned(),\\n'
        '        behavior_transition_event_count: behavior_telemetry.event_count,\\n'
        '        behavior_transition_stream_sha256: behavior_telemetry.event_stream_sha256,\\n'
        '        behavior_transition_sequence_contiguous: behavior_telemetry.sequence_contiguous,\\n'
        '        behavior_transition_telemetry_bounded: behavior_telemetry.bounded,\\n'
        '        behavior_transition_telemetry_deterministic: behavior_telemetry.deterministic,\\n'
        '        behavior_transition_telemetry_observational_only: behavior_telemetry.observational_only,\\n'
        '        behavior_transition_reasons_complete: behavior_telemetry.reasons_complete,\\n'
        '        behavior_intent_snapshot_available: behavior_telemetry.intent_snapshot_available,\\n'
        r'\\2'
    ),
)

'''
    script = replace_patcher_block(
        script,
        fields_start,
        'workflow = ".github/workflows/windows.yml"',
        receipt_fields,
        "self-test receipt telemetry fields",
    )

    workflow_patch = '''workflow = ".github/workflows/windows.yml"
replace_once(
    workflow,
    "          if ($receipt.schema_version -ne 6) { throw 'Self-test receipt schema is stale.' }\\n",
    "          if ($receipt.schema_version -ne 7) { throw 'Self-test receipt schema is stale.' }\\n",
)
replace_regex_once(
    workflow,
    r"(          if \\(-not \\$receipt\\.behavior_controller_authoritative\\) \\{ throw 'Behavior controller is not authoritative\\.' \\}\\n)(          if \\(\\$receipt\\.escape_envelope_ms -ne 198\\) \\{ throw 'Escape timing differs from the recorded controller\\.' \\}\\n)",
    (
        r"\\1"
        "          if ($receipt.behavior_telemetry_schema_version -ne 1) { throw 'Behavior telemetry schema is stale.' }\\n"
        "          if ($receipt.behavior_telemetry_controller -ne 'legacy-threshold-hold-v1-observed') { throw 'Behavior telemetry controller identity changed.' }\\n"
        "          if (-not $receipt.behavior_transition_telemetry_observational_only) { throw 'Behavior telemetry changed controller semantics.' }\\n"
        "          if (-not $receipt.behavior_transition_telemetry_deterministic) { throw 'Behavior telemetry is not deterministic.' }\\n"
        "          if (-not $receipt.behavior_transition_telemetry_bounded) { throw 'Behavior telemetry is not bounded.' }\\n"
        "          if (-not $receipt.behavior_transition_sequence_contiguous) { throw 'Behavior transition sequence is discontinuous.' }\\n"
        "          if (-not $receipt.behavior_transition_reasons_complete) { throw 'Behavior transition reasons are incomplete.' }\\n"
        "          if (-not $receipt.behavior_intent_snapshot_available) { throw 'Behavior intent snapshot is absent.' }\\n"
        "          if ($receipt.behavior_transition_event_count -le 0) { throw 'Behavior transition telemetry recorded no events.' }\\n"
        "          if ([string]$receipt.behavior_transition_stream_sha256 -notmatch '^[0-9a-f]{64}$') { throw 'Behavior transition stream digest is invalid.' }\\n"
        r"\\2"
    ),
)
replace_once(
    workflow,
    "      - name: Exercise real Windows GUI startup boundary\\n",
    """      - name: Run deterministic behavior-transition telemetry baseline smoke
        shell: powershell
        run: |
          $executable = (Resolve-Path .\\host-windows\\bin\\MechoFly.exe).Path
          $receiptPath = (Join-Path $PWD 'artifacts\\behavior-telemetry-smoke.json')
          $arguments = '--behavior-baseline \\\"{0}\\\" --baseline-seeds 2 --baseline-repeats 2 --baseline-seconds 10' -f $receiptPath
          $process = Start-Process -FilePath $executable -ArgumentList $arguments -Wait -PassThru
          if ($process.ExitCode -ne 0) { throw \\\"Behavior baseline exited $($process.ExitCode)\\\" }
          $baseline = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
          if ($baseline.schema_version -ne 1) { throw 'Behavior baseline schema is stale.' }
          if ($baseline.status -ne 'PASS') { throw 'Behavior baseline failed.' }
          if ($baseline.campaign_classification -ne 'local_smoke') { throw 'CI baseline must remain a smoke campaign.' }
          if ($baseline.scenario_count -ne 5 -or $baseline.run_count -ne 20) { throw 'Behavior baseline matrix is incomplete.' }
          if (-not $baseline.all_repeat_groups_equal) { throw 'Behavior baseline repeats are not deterministic.' }
          if (-not $baseline.telemetry_observational_only -or $baseline.controller_semantics_changed) { throw 'Behavior telemetry changed controller semantics.' }
          if ($baseline.total_transition_count -le 0) { throw 'Behavior baseline recorded no transitions.' }
      - name: Exercise real Windows GUI startup boundary
""",
)

'''
    script = replace_patcher_block(
        script,
        'workflow = ".github/workflows/windows.yml"',
        'setup = "tools/Setup-AI100-MechoFly.ps1"',
        workflow_patch,
        "Windows workflow telemetry gates",
    )

    setup_patch = '''setup = "tools/Setup-AI100-MechoFly.ps1"
replace_once(
    setup,
    """$Receipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
if ($Receipt.status -ne 'PASS' -or -not $Receipt.live_state_unchanged -or
""",
    """$Receipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
if ($Receipt.schema_version -ne 7 -or
    $Receipt.behavior_telemetry_schema_version -ne 1 -or
    -not $Receipt.behavior_transition_telemetry_observational_only -or
    -not $Receipt.behavior_transition_telemetry_deterministic -or
    -not $Receipt.behavior_transition_telemetry_bounded -or
    -not $Receipt.behavior_transition_sequence_contiguous -or
    -not $Receipt.behavior_transition_reasons_complete -or
    -not $Receipt.behavior_intent_snapshot_available -or
    $Receipt.behavior_transition_event_count -le 0 -or
    $Receipt.status -ne 'PASS' -or -not $Receipt.live_state_unchanged -or
""",
)

'''
    script = replace_patcher_block(
        script,
        'setup = "tools/Setup-AI100-MechoFly.ps1"',
        "for path in [",
        setup_patch,
        "AI100 setup telemetry gates",
    )

    namespace = {
        "__name__": "__main__",
        "__file__": str(PAYLOAD_WORKFLOW),
    }
    exec(compile(script, str(PAYLOAD_WORKFLOW), "exec"), namespace)


if __name__ == "__main__":
    main()
