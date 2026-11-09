// Verify the libmpv artifacts before a Tauri bundle. The DLLs stay gitignored,
// so a clean checkout must run the setup first. Hashes are the P0 pins from
// docs/LIBMPV-PRE-P1.md.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const files = [
  {
    name: "libmpv-wrapper.dll",
    sha256: "0d5adead5f175c55e0790a80924ec0a2636f72e3675c79a6d9d9568b2ed2384a",
  },
  {
    name: "libmpv-2.dll",
    sha256: "2a287ecb1fe2bc5b6ab636cfec0cacbb12ffec05e4eb4e5c2cade1752e2d15e6",
  },
];

let failed = false;
for (const file of files) {
  const path = resolve(root, "src-tauri/lib", file.name);
  if (!existsSync(path)) {
    console.error(`Missing ${path}. Run scripts/prepare-libmpv-dev.ps1 first.`);
    failed = true;
    continue;
  }
  const hash = createHash("sha256").update(readFileSync(path)).digest("hex");
  if (hash !== file.sha256) {
    console.error(`Hash mismatch for ${file.name}: ${hash}`);
    failed = true;
    continue;
  }
  console.log(`${file.name} sha256 ok`);
}

if (failed) {
  process.exitCode = 1;
}
