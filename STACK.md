# STACK.md — tools and hosting

This file wins on tools and hosting. If it disagrees with `SPEC.md`, `SPEC.md` still wins on product behavior.

## Stack

Stackrun is a **standalone Rust CLI**. Node.js is optional and is invoked only as an out-of-process helper when a JS/TS config file must be evaluated.

```
stackrun (Rust)
├── clap CLI
├── native config (JSON / JSONC / JSON5 / YAML / TOML / .env / .stackrc)
├── optional Node + Jiti subprocess (JS/TS config only)
├── stack run (hooks, per-command tunnel siblings, concurrent commands)
├── process (std::process + threads; no Tokio unless later justified)
└── tunnel (independent; cloudflared CLI only — not embedded JS, no REST DNS)
```

### Layout

| Path | Role |
| --- | --- |
| `Cargo.toml` | Single binary crate |
| `src/main.rs` | CLI entry |
| `src/lib.rs` | Config load, stack run, process, tunnel (Jiti bridge is crate-private) |
| `tests/` | Integration tests |
| `npm/stackrun` | npm bin + `stackrun()` / `defineStackrunConfig` (spawn native binary) |
| `scripts/install.sh` | Curl installer (copied to GitHub Pages at publish) |
| `scripts/generate-pages.sh` | Builds `site/` for the `docs` branch |

Single crate at the repo root. Split into a workspace later only if modules need independent versioning.

### Rust crates

| Crate | Why |
| --- | --- |
| `clap` (derive) | Mature CLI; flags/aliases/env |
| `serde` / `serde_json` | Canonical config + JSON |
| `json5` | JSON5 and JSONC (comments / trailing commas) |
| `serde_yaml` | YAML `.yaml` / `.yml` |
| `toml` | TOML |
| (custom `.env` parser) | `.env` parse + `${VAR}` interpolation matching c12 |
| `thiserror` | Library errors |
| `anyhow` | CLI boundary |
| `tracing` / `tracing-subscriber` | Logging |
| `ctrlc` | SIGINT / graceful shutdown |
| `owo-colors` | Prefix colors |

External tools:

- **`cloudflared`** — required on PATH when any tunnel is on. Quick: `tunnel --url`. Named: `tunnel create` / `route dns` / `tunnel run --url` / `tunnel delete -f`.
- **`node` + local `jiti`** — JS/TS config only. Optional `--jiti npx` / `STACKRUN_JITI=npx` uses `npx -p jiti` after a local miss. Never install into the project.

Not used:

- **c12** — forbidden as a dependency. Behavior reproduced only where Stackrun actually used it.
- **Any JS runtime in the binary** (no `deno_core`, `v8`, `boa` as a config engine).
- **Tokio** — not a dependency. Process orchestration stays `std::process` + threads.
- **reqwest / Cloudflare HTTP API** — DNS is `cloudflared tunnel route dns` only.

### JS/TS config bridge

When a discovered or explicit path is `.js` / `.mjs` / `.cjs` / `.ts` / `.mts` / `.cts`:

1. Detect JS/TS.
2. Require `node` on PATH (`NodeRequired` if missing).
3. Run a small Node script (`STACKRUN_BRIDGE_FILE` + the same bridge source) that `import("jiti")` from the project cwd.
4. If jiti is missing: default is `JitiRequired` (YAML/TOML/JSON, `npm i -D jiti`, or `--jiti npx`). With `--jiti npx` / `STACKRUN_JITI=npx`, retry `npx -p jiti node --input-type=module -e <script>` (`NpxRequired` if `npx` is missing). First `npx` run may use the network. No `--yes` unless npx cannot run without it. Never `npm i` / silent install. Never default to `npx`.
5. Resolve `default` export (or module).
6. Write JSON to stdout.
7. Rust deserializes into `StackrunConfig`.

Jiti is **not** a Rust crate. It is a Node helper used only for this path.

### Hosting / runtime

- **Where it runs:** developer machines and CI as a local CLI. No server, no hosted control plane.
- **Tunnel:** `cloudflared` on the developer machine. Named tunnels need `cloudflared tunnel login` (`cert.pem`) and a hostname on that Cloudflare account. Quick tunnels need the binary only.
- **OS:** Unix first (process groups). Windows via `cmd /c` without process-group extras until a later pass.
- **CI:** GitHub Actions, `cargo fmt` / clippy / test.
- **Toolchain:** `rust-toolchain.toml` pins `stable`.

### Packaging

- Native binaries: linux gnu/musl, macOS, Windows x64 (GitHub Actions on merge to `main`).
- npm `stackrun`: `bin` wrapper + `import { stackrun, defineStackrunConfig }` (spawn the native binary, `--json` for a config object).
- Platform packages: `@jasenmichael/stackrun-<os>-<arch>` as `optionalDependencies`.
- Publish (GH Release, crates.io, npm) only from release.yml after tests pass and the crate version is newer than the last tag. Publish jobs stay guarded until the first release PR.
- Docs generators: automd (README), changelogen (CHANGELOG / GitHub Release notes). Run those in the release PR, not from a laptop publish.
- GitHub Pages: publish job generates `site/` (`install.sh` + landing `index.html` + README) and force-pushes the `docs` branch. Repo Pages source should be `docs` / root → https://jasenmichael.github.io/stackrun/
- Curl install: `curl -fsSL https://jasenmichael.github.io/stackrun/install.sh | sh` (source: `scripts/install.sh`).
