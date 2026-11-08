# Tasks: CI rust job can load jiti

## Task 1: Fail CI when repo cannot import jiti

**Description:** Add a `tests/jiti_bridge.rs` guard so GitHub Actions (`CI=true`) fails instead of silently skipping JS/TS tests when `node` cannot `import("jiti")` from the crate root. Local runs without `CI` keep the existing skip.

**Acceptance criteria:**
- [x] When `CI=true` and repo-root `import("jiti")` fails, the test panics with a message that names `jiti`, `npm ci`, and `npx -p jiti@2`
- [x] When `CI` is unset, the test does not fail a machine that lacks jiti

**Verification:**
- [x] `cargo test --test jiti_bridge ci_requires_repo_jiti -- --exact`
- [x] `cargo test`
- [x] `cargo build`

**Dependencies:** None

**Files likely touched:**
- `tests/jiti_bridge.rs`

**Estimated scope:** Small: 1-2 files

## Checkpoint: After Task 1

- [x] New test exists and suite still passes locally (this machine has node_modules/jiti)

## Task 2: Install Node + npm deps in the rust CI job

**Description:** Match product install: jiti lives in the project (`npm i -D jiti` / `npm ci`), and npx is on PATH for `--jiti npx`. Add `jiti` as a direct workspace devDependency. In the `rust` matrix job, run `actions/setup-node@v4` (Node 22, npm cache) and `npm ci` before `cargo test`. Do not set `STACKRUN_JITI=npx` on the job; tests mock npx and expect Local unless they opt in.

**Acceptance criteria:**
- [x] Root `package.json` lists `jiti` under `devDependencies`
- [x] `.github/workflows/ci.yml` rust job installs Node 22 and runs `npm ci` before `cargo test`
- [x] After `npm ci`, `node --input-type=module -e "await import('jiti')"` succeeds
- [x] Job does not export `STACKRUN_JITI=npx`

**Verification:**
- [x] `node --input-type=module -e "await import('jiti')"`
- [x] `cargo test`
- [x] `cargo build`
- [x] Workflow YAML review: rust job has setup-node + npm ci

**Dependencies:** Task 1

**Files likely touched:**
- `package.json`
- `package-lock.json`
- `.github/workflows/ci.yml`

**Estimated scope:** Small: 1-2 files (lockfile extra)

## Checkpoint: Complete

- [x] All acceptance criteria met
- [ ] Commits pushed to `seo/npm-keywords-and-docs`
- [ ] PR https://github.com/jasenmichael/stackrun/pull/9 updated
