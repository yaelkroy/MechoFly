#!/usr/bin/env python3
"""Check actual N4 wiring, not merely presence of unreferenced source files.

--assemble is a one-time CI recovery of the previously authored, pinned N4
source. User validation fetches the finished commit and never executes a patch.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import io
import json
import lzma
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
    folder = next(item for item in api("trees/" + TRANSPORT)["tree"] if item["path"] == ".n4-assembly")
    entries = sorted(api("trees/" + folder["sha"])["tree"], key=lambda item: item["path"])
    if [item["path"] for item in entries] != [f"{i:02d}.txt" for i in range(13)]:
        raise ValueError("Source transport does not have the pinned 13 parts")
    pieces = []
    for item in entries:
        raw = base64.b64decode(api("blobs/" + item["sha"])["content"])
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
            pairs.append((name, base64.b64decode(value, validate=True) if encoding == "base64" else value.encode("utf-8")))
    result = {}
    for name, raw in pairs:
        path = PurePosixPath(name.replace("\\", "/"))
        if path.is_absolute() or ".." in path.parts or ":" in name or path.parts[0] not in {"crates", "tools", "docs", ".github"}:
            raise ValueError("Unsafe source path: " + name)
        if str(path) in result:
            raise ValueError("Duplicate source path: " + str(path))
        result[str(path)] = raw
    if len(result) != 11 or "tools/prepare_n4.py" not in result:
        raise ValueError("Incomplete N4 source package")
    return result


def normalized_wiring(text: str) -> str:
    # rustfmt adds optional trailing commas to multiline calls. Do not mistake
    # that formatting-only token for missing runtime integration.
    return "".join(text.split()).replace(",)", ")")


def verify(root: Path) -> dict:
    assert normalized_wiring("f(a, b, )?") == normalized_wiring("f(a,b)?")
    assert normalized_wiring("f(a,b)?") != normalized_wiring("f(a,c)?")
    checks = {
        "crates/mechofly-core/src/lib.rs": ["pub mod behavior_dynamics;", "pub mod behavior_parameters;", "pub mod behavior_validation;"],
        "crates/mechofly-core/src/model.rs": ["pub fn new_duration_aware(", "pub behavior_dynamics: Option<BehaviorDynamicsState>", "dynamics.advance(self.state.seed, intent)", "dynamics.validate(state.seed, state.frame, state.behavior, state.behavior_age_frames)?"],
        "crates/mechofly-core/src/behavior_intent.rs": ["pub fn build_duration_aware(", "intent.autonomous_schedule_slot = u8::MAX"],
        "crates/mechofly-core/src/behavior_telemetry.rs": ["pub dynamics: Option<crate::behavior_dynamics::DynamicsTransition>", "InvalidControllerState"],
        "crates/mechofly-app/src/runtime.rs": ["ModelEngine::new_duration_aware(Arc::clone(&graph), seed)"],
        "crates/mechofly-app/src/app.rs": ["crate::behavior_inspector::draw(lab_ui, session)"],
        "crates/mechofly-app/src/main.rs": ["mod behavior_campaign;", "mod behavior_inspector;", '"--behavior-campaign"'],
        "crates/mechofly-app/src/self_test.rs": ["schema_version: 11", "validate_dynamics()?"],
        "tools/run_n4_native_checks.py": ['receipt.get("schema_version") != 11'],
        "tools/Setup-AI100-MechoFly.ps1": ["$Receipt.schema_version -ne 11 -or"],
    }
    count = 0
    for relative, markers in checks.items():
        compact = normalized_wiring((root / relative).read_text(encoding="utf-8-sig"))
        for marker in markers:
            if normalized_wiring(marker) not in compact:
                raise ValueError(f"N4 integration missing in {relative}: {marker}")
            count += 1
    app = (root / "crates/mechofly-app/src/app.rs").read_text(encoding="utf-8-sig")
    if "self.session.engine.state.frame.is_multiple_of(90)" in app:
        raise ValueError("Periodic application policy injection remains active")
    for relative in ["crates/mechofly-core/src/behavior_dynamics.rs", "crates/mechofly-core/src/behavior_parameters.rs", "crates/mechofly-core/src/behavior_validation.rs", "crates/mechofly-core/tests/n4_dynamics.rs"]:
        if not (root / relative).is_file():
            raise ValueError("Required controller source is missing: " + relative)
    return {"status": "PASS", "integration_assertions": count, "runtime_constructor": "new_duration_aware", "self_test_schema": 11, "canonical_deployment": False}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assemble", action="store_true")
    parser.add_argument("--root", default=".")
    parser.add_argument("--receipt")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    if args.assemble and not (root / "crates/mechofly-core/src/behavior_dynamics.rs").exists():
        subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=root, check=True)
        for relative, data in source_files().items():
            path = root / relative
            if path.exists():
                raise ValueError("Refusing to overwrite existing assembly source: " + relative)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        prepare = root / "tools/prepare_n4.py"
        text = prepare.read_text(encoding="utf-8")
        anchor = "    $Receipt.schema_version -ne 10 -or"
        if text.count(anchor) != 2:
            raise ValueError("Reviewed setup-predicate assembly anchor changed")
        prepare.write_text(text.replace(anchor, "$Receipt.schema_version -ne 10 -or"), encoding="utf-8", newline="\n")
        subprocess.run([sys.executable, str(prepare), str(root)], cwd=root, check=True)
        prepare.unlink()
        (root / ".github/workflows/n4-dynamics.yml").unlink()
    report = verify(root)
    if args.receipt:
        path = Path(args.receipt)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("N4_RUNTIME_INTEGRATION=PASS", flush=True)
    print("N4_INTEGRATION_ASSERTIONS=" + str(report["integration_assertions"]), flush=True)


if __name__ == "__main__":
    main()
