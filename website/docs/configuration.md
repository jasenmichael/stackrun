---
id: configuration
sidebar_position: 5
title: Configuration
description: Configure stackrun with YAML, TOML, JSON, or JS/TS to orchestrate monorepo scripts and local processes.
---

## Formats

`.json`, `.jsonc`, `.json5`, `.yaml`, `.yml`, `.toml`.

JS/TS (`.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`) need `node` on PATH and [jiti](https://github.com/unjs/jiti) in this project (`npm i -D jiti`), or `--jiti npx`.

Prefer YAML or TOML unless you need a programmed config.

## Discovery

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

## Precedence

Lowest to highest:

1. Built-in defaults
2. Configuration files (main file, local `extends`, CWD `.stackrc`)
3. Environment variables (`TUNNEL=true`, `NODE_ENV` overlays, `STACKRUN_JITI`)
4. CLI (`--json`, `--tunnel`, `--command`, `--jiti`)

`NODE_ENV` selects `$<envName>` and `$env.<envName>` overlays inside a layer (for example `$development`).

## Reference

Top-level keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `commands` | array | `[]` | Required to run |
| `before` | string array | `[]` | Sequential hooks before services. Failure aborts the run. |
| `after` | string array | `[]` | Sequential hooks after commands have exited (ok, fail, or Ctrl+C). Empty/omitted is a no-op. |
| `process` | object | see below | Process-manager options. |

### `commands`

Entries without a string `run` are ignored.

| Field | Type | Description |
| --- | --- | --- |
| `run` | string | Shell command to run (required). |
| `name` | string | Log prefix. Truncated to `prefixLength` (default 10) and printed as `[name]`; only that token is colored |
| `prefix` | string | Optional log token. When set, used as-is (not sliced). Else `name` (sliced). |
| `cwd` | string | Working directory |
| `env` | map | Environment variables (`string` or `boolean`) |
| `color` | string | Prefix color (`red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`) |
| `tunnel` | object | Optional per-command tunnel |

A command may also be a bare string: `commands: ["echo hello"]`.

### `before` / `after`

String arrays. Each item runs sequentially in a shell with inherited stdio.

`before` failure aborts — no commands, no `after`. Empty/omitted `before` / `after` is a no-op (no error).

`after` always runs once commands have exited (ok, fail, or Ctrl+C). `killOthers` only kills siblings. A failed `after` hook still fails the run.

### `process`

```yaml
process:
  killOthers: failure
  handleInput: true
  colors: auto
  prefixLength: 10
  cwd: .
```

`process.cwd` is the default working directory for commands and hooks that omit `cwd`. A per-command `cwd` wins.

Child logs look like `[api] …`. Color is on the bracketed name only.

`extends: [a, b]` treats `a` as higher-priority defaults than `b` (main file and `.stackrc` still win over both).

## Environment variables

| Variable | Effect |
| --- | --- |
| `TUNNEL=true` | Same as `--tunnel` (exact string `"true"`, not `1` / `yes`) |
| `NODE_ENV` | Selects `$development` / `$production` / `$test` (and `$env.<name>`) overlays |
| `STACKRUN_JITI` | Same as `--jiti` (`local` or `npx`). JS/TS configs only. |
| `STACKRUN_BINARY` | npm wrapper only. Path to a native `stackrun` binary. |
| `STACKRUN_CACHE` | npm wrapper only. Cache dir (default `~/.cache/stackrun`). |
| `STACKRUN_SKIP_DOWNLOAD` | npm wrapper only. `1` skips GitHub download. |

## JS/TS config

Needs `node` on PATH and `jiti` importable from this project (`npm i -D jiti`).

```ts
export default {
  commands: [
    { name: "api", run: "npm run dev", cwd: "./api" },
    { name: "web", run: "npm run dev", cwd: "./web" },
  ],
};
```

```sh
stackrun --config ./stack.config.ts
stackrun --config ./stack.config.ts --jiti npx
```
