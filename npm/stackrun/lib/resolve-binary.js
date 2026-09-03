import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

const PLATFORMS = {
  "linux-x64-gnu": "@jasenmichael/stackrun-linux-x64-gnu",
  "linux-arm64-gnu": "@jasenmichael/stackrun-linux-arm64-gnu",
  "linux-x64-musl": "@jasenmichael/stackrun-linux-x64-musl",
  "darwin-x64": "@jasenmichael/stackrun-darwin-x64",
  "darwin-arm64": "@jasenmichael/stackrun-darwin-arm64",
  "win32-x64-msvc": "@jasenmichael/stackrun-win32-x64-msvc",
};

function linuxLibc() {
  try {
    const report = process.report?.getReport?.();
    if (report?.header?.glibcVersionRuntime) {
      return "gnu";
    }
  } catch {
    // fall through
  }
  return existsSync("/lib/ld-musl-x86_64.so.1") ||
    existsSync("/lib/ld-musl-aarch64.so.1")
    ? "musl"
    : "gnu";
}

export function platformKey() {
  const { platform, arch } = process;
  if (platform === "linux") {
    return `linux-${arch}-${linuxLibc()}`;
  }
  if (platform === "darwin") {
    return `darwin-${arch}`;
  }
  if (platform === "win32" && arch === "x64") {
    return "win32-x64-msvc";
  }
  return `${platform}-${arch}`;
}

function binaryName() {
  return process.platform === "win32" ? "stackrun.exe" : "stackrun";
}

export function resolveBinary() {
  if (process.env.STACKRUN_BINARY) {
    return process.env.STACKRUN_BINARY;
  }

  const key = platformKey();
  const pkg = PLATFORMS[key];
  if (pkg) {
    try {
      const pkgJson = require.resolve(`${pkg}/package.json`);
      const candidate = join(dirname(pkgJson), binaryName());
      if (existsSync(candidate)) {
        return candidate;
      }
    } catch {
      // optionalDependency missing
    }
  }

  const bundled = join(here, "..", "bin", binaryName());
  if (existsSync(bundled) && bundled.endsWith(binaryName())) {
    // bin/ holds the JS wrapper, not the native binary
  }

  const err = new Error(
    `stackrun native binary not found for ${key}. Set STACKRUN_BINARY or install the matching optional platform package.`,
  );
  err.code = "STACKRUN_BINARY_MISSING";
  throw err;
}
