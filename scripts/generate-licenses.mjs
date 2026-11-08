import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const version = readFileSync(resolve(root, "VERSION"), "utf8").trim();
const generatedAt = new Date().toISOString().slice(0, 10);
const licenseTexts = new Map();

function read(path) {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return null;
  }
}

function clean(text) {
  return (text ?? "").replace(/\r\n/g, "\n").trim();
}

function addLicenseText(license, text, source) {
  const key = license.trim();
  if (!key || key === "not recorded" || licenseTexts.has(key)) return;
  const value = clean(text);
  if (!value) return;
  licenseTexts.set(key, {
    license: key,
    text: value.slice(0, 60000),
    source,
  });
}

function licenseOf(pkg) {
  const value = pkg.license ?? pkg.licenses;
  if (typeof value === "string") return value.trim() || "not recorded";
  if (Array.isArray(value)) {
    const parts = value
      .map((entry) => (typeof entry === "string" ? entry : entry?.type))
      .filter(Boolean);
    return parts.join(" OR ") || "not recorded";
  }
  if (value && typeof value === "object" && typeof value.type === "string") {
    return value.type;
  }
  return "not recorded";
}

function sourceOf(pkg) {
  const repo = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
  const raw = (repo || pkg.homepage || "").trim();
  if (!raw) return "";
  return raw
    .replace(/^git\+/, "")
    .replace(/^git:\/\//, "https://")
    .replace(/^github:/, "https://github.com/")
    .replace(/\.git$/, "");
}

const LICENSE_FILES = [
  "LICENSE",
  "LICENSE.md",
  "LICENSE.txt",
  "LICENCE",
  "LICENCE.md",
  "LICENCE.txt",
  "COPYING",
  "COPYING.md",
  "COPYING.txt",
  "UNLICENSE",
  "UNLICENSE.txt",
];

function findLicenseFile(dir) {
  for (const name of LICENSE_FILES) {
    const path = join(dir, name);
    if (existsSync(path) && statSync(path).isFile()) return path;
  }
  return null;
}

function scanNodeModules() {
  const found = new Map();

  function readPackage(dir) {
    const raw = read(join(dir, "package.json"));
    if (!raw) return;
    let pkg;
    try {
      pkg = JSON.parse(raw);
    } catch {
      return;
    }
    if (!pkg.name || !pkg.version || pkg.private) return;
    const license = licenseOf(pkg);
    const key = `${pkg.name}::${license}`;
    if (!found.has(key)) {
      found.set(key, {
        name: pkg.name,
        versions: new Set(),
        license,
        source: sourceOf(pkg),
      });
    }
    found.get(key).versions.add(pkg.version);
    if (license !== "not recorded") {
      const file = findLicenseFile(dir);
      if (file) addLicenseText(license, read(file), `${pkg.name}@${pkg.version}`);
    }
  }

  function readNodeModules(nodeModules) {
    if (!existsSync(nodeModules)) return;
    for (const entry of readdirSync(nodeModules, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      if (entry.name.startsWith("@")) {
        const scope = join(nodeModules, entry.name);
        for (const scoped of readdirSync(scope, { withFileTypes: true })) {
          if (scoped.isDirectory()) readPackage(join(scope, scoped.name));
        }
      } else {
        readPackage(join(nodeModules, entry.name));
      }
    }
  }

  const pnpmDir = resolve(root, "node_modules/.pnpm");
  if (existsSync(pnpmDir)) {
    for (const entry of readdirSync(pnpmDir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        readNodeModules(join(pnpmDir, entry.name, "node_modules"));
      }
    }
  }
  readNodeModules(resolve(root, "node_modules"));

  return [...found.values()]
    .map((entry) => ({
      name: entry.name,
      versions: [...entry.versions].sort(),
      license: entry.license,
      source: entry.source,
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function scanRust() {
  let output;
  let platform = null;
  try {
    const host = execFileSync("rustc", ["-vV"], {
      cwd: root,
      encoding: "utf8",
    });
    platform = host.match(/^host:\s*(.+)$/m)?.[1]?.trim() ?? null;
  } catch {
    platform = null;
  }

  const args = [
    "metadata",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--format-version",
    "1",
  ];
  if (platform) args.push("--filter-platform", platform);

  try {
    output = execFileSync("cargo", args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 128 * 1024 * 1024,
    });
  } catch (error) {
    throw new Error(
      `cargo metadata failed. Install Rust and run this again. ${error instanceof Error ? error.message : ""}`,
    );
  }

  const metadata = JSON.parse(output);
  const rootId = metadata.resolve?.root ?? null;
  const nodeIds = new Set((metadata.resolve?.nodes ?? []).map((node) => node.id));
  const found = new Map();

  for (const pkg of metadata.packages ?? []) {
    if (!pkg.name || !pkg.version) continue;
    if (pkg.id === rootId) continue;
    if (nodeIds.size > 0 && !nodeIds.has(pkg.id)) continue;

    const license = pkg.license?.trim() || "not recorded";
    const key = `${pkg.name}::${license}`;
    if (!found.has(key)) {
      found.set(key, {
        name: pkg.name,
        versions: new Set(),
        license,
        source: (pkg.repository || pkg.homepage || "").replace(/\.git$/, ""),
      });
    }
    found.get(key).versions.add(pkg.version);

    if (license !== "not recorded" && pkg.manifest_path) {
      const dir = dirname(pkg.manifest_path);
      const file = pkg.license_file
        ? resolve(dir, pkg.license_file)
        : findLicenseFile(dir);
      if (file && existsSync(file)) {
        addLicenseText(license, read(file), `${pkg.name}@${pkg.version}`);
      }
    }
  }

  return {
    platform,
    packages: [...found.values()]
      .map((entry) => ({
        name: entry.name,
        versions: [...entry.versions].sort(),
        license: entry.license,
        source: entry.source,
      }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  };
}

addLicenseText("MIT", read(resolve(root, "LICENSE")), "Kaminari LICENSE");
addLicenseText(
  "SIL Open Font License 1.1",
  read(resolve(root, "licenses/OFL-1.1.txt")),
  "Kaminari font notices",
);

const bundled = [
  {
    name: "Kaminari",
    versions: [version],
    license: "MIT",
    source: "https://github.com/graynb25/kaminari",
  },
  {
    name: "Sora",
    versions: ["bundled"],
    license: "SIL Open Font License 1.1",
    source: "https://github.com/sora-xor/sora-font",
  },
  {
    name: "Inter",
    versions: ["bundled"],
    license: "SIL Open Font License 1.1",
    source: "https://github.com/rsms/inter",
  },
  {
    name: "JetBrains Mono",
    versions: ["bundled"],
    license: "SIL Open Font License 1.1",
    source: "https://github.com/JetBrains/JetBrainsMono",
  },
  {
    name: "Public Domain Animation sample",
    versions: ["built in"],
    license: "MIT",
    source: "https://github.com/graynb25/kaminari/tree/main/extensions/public-domain",
  },
];

const rustScan = scanRust();
const rust = rustScan.packages;
const javascript = scanNodeModules();

function withKeys(group, packages) {
  return packages.map((pkg) => ({
    key: `${group}:${pkg.name}::${pkg.license}`,
    ...pkg,
  }));
}

const groups = [
  {
    id: "bundled",
    label: "Bundled with this install",
    packages: withKeys("bundled", bundled),
  },
  {
    id: "rust",
    label: "Rust",
    packages: withKeys("rust", rust),
  },
  {
    id: "javascript",
    label: "JavaScript",
    packages: withKeys("javascript", javascript),
  },
];

const packages = groups.flatMap((group) => group.packages);
const licenseIds = new Set(packages.map((pkg) => pkg.license));
const notRecorded = packages.filter((pkg) => pkg.license === "not recorded").length;
const proprietary = packages.filter((pkg) =>
  /unlicensed|see license|commercial|proprietary/i.test(pkg.license),
).length;

const payload = {
  generatedAt,
  appVersion: version,
  platform: rustScan.platform,
  summary: {
    packages: packages.length,
    licenses: licenseIds.size,
    notRecorded,
    proprietary,
  },
  groups,
  licenseTexts: [...licenseTexts.values()].sort((a, b) =>
    a.license.localeCompare(b.license),
  ),
  notes: [
    "Generated from Cargo.lock and the packages installed under node_modules. License identifiers come from each package manifest.",
    "Full texts are included where a license file was found in the installed package.",
    "License texts are the standard text for each identifier. Copyright notices remain with their packages.",
    "For dual-licensed packages, the text shown is the alternative found in the installed package.",
    "Regenerate this file with `pnpm licenses:build` after dependency changes.",
    "AniList metadata is a service, not a bundled package. It is used under AniList's API terms.",
    "The built-in Public Domain Animation sample is project code, not a third-party source.",
  ],
};

writeFileSync(
  resolve(root, "public/licenses.json"),
  `${JSON.stringify(payload, null, 2)}\n`,
);

console.log(
  `Licenses: ${payload.summary.packages} packages, ${payload.summary.licenses} license IDs, ${payload.licenseTexts.length} texts`,
);
