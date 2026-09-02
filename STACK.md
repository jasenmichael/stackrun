# STACK.md — tools and hosting

This file wins on tools and hosting. If it disagrees with `SPEC.md`, `SPEC.md` still wins on product behavior.

## Stack

Stackrun is a **standalone Rust CLI**. Node.js is optional and is invoked only as an out-of-process helper when a JS/TS config file must be evaluated.

```
stackrun (Rust)
├── clap CLI
├── native config (JSON / JSONC / JSON5 / YAML / TOML / .env / .stackrc)
├── optional Node + Jiti subprocess (JS/TS config only)
├── stack run (hooks, tunnel session, concurrent commands)
├── process (std::process + threads; no Tokio unless later justified)
└── tunnel (independent; cloudflared / Cloudflare API — not embedded JS)
```

### Layout

| Path | Role |
| --- | --- |
| `Cargo.toml` | Single binary crate |
| `src/main.rs` | CLI entry |
| `src/lib.rs` | Config load, stack run, process, tunnel (Jiti bridge is crate-private) |
| `tests/` | Integration tests |
| `src/` on `main` | Historical Node implementation (not on this branch) |

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
| `reqwest` (blocking, rustls) | Cloudflare DNS API |
| `ctrlc` | SIGINT / graceful shutdown |
| `owo-colors` | Prefix colors |

External tools:

- **`cloudflared`** — locally-managed tunnel create/list/delete/route/run (required only when tunneling).
- **`node` + `jiti`** — JS/TS config only.

Not used:

- **c12** — forbidden as a dependency. Behavior reproduced only where Stackrun actually used it.
- **Any JS runtime in the binary** (no `deno_core`, `v8`, `boa` as a config engine).
- **Tokio** — not a direct dependency. `reqwest` blocking may pull it transitively. Process orchestration stays `std::process` + threads.

### JS/TS config bridge

When a discovered or explicit path is `.js` / `.mjs` / `.cjs` / `.ts` / `.mts` / `.cts`:

1. Detect JS/TS.
2. Require `node` on PATH (clear error if missing).
3. Run a small Node script that uses [Jiti](https://github.com/unjs/jiti) to load the file.
4. Resolve `default` export (or module).
5. Write JSON to stdout.
6. Rust deserializes into `StackrunConfig`.

Jiti is **not** a Rust crate. It is a Node helper used only for this path. The helper is not the old Node application.

### Hosting / runtime

- **Where it runs:** developer machines and CI as a local CLI. No server, no hosted control plane.
- **Tunnel:** user’s Cloudflare account via API token + `cloudflared`.
- **OS:** Unix first (process groups). Windows via `cmd /c` without process-group extras until a later pass.
- **CI:** GitHub Actions, `cargo fmt` / clippy / test.
- **Toolchain:** `rust-toolchain.toml` pins `stable`.

### Packaging (later)

- Native binaries (`x86_64` / `aarch64`, linux / macos / windows).
- Optional thin npm `bin` wrapper that execs the native binary (Phase 9). Not required for the Rust CLI itself.
