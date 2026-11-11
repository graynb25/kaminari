import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const version = readFileSync(resolve(root, "VERSION"), "utf8").trim();
const nsis = resolve(root, "src-tauri/target/release/bundle/nsis");
const installer = resolve(nsis, `Kaminari_${version}_x64-setup.exe`);
const signaturePath = `${installer}.sig`;

if (!existsSync(installer)) {
  throw new Error(`Missing signed installer for ${version}: ${installer}`);
}
if (!existsSync(signaturePath)) {
  throw new Error(`Missing updater signature for ${version}: ${signaturePath}`);
}

const signature = readFileSync(signaturePath, "utf8").trim();
const signedText = Buffer.from(signature, "base64").toString("utf8");
const signedVersion = signedText.match(/^trusted comment:.*\bversion:(\S+)/m)?.[1];
if (!signedVersion) {
  throw new Error("The signature has no trusted-comment version. Rebuild with a Tauri CLI that records it.");
}
if (signedVersion !== version) {
  throw new Error(`The signature is for ${signedVersion}, but VERSION is ${version}.`);
}

const args = process.argv.slice(2);
const valueFor = (name) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
};

const notes = valueFor("--notes") ?? `Kaminari ${version} release. See CHANGELOG.md for details.`;
const url =
  valueFor("--url") ??
  `https://github.com/graynb25/kaminari/releases/download/v${version}/Kaminari_${version}_x64-setup.exe`;

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  platforms: {
    "windows-x86_64": { signature, url },
  },
};

const text = `${JSON.stringify(manifest, null, 2)}\n`;
const manifestPath = resolve(nsis, "latest.json");
writeFileSync(manifestPath, text, "utf8");

const first = readFileSync(manifestPath)[0];
if (first === 0xef) {
  throw new Error("latest.json has a UTF-8 BOM; the updater rejects it.");
}

console.log(`Manifest ${version} written to ${manifestPath}`);
console.log(`Signed version ${signedVersion}`);
console.log(`URL ${url}`);
