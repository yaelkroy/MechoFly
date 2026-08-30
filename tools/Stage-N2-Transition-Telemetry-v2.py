#!/usr/bin/env python3
"""Run the N2 staging tool with literal-newline expansion repaired."""

from pathlib import Path


path = Path("tools/Stage-N2-Transition-Telemetry.py")
source = path.read_text(encoding="utf-8")
old = '''    updated = text[: match.start()] + expanded + text[match.end() :]
    write(path, updated)
'''
new = r'''    expanded = expanded.replace("\\n", "\n")
    updated = text[: match.start()] + expanded + text[match.end() :]
    write(path, updated)
'''
if source.count(old) != 1:
    raise SystemExit("expected one N2 regex-helper insertion point")
source = source.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(path),
}
exec(compile(source, str(path), "exec"), namespace)
