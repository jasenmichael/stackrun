# PLAN.md — Stackrun engineering

**Crate location:** repo root (`Cargo.toml` + `src/`). npm shim: `npm`.

## Phase status

| Phase | Name | Status |
| --- | --- | --- |
| 1 | Full repo analysis | Done (this file + SPEC.md) |
| 2 | Rust foundation (Cargo, types, CLI types, errors, logging, modules) | Done (`src/`) |
| 3 | Native configuration | Done (JSON/JSONC/JSON5/YAML/TOML/.env/.stackrc/extends/$env) |
| 4 | JS/TS Jiti bridge | Done — local `import("jiti")` first; `--jiti npx` / `STACKRUN_JITI=npx` escape hatch; tests skip if `jiti` missing |
| 5 | Process manager | Done — spawn, prefixes, auto colors, stdin `handleInput`, before/after, kill-others, SIGINT |
| 6 | Tunnel manager | Done — per-command quick or named `cloudflared` siblings; no API token / REST DNS |
| 7 | CLI | clap: `--config`, `--json`, `--tunnel`, `--command`, `--dry-run`, `--jiti` |
| 8 | Tests | Native config, process lifecycle, mocked tunnel setup, optional JS/TS, CLI flags + config formats via `--dry-run` |
| 9 | Packaging | Done — one npm `stackrun` (download GH binary); native six-target release; Docusaurus Pages |

## Architecture

```
src/
├── cli          clap Args
├── config       discover + parse + merge + env  →  StackrunConfig
│     native     json/jsonc/json5/yaml/toml/.env/.stackrc
│     bridge     Node+Jiti subprocess (JS/TS only)
├── stack        before, per-command tunnel siblings, concurrent commands, after, cleanup
├── process      spawn, prefix, signals, kill-others (children only)
└── tunnel       named create/route/run + quick `--url` (cloudflared CLI only)
npm              bin shim + stackrun() / defineStackrunConfig (spawn native binary)
```

`StackrunConfig` is canonical and serde-based. Only SPEC keys deserialize. Process and tunnel never import c12/jiti.

### Module map

| Module | Responsibility |
| --- | --- |
| `error` | `thiserror` error type |
| `config` | Public: `load_config`, types. Private: discover, parse, merge, dotenv, rc |
| `cli` | clap definition, version |
| `bridge` | Detect JS/TS; spawn Node+Jiti |
| `stack` | Stack run: hooks, per-command tunnel siblings, concurrent commands, cleanup |
| `process` | Child spawn, prefix, SIGINT, kill-others |
| `tunnel` | Named `TunnelSession` + quick/named run lines. `CloudflaredOps` only (no REST DNS) |
| `logging` | tracing setup |

## Architecture deepening

| # | Change | Status | Notes |
| --- | --- | --- | --- |
| 1 | Extract stack run from `process` | Done | `stack::run` / `stack::run_with_tunnel` own lifecycle. `process` only children. |
| 2 | Effective `StackrunConfig` at load | Done | `load_config` applies defaults once. `--dry-run` only redacts. |
| 3 | Domain names | Done | File keys: `process`, `run`, `color`, `before` / `after`. No legacy aliases. |
| 4 | Split `TunnelSession` vs tunnel command | Done | One named session per command with `public`. Each cloudflared is a prefixed sibling. |
| 5 | Hide config pipeline behind load | Done | External interface is `load_config`. RC beats `extends` when main omits a key. |
| 6 | Process spawn seam | Done (fakes only) | `MockCloudflared` exported; tests call `stack::run_with_tunnel`. |
| 7 | Per-command tunnels | Done | `tunnel.local` only → quick. `local` + `public` → named. Mix in one stack. |
| 8 | No token / REST DNS | Done | `cloudflared` CLI only. Cleanup does not delete the CNAME. |
| 9 | SPEC-only config keys | Done | No serde aliases for dropped keys. |
| 10 | Namable tunnel sibling | Done | Sibling log `[tunnel-<prefix>]` / `cyan`. CF object is `resource`. |

`src/stack.rs` is the product entry. `process` exposes `run_hook` + `run_concurrent`. Tunnel is a one-adapter seam (`CloudflaredOps`).

### Crate choices

See `Cargo.toml`. Short why:

- **clap** — standard CLI
- **serde family** — config is data, not code (except JS/TS bridge)
- **json5** — JSON5 + JSONC
- **serde_yaml / toml** — native formats
- **custom `.env` parser** — `${VAR}` interpolation, no override of real env
- **thiserror + anyhow** — lib vs bin
- **std::process, not Tokio** — spawn/wait
- **ctrlc / owo-colors** — signals and prefixes

## Decisions

- Do not load `package.json` config.
- Do not load home-directory RC.
- Do not implement remote `extends`.
- Extension order: JS/TS before YAML.
- JS/TS jiti: local `import` only by default. `--jiti npx` / `STACKRUN_JITI=npx` retries via `npx -p jiti`.
- `TUNNEL=true` exact match (not `1` / `yes`).
- Any command `tunnel.local` enables tunnels. Top-level `tunnel` is ignored. Omit command `tunnel` to skip that sibling.
- Filter commands without a string `run`.
- Truncate names to `prefixLength` (default 10).
- Honor explicit `handleInput: false`.
- `--tunnel` with no `tunnel.local` aborts before hooks.
- `after` always runs once commands have exited. Ctrl+C stops every command, then `after` runs, then exit 0 unless a command already failed.
- Prefix-log every child including each cloudflared.

## Packaging (Phase 9)

- Native binaries on merge to `main` (six targets; no musl, no `cross`).
- One npm package `stackrun`: `bin` + `import { stackrun }`. Install downloads the matching GitHub Release binary. No `@jasenmichael/stackrun-*` packages.
- Publish from `release.yml` when the crate version is newer than the last tag. No laptop `npm publish` / `cargo publish`. npm uses trusted publishers (OIDC on the `ship` job). crates.io uses `CARGO_REGISTRY_TOKEN`.
- Dynamic docs: automd + changelogen in the release PR (no tag from the laptop).
- GitHub Pages: Docusaurus `website/` publishes to `gh-pages` on every `main` push and on each release. Curl install: `https://jasenmichael.github.io/stackrun/install.sh`.
- Post-1.0 ideas: [ROADMAP.md](ROADMAP.md).

## Verification

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --help
cargo run -- --command 'echo hello'
cargo run -- --dry-run --command 'echo hello'
cargo run -- --tunnel --command 'echo x' --json '{"commands":[{"run":"echo x","tunnel":{"local":"http://localhost:1","public":"https://x.example"}}]}'
```
