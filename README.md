# 🥞 stackrun 🏃

[![CI](https://github.com/jasenmichael/stackrun/actions/workflows/ci.yml/badge.svg)](https://github.com/jasenmichael/stackrun/actions/workflows/ci.yml)

<!-- automd:badges license -->
<!-- /automd -->

Process-orchestration CLI. Run one or more OS commands at the same time, with prefixed logs, lifecycle hooks, and optional Cloudflare tunnels.

Commands are opaque processes. Python, Node, Go, shell, or any executable works. Stackrun does not interpret your app language.

It is a standalone Rust binary. Node evaluates JS/TS config files and, in the npm package, shims `stackrun` / `import { stackrun }`.

Use it when a local stack is several processes you would otherwise start in separate terminals.

## Features

- One config file for a local stack
- Concurrent processes with `[name]` prefixes (color on the name only)
- Ctrl+C stops every command, then runs `after`
- JSON, JSONC, JSON5, YAML, TOML, `.env`, and `.stackrc`
- Optional JS/TS config (`node` plus local `jiti`, or `--jiti npx`)
- Lifecycle hooks (`before` / `after`)
- Per-command `env`, `cwd`, and `tunnel.env`
- One-off runs with `--command` (no config file)
- `--dry-run` prints the loaded config as JSON
- Per-command Cloudflare tunnels: quick (`*.trycloudflare.com`) or named (your zone)

## Requirements

Unix is the primary platform. On SIGINT, Stackrun stops each child process group. Windows uses `cmd /c` without that extra group handling.

Tunneling needs [`cloudflared`](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/) on PATH.

Named tunnels also need `cloudflared tunnel login` once (`cert.pem`). Quick tunnels need the binary only. No API token.

Node is needed only for the npm package (`npx`, `npm i`, or `import { stackrun }`) or for JS/TS config files. See [JS/TS config](#jsts-config). YAML, TOML, and JSON need no Node.

## Installation

### GitHub Releases

No Rust or Node. The install script detects OS/arch, downloads the matching archive, checks `SHA256SUMS`, and puts `stackrun` in `~/.local/bin`.

```sh
curl -fsSL https://jasenmichael.github.io/stackrun/install.sh | sh
```

Pin a version or install directory:

```sh
curl -fsSL https://jasenmichael.github.io/stackrun/install.sh | STACKRUN_VERSION=1.0.0 sh
curl -fsSL https://jasenmichael.github.io/stackrun/install.sh | STACKRUN_INSTALL=/usr/local/bin sh
```

The script and landing page live on GitHub Pages (`docs` branch): https://jasenmichael.github.io/stackrun/

Manual install: pick an archive from [GitHub Releases](https://github.com/jasenmichael/stackrun/releases). Names are `stackrun-v<version>-<target>.tar.gz` (Unix) or `.zip` (Windows).

Targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

Extract `stackrun` (or `stackrun.exe`) onto your `PATH`. Windows has no curl script; use the zip.

```sh
# Example: Linux x86_64 (gnu). Get the archive and SHA256SUMS from the release.
tar -xzf stackrun-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
sudo mv stackrun-v1.0.0-x86_64-unknown-linux-gnu/stackrun /usr/local/bin/
sha256sum -c SHA256SUMS
```

### crates.io

Needs `cargo` because cargo builds the crate.

```sh
cargo install stackrun
```

### npm

Needs Node. Use `npx` with no install, a global install, or a per-project install. See [Programmatic API](#programmatic-api) for `import { stackrun }`.

```sh
npx stackrun
npm i -g stackrun
npm i -D stackrun   # then ./node_modules/.bin/stackrun
```

<!-- automd:pm-install name="stackrun" -->
<!-- /automd -->

<!-- automd:pm-x name="stackrun" -->
<!-- /automd -->

### From source

A [Rust](https://rustup.rs/) stable toolchain (`rust-toolchain.toml` pins `stable`).

```sh
git clone https://github.com/jasenmichael/stackrun
cd stackrun
cargo install --path .
```

This puts `stackrun` on your PATH.

Or run from the repo without installing:

```sh
cargo run -- --help
```

A release build writes `target/release/stackrun`:

```sh
cargo build --release
```

## Quick start

```sh
# Discover stack.config.* in the current directory
stackrun

# Explicit YAML
stackrun --config ./stack.config.yaml

# One-off command (no config file)
stackrun --command "python server.py"
```

Minimal YAML:

```yaml
# stack.config.yaml
commands:
  - name: api
    run: python server.py
    cwd: ./api
  - name: web
    run: npm run dev
    cwd: ./web
```

Put that file in the project root and run `stackrun`.

## Programmatic API

The npm package exports `stackrun` and `defineStackrunConfig`. Both spawn the native binary. They do not run the stack in Node.

<!-- automd:jsimport name="stackrun" imports="stackrun,defineStackrunConfig" -->
<!-- /automd -->

```ts
import { stackrun, defineStackrunConfig } from "stackrun";

export default defineStackrunConfig({
  commands: [{ name: "api", run: "npm run dev", cwd: "./api" }],
});

await stackrun();
await stackrun({ commands: [{ name: "api", run: "npm run dev" }] });
```

## How it works

Stackrun loads one config from the current directory, then applies `.env`, `.stackrc`, local `extends`, and `NODE_ENV` overlays. CLI flags win last.

`--dry-run` uses that same load path and prints the result as JSON. It does not start processes, hooks, or tunnels.

If tunneling is on, Stackrun checks for `cloudflared` (and `cert.pem` for named tunnels) before any hook runs.

`before` hooks run one at a time with inherited stdio. A failed hook aborts the stack.

Every command then starts at once. Each child, including each `cloudflared`, prints as `[name] line`. Color applies to the name token only.

If one command fails, the rest are killed (`killOthers: failure`). Ctrl+C stops every process group on Unix.

`after` hooks run when every command exits 0, or after Ctrl+C. They are skipped if a command failed.

Named tunnels are deleted on exit. DNS CNAMEs are left in place.

## Usage

```text
stackrun [OPTIONS] [CONFIG]
```

| Flag | Meaning |
| --- | --- |
| `-c`, `--config <PATH>` | Config path. Extension may be omitted. Default: `stack.config` |
| `[CONFIG]` | Positional config path, used when `-c` / `--config` is absent |
| `--command <SHELL>` | Run a single shell command. Does not require a config file. |
| `--json <JSON>` | JSON config overlay (highest data priority). |
| `-t`, `--tunnel` | Force tunnels on. Exit 1 before hooks if no command has `tunnel.local`. |
| `--dry-run` | Print effective config as JSON and exit. Does not start processes or tunnels. |
| `--jiti <local\|npx>` | JS/TS configs only. Default `local`. See [JS/TS config](#jsts-config). |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Print crate version |

Examples:

```sh
stackrun
stackrun --config ./stack.config.yaml
stackrun custom.yaml
stackrun --command "echo hello"
stackrun --json '{"commands":[{"name":"hi","run":"echo hello"}]}'
stackrun --tunnel --config ./stack.config.yaml
stackrun --dry-run
stackrun --dry-run --command "echo hello"
```

`--command` replaces the `commands` list. `--json` is a JSON string, not a file path. Either flag is enough to run without a config file.

`--dry-run` prints:

```json
{
  "configFile": "/abs/path/to/stack.config.yaml",
  "config": { "commands": [], "tunnel": false }
}
```

`configFile` is `null` when no file was used. Exit 0 on a successful load; exit 1 on load errors. Process env is not dumped.

## Configuration

### Formats

`.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`.

JS/TS (`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`) need `node` on PATH and [jiti](https://github.com/unjs/jiti) in this project (`npm i -D jiti`), or `--jiti npx`.

Prefer YAML or TOML unless you need a programmed config.

### Discovery

Default base name is `stack.config`. The first existing file wins. Extensions are tried in this order:

`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`

Search paths:

1. `{cwd}/{configFile}{ext}` — e.g. `stack.config.yaml`
2. `{cwd}/.config/{name-without-.config}{ext}` — e.g. `.config/stack.yaml`
3. `{cwd}/.config/{configFile}{ext}` — e.g. `.config/stack.config.yaml`

If both `stack.config.ts` and `stack.config.yaml` exist, TypeScript wins and needs `node` plus jiti. Pass `--config stack.config.yaml` to force YAML.

Also loaded from the current working directory (not the home directory):

- `.env` — interpolated (`${VAR}` / `$VAR`); does not override already-set env vars; keys starting with `_` are skipped.
- `.stackrc` — rc-style `KEY=VALUE` (dotted keys)

YAML, JSON, and TOML values are not interpolated. JS/TS configs can read `process.env` after `.env` loads.

`extends` is supported for local paths only. `package.json` is not read as config.

### Precedence

Lowest to highest:

1. Built-in defaults
2. Configuration files (main file, local `extends`, CWD `.stackrc`)
3. Environment variables (`TUNNEL=true`, `NODE_ENV` overlays, `STACKRUN_JITI`)
4. CLI (`--json`, `--tunnel`, `--command`, `--jiti`)

`NODE_ENV` selects `$<envName>` and `$env.<envName>` overlays inside a layer (for example `$development`).

## Configuration reference

Top-level keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `commands` | array | `[]` | Required to run, unless `--command` or `--json` supplies them |
| `before` | string array | `[]` | Sequential hooks before services. Failure aborts the run. |
| `after` | string array | `[]` | Sequential hooks after all commands stop. Skipped if a command fails. Still run after Ctrl+C. |
| `process` | object | see below | Process-manager options. |
| `tunnel` | `false` or object | auto | `false` disables. Object is named-tunnel defaults (`removeExisting`). Omitted + any `tunnel.local` enables. |

### `commands`

Entries without a string `run` are ignored.

| Field | Type | Description |
| --- | --- | --- |
| `run` | string | Shell command to run (required). |
| `name` | string | Log prefix. Truncated to `prefixLength` (default 10) and printed as `[name]`; only that token is colored |
| `cwd` | string | Working directory |
| `env` | map | Environment variables (`string` or `boolean`) |
| `color` | string | Prefix color (`red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`) |
| `tunnel` | object | Optional per-command tunnel (see [Tunnels](#tunnels)) |

A command may also be a bare string: `commands: ["echo hello"]`.

### `before` / `after`

String arrays. Each item runs sequentially in a shell with inherited stdio.

`before` failure aborts. `after` runs when every concurrent command exits 0, or after Ctrl+C has stopped every command.

They are skipped if a command fails (`killOthers: failure` kills the rest first).

### `process`

Defaults applied at load (same values `--dry-run` prints):

```yaml
process:
  killOthers: failure
  handleInput: true
  colors: auto
  prefixLength: 10
```

Stackrun starts every `commands` entry at once. It honors `killOthers`, `prefixLength`, `handleInput` (stdin inherit vs null), and `colors: auto`.

Child logs look like `[api] …`. Color is on the bracketed name only.

## Tunnels

A command with no `tunnel` key has no `cloudflared` sibling.

| Field | Description |
| --- | --- |
| `local` | Local origin (`http://127.0.0.1:4000`). |
| `public` | Public hostname. Set this for a named tunnel + `route dns`. Omit it for a quick tunnel. |
| `env` | Merged over `env` when tunneling is on. |
| `resource` | Cloudflare object name. Else stack `tunnel.resource`, else `command.name`. Unique among named tunnels. |
| `prefix` | Cloudflared log prefix. Else stack `tunnel.prefix`, else `Tunnel`. |
| `color` | Cloudflared prefix color. Else stack `tunnel.color`, else `cyan`. |
| `removeExisting` | Per-command override of stack `tunnel.removeExisting`. |

**Quick** (`local` only): `cloudflared tunnel --url <local>` opens a random `*.trycloudflare.com` host. No login, no token, no DNS.

**Named** (`local` + `public`): `cloudflared tunnel create` + `route dns` + `tunnel run --url <local> <resource>`.

Named tunnels need `cert.pem` from `cloudflared tunnel login`. Stack `tunnel.removeExisting: true` deletes an existing name and overwrites DNS.

Cleanup deletes the tunnel and local creds. It does not delete the CNAME. A leftover host shows Cloudflare `1016`.

`--tunnel` / `TUNNEL=true` force tunnels on. `tunnel: false` at stack level runs commands without `cloudflared` and without `tunnel.env`.

`--tunnel` with zero `local` exits 1 before hooks. If tunneling is on and `cloudflared` is missing, Stackrun exits before hooks.

## Environment variables

| Variable | Effect |
| --- | --- |
| `TUNNEL=true` | Same as `--tunnel` (exact string `"true"`, not `1` / `yes`) |
| `NODE_ENV` | Selects `$development` / `$production` / `$test` (and `$env.<name>`) overlays |
| `STACKRUN_JITI` | Same as `--jiti` (`local` or `npx`). JS/TS configs only. |

`.env` in the working directory is loaded as described under [Configuration](#configuration).

## Examples

### Two services

```yaml
# stack.config.yaml
commands:
  - name: api
    run: python -m http.server 4000
    cwd: ./api
    color: green
  - name: web
    run: npm run dev
    cwd: ./web
    color: blue
```

```sh
stackrun --config ./stack.config.yaml
```

### Lifecycle hooks

```yaml
before:
  - docker compose -f docker-compose.dev.yml up -d db
after:
  - docker compose -f docker-compose.dev.yml down db
commands:
  - name: api
    run: npm run dev
    cwd: ./api
```

### Tunnel config

Needs `cloudflared` on PATH. Named hostnames must be on a zone in the account you selected with `cloudflared tunnel login`.

```yaml
tunnel:
  removeExisting: true
  prefix: Tunnel   # [Tunnel] on cloudflared lines
  color: cyan
commands:
  - name: api
    run: python3 -m http.server 4000 --bind 127.0.0.1
    color: green
    tunnel:
      local: http://127.0.0.1:4000
      public: https://api.example.dev
      resource: api-cf              # Cloudflare object name
      env: { PUBLIC_API: https://api.example.dev }
  - name: web
    run: python3 -m http.server 3000 --bind 127.0.0.1
    color: blue
    tunnel:
      local: http://127.0.0.1:3000   # quick: *.trycloudflare.com
```

### JS/TS config

Needs `node` on PATH and `jiti` importable from this project (`npm i -D jiti`). A global `npm i -g jiti` is not visible to `import`.

If local jiti is missing, switch to YAML/TOML/JSON, install jiti in the project, or retry with `--jiti npx`.

That runs `npx -p jiti node ...` so jiti is on that process's module path. First run may download jiti (needs network).

Stackrun never runs `npm i` for you and never defaults to `npx`.

```ts
export default {
  commands: [{ name: "api", run: "npm run dev", cwd: "./api" }],
};
```

```sh
stackrun --config ./stack.config.ts
stackrun --config ./stack.config.ts --jiti npx
```

## Development

Requires a Rust stable toolchain.

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Behavior: [SPEC.md](SPEC.md). Tools: [STACK.md](STACK.md). Output: [DESIGN.md](DESIGN.md). Plan: [PLAN.md](PLAN.md).

Checked-in fixtures live next to the integration tests: `tests/config_formats/`, `tests/cli_flags/`, and `tests/docker_stack/`.

Docker Compose fixtures skip without a Docker daemon.

## Contributing

Issues and pull requests are welcome.

- Format with `rustfmt` (`cargo fmt`)
- Keep `cargo clippy --all-targets -- -D warnings` clean
- Add or update tests for behavior changes (`cargo test`)

## License

Published under the [MIT](./LICENSE) license.

Maintained by [@jasenmichael](https://github.com/jasenmichael).

<!-- automd:contributors -->
<!-- /automd -->

<!-- automd:with automd -->
<!-- /automd -->
