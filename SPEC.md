# SPEC.md — Stackrun product behavior

This file is the product-behavior contract. If it disagrees with `STACK.md`, this file wins on behavior.

Historical Node implementation lives on `main` (`src/cli.ts`, `src/index.ts`, `test/`). This branch is Rust-only: `src/` is the CLI. Match Node behavior unless a difference is listed under [Intentional Rust differences](#intentional-rust-differences) or [Open questions](#open-questions).

## What Stackrun is

Stackrun is a process-orchestration CLI. It runs one or more arbitrary OS commands (Node, Python, Go, shell, or any executable) concurrently, with optional Cloudflare tunneling, lifecycle hooks, and prefixed log output.

It is not tied to a single application language. Commands are opaque argv/shell strings executed as child processes.

## Entry points

| Surface | Path | Role |
| --- | --- | --- |
| CLI binary | `src/main.rs` | Parse CLI, load config, run process manager |
| Library | `src/lib.rs` | Config, process, tunnel modules |

Historical Node CLI (citty) is on `main` only.

## CLI

### Flags (historical Node on `main`; Rust recreates these)

| Flag | Type | Default | Meaning |
| --- | --- | --- | --- |
| `-c`, `--config` | string | `stack.config` | Config path. May omit extension. Passed to c12 as `configFile`. |
| positional `[config]` | string | — | `args._[0]` used if `-c`/`--config` missing (citty default still supplies `stack.config`) |
| `--json` | string | — | Documented as “input config as JSON”. **Not read by `run()` today.** Dead flag. |
| `-t`, `--tunnel` | boolean | false | Sets `config.tunnelEnabled = true` |
| `--dry-run` | boolean | false | Load config (same path as a real run) and print effective options as JSON. Does not spawn processes, tunnels, `beforeCommands`, or `afterCommands`. **Rust-only.** |
| `-h`, `--help` | boolean | — | Show help |
| `-V` / `--version` | boolean | — | Print crate version (`CARGO_PKG_VERSION`; Node used `package.json`) |

### Environment variables that affect CLI / runtime

| Variable | Effect |
| --- | --- |
| `TUNNEL=true` | Same as `--tunnel` (exact string `"true"`) |
| `CLOUDFLARE_TOKEN` | Default `cfTunnelConfig.cfToken`; also fallback at tunnel-enable time |
| `CF_TOKEN` | Fallback token if `cfTunnelConfig.cfToken` is unset |
| `CLOUDFLARE_TUNNEL_NAME` | Default tunnel name (`"stackrun"` if unset) |
| `CF_TUNNEL_NAME` | Fallback tunnel name after config and `CLOUDFLARE_TUNNEL_NAME` |
| `NODE_ENV` | c12 `envName` for `$development` / `$production` / `$test` / `$env` overlays |
| `.env` (via c12 `dotenv: true`) | Loaded into `process.env` before config import; `${VAR}` / `$VAR` interpolation in the env file only; does not override already-set env vars; keys starting with `_` skipped. Native YAML/JSON/TOML values are not interpolated. JS/TS configs can read `process.env`. |

No `STACKRUN_*` prefix exists in the Node CLI.

### CLI flow (Node)

1. If `--version` or `-V`: print version, exit 0.
2. Resolve `configPath = args.c \|\| args.config \|\| args._[0]` (default `"stack.config"`).
3. `c12.loadConfig({ name: "stack", configFile: configPath, cwd: process.cwd(), dotenv: true })`.
4. If `--tunnel` or `process.env.TUNNEL === "true"`: `config.tunnelEnabled = true`.
5. If `config` is missing: error, exit 1.
6. **Rust:** If `--dry-run`: print a pretty JSON envelope `{ configFile, config }` (effective `StackrunConfig` after overlays and run-time defaults; `cfTunnelConfig.cfToken` redacted to `"[redacted]"` when present). Exit 0. Do not spawn children or set up tunnels. Missing-config and parse errors still exit 1.
7. If `existsSync(configFile)`: log path, `await stackrun(config)`, exit 0.
8. Else: error `No valid configuration found at ${configPath}`, exit 1.

Node **requires a resolved config file on disk**. There is no `--command` flag in the Node CLI.

## Configuration

### c12 usage (exact)

```ts
loadConfig({
  name: "stack",
  configFile: typeof configPath === "string" ? configPath : undefined,
  cwd: process.cwd(),
  dotenv: true,
});
```

Not passed (c12 defaults therefore apply):

| Option | Default used | Stackrun consequence |
| --- | --- | --- |
| `rcFile` | `.stackrc` | CWD `.stackrc` is loaded (rc9 KEY=VALUE, unflattened) |
| `globalRc` | unset / falsy | Home/workspace RC is **not** loaded |
| `packageJson` | unset / falsy | `package.json` `stack` key is **not** loaded |
| `extend` | `{ extendKey: "extends" }` | Local `extends` is enabled |
| `envName` | `process.env.NODE_ENV` | `$<envName>` and `$env.<envName>` overlays apply |
| `defaults` / `overrides` | none | None |

### Discovery (c12 `resolveConfig`)

Base name from CLI: `stack.config` (or an explicit path).

Lookup, first hit wins, extensions in this order:

`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`

Search paths:

1. `{cwd}/{configFile}{ext}` e.g. `stack.config.yaml`
2. `{cwd}/.config/{name-without-.config}{ext}` e.g. `.config/stack.yaml`
3. `{cwd}/.config/{configFile}{ext}` e.g. `.config/stack.config.yaml`

JS/TS/JSON are loaded with jiti. yaml/yml/jsonc/json5/toml are loaded with confbox.

### Merge order (c12 + defu)

Higher priority first:

1. Config file (main)
2. CWD RC (`.stackrc`)
3. Extended layers (`extends`) as fallbacks
4. Environment-specific keys mixed into each layer (`$<NODE_ENV>`, `$env[NODE_ENV]`)

defu semantics: objects deep-merge; arrays concatenate and de-duplicate; primitives from the higher-priority object win.

### Canonical config shape (`StackrunConfig`)

```ts
{
  concurrentlyOptions?: ConcurrentlyOptions; // concurrently API
  tunnelEnabled?: boolean;                   // default false
  cfTunnelConfig?: { ... };                  // see tunnel section
  beforeCommands?: string[];                 // default []
  afterCommands?: string[];                  // default []
  commands?: StackrunConfigCommands[];       // default []
}
```

`defineStackrunConfig` is identity (types only).

### Command entries

Each item is concurrently `Command` plus:

| Field | Type | Notes |
| --- | --- | --- |
| `command` | string | Required to run. Entries without a string `command` are **filtered out**. |
| `name` | string | Log prefix. Truncated to `concurrentlyOptions.prefixLength` (default 10), then printed as `[name]` (color on that token only). |
| `cwd` | string | Per-command working directory |
| `env` | `Record<string, string \| boolean \| undefined>` | Base env |
| `prefixColor` | string | concurrently/chalk color |
| `ipc` | number | concurrently IPC fd (>2) |
| `raw` | boolean | Raw output |
| `url` | string | Local URL; required with `tunnelUrl` to create ingress |
| `tunnelUrl` | string | Public URL; required with `url` |
| `tunnelEnv` | same as `env` | Merged over `env` when `tunnelEnabled` |

When tunneling: `env = { ...env, ...tunnelEnv }`. When not: `env` only.

### Concurrently defaults applied in `stackrun()`

Using `||`, so falsy user values are replaced:

| Option | Default | Quirk |
| --- | --- | --- |
| `killOthers` | `"failure"` | Cannot set a falsy value |
| `handleInput` | `true` | `false` is treated as unset and becomes `true` |
| `prefixColors` | `"auto"` | |
| `prefixLength` | `10` | Names sliced to this length |

Other concurrently options (`cwd`, `maxProcesses`, `raw`, `restartTries`, `restartDelay`, `successCondition`, `timings`, `prefix`, `group`, `hide`, `killSignal`, `teardown`, …) are passed through.

## Process lifecycle (Rust)

1. If `tunnelEnabled` (explicit true, `--tunnel` / `TUNNEL=true`, or omitted when commands have `url`+`tunnelUrl`): validate token + ingress. Explicit `tunnelEnabled: false` skips the tunnel. Missing token or no `url`/`tunnelUrl` pairs **aborts with a non-zero error** (Node logged and returned without an exit code).
2. Log “Tunneling is enabled/disabled”.
3. `beforeCommands`: sequential shell, stdio inherit. Failure aborts.
4. If tunneling: create named tunnel (`cloudflared tunnel create`), route DNS, write `config.yml`, then spawn `cloudflared tunnel run` as a sibling process with prefix `[Tunnel]` (or `commandOptions.name`, e.g. bugpin `[tunnel]`).
5. Spawn user commands concurrently (`sh -c` / `cmd /c`). `tunnelEnv` overlays `env` when tunneling is on. `handleInput: true` inherits stdin; `false` uses null stdin. Child stdout/stderr lines are prefixed `[name]<space>` (name already sliced to `prefixLength`). `prefixColor` / `prefixColors: auto` color only `[name]`; the line body is uncolored. `prefixColors: false` prints `[name]` with no ANSI. Stderr lines still go to stderr.
6. `afterCommands`: sequential shell. **Skipped if any concurrent command failed** (no `finally`).
7. If a tunnel session was started: delete tunnel, DNS, local creds/`config.yml` (cf-tunnel cleanup), even on failure.
8. Log “Stackrun completed”.

Unix: each child is a process group; SIGINT kills groups. `maxProcesses`, restart, `ipc`, `raw`, and other concurrently keys are accepted in config and **not wired**.

## Cloudflare tunnel (Rust)

Matches [cf-tunnel 0.1.9](https://github.com/jasenmichael/cf-tunnel): locally-managed named tunnel, not a Quick Tunnel.

1. Require `cloudflared` on PATH (clear error if missing).
2. Require `cert.pem` in the cloudflared config dir. Do **not** run interactive `cloudflared tunnel login`; tell the user to run it once.
3. Token: `cfTunnelConfig.cfToken` or `CF_TOKEN` or `CLOUDFLARE_TOKEN`.
4. Ingress from commands with both `url` and `tunnelUrl` (strip `http://` / `https://` from hostname).
5. If `removeExistingTunnel` / `removeExistingDns` is false and a same-name tunnel or DNS record exists: error (same messages as cf-tunnel).
6. `cloudflared tunnel create` / `route dns`, Cloudflare HTTP API for DNS lookup/delete, spawn `cloudflared tunnel run`.
7. Cleanup on exit: delete tunnel + DNS + `config.yml` + credential JSON.

Rainbow per-character tunnel name is **not** implemented. Unset `commandOptions.prefixColor` uses cyan.

Missing token or empty ingress: **do not run `beforeCommands` or processes**; exit 1.

## Precedence for the Rust port

`defaults → configuration files (including RC + extends) → environment variables → CLI arguments`

CLI is highest. This matches how Node applies `--tunnel` / `TUNNEL` after c12, and is the rule for new overrides (`--command`, implemented `--json`).

## Intentional Rust differences

These are required by the refactor goals, not Node bugs:

1. **Rust is the application.** Node is optional and used only to evaluate JS/TS config via a Jiti subprocess. No V8/Deno/Node is embedded in the binary.
2. **Native formats work without Node.** JSON, JSONC, JSON5, YAML, TOML, `.env`, and `.stackrc` load in Rust.
3. **`--command <shell>`** is a new CLI override: build a one-entry `commands` list and do not require a config file. Needed so `stackrun --command "python server.py"` works without Node.
4. **`--json` is implemented** as a CLI overlay (Node declares it but never reads it).
5. **Process manager is native**, not the `concurrently` npm package. Wired: concurrent spawn, names, `[name]` prefixes (color on the bracket token only, including `prefixColors: auto`), `handleInput`, env/`tunnelEnv`, cwd, before/after, kill-others-on-failure, SIGINT process-group cleanup. Other concurrently keys deserialize and are ignored.
6. **Tunnel manager is independent.** Uses `cloudflared` CLI + Cloudflare DNS HTTP API. Does not embed Node or call `cf-tunnel`. Abort on missing token/ingress is a non-zero error instead of Node’s silent `return`.
7. **This branch is Rust-only.** Node application sources are not present; refer to `main` for the old implementation.
8. **Explicit `handleInput: false` is honored.** Node’s `value || true` treated `false` as unset.
9. **No rainbow tunnel prefix.** Default tunnel prefix color is cyan.
10. **`--dry-run`** prints the loaded/effective config as JSON and exits without running processes or tunnels. `cfToken` in that JSON is redacted. Process env secrets are not dumped.
11. **Omitted `tunnelEnabled` follows ingress.** If the file never sets the flag (Node playground / bugpin) but a command has both `url` and `tunnelUrl`, tunneling is on. Explicit `false` still disables. `--tunnel` / `TUNNEL=true` still force on.

## Open questions

See `PLAN.md` § Compatibility concerns. Conservative choices already taken are listed there; items marked “needs Jasen” must not silently expand scope.
