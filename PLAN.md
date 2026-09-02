# PLAN.md — Rust refactor

Engineering plan for porting Stackrun from Node.js to a standalone Rust CLI.

**Branch:** `refactor/rust`  
**Crate location:** repo root (`Cargo.toml` + `src/`). Historical Node lives on `main` only.

## Phase status

| Phase | Name | Status |
| --- | --- | --- |
| 1 | Full repo analysis | Done (this file + SPEC.md) |
| 2 | Rust foundation (Cargo, types, CLI types, errors, logging, modules) | Done (`src/`) |
| 3 | Native configuration | Done (JSON/JSONC/JSON5/YAML/TOML/.env/.stackrc/extends/$env) |
| 4 | JS/TS Jiti bridge | Done — Node+Jiti subprocess; tests skip if `jiti` missing |
| 5 | Process manager | Done for used subset — spawn, prefixes, auto colors, stdin `handleInput`, before/after, kill-others, SIGINT |
| 6 | Tunnel manager | Done — cf-tunnel lifecycle via `cloudflared` + Cloudflare DNS API; abort if token/ingress missing |
| 7 | CLI compatibility | clap flags match Node plus `--command`; `--json` and `--dry-run` implemented |
| 8 | Tests | Native config, process lifecycle, mocked tunnel setup, optional JS/TS, CLI flags + config formats via `--dry-run` |
| 9 | Packaging | Out of scope this pass |

## Historical Node architecture (`main`)

```
dist/cli.mjs (citty)
    flags: --config/-c, --json (unused), --tunnel/-t, --help, -V
    (Rust adds --command, implements --json, adds --dry-run)
    env: TUNNEL=true
    c12.loadConfig({ name: "stack", configFile, cwd, dotenv: true })
    stack.config.{js,ts,...json,yaml,toml} | .config/stack.* | .stackrc
    jiti for JS/TS/JSON; confbox for yaml/jsonc/json5/toml; rc9 for RC
    StackrunConfig
    beforeCommands  —  child_process.execSync (sequential, stdio inherit)
    concurrently(commands [+ tunnel command])
    afterCommands   —  execSync (only if concurrently result fulfills)
```

Tunnel was an extra concurrently job: `node --no-warnings -e 'require("cf-tunnel").cfTunnel(...)'`. Missing token or empty ingress aborted the whole run before `beforeCommands`.

## Rust architecture

```
src/
├── cli          clap Args  →  CliOverrides
├── config       discover + parse + merge + env  →  StackrunConfig
│     native     json/jsonc/json5/yaml/toml/.env/.stackrc
│     bridge     Node+Jiti subprocess (JS/TS only)
├── stack        beforeCommands, tunnel session, concurrent commands, afterCommands, cleanup
├── process      spawn, prefix, signals, kill-others (children only)
└── tunnel       build ingress + run/stop tunnel (independent)
```

`StackrunConfig` is canonical and serde-based. Process and tunnel never import c12/jiti.

### Module map

| Module | Responsibility |
| --- | --- |
| `error` | `thiserror` error type |
| `config` | Public: `load_config`, types. Private: discover, parse, merge, dotenv, rc |
| `cli` | clap definition, version, aliases |
| `bridge` | Detect JS/TS; spawn Node+Jiti |
| `stack` | Stack run: hooks, tunnel session, concurrent commands, cleanup |
| `process` | Child spawn, prefix, SIGINT, kill-others |
| `tunnel` | Cloudflare tunnel resource (`TunnelSession`) + cloudflared / DNS |
| `logging` | tracing setup |

## Architecture deepening

Follow-on after the Rust port. Behavior stays the same unless a row says otherwise. Do in this order.

| # | Change | Status | Notes |
| --- | --- | --- | --- |
| 1 | Extract stack run from `process` | Done | `stack::run` / `stack::run_with_tunnel` own lifecycle. `process` only children. `main` and tests call `stack`. |
| 2 | Effective `StackrunConfig` at load | Done | `load_config` applies defaults once. `--dry-run` only redacts. `stack` does not call `apply_defaults`. |
| 3 | Domain names for Rust types | Done | `ProcessOptions` / `Command`. Field `process_options` serde-renames to `concurrentlyOptions`. Ctrl+C stops every command then runs `afterCommands` (failure still skips). |
| 4 | Split `TunnelSession` vs tunnel command | Done | Session = cloudflared resource (id, creds, ingress). Tunnel child = a `Command` from `commandOptions` in `stack`. |
| 5 | Hide config pipeline behind load | Done | `discover`/`parse`/`merge`/`dotenv`/`rc` are crate-private. External interface is `load_config`. RC beats `extends` when main omits a key (SPEC). No `ConfigParser` trait. |
| 6 | Process spawn seam | Done (fakes only) | No spawn trait (one adapter). `MockCloudflared` exported; `tests/tunnel_fakes.rs` calls `stack::run_with_tunnel`. |

#1 first: `src/stack.rs` is the product entry. `process` exposes `run_hook` + `run_concurrent`. Tunnel stays a two-adapter seam (`CloudflaredOps`, `DnsApi`). No new traits.

### Crate choices

See `STACK.md`. Short why:

- **clap** — standard CLI; matches flags/aliases.
- **serde family** — config is data, not code (except JS/TS bridge).
- **json5** — JSON5 + JSONC without pulling confbox.
- **serde_yaml / toml** — mature native formats.
- **custom `.env` parser** — interpolation reimplemented to match c12 (`${VAR}`, no override of real env).
- **thiserror + anyhow** — lib vs bin.
- **std::process, not Tokio** — current orchestration is spawn/wait. Revisit if async I/O on many pipes becomes a measured problem.
- **ctrlc / owo-colors** — signals and prefixes.

## Compatibility concerns (conservative choices)

### Needs Jasen confirmation

1. **`--command`** — name/alias (`-x`?); ignore a present config vs overlay as sole `commands` (today: overlay replace, rest of file kept).
2. **`--json`** — keep as highest-priority overlay?
7. **`package.json` `stack` key** — stay off unless you want it.
8. **Global `~/.stackrc`** — stay off unless you want it.
9. **Remote `extends`** — stay local-only unless you want it.
10. **Also auto-discover `stackrun.config.*`?** Conservative: no.
11. **Distribution** — cargo install only this pass.

### Decisions already taken (conservative)

- Do not load `package.json` config.
- Do not load home-directory RC.
- Do not implement giget/remote extends.
- Keep c12 extension order (JS/TS before YAML). If `stack.config.ts` and `stack.config.yaml` both exist, JS/TS wins and requires Node.
- Keep `TUNNEL=true` exact match (not truthy `1` / `yes`).
- Omitted `tunnelEnabled` + `url`/`tunnelUrl` enables the tunnel (explicit `false` still off).
- Filter commands without a string `command`.
- Truncate names to `prefixLength` (default 10).
- Honor explicit `handleInput: false` (Node quirk dropped).
- Missing token / empty ingress aborts with a non-zero error (no processes).
- Skip `afterCommands` on process-group failure. Ctrl+C stops every command, then `afterCommands` run.
- No rainbow tunnel name; default cyan.
- This branch has no Node application. Reference `main` for the old sources.

## Packaging plan (Phase 9, not this pass)

- Publish platform binaries.
- Optional later: npm optionalDependencies with platform packages and a `bin` shim that execs the native binary.
- JS/TS configs can keep using a tiny Jiti helper; that is not the old Node runner.

## Verification

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --help
cargo run -- --command 'echo hello'
cargo run -- --dry-run --command 'echo hello'
cargo run -- --tunnel --command 'echo x' --json '{"commands":[{"command":"echo x","url":"http://localhost:1","tunnelUrl":"https://x.example"}]}'
```
