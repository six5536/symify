#!/usr/bin/env node
// Behavioural smoke test for a compiled symify binary: adopt a file into a
// temp store, then assert the store content and a clean status. Release CI
// runs it against each built artifact its runner can execute; locally run
// `npm run smoke` after `cargo build --release -p symify`.
//
// The adopt creates a symlink, so on Windows it needs Developer Mode or
// elevation; CI runners execute elevated.

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const bin =
  process.argv[2] ??
  join("target", "release", process.platform === "win32" ? "symify.exe" : "symify");
if (!existsSync(bin)) {
  fail(`no binary at ${bin} — build first: cargo build --release -p symify`);
}

function fail(message) {
  console.error(`release-smoke: ${message}`);
  process.exit(1);
}

/** Run the binary, asserting the exit status; returns the spawn result. */
function run(args, expectStatus) {
  const r = spawnSync(bin, args, { encoding: "utf8" });
  if (r.error) {
    fail(`failed to run ${bin}: ${r.error.message}`);
  }
  if (r.status !== expectStatus) {
    console.error(r.stdout);
    console.error(r.stderr);
    fail(`\`symify ${args.join(" ")}\` exited ${r.status}, expected ${expectStatus}`);
  }
  return r;
}

// TOML basic strings treat `\` as an escape, so double Windows separators.
const tomlPath = (p) => p.replaceAll("\\", "\\\\");

const base = mkdtempSync(join(tmpdir(), "symify-smoke-"));
try {
  const live = join(base, "live");
  const store = join(base, "store");
  mkdirSync(live);
  mkdirSync(store);
  const config = join(base, "symify.toml");
  writeFileSync(
    config,
    `[settings]\nlive = "${tomlPath(live)}"\nstore = "${tomlPath(store)}"\n\n` +
      `[mappings.dotfiles.links]\n`,
  );
  const payload = "export EDITOR=vim\n";
  writeFileSync(join(live, ".bashrc"), payload);

  const version = run(["--version"], 0).stdout.trim();
  if (!/^symify \d+\.\d+\.\d+/.test(version)) {
    fail(`unexpected --version output: ${version}`);
  }

  run(["add", join(live, ".bashrc"), "-c", config], 0);

  const adopted = readFileSync(join(store, ".bashrc"), "utf8");
  if (adopted !== payload) {
    fail(`store content after adopt is ${JSON.stringify(adopted)}`);
  }

  const status = run(["status", "-c", config, "--json"], 0);
  const doc = JSON.parse(status.stdout);
  if (doc.summary.clean !== 1 || doc.entries[0]?.status !== "ok") {
    fail(`status after adopt is not clean: ${status.stdout}`);
  }

  console.log(`release-smoke OK: ${version} adopts and reports clean (${bin})`);
} finally {
  rmSync(base, { recursive: true, force: true });
}
