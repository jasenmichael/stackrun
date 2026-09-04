import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const REPO = "jasenmichael/stackrun";

const TARGETS = {
  "linux-x64-gnu": { triple: "x86_64-unknown-linux-gnu", archive: "tar.gz" },
  "linux-arm64-gnu": { triple: "aarch64-unknown-linux-gnu", archive: "tar.gz" },
  "darwin-x64": { triple: "x86_64-apple-darwin", archive: "tar.gz" },
  "darwin-arm64": { triple: "aarch64-apple-darwin", archive: "tar.gz" },
  "win32-x64-msvc": { triple: "x86_64-pc-windows-msvc", archive: "zip" },
  "win32-arm64-msvc": { triple: "aarch64-pc-windows-msvc", archive: "zip" },
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
  if (platform === "win32" && (arch === "x64" || arch === "arm64")) {
    return `win32-${arch}-msvc`;
  }
  return `${platform}-${arch}`;
}

export function rustTriple() {
  return TARGETS[platformKey()]?.triple ?? null;
}

function binaryName() {
  return process.platform === "win32" ? "stackrun.exe" : "stackrun";
}

function packageVersion() {
  const pkg = require("../package.json");
  return String(pkg.version).replace(/^v/, "");
}

function cacheDir(version) {
  const base =
    process.env.STACKRUN_CACHE ||
    join(homedir() || tmpdir(), ".cache", "stackrun");
  return join(base, version);
}

function cachedBinary(version) {
  return join(cacheDir(version), binaryName());
}

function downloadText(url) {
  const res = spawnSync(
    process.execPath,
    [
      "-e",
      `fetch(${JSON.stringify(url)}).then(r=>{if(!r.ok){console.error(r.status);process.exit(1)}return r.text()}).then(t=>process.stdout.write(t)).catch(e=>{console.error(e);process.exit(1)})`,
    ],
    { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
  );
  if (res.status !== 0) {
    throw new Error(res.stderr || `download failed: ${url}`);
  }
  return res.stdout;
}

function downloadFile(url, dest) {
  const res = spawnSync(
    process.execPath,
    [
      "-e",
      `fetch(${JSON.stringify(url)}).then(r=>{if(!r.ok){console.error(r.status);process.exit(1)}return r.arrayBuffer()}).then(b=>require('fs').writeFileSync(process.argv[1],Buffer.from(b))).catch(e=>{console.error(e);process.exit(1)})`,
      dest,
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (res.status !== 0) {
    throw new Error(res.stderr || `download failed: ${url}`);
  }
}

function verifySha256(file, sumsText, archiveName) {
  const line = sumsText
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l.endsWith(archiveName));
  if (!line) {
    throw new Error(`no SHA256SUMS entry for ${archiveName}`);
  }
  const expected = line.split(/\s+/)[0];
  const actual = createHash("sha256").update(readFileSync(file)).digest("hex");
  if (actual !== expected) {
    throw new Error(`checksum mismatch for ${archiveName}`);
  }
}

function extractArchive(archivePath, destDir, kind) {
  mkdirSync(destDir, { recursive: true });
  if (kind === "zip") {
    const res = spawnSync(
      "tar",
      ["-xf", archivePath, "-C", destDir],
      { encoding: "utf8" },
    );
    if (res.status !== 0) {
      const ps = spawnSync(
        "powershell",
        ["-NoProfile", "-Command", `Expand-Archive -Force -Path '${archivePath}' -DestinationPath '${destDir}'`],
        { encoding: "utf8" },
      );
      if (ps.status !== 0) {
        throw new Error(ps.stderr || res.stderr || "extract zip failed");
      }
    }
    return;
  }
  const res = spawnSync("tar", ["-xzf", archivePath, "-C", destDir], {
    encoding: "utf8",
  });
  if (res.status !== 0) {
    throw new Error(res.stderr || "extract tar.gz failed");
  }
}

export function ensureBinary() {
  if (process.env.STACKRUN_BINARY) {
    return process.env.STACKRUN_BINARY;
  }
  const version = packageVersion();
  const cached = cachedBinary(version);
  if (existsSync(cached)) {
    return cached;
  }
  if (process.env.STACKRUN_SKIP_DOWNLOAD === "1") {
    const err = new Error(
      `stackrun native binary not found for ${platformKey()}. Set STACKRUN_BINARY or install with network access.`,
    );
    err.code = "STACKRUN_BINARY_MISSING";
    throw err;
  }

  const spec = TARGETS[platformKey()];
  if (!spec) {
    const err = new Error(
      `stackrun has no GitHub Release binary for ${platformKey()}. Set STACKRUN_BINARY or build from source.`,
    );
    err.code = "STACKRUN_BINARY_MISSING";
    throw err;
  }

  const archiveName = `stackrun-v${version}-${spec.triple}.${spec.archive}`;
  const base = `https://github.com/${REPO}/releases/download/v${version}`;
  const tmp = join(tmpdir(), `stackrun-${process.pid}`);
  mkdirSync(tmp, { recursive: true });
  const archivePath = join(tmp, archiveName);
  downloadFile(`${base}/${archiveName}`, archivePath);
  const sums = downloadText(`${base}/SHA256SUMS`);
  verifySha256(archivePath, sums, archiveName);
  extractArchive(archivePath, tmp, spec.archive);

  const found = findFile(tmp, binaryName());
  if (!found) {
    throw new Error(`archive ${archiveName} had no ${binaryName()}`);
  }
  mkdirSync(cacheDir(version), { recursive: true });
  writeFileSync(cached, readFileSync(found));
  try {
    const { chmodSync } = require("node:fs");
    chmodSync(cached, 0o755);
  } catch {
    // windows
  }
  return cached;
}

function findFile(root, name) {
  const { readdirSync, statSync } = require("node:fs");
  const stack = [root];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of readdirSync(dir)) {
      const p = join(dir, entry);
      if (statSync(p).isDirectory()) {
        stack.push(p);
      } else if (entry === name) {
        return p;
      }
    }
  }
  return null;
}

export function resolveBinary() {
  return ensureBinary();
}
