# stackrun

Process-orchestration CLI. Run one or more arbitrary commands concurrently, with prefixed log output, lifecycle hooks, and optional Cloudflare tunneling.

Commands are opaque OS processes: Python, Go, shell, or any executable.

## Features

- One config file for a local stack of services
- Concurrent processes with prefixed, color-coded output
- Ctrl+C stops every command, then runs `after`
- Config: JSON, JSONC, JSON5, YAML, TOML, `.env`, and `.stackrc`
- Optional JS/TS config (needs `node` plus local `jiti`, or `--jiti npx`)
- Lifecycle hooks (`before` / `after`)
- Per-command `env`, `cwd`, and `tunnel.env` overlays
- One-off runs with `--command` (no config file required)
- `--dry-run` prints the loaded config as JSON without starting processes or tunnels
- Per-command Cloudflare tunnels via `cloudflared`: quick (`tunnel.local` only → `*.trycloudflare.com`) or named (`local` + `public` on your zone)

Requires `cloudflared` on PATH when any tunnel is on. Named tunnels also need `cloudflared tunnel login` (`cert.pem`). No API token.

## Requirements

- A [Rust](https://rustup.rs/) stable toolchain (`rust-toolchain.toml` pins `stable`)
- Unix is the primary platform (process groups on SIGINT). Windows uses `cmd /c` without the extra process-group handling.
- Tunneling: [`cloudflared`](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/) on PATH. Named tunnels: `cloudflared tunnel login` once (`cert.pem`). Quick tunnels need the binary only.

## Installation

From a clone of this repo:

```sh
cargo install --path .
```

Or run without installing:

```sh
cargo run -- --help
```

## Build

Needs a [Rust](https://rustup.rs/) stable toolchain (`rust-toolchain.toml` pins `stable`).

```sh
# debug binary: target/debug/stackrun
cargo build

# release binary: target/release/stackrun
cargo build --release

# run without installing
cargo run -- --help
cargo run -- --config ./stack.config.yaml
```

`cargo install --path .` puts `stackrun` on your PATH. Tests: `cargo test`.

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

`--command` replaces the `commands` list. `--json` is a JSON **string**, not a file path. Either flag is enough to run without a config file on disk.

`--dry-run` uses the same load path as a real run (file discovery, `.stackrc`, `.env`, local `extends`, `NODE_ENV` overlays, then CLI flags), then prints:

```json
{
  "configFile": "/abs/path/to/stack.config.yaml",
  "config": { "commands": [], "tunnel": false }
}
```

`configFile` is `null` when no file was used (`--command` / `--json` only). Exit 0 on a successful load; exit 1 on load errors. Process env is not dumped.

## Configuration

### Formats

`.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`.

JS/TS (`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`) need `node` on `PATH` and [jiti](https://github.com/unjs/jiti) in this project (`npm i -D jiti`), or `--jiti npx`. Prefer YAML or TOML unless you need a programmed config.

### Discovery

Default base name is `stack.config`. Lookup, first existing file wins, extensions in this order:

`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`

Search paths:

1. `{cwd}/{configFile}{ext}` — e.g. `stack.config.yaml`
2. `{cwd}/.config/{name-without-.config}{ext}` — e.g. `.config/stack.yaml`
3. `{cwd}/.config/{configFile}{ext}` — e.g. `.config/stack.config.yaml`

If both `stack.config.ts` and `stack.config.yaml` exist, the TypeScript file wins and needs `node` + jiti. Pass `--config stack.config.yaml` to force the YAML file.

Also loaded from the current working directory (not the home directory):

- `.env` — interpolated (`${VAR}` / `$VAR`); does not override already-set env vars; keys starting with `_` are skipped. YAML/JSON/TOML config values are not interpolated; JS/TS configs can read `process.env` after this load.
- `.stackrc` — rc-style `KEY=VALUE` (dotted keys)

`extends` is supported for **local paths only**. `package.json` is not read as config.

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
| `tunnel` | object | Optional per-command tunnel (see below) |

A command may also be a bare string: `commands: ["echo hello"]`.

### `before` / `after`

String arrays. Each item is run sequentially in a shell with inherited stdio. `before` failure aborts. `after` runs when every concurrent command exits 0, **or** after Ctrl+C has stopped every command. They are skipped if a command fails (`killOthers: failure` kills the rest first).

### `process`

Defaults applied at load (same value `--dry-run` prints):

```yaml
process:
  killOthers: failure
  handleInput: true
  colors: auto
  prefixLength: 10
```

Stackrun starts every `commands` entry at once (that is the point). It honors `killOthers` (kill the rest on failure), `prefixLength`, `handleInput` (stdin inherit vs null), and `colors: auto` (cycle colors onto `[name]` when `color` is unset). Child logs look like `[api] …` — color on the bracketed name only.

Ctrl+C stops every running command (process groups on Unix), then runs `after`.

### `tunnel` (per command)

A command with no `tunnel` key has no cloudflared sibling.

| Field | Description |
| --- | --- |
| `local` | Local origin (`http://127.0.0.1:4000`). |
| `public` | Public hostname. Set this for a named tunnel + `route dns`. Omit it for a quick tunnel. |
| `env` | Merged over `env` when tunneling is on. |
| `resource` | Cloudflare object name. Alias: `name`. Else stack `tunnel.resource`, else `command.name`. Unique among named tunnels. |
| `prefix` | Cloudflared log prefix. Else stack `tunnel.prefix`, else `Tunnel`. |
| `color` | Cloudflared prefix color. Else stack `tunnel.color`, else `cyan`. |
| `removeExisting` | Per-command override of stack `tunnel.removeExisting`. |

**Quick** (`local` only): `cloudflared tunnel --url <local>` → random `*.trycloudflare.com`. No login, no token, no create, no DNS.

**Named** (`local` + `public`): `cloudflared tunnel create` + `route dns` + `tunnel run --url <local> <resource>`. Needs `cert.pem` from `cloudflared tunnel login`. Stack `tunnel.removeExisting: true` runs `tunnel delete -f` if the name exists and `route dns --overwrite-dns`. Cleanup deletes the tunnel and local creds; it does **not** delete the CNAME (leftover host shows `1016`).

`--tunnel` / `TUNNEL=true` force tunnels on. `tunnel: false` at stack level runs the commands without cloudflared and without `tunnel.env`. `--tunnel` with zero `local` exits 1 before hooks. If tunneling is on and `cloudflared` is missing, exit `CloudflaredMissing` before hooks.

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
      resource: api-cf              # Cloudflare object; alias: name
      env: { PUBLIC_API: https://api.example.dev }
  - name: web
    run: python3 -m http.server 3000 --bind 127.0.0.1
    color: blue
    tunnel:
      local: http://127.0.0.1:3000   # quick: *.trycloudflare.com
```

### JS/TS config

Needs `node` on `PATH` and `jiti` importable from this project (`npm i -D jiti`). Global `npm i -g jiti` is not visible to `import`.

If local jiti is missing, either switch to YAML/TOML/JSON, install jiti in the project, or retry with `--jiti npx` (or `STACKRUN_JITI=npx`). That runs `npx -p jiti node ...` so jiti is on that process’s module path. First run may download jiti (needs network). Stackrun never runs `npm i` for you and never defaults to `npx`.

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
cargo run -- --help
cargo run -- --command 'echo hello'
cargo run -- --dry-run --command 'echo hello'
```

Checked-in config fixtures live next to the integration tests: `tests/config_formats/` (one real file per format), `tests/cli_flags/`, and `tests/docker_stack/` (Compose stacks with `before` up / `after` down; live tests skip without a Docker daemon). YAML/JSON/TOML values are not interpolated; `.env` `${VAR}` expansion applies to the env file itself, and JS/TS configs can read `process.env` after that load.

## Project docs

| File | Owns |
| --- | --- |
| [SPEC.md](SPEC.md) | Product behavior (wins if it disagrees with STACK) |
| [STACK.md](STACK.md) | Tools and hosting |
| [DESIGN.md](DESIGN.md) | Terminal output / visual non-goals |
| [PLAN.md](PLAN.md) | Engineering plan and open questions |

## Status

**Works today**

- Config load (JSON / JSONC / JSON5 / YAML / TOML / `.env` / `.stackrc` / local `extends` / `NODE_ENV` overlays)
- CLI: `--config`, positional config, `--command`, `--json`, `--tunnel`, `--dry-run`, `--jiti`, `--help`, `--version`
- Process manager: concurrent shell spawn of every command, `[name]` prefixes (color on the bracket token only), `color` / `colors: auto`, `handleInput`, `before` / `after`, kill-others-on-failure, Ctrl+C stops all commands then `after`
- Per-command tunnels: quick (`tunnel --url`) and/or named (`create` + `route dns` + `tunnel run --url`); prefix-log every cloudflared; no API token
- JS/TS config via `node` + local `jiti`, or `--jiti npx`

**Not in this pass**

- Wait-parse trycloudflare URLs into command env
- Packaging: no GitHub-release binaries

## Contributing

Issues and pull requests are welcome.

- Format with `rustfmt` (`cargo fmt`)
- Keep `cargo clippy --all-targets -- -D warnings` clean
- Add or update tests for behavior changes (`cargo test`)

## License

Published under the [MIT](./LICENSE) license.

Maintained by [@jasenmichael](https://github.com/jasenmichael).

## Goals

1. A standalone Rust CLI that runs mixed-language local stacks from one config file.
2. JSON, YAML, TOML, and relatives as the usual config; JS/TS only when those files are used (`node` + local `jiti`, or `--jiti npx`).
3. Per-command Cloudflare tunnels (`cloudflared`; quick and/or named; no API token).
4. Publish platform binaries later.
