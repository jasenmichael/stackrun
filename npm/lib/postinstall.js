#!/usr/bin/env node
import { ensureBinary } from "./resolve-binary.js";

if (process.env.STACKRUN_SKIP_DOWNLOAD === "1") {
  process.exit(0);
}

try {
  const bin = ensureBinary();
  console.log(`stackrun: native binary ready (${bin})`);
} catch (err) {
  console.warn(`stackrun: could not download native binary: ${err.message}`);
  console.warn(
    "Set STACKRUN_BINARY or retry with network. First run will try GitHub Releases again.",
  );
}
