#!/usr/bin/env python3
"""Wait for the real Windows GUI-subsystem executable using its process handle.

Do not interpret PowerShell's stale/null LASTEXITCODE as this executable's exit.
"""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import subprocess


def execute(exe: Path, arguments: list[str], label: str) -> None:
    result = subprocess.run([str(exe), *arguments], capture_output=True, timeout=600, check=False)
    print(result.stdout.decode("utf-8", "replace"), end="", flush=True)
    print(result.stderr.decode("utf-8", "replace"), end="", flush=True)
    print(f"N4_NATIVE_PROCESS={label};exit_code={result.returncode}", flush=True)
    if result.returncode != 0:
        raise RuntimeError(f"{label} process failed with exit {result.returncode}")


def main() -> None:
    root = Path.cwd()
    exe = (root / "target/release/MechoFly.exe").resolve()
    authority = hashlib.sha256(exe.read_bytes()).hexdigest()
    artifacts = root / "artifacts"
    artifacts.mkdir(exist_ok=True)
    receipt_path = artifacts / "n4-self-test.json"
    execute(exe, ["--self-test", str(receipt_path)], "self-test")
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    if (receipt.get("schema_version") != 10 or receipt.get("status") != "PASS"
            or receipt.get("n4", {}).get("passed") is not True
            or receipt.get("runtime_behavior_controller") != "n4-explicit-duration-engineering-v1"):
        raise RuntimeError("The executable is not the validated N4 runtime")
    parameters = root / "crates/mechofly-core/parameters/n4-engineering-v1.json"
    if receipt["n4"]["parameter_sha256"] != hashlib.sha256(parameters.read_bytes()).hexdigest():
        raise RuntimeError("Compiled parameter artifact differs from source bytes")
    directory = artifacts / "n4-campaign"
    arguments = ["--behavior-campaign", str(directory), "--campaign-authority", authority,
                 "--campaign-seeds", "2", "--campaign-repeats", "2", "--campaign-seconds", "30"]
    execute(exe, arguments, "semantic-smoke")
    before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in directory.iterdir() if p.is_file()}
    execute(exe, arguments, "completed-run-resume")
    after = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in directory.iterdir() if p.is_file()}
    if before != after:
        raise RuntimeError("Completed-run resume changed validated campaign contents")
    report = json.loads((directory / "campaign.json").read_text(encoding="utf-8"))
    if len(report["runs"]) != 20 or report["authority"] != authority:
        raise RuntimeError("Native semantic matrix incomplete or misidentified")
    summary = {"status": "PASS", "executable_sha256": authority, "run_count": 20,
               "completed_run_resume_byte_equal": True, "n4": receipt["n4"]}
    (artifacts / "n4-native-verification.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == "__main__":
    main()
