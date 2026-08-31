#!/usr/bin/env python3
"""N4 source integration gate; optional one-time recovery from pinned source.

The old staged runner copied the 11 source files but omitted prepare_n4.py.
This tool must prove actual runtime wiring, not just module-file presence.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import io
import json
import lzma
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import urllib.request
import zipfile

BASE = "d9c4944f382c6cff0167acc9a0dd5c2be3d8a31f"
TRANSPORT = "7ec988a68a2430728e8396c1c872020a81591593"
XZ_SHA = "6282e51bc276f484280c993e779ccf862658631bb77c030ccc4216e2710549f6"
DECODED_SHA = "168cb278c9425a6a27eeee4a4f8aa6505822f28a37a7b57f73bc85ab14e4fb16"
MAX_BYTES = 64 * 1024 * 1024


def api(path: str) -> dict:
    request = urllib.request.Request(
        "https://api.github.com/repos/yaelkroy/MechoFly/git/" + path,
        headers={"User-Agent": "MechoFly-N4-integration-repair", "Accept": "application/vnd.github+json"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError("Source response is too large")
    return json.loads(data)


def source_files() -> dict[str, bytes]:
    tree = api("trees/" + TRANSPORT)
    folder = next(item for item in tree["tree"] if item["path"] == ".n4-assembly")
    entries = sorted(api("trees/" + folder["sha"])["tree"], key=lambda item: item["path"])
    if [item["path"] for item in entries] != [f"{i:02d}.txt" for i in range(13)]:
        raise ValueError("Source transport does not have the pinned 13 parts")
    pieces = []
    for item in entries:
        blob = api("blobs/" + item["sha"])
        raw = base64.b64decode(blob["content"])
        calculated = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        if calculated != item["sha"]:
            raise ValueError("Transport Git blob SHA mismatch")
        pieces.append(raw)
    compressed = base64.b64decode(b"".join(pieces), validate=True)
    if hashlib.sha256(compressed).hexdigest() != XZ_SHA:
        raise ValueError("Compressed source identity mismatch")
    decoder = lzma.LZMADecompressor()
    decoded = decoder.decompress(compressed, max_length=MAX_BYTES)
    if not decoder.eof or decoder.unused_data or hashlib.sha256(decoded).hexdigest() != DECODED_SHA:
        raise ValueError("Decoded source identity mismatch")
    if zipfile.is_zipfile(io.BytesIO(decoded)):
        with zipfile.ZipFile(io.BytesIO(decoded)) as archive:
            if sum(item.file_size for item in archive.infolist()) > MAX_BYTES:
                raise ValueError("Expanded source is too large")
            pairs = [(item.filename, archive.read(item)) for item in archive.infolist() if not item.is_dir()]
    else:
        data = json.loads(decoded)
        data = data.get("files", data.get("source_files", data))
        if isinstance(data, list):
            data = {item["path"]: item.get("content", item.get("text")) for item in data}
        pairs = []
        for name, value in data.items():
            if "/" not in name:
                continue
            encoding = "utf-8"
            if isinstance(value, dict):
                encoding = value.get("encoding", encoding)
                value = value.get("content", value.get("text"))
            raw = base64.b64decode(value, validate=True) if encoding == "base64" else value.encode("utf-8")
            pairs.append((name, raw))
    result = {}
    for name, raw in pairs:
        path = PurePosixPath(name.replace("\\", "/"))
        if path.is_absolute() or ".." in path.parts or ":" in name or path.parts[0] not in {"crates", "tools", "docs", ".github"}:
            raise ValueError("Unsafe source path: " + name)
        key = str(path)
        if key in result:
            raise ValueError("Duplicate source path: " + key)
        result[key] = raw
    if len(result) != 11 or "tools/prepare_n4.py" not in result:
        raise ValueError("Incomplete N4 source package")
    return result


def verify(root: Path) -> dict:
    checks = {
        "crates/mechofly-core/src/lib.rs": ["pub mod behavior_dynamics;", "pub mod behavior_parameters;", "pub mod behavior_validation;"],
        "crates/mechofly-core/src/model.rs": ["pub fn new_duration_aware(", "pub behavior_dynamics: Option<BehaviorDynamicsState>", "dynamics.advance(self.state.seed, intent)", "dynamics.validate(state.seed, state.frame, state.behavior, state.behavior_age_frames)?"],
        "crates/mechofly-core/src/behavior_intent.rs": ["pub fn build_duration_aware(", "intent.autonomous_schedule_slot = u8::MAX"],
        "crates/mechofly-core/src/behavior_telemetry.rs": ["pub dynamics: Option<crate::behavior_dynamics::DynamicsTransition>", "InvalidControllerState"],
        "crates/mechofly-app/src/runtime.rs": ["ModelEngine::new_duration_aware(Arc::clone(&graph), seed)"],
        "crates/mechofly-app/src/app.rs": ["crate::behavior_inspector::draw(lab_ui, session)"],
        "crates/mechofly-app/src/main.rs": ["mod behavior_campaign;", "mod behavior_inspector;", '"--behavior-campaign"'],
        "crates/mechofly-app/src/self_test.rs": ["schema_version: 10", "validate_dynamics()?"],
    }
    count = 0
    for relative, markers in checks.items():
        text = (root / relative).read_text(encoding="utf-8-sig")
        compact = "".join(text.split())
        for marker in markers:
            if "".join(marker.split()) not in compact:
                raise ValueError(f"N4 integration missing in {relative}: {marker}")
            count += 1
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8-sig")
    if "self.session.engine.state.frame.is_multiple_of(90)" in app:
        raise ValueError("Periodic application policy injection remains active")
    for relative in ["crates/mechofly-core/src/behavior_dynamics.rs", "crates/mechofly-core/src/behavior_parameters.rs", "crates/mechofly-core/src/behavior_validation.rs", "crates/mechofly-core/tests/n4_dynamics.rs"]:
        if not (root / relative).is_file():
            raise ValueError("Required controller source is missing: " + relative)
    return {"status": "PASS", "integration_assertions": count, "runtime_constructor": "new_duration_aware", "self_test_schema": 10, "canonical_deployment": False}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assemble", action="store_true")
    parser.add_argument("--root", default=".")
    parser.add_argument("--receipt")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    if args.assemble and not (root / "crates/mechofly-core/src/behavior_dynamics.rs").exists():
        subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=root, check=True)
        files = source_files()
        for relative, data in files.items():
            path = root / relative
            if path.exists():
                raise ValueError("Refusing to overwrite existing assembly source: " + relative)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        # Critical fix: integrate BEFORE committing, compiling, or claiming N4.
        subprocess.run([sys.executable, str(root / "tools/prepare_n4.py"), str(root)], cwd=root, check=True)
        (root / "tools/prepare_n4.py").unlink()
    report = verify(root)
    if args.receipt:
        path = Path(args.receipt)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("N4_RUNTIME_INTEGRATION=PASS", flush=True)
    print("N4_INTEGRATION_ASSERTIONS=" + str(report["integration_assertions"]), flush=True)


if __name__ == "__main__":
    main()
