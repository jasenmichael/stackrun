---
id: usage
sidebar_position: 4
title: Usage
---

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
| `--jiti <local\|npx>` | JS/TS configs only. Default `local`. |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Print crate version |

```sh
stackrun
stackrun --config ./stack.config.yaml
stackrun custom.yaml
stackrun --tunnel --config ./stack.config.yaml
stackrun --dry-run
```

`--json` is a JSON string overlay, not a file path.

`--dry-run` prints:

```json
{
  "configFile": "/abs/path/to/stack.config.yaml",
  "config": { "commands": [], "forceTunnel": false }
}
```

`configFile` is `null` when no file was used. Exit 0 on a successful load; exit 1 on load errors. Process env is not dumped. Keys in `commands[].env` and `tunnel.env` that look like secrets (`token`, `secret`, `password`, `key`, `authorization`) are replaced with `[redacted]`.

## How it works

Stackrun loads one config from the current directory, then applies `.env`, `.stackrc`, local `extends`, and `NODE_ENV` overlays. CLI flags win last.

If tunneling is on, Stackrun checks for `cloudflared` (and `cert.pem` for named tunnels) before any hook runs.

`before` hooks run one at a time with inherited stdio. A failed hook aborts the stack.

Every command then starts at once. Each child, including each `cloudflared`, prints as `[name] line`. Color applies to the name token only.

If one command fails, siblings are killed (`killOthers: failure`). Ctrl+C stops every process group on Unix and is a stop, not a failure (exit 0 unless a command already failed).

`after` always runs once commands have exited (ok, fail, or Ctrl+C), unless `before` failed or the run never started.

Named tunnels are deleted on exit. DNS CNAMEs are left in place.
