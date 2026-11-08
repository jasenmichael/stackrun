# Implementation Plan: CI rust job can load jiti

## Overview

PR #9 failed `rust (macos-latest)` at `cargo test`. Product JS/TS loading uses the npm package **jiti** (not “Jitty” / “Jitti”). Local `import("jiti")` runs first; `--jiti npx` / `STACKRUN_JITI=npx` retries with `npx -p jiti@2`. The rust CI job never installs Node workspace deps, so repo `jiti` is missing, npx-backed tests skip, and CI does not exercise that path. Wire the rust job to the same Node/npm/npx environment the product expects.

## Architecture Decisions

- Real package name is **jiti** (`npx -p jiti@2`). Confirmed in `src/bridge/mod.rs`, `src/cli.rs`, `SPEC.md`. No package named jitty/jitti.
- Default mode stays **local**. Product does not auto-npx unless `--jiti npx` / `STACKRUN_JITI=npx`.
- Integration tests in `tests/jiti_bridge.rs` **mock npx** and skip unless repo-root `import("jiti")` works. CI must `npm ci` so that local module exists; do not set `STACKRUN_JITI=npx` on the job (would change tests that expect Local).
- `setup-node` puts `node` / `npx` on PATH so the real npx fallback can run when a test asks for it.
- Add `jiti` as a direct workspace `devDependency` so it is not only a transitive automd dep.

## Task List

### Phase 1: Proof and CI wiring

- [x] Task 1: Fail CI when repo cannot import jiti
- [x] Task 2: Install Node + npm deps (including jiti) in the rust CI job

### Checkpoint: After Tasks 1-2

- [x] `cargo test` passes locally
- [x] `cargo build` succeeds
- [x] rust job YAML has setup-node + npm ci before `cargo test`

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| macos `two_servers` flake (`api never served`) is independent of jiti | Med | This plan does not change that test. Re-check PR after push; only then harden wait_http if still red. |
| Transitive jiti disappears if automd drops it | Low | Direct `devDependency` on jiti. |

## Open Questions

- None. Product contract is in SPEC.md § JS/TS config (jiti).
