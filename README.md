# stackrun

Process-orchestration CLI. Run one or more arbitrary commands concurrently, with prefixed log output, lifecycle hooks, and optional Cloudflare tunneling.

Commands are opaque OS processes: Node, Python, Go, shell, or any executable. This branch is a standalone Rust binary. The historical Node implementation lives on `main`.

## Features

- One config file for a local stack of services
- Concurrent processes with prefixed, color-coded output
- Native config without Node: JSON, JSONC, JSON5, YAML, TOML, `.env`, and `.stackrc`
- Optional JS/TS config via Node + Jiti (only when you use those files)
- Lifecycle hooks (`beforeCommands` / `afterCommands`)
- Per-command `env`, `cwd`, and `tunnelEnv` overlays
- One-off runs with `--command` (no config file required)
- `--dry-run` prints the loaded config as JSON without starting processes or tunnels
- Named Cloudflare tunnels via `cloudflared` (token + `url`/`tunnelUrl` pairs)

Requires `cloudflared` login (`cert.pem`) and a Cloudflare API token when tunneling. Missing token or ingress exits before any processes start.

## Requirements

- A [Rust](https://rustup.rs/) stable toolchain (`rust-toolchain.toml` pins `stable`)
- Unix is the primary platform (process groups on SIGINT). Windows uses `cmd /c` without the extra process-group handling.
- Tunneling: [`cloudflared`](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/) on PATH, `cloudflared tunnel login` once, and a Cloudflare API token (`CF_TOKEN` / `CLOUDFLARE_TOKEN`).

Node.js is **not** required for YAML, TOML, JSON, JSONC, or JSON5. It is required only if the discovered config is JS or TS (`jiti` must be importable, typically from the project's `node_modules`).

## Installation

From a clone of this repo:

```sh
cargo install --path .
```

Or run without installing:

```sh
cargo run -- --help
```

This branch does not publish an npm package. Use the Rust binary.

## Quick start

```sh
# Discover stack.config.* in the current directory
stackrun

# Explicit native config (no Node)
stackrun --config ./stack.config.yaml

# One-off command (no config file)
stackrun --command "python server.py"
```

Minimal YAML:

```yaml
# stack.config.yaml
commands:
  - name: api
    command: python server.py
    cwd: ./api
  - name: web
    command: npm run dev
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
| `--command <SHELL>` | Run a single shell command. Does not require a config file. New in the Rust CLI. |
| `--json <JSON>` | JSON config overlay (highest data priority). Implemented in Rust. |
| `-t`, `--tunnel` | Force `tunnelEnabled` to true |
| `--dry-run` | Print effective config as JSON and exit. Does not start processes or tunnels. `cfToken` is redacted. |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Print crate version |

Examples:

```sh
stackrun
stackrun --config ./stack.config.yaml
stackrun custom.yaml
stackrun --command "echo hello"
stackrun --json '{"commands":[{"name":"hi","command":"echo hello"}]}'
stackrun --tunnel --config ./stack.config.ts
stackrun --dry-run
stackrun --dry-run --command "echo hello"
```

`--command` replaces the `commands` list. `--json` is a JSON **string**, not a file path. Either flag is enough to run without a config file on disk.

`--dry-run` uses the same load path as a real run (file discovery, `.stackrc`, `.env`, local `extends`, `NODE_ENV` overlays, then CLI flags), then prints:

```json
{
  "configFile": "/abs/path/to/stack.config.yaml",
  "config": { "commands": [], "tunnelEnabled": false }
}
```

`configFile` is `null` when no file was used (`--command` / `--json` only). Exit 0 on a successful load; exit 1 on load errors. Secrets in `cfTunnelConfig.cfToken` print as `"[redacted]"`. Env tokens (`CF_TOKEN`, `CLOUDFLARE_TOKEN`) are not dumped.

## Configuration

### Formats

Native (no Node): `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`.

JS/TS (needs `node` on `PATH` and `jiti`): `.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`. Prefer YAML or TOML unless you need a programmed config.

### Discovery

Default base name is `stack.config` (not `stackrun.config`). Lookup, first existing file wins, extensions in this order:

`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`

Search paths:

1. `{cwd}/{configFile}{ext}` — e.g. `stack.config.yaml`
2. `{cwd}/.config/{name-without-.config}{ext}` — e.g. `.config/stack.yaml`
3. `{cwd}/.config/{configFile}{ext}` — e.g. `.config/stack.config.yaml`

If both `stack.config.ts` and `stack.config.yaml` exist, the TypeScript file wins and requires Node. Pass `--config stack.config.yaml` to force the native file.

Also loaded from the current working directory (not the home directory):

- `.env` — interpolated (`${VAR}` / `$VAR`); does not override already-set env vars; keys starting with `_` are skipped. Native YAML/JSON/TOML config values are not interpolated; JS/TS configs can read `process.env` after this load.
- `.stackrc` — rc-style `KEY=VALUE` (dotted keys)

`extends` is supported for **local paths only**. `package.json` is not read as config.

### Precedence

Lowest to highest:

1. Built-in defaults
2. Configuration files (main file, local `extends`, CWD `.stackrc`)
3. Environment variables (`TUNNEL=true`, token/name fallbacks, `NODE_ENV` overlays)
4. CLI (`--json`, `--tunnel`, `--command`)

`NODE_ENV` selects `$<envName>` and `$env.<envName>` overlays inside a layer (for example `$development`).

## Configuration reference

Top-level keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `commands` | array | `[]` | Required to run, unless `--command` or `--json` supplies them |
| `beforeCommands` | string array | `[]` | Sequential hooks before services. Failure aborts the run. |
| `afterCommands` | string array | `[]` | Sequential hooks after services. Skipped if the process group fails. |
| `tunnelEnabled` | boolean | auto | `true` / `--tunnel` / `TUNNEL=true` enable. Omitted with `url`+`tunnelUrl` also enables (Node configs like bugpin never set the flag). Explicit `false` disables. |
| `cfTunnelConfig` | object | see below | Token, name, cleanup, tunnel process options |
| `concurrentlyOptions` | object | see below | Process-manager options |

### `commands`

Entries without a string `command` are ignored.

| Field | Type | Description |
| --- | --- | --- |
| `command` | string | Shell command to run (required) |
| `name` | string | Log prefix. Truncated to `prefixLength` (default 10) and printed as `[name]`; only that token is colored |
| `cwd` | string | Working directory |
| `env` | map | Environment variables (`string` or `boolean`) |
| `prefixColor` | string | Prefix color (`red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`) |
| `url` | string | Local URL; required together with `tunnelUrl` to create ingress |
| `tunnelUrl` | string | Public URL for the tunnel |
| `tunnelEnv` | map | Merged over `env` when tunneling is enabled |
| `ipc` | number | Accepted for compatibility |
| `raw` | boolean | Accepted for compatibility |

A command may also be a bare string: `commands: ["echo hello"]`.

### `beforeCommands` / `afterCommands`

String arrays. Each item is run sequentially in a shell with inherited stdio. `beforeCommands` failure aborts. `afterCommands` run only if the concurrent commands succeed.

### `concurrentlyOptions`

Defaults applied at run time:

```yaml
concurrentlyOptions:
  killOthers: failure
  handleInput: true
  prefixColors: auto
  prefixLength: 10
```

The process manager honors `killOthers` (kill the rest on failure), `prefixLength`, `handleInput` (stdin inherit vs null), and `prefixColors: auto` (cycle colors onto `[name]` when `prefixColor` is unset). Child logs look like `[nuxt] …` / `[tunnel] …` — color on the bracketed name only. Other concurrently-style keys (`maxProcesses`, `raw`, `restartTries`, and so on) are accepted in the file for compatibility and ignored.

### `cfTunnelConfig`

Ingress is generated from command `url` / `tunnelUrl` pairs. You do not set ingress yourself.

| Field | Default |
| --- | --- |
| `cfToken` | `CLOUDFLARE_TOKEN` (then `CF_TOKEN` at enable time) |
| `tunnelName` | `CLOUDFLARE_TUNNEL_NAME` or `"stackrun"` (`CF_TUNNEL_NAME` is also read) |
| `removeExistingTunnel` | `false` |
| `removeExistingDns` | `false` |
| `cloudflaredConfigDir` | unset (tooling default) |
| `commandOptions.name` | `"Tunnel"` |
| `commandOptions.prefixColor` | unset |

```yaml
cfTunnelConfig:
  cfToken: ${CF_TOKEN}
  tunnelName: my-project
  commandOptions:
    name: TUNNEL
    prefixColor: cyan
```

Token resolution when tunneling is enabled: `cfTunnelConfig.cfToken`, then `CF_TOKEN`, then `CLOUDFLARE_TOKEN`. Missing token or empty ingress aborts before `beforeCommands` with exit code 1. If a tunnel or DNS record already exists, set `removeExistingTunnel` / `removeExistingDns` to `true` (playground default) or the run errors.

Tunneling turns on when `tunnelEnabled` is true, `--tunnel` / `TUNNEL=true`, **or** `tunnelEnabled` is omitted and a command has both `url` and `tunnelUrl`. Set `tunnelEnabled: false` to run those commands without `cloudflared`.

## Environment variables

| Variable | Effect |
| --- | --- |
| `TUNNEL=true` | Same as `--tunnel` (exact string `"true"`, not `1` / `yes`) |
| `CLOUDFLARE_TOKEN` | Default / fallback tunnel token |
| `CF_TOKEN` | Fallback token if `cfTunnelConfig.cfToken` is unset |
| `CLOUDFLARE_TUNNEL_NAME` | Default tunnel name (`"stackrun"` if unset) |
| `CF_TUNNEL_NAME` | Alternate tunnel name |
| `NODE_ENV` | Selects `$development` / `$production` / `$test` (and `$env.<name>`) overlays |

There is no `STACKRUN_*` prefix in the CLI. `.env` in the working directory is loaded as described under [Configuration](#configuration).

## Examples

### Two services

```yaml
# stack.config.yaml
commands:
  - name: api
    command: python -m http.server 4000
    cwd: ./api
    prefixColor: green
  - name: web
    command: npm run dev
    cwd: ./web
    prefixColor: blue
```

```sh
stackrun --config ./stack.config.yaml
```

### Lifecycle hooks

```yaml
beforeCommands:
  - docker compose -f docker-compose.dev.yml up -d db
afterCommands:
  - docker compose -f docker-compose.dev.yml down db
commands:
  - name: api
    command: npm run dev
    cwd: ./api
```

### Tunnel config

Needs `cloudflared` on PATH, a prior `cloudflared tunnel login`, and an API token. Hostnames in `tunnelUrl` must be on a zone in that account.

```yaml
tunnelEnabled: true
cfTunnelConfig:
  cfToken: ${CF_TOKEN}
  tunnelName: my-project
  removeExistingTunnel: true
  removeExistingDns: true
commands:
  - name: api
    command: npm run dev
    cwd: ./api
    url: http://localhost:4000
    tunnelUrl: https://api.example.dev
    prefixColor: green
```

### JS/TS config

`stack.config.ts` (and `.js` / `.mjs` / `.cjs` / `.mts` / `.cts`) works if `node` is on `PATH` and `jiti` can be imported. Native formats never need Node.

```ts
export default {
  commands: [{ name: "api", command: "npm run dev", cwd: "./api" }],
};
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

Checked-in config fixtures live next to the integration tests: `tests/config_formats/` (one real file per format), `tests/cli_flags/`, and `tests/docker_stack/` (Compose stacks with `beforeCommands` up / `afterCommands` down; live tests skip without a Docker daemon). Native YAML/JSON/TOML values are not interpolated; `.env` `${VAR}` expansion applies to the env file itself, and JS/TS configs can read `process.env` after that load.

Historical Node sources (`src/cli.ts`, vitest, unbuild) are on `main` only.

## Project docs

| File | Owns |
| --- | --- |
| [SPEC.md](SPEC.md) | Product behavior (wins if it disagrees with STACK) |
| [STACK.md](STACK.md) | Tools and hosting |
| [DESIGN.md](DESIGN.md) | Terminal output / visual non-goals |
| [PLAN.md](PLAN.md) | Rust port phases and open questions |

## Status

Rust CLI on `refactor/rust`. Historical Node package is on `main`.

**Works today**

- Native config load (JSON / JSONC / JSON5 / YAML / TOML / `.env` / `.stackrc` / local `extends` / `NODE_ENV` overlays)
- CLI: `--config`, positional config, `--command`, `--json`, `--tunnel`, `--dry-run`, `--help`, `--version`
- Process manager: concurrent shell spawn, `[name]` prefixes (color on the bracket token only), `prefixColor` / `prefixColors: auto`, `handleInput`, `beforeCommands` / `afterCommands`, kill-others-on-failure, SIGINT cleanup
- Cloudflare named tunnel: `cloudflared` create/route/run + DNS API; abort if token or ingress missing; cleanup on exit
- JS/TS config via Node + Jiti when those files are used

**Not in this pass**

- Several concurrently options (`maxProcesses`, restart, `ipc`, `raw`, …) deserialize and are ignored
- Rainbow per-character tunnel name (uses cyan if `prefixColor` unset)
- Packaging: no GitHub-release binaries and no npm wrapper

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
2. Native config (YAML, TOML, JSON, and relatives) with no Node; JS/TS only when those files are used.
3. Match historical Node stackrun behavior except documented Rust differences (`--command`, implemented `--json`, native process manager).
4. Named Cloudflare tunnels (`cloudflared`, token, ingress from `url` / `tunnelUrl`).
5. Publish platform binaries later. An optional npm `bin` wrapper is not part of this branch.
