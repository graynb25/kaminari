// Build an installer-free Windows zip. The app still uses the normal per-user
// data folder; this only removes the installer step.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const version = readFileSync(path.join(root, "VERSION"), "utf8").trim();
const release = path.join(root, "src-tauri", "target", "release");
const output = path.join(release, "bundle", "portable");
const stage = path.join(output, `Kaminari_${version}_x64-portable`);
const zip = path.join(output, `Kaminari_${version}_x64-portable.zip`);
const exe = path.join(release, "kaminari.exe");
const lib = path.join(release, "lib");

if (!existsSync(exe)) {
  throw new Error(`release exe is missing: ${exe}. Run the signed Tauri build first.`);
}
if (!existsSync(lib)) {
  throw new Error(`release lib folder is missing: ${lib}.`);
}

rmSync(stage, { recursive: true, force: true });
rmSync(zip, { force: true });
mkdirSync(stage, { recursive: true });

cpSync(exe, path.join(stage, "Kaminari.exe"));
cpSync(lib, path.join(stage, "lib"), { recursive: true });
for (const file of ["LICENSE", "README.md", "CHANGELOG.md"]) {
  const source = path.join(root, file);
  if (existsSync(source)) cpSync(source, path.join(stage, file));
}

writeFileSync(
  path.join(stage, "PORTABLE.txt"),
  [
    `Kaminari ${version} portable build`,
    "",
    "Run Kaminari.exe. There is no installer and nothing is written to Program Files.",
    "User data still lives in the normal Windows app data folder, so settings and the library",
    "survive replacing this folder with a newer portable build.",
    "",
    "Microsoft Edge WebView2 Runtime is required. Windows 10 and Windows 11 normally have it.",
    "The lib folder contains the optional libmpv runtime. Keep it next to Kaminari.exe.",
    "",
    "This build ships no content and no source list. Sources are installed by you.",
    "",
  ].join("\n"),
  "utf8",
);

execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-Command",
    `Compress-Archive -Path '${stage.replaceAll("'", "''")}\\*' -DestinationPath '${zip.replaceAll("'", "''")}' -Force`,
  ],
  { stdio: "inherit" },
);

console.log(zip);
