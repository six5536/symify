#!/usr/bin/env node
// Set one version across the whole project in lockstep: the Cargo workspace
// version (and the internal symify-core dep), every package.json under
// packages/, and the launcher's pinned optionalDependencies.
//
// Usage: node scripts/set-version.mjs <version>

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: node scripts/set-version.mjs <semver>");
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Cargo.toml: workspace package version + internal symify-core dependency pin.
const cargoPath = join(root, "Cargo.toml");
let cargo = readFileSync(cargoPath, "utf8");
cargo = cargo.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
cargo = cargo.replace(
  /(symify-core = \{ path = "crates\/lib\/symify-core", version = ")[^"]*(" \})/,
  `$1${version}$2`,
);
writeFileSync(cargoPath, cargo);

// Every packages/*/package.json: version, plus the launcher's optionalDependencies.
const pkgsDir = join(root, "packages");
for (const name of readdirSync(pkgsDir)) {
  const p = join(pkgsDir, name, "package.json");
  let json;
  try {
    json = JSON.parse(readFileSync(p, "utf8"));
  } catch {
    continue;
  }
  json.version = version;
  if (json.optionalDependencies) {
    for (const dep of Object.keys(json.optionalDependencies)) {
      if (dep.startsWith("@six5536/symify-")) {
        json.optionalDependencies[dep] = version;
      }
    }
  }
  writeFileSync(p, JSON.stringify(json, null, 2) + "\n");
}

console.log(`set version to ${version} across Cargo workspace and packages/`);
