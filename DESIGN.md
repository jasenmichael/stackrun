# DESIGN.md — visual UI

Stackrun is a **CLI-only** tool. There is no web UI, TUI framework, or graphical surface.

## Output (terminal)

Visual design is log formatting in the concurrently style:

- **Info / warn / error** lines from Stackrun itself (`tracing`).
- **Prefixed child output:** concurrently-style `[name]` then a space then the log line. `prefixColor` (or `prefixColors: "auto"`) colors only the `[name]` token; the rest of the line stays default terminal color. `prefixColors: false` prints `[name]` with no ANSI.
- **Prefix length:** default 10 characters; the name *inside* the brackets is sliced to that length (`[verylongna]` for a 12-character name).
- **Tunnel process:** default name `"Tunnel"` so lines look like `[Tunnel] ...`. If no `cfTunnelConfig.commandOptions.prefixColor` is set, Node on `main` painted the name with a chalk rainbow. If `prefixColor` is set, that color is used instead. Unset on Rust uses cyan (same auto cycle as other unnamed-color commands).
- **Input:** `handleInput: true` by default so stdin can be forwarded to child processes.

Rust should keep this information architecture: Stackrun logs vs prefixed child stdout/stderr. Exact rainbow/chalk parity is not required on day one; readable prefixes and `prefixColor` names are.

## Non-goals

- No HTML dashboard.
- No interactive config wizard in the first Rust phases.
