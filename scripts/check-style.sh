#!/usr/bin/env sh
# Check for em dash characters in tracked text files.
# Run before commit. Fails if any are found.
# Uses python so this script does not contain the character itself.
set -eu
if command -v python >/dev/null 2>&1; then
  PY=python
else
  PY=python3
fi
"$PY" - <<'PY'
import pathlib, sys
char = chr(0x2014)
roots = [pathlib.Path(p) for p in ["CONTRIBUTING.md","README.md","LICENSE","SECURITY.md","CHANGELOG.md","docs"]]
roots += [pathlib.Path("src"), pathlib.Path("src-tauri")]
skip = {".git", "node_modules", "target", "dist", "gen"}
found = []
for r in roots:
    if not r.exists():
        continue
    if r.is_file():
        files = [r]
    else:
        files = [p for p in r.rglob("*") if p.is_file() and not (set(p.parts) & skip)]
    for f in files:
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
