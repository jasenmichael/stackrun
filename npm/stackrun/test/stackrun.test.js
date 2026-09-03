import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { defineStackrunConfig, stackrun } from "../index.js";
import { buildArgs } from "../lib/stackrun.js";
import { platformKey, resolveBinary } from "../lib/resolve-binary.js";

function fakeBinary() {
  const dir = mkdtempSync(join(tmpdir(), "stackrun-"));
  const path = join(dir, "stackrun");
  writeFileSync(
    path,
    `#!/bin/sh
printf '%s\\n' "$@" > "${dir}/args.txt"
exit 0
`,
  );
  chmodSync(path, 0o755);
  return { path, argsFile: join(dir, "args.txt") };
}

test("defineStackrunConfig returns the same object", () => {
  const config = { commands: [{ name: "api", run: "echo hi" }] };
  assert.equal(defineStackrunConfig(config), config);
});

test("buildArgs with no config is empty", () => {
  assert.deepEqual(buildArgs(), []);
  assert.deepEqual(buildArgs(undefined), []);
});

test("buildArgs serializes --json and flags", () => {
  const config = { commands: [{ run: "echo hi" }] };
  assert.deepEqual(buildArgs(config, { tunnel: true, dryRun: true }), [
    "--json",
    JSON.stringify(config),
    "--tunnel",
    "--dry-run",
  ]);
});

test("stackrun() spawns the binary with no extra args", async () => {
  const fake = fakeBinary();
  process.env.STACKRUN_BINARY = fake.path;
  await stackrun();
  const args = (await import("node:fs")).readFileSync(fake.argsFile, "utf8").trim();
  assert.equal(args, "");
  delete process.env.STACKRUN_BINARY;
});

test("stackrun(config) passes --json", async () => {
  const fake = fakeBinary();
  process.env.STACKRUN_BINARY = fake.path;
  const config = { commands: [{ run: "echo hi" }] };
  await stackrun(config);
  const raw = (await import("node:fs")).readFileSync(fake.argsFile, "utf8").trim();
  assert.deepEqual(raw.split("\n"), ["--json", JSON.stringify(config)]);
  delete process.env.STACKRUN_BINARY;
});

test("stackrun(config, { tunnel: true }) adds --tunnel", async () => {
  const fake = fakeBinary();
  process.env.STACKRUN_BINARY = fake.path;
  await stackrun({ commands: [{ run: "echo x" }] }, { tunnel: true });
  const args = (await import("node:fs")).readFileSync(fake.argsFile, "utf8");
  assert.match(args, /--tunnel/);
  delete process.env.STACKRUN_BINARY;
});

test("resolveBinary uses STACKRUN_BINARY", () => {
  process.env.STACKRUN_BINARY = "/tmp/custom-stackrun";
  assert.equal(resolveBinary(), "/tmp/custom-stackrun");
  delete process.env.STACKRUN_BINARY;
});

test("resolveBinary throws when missing", () => {
  delete process.env.STACKRUN_BINARY;
  assert.throws(() => resolveBinary(), { code: "STACKRUN_BINARY_MISSING" });
});

test("platformKey is a non-empty string", () => {
  assert.ok(platformKey().length > 0);
});
