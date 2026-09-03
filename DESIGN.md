# DESIGN.md — visual UI

Stackrun is a **CLI-only** tool. There is no web UI, TUI framework, or graphical surface.

## Output (terminal)

Visual design is log formatting with `[name]` prefixes:

- **Info / warn / error** lines from Stackrun itself (`tracing`).
- **Prefixed child output:** `[name]` then a space then the log line. `color` (or `colors: "auto"`) colors only the `[name]` token; the rest of the line stays default terminal color. `colors: false` prints `[name]` with no ANSI.
- **Prefix length:** default 10 characters; the name *inside* the brackets is sliced to that length (`[verylongna]` for a 12-character name).
- **Tunnel process:** each cloudflared sibling uses the same `[name]` as its command. Quick-tunnel URLs stay in that process’s stdout; do not parse or inject them.
- **Input:** `handleInput: true` by default so stdin can be forwarded to child processes.

Rust should keep this information architecture: Stackrun logs vs prefixed child stdout/stderr. Readable prefixes and `color` names are enough.

## Non-goals

- No HTML dashboard.
- No interactive config wizard in the first Rust phases.
