# SPEC.md — Stackrun product behavior

This file is the product-behavior contract. If it disagrees with `STACK.md`, this file wins on behavior.

## What Stackrun is

Stackrun is a process-orchestration CLI. It runs one or more arbitrary OS commands concurrently, with optional Cloudflare tunneling, lifecycle hooks, and prefixed log output.

It is not tied to a single application language. Commands are opaque argv/shell strings executed as child processes.

## Entry points

| Surface | Path | Role |
| --- | --- | --- |
| CLI binary | `src/main.rs` | Parse CLI, load config, run stack |
| Library | `src/lib.rs` | Config, stack run, process, tunnel modules |

## CLI

### Flags

| Flag | Type | Default | Meaning |
| --- | --- | --- | --- |
| `-c`, `--config` | string | `stack.config` | Config path. May omit extension. |
| positional `[config]` | string | — | Used if `-c` / `--config` is missing. Default is still `stack.config`. |
| `--json` | string | — | JSON config overlay (highest data priority). |
| `-t`, `--tunnel` | boolean | false | Force tunnels on. Abort before hooks if no command has `tunnel.local`. |
| `--dry-run` | boolean | false | Load config (same path as a real run) and print effective options as JSON. Does not spawn processes, tunnels, `before`, or `after`. |
| `--jiti` | `local` \| `npx` | `local` | JS/TS configs only. See [JS/TS config (jiti)](#jsts-config-jiti). Env: `STACKRUN_JITI`. |
| `--command` | string | — | One-entry `commands` override. Does not require a config file. |
| `-h`, `--help` | boolean | — | Show help |
| `-V` / `--version` | boolean | — | Print crate version (`CARGO_PKG_VERSION`) |

### Environment variables that affect CLI / runtime

| Variable | Effect |
| --- | --- |
| `TUNNEL=true` | Same as `--tunnel` (exact string `"true"`) |
| `NODE_ENV` | Selects `$<envName>` and `$env.<envName>` overlays (`$development` / `$production` / `$test` / …) |
| `STACKRUN_JITI` | Same as `--jiti` (`local` or `npx`). JS/TS configs only. |
| `.env` | Loaded into the process environment before config import; `${VAR}` / `$VAR` interpolation in the env file only; does not override already-set env vars; keys starting with `_` skipped. YAML/JSON/TOML values are not interpolated. JS/TS configs can read `process.env`. |

### CLI flow

1. If `--version` or `-V`: print version, exit 0.
2. Resolve `configPath` from `-c` / `--config` / positional (default `"stack.config"`).
3. Load config from cwd (file discovery, `.env`, `.stackrc`, local `extends`, `NODE_ENV` overlays).
4. If `--tunnel` or `TUNNEL=true`: force tunnels on.
5. If `--command` or `--json` is set, a config file is optional. Otherwise a resolved file is required.
6. If `--dry-run`: print a pretty JSON envelope `{ configFile, config }` (effective `StackrunConfig` after overlays and load defaults). Exit 0. Do not spawn children or set up tunnels. Missing-config and parse errors still exit 1.
7. If a config file was used: log its path, run the stack, exit 0.
8. If no file and no `--command` / `--json`: error `No valid configuration found at ${configPath}`, exit 1.

## Configuration

### Load defaults

| Behavior | Rule |
| --- | --- |
| RC file | CWD `.stackrc` is loaded (`KEY=VALUE`, unflattened) |
| Home / workspace RC | **Not** loaded |
| `package.json` `stack` key | **Not** loaded |
| `extends` | Local paths only |
| Env overlays | `NODE_ENV` selects `$<envName>` and `$env.<envName>` |

### Discovery

Base name from CLI: `stack.config` (or an explicit path).

Lookup, first hit wins, extensions in this order:

`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`, `.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`

Search paths:

1. `{cwd}/{configFile}{ext}` e.g. `stack.config.yaml`
2. `{cwd}/.config/{name-without-.config}{ext}` e.g. `.config/stack.yaml`
3. `{cwd}/.config/{configFile}{ext}` e.g. `.config/stack.config.yaml`

JSON / JSONC / JSON5 / YAML / TOML load natively. JS/TS load via the jiti bridge below.

### JS/TS config (jiti)

When the resolved path is `.js` / `.mjs` / `.cjs` / `.ts` / `.mts` / `.cts`:

1. Require `node` on PATH (`NodeRequired` if missing). Do not mention other tools.
2. Run a Node script (`STACKRUN_BRIDGE_FILE` + the same bridge source) that `import("jiti")` from the project cwd.
3. Default (`--jiti local` / unset `STACKRUN_JITI`): if jiti is missing, fail (`JitiRequired`). Tell the user to use YAML/TOML/JSON, run `npm i -D jiti` in this project, or retry with `--jiti npx` / `STACKRUN_JITI=npx`. Do **not** tell them to install jiti globally (`import` will not see `npm i -g jiti`).
4. `--jiti npx` / `STACKRUN_JITI=npx`: after a local miss, retry with `npx -p jiti node --input-type=module -e <same script>`. Requires `npx` on PATH (`NpxRequired` if missing: install Node/npm, add local jiti, or use YAML). Do not pass `--yes` unless npx cannot run without it. First run may use the network. Never `npm i` / `npm i -g` / silent project install. Never default to `npx` on every JS/TS config.
5. Resolve `default` export (or the module; call a function export). Write JSON to stdout. Rust deserializes into `StackrunConfig`.

### Merge order

Higher priority first:

1. Config file (main)
2. CWD RC (`.stackrc`)
3. Extended layers (`extends`) as fallbacks
4. Environment-specific keys mixed into each layer (`$<NODE_ENV>`, `$env[NODE_ENV]`)

Merge semantics: objects deep-merge; arrays concatenate and de-duplicate; primitives from the higher-priority object win.

### Canonical config shape (`StackrunConfig`)

```ts
{
  before?: string[];                 // alias: beforeCommands
  after?: string[];                  // alias: afterCommands
  process?: ProcessOptions;          // alias: concurrentlyOptions
  tunnel?: false | true | {
    removeExisting?: boolean;
    prefix?: string;                 // sibling log name; default Tunnel
    color?: string;                  // sibling color; default cyan
    resource?: string;               // default Cloudflare object name
  };
  commands?: StackrunConfigCommands[];
}
```

One-cycle aliases still deserialize. Prefer the new names in new configs.

`tunnel: false` disables every cloudflared sibling and skips `tunnel.env`. Omitted `tunnel` (or a defaults object) turns tunneling on when any command has `tunnel.local`. `--tunnel` / `TUNNEL=true` force on and abort if no `local` is set.

### Command entries

| Field | Type | Notes |
| --- | --- | --- |
| `run` | string | Required to run. Alias: `command`. Entries without a string `run` are **filtered out**. |
| `name` | string | Log prefix. Truncated to `process.prefixLength` (default 10), then printed as `[name]` (color on that token only). |
| `cwd` | string | Per-command working directory |
| `env` | `Record<string, string \| boolean \| undefined>` | Base env |
| `color` | string | Prefix color. Alias: `prefixColor`. |
| `tunnel` | object | Optional. `local` starts a cloudflared sibling. `public` makes it a named tunnel. |

`tunnel` fields:

| Field | Type | Notes |
| --- | --- | --- |
| `local` | string | Local origin. Alias: `url`. Required for a tunnel sibling. |
| `public` | string | Public hostname. Alias: `tunnelUrl`. When set: named tunnel + `route dns`. When omitted: quick tunnel (`*.trycloudflare.com`). |
| `env` | same as `env` | Merged over command `env` when tunneling is on. Alias: `tunnelEnv`. |
| `resource` | string | Named-tunnel Cloudflare object name. Alias: `name`. Else stack `tunnel.resource`, else `command.name`. Must be unique among named tunnels. |
| `prefix` | string | Cloudflared sibling log prefix. Else stack `tunnel.prefix`, else `Tunnel`. |
| `color` | string | Sibling prefix color. Else stack `tunnel.color`, else `cyan`. |
| `removeExisting` | boolean | Per-command override of stack `tunnel.removeExisting`. |

When tunneling is on: `env = { ...env, ...tunnel.env }`. When not: command `env` only. One command is either quick or named, not both.

### Process defaults applied at load

| Option | Default | Notes |
| --- | --- | --- |
| `killOthers` | `"failure"` | Kill siblings when a command fails |
| `handleInput` | `true` | Explicit `false` is honored |
| `colors` | `"auto"` | Alias: `prefixColors` |
| `prefixLength` | `10` | Names sliced to this length |

## Process lifecycle

1. If tunneling is on (`--tunnel` / `TUNNEL=true`, omitted `tunnel` when any command has `tunnel.local`, or `tunnel: true` / defaults object): require at least one `tunnel.local` and `cloudflared` on PATH. Explicit `tunnel: false` skips every sibling. `--tunnel` with zero `local` **aborts before hooks**.
2. Print “Tunneling is enabled/disabled” (always, not only `tracing`). If tunneling is off and any command has `tunnel.local`, also print that `--tunnel` / `TUNNEL=true` / omitting `tunnel: false` starts siblings.
3. `before`: sequential shell, stdio inherit. Failure aborts.
4. For each command with `tunnel.local`:
   - **Quick** (no `public`): spawn `{cloudflared} tunnel --url {local}` as a prefixed sibling. No login, no create, no DNS.
   - **Named** (`public` set): require `cert.pem`; `tunnel create` + `route dns` [+ `--overwrite-dns` when `removeExisting`] + spawn `{cloudflared} tunnel run --url {local} {resource}`. Resource: command `tunnel.resource` / `name`, else stack `tunnel.resource`, else `command.name`.
   - Sibling `[name]` / color: command `tunnel.prefix` / `tunnel.color`, else stack defaults, else `Tunnel` + `cyan`. Do not copy the user command’s name or color. Print when each sibling starts.
5. Spawn user commands concurrently (`sh -c` / `cmd /c`). `tunnel.env` overlays `env` when tunneling is on. `handleInput: true` inherits stdin; `false` uses null stdin. Child stdout/stderr lines are prefixed `[name]<space>` (name already sliced to `prefixLength`). `color` / `colors: auto` color only `[name]`; the line body is uncolored. `colors: false` prints `[name]` with no ANSI. Stderr lines still go to stderr. Every child including each cloudflared is prefix-logged. Do not wait-parse trycloudflare URLs.
6. `after`: sequential shell. **Skipped if any concurrent command failed** (no `finally`). **Ctrl+C** stops every command (Unix process groups), then `after` still runs.
7. Cleanup named sessions only: `cloudflared tunnel delete -f` + local credential JSON. **Do not delete the CNAME** (leftover hostname shows Cloudflare `1016`). Quick tunnels have no account resource.
8. Log “Stackrun completed”.

Unix: each child is a process group; SIGINT kills every group, then `after` runs.

## Cloudflare tunnel

Per-command sibling. Mix quick and named in one stack.

1. When any tunnel is on, require `cloudflared` on PATH **before hooks** (`CloudflaredMissing`).
2. Named only: require `cert.pem` (`CloudflaredLoginRequired`). Do **not** run interactive `cloudflared tunnel login`. Quick tunnels do not need a cert.
3. No API token. No Cloudflare REST DNS client.
4. Named: `cloudflared tunnel create` / `route dns` / `tunnel run --url`. `removeExisting` → `tunnel delete -f` if the name exists, and `route dns --overwrite-dns`. There is no cloudflared command to list or delete a CNAME.
5. Quick: `cloudflared tunnel --url {local}` only.
6. Do not write a shared ingress `config.yml`.
7. Cleanup named: `tunnel delete -f` + local creds. Leave DNS alone.

`--tunnel` with no `tunnel.local`: **do not run `before` or processes**; exit 1.

## Precedence

`defaults → configuration files (including RC + extends) → environment variables → CLI arguments`

CLI is highest.

## Intentional differences

1. **Rust is the application.** Node is used only to evaluate JS/TS config via the jiti subprocess. No JS runtime is embedded in the binary.
2. **JSON, JSONC, JSON5, YAML, TOML, `.env`, and `.stackrc` load in Rust.** JS/TS need `node` plus local `jiti` or `--jiti npx`.
3. **`--command <shell>`** builds a one-entry `commands` list and does not require a config file.
4. **`--json` is a CLI overlay** (highest data priority).
5. **`stack` owns before/after and per-command tunnel siblings.** `process` owns concurrent spawn, names, `[name]` prefixes (color on the bracket token only, including `colors: auto`), `handleInput`, env/`tunnel.env`, cwd, kill-others-on-failure, SIGINT process-group cleanup. Ctrl+C stops every command, then `after` runs.
6. **Tunnel manager uses `cloudflared` CLI only** (create / route dns / run / delete). No API token and no REST DNS. Quick tunnels need the binary only. Named tunnels need `cert.pem` from `cloudflared tunnel login`.
7. **Explicit `handleInput: false` is honored.**
8. **Every child is prefix-logged**, including each cloudflared. Do not wait-parse trycloudflare URLs. No rainbow prefix.
9. **`--dry-run`** prints the loaded/effective config as JSON and exits without running processes or tunnels. Process env secrets are not dumped.
10. **Omitted `tunnel` follows `tunnel.local`.** If any command has `local`, tunneling is on. Explicit `tunnel: false` still disables. `--tunnel` / `TUNNEL=true` still force on (and abort when no `local` is set).

## Open questions

See `PLAN.md` § Compatibility concerns. Conservative choices already taken are listed there; items marked “needs Jasen” must not silently expand scope.
