#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { resolveBinary } from "../lib/resolve-binary.js";

let bin;
try {
  bin = resolveBinary();
} catch (err) {
  console.error(err.message);
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
