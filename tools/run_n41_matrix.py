#!/usr/bin/env python3
"""Run matched N3/N4/N4.1 child processes and sample their working sets."""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import threading
import time

PROFILES = ("n3", "n4", "n41-a", "n41-b", "n41-c")
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
PROCESS_VM_READ = 0x0010


class ProcessMemoryCounters(ctypes.Structure):
    _fields_ = [
        ("cb", ctypes.c_ulong),
        ("PageFaultCount", ctypes.c_ulong),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def windows_memory(pid: int) -> tuple[int, int] | None:
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    kernel32.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_int
    psapi.GetProcessMemoryInfo.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ProcessMemoryCounters),
        ctypes.c_ulong,
    ]
    psapi.GetProcessMemoryInfo.restype = ctypes.c_int
    handle = kernel32.OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, False, pid
    )
    if not handle:
        return None
    try:
        counters = ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(
            handle, ctypes.byref(counters), ctypes.sizeof(counters)
        ):
            return None
        return int(counters.WorkingSetSize), int(counters.PeakWorkingSetSize)
    finally:
        kernel32.CloseHandle(handle)


def proc_memory(pid: int) -> tuple[int, int] | None:
    try:
        values: dict[str, int] = {}
        for line in Path(f"/proc/{pid}/status").read_text(encoding="utf-8").splitlines():
            if line.startswith(("VmRSS:", "VmHWM:")):
                key, raw, _unit = line.split()
                values[key.rstrip(":")] = int(raw) * 1024
        if "VmRSS" in values:
            return values["VmRSS"], values.get("VmHWM", values["VmRSS"])
    except (FileNotFoundError, PermissionError, ProcessLookupError, ValueError):
        return None
    return None


def memory(pid: int) -> tuple[int, int] | None:
    if os.name == "nt":
        return windows_memory(pid)
    if sys.platform.startswith("linux"):
        return proc_memory(pid)
    return None


def pump(stream, label: str, captured: list[str]) -> None:
    for line in iter(stream.readline, ""):
        captured.append(line)
        print(f"[{label}] {line}", end="", flush=True)
    stream.close()


def run_profile(
    executable: Path,
    output_directory: Path,
    authority: str,
    profile: str,
    seeds: int,
    repeats: int,
    seconds: int,
    timeout_seconds: int,
) -> dict:
    report_path = output_directory / f"{profile}.json"
    command = [
        str(executable),
        "--output",
        str(report_path),
        "--authority",
        authority,
        "--profile",
        profile,
        "--seeds",
        str(seeds),
        "--repeats",
        str(repeats),
        "--seconds",
        str(seconds),
    ]
    started = time.monotonic()
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    )
    stdout: list[str] = []
    stderr: list[str] = []
    workers = [
        threading.Thread(target=pump, args=(process.stdout, profile, stdout), daemon=True),
        threading.Thread(target=pump, args=(process.stderr, profile, stderr), daemon=True),
    ]
    for worker in workers:
        worker.start()
    peak_working_set = 0
    final_working_set = 0
    samples = 0
    while process.poll() is None:
        observed = memory(process.pid)
        if observed is not None:
            final_working_set, operating_system_peak = observed
            peak_working_set = max(peak_working_set, operating_system_peak, final_working_set)
            samples += 1
        if time.monotonic() - started > timeout_seconds:
            process.kill()
            raise TimeoutError(f"{profile} exceeded {timeout_seconds} seconds")
        time.sleep(0.05)
    for worker in workers:
        worker.join(timeout=10)
    exit_code = process.wait()
    (output_directory / f"{profile}.stdout.log").write_text("".join(stdout), encoding="utf-8")
    (output_directory / f"{profile}.stderr.log").write_text("".join(stderr), encoding="utf-8")
    if exit_code != 0:
        raise RuntimeError(f"{profile} exited with code {exit_code}")
    report = json.loads(report_path.read_text(encoding="utf-8"))
    if (
        report.get("status") != "PASS"
        or report.get("profile") != profile
        or report.get("executable_authority_sha256") != authority
    ):
        raise RuntimeError(f"{profile} report identity or status is invalid")
    return {
        "profile": profile,
        "pid": process.pid,
        "exit_code": exit_code,
        "wall_time_ms": round((time.monotonic() - started) * 1000),
        "memory_sample_count": samples,
        "final_working_set_bytes": final_working_set,
        "peak_working_set_bytes": peak_working_set,
        "report": report_path.name,
        "report_sha256": sha256(report_path),
    }


def atomic_json(path: Path, value: object) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--seeds", type=int, default=2, choices=range(1, 21))
    parser.add_argument("--repeats", type=int, default=2, choices=range(2, 5))
    parser.add_argument("--seconds", type=int, default=600, choices=range(600, 1801))
    parser.add_argument("--timeout-seconds", type=int, default=7_200)
    args = parser.parse_args()
    executable = args.exe.resolve(strict=True)
    output_directory = args.output_dir.resolve()
    output_directory.mkdir(parents=True, exist_ok=True)
    authority = sha256(executable)
    records = []
    for profile in PROFILES:
        records.append(
            run_profile(
                executable,
                output_directory,
                authority,
                profile,
                args.seeds,
                args.repeats,
                args.seconds,
                args.timeout_seconds,
            )
        )
    envelope = {
        "schema_version": 1,
        "status": "PASS",
        "classification": "offline_matched_process_isolation_not_deployment",
        "executable": str(executable),
        "executable_sha256": authority,
        "profiles": list(PROFILES),
        "seeds": args.seeds,
        "repeats": args.repeats,
        "seconds": args.seconds,
        "platform": platform.platform(),
        "python": platform.python_version(),
        "logical_cpu_count": os.cpu_count(),
        "records": records,
    }
    atomic_json(output_directory / "matrix-envelope.json", envelope)
    print(f"N41_MATRIX=PASS executable_sha256={authority} profiles={len(records)}")


if __name__ == "__main__":
    main()
