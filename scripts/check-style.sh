#!/usr/bin/env sh
# Check for em dash characters in tracked text files.
# Run before commit. Fails if any are found.
# Uses python so this script does not contain the character itself.
set -eu
python3 - <<'PY'
import pathlib, sys
char = chr(0x2014)
roots = [pathlib.Path(p) for p in ["AGENTS.md","CONTRIBUTING.md","README.md","LICENSE","SECURITY.md","CHANGELOG.md","docs"]]
roots += [pathlib.Path("src"), pathlib.Path("src-tauri")]
found = []
for r in roots:
    if not r.exists():
        continue
    files = [r] if r.is_file() else list(r.rglob("*"))
    for f in files:
        if f.is_file():
            try:
                if char in f.read_text(encoding="utf-8", errors="ignore"):
                    found.append(str(f))
            except Exception:
                pass
if found:
    print("Found an em dash in:")
    print("\n".join(found))
    print("Use commas or periods instead.")
    sys.exit(1)
print("Style check passed.")
PY
