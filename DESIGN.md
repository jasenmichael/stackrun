# DESIGN.md — visual UI

Stackrun is a **CLI-only** tool. There is no web UI, TUI framework, or graphical surface.

## Output (terminal)

Visual design is log formatting in the concurrently style:

- **Info / warn / error** lines from Stackrun itself (`tracing`).
- **Prefixed child output:** each command gets a name prefix, optionally colored (`prefixColor`, or `prefixColors: "auto"`).
- **Prefix length:** default 10 characters; names are sliced to that length.
- **Tunnel process:** default name `"Tunnel"`. If no `cfTunnelConfig.commandOptions.prefixColor` is set, Node on `main` painted the name with a chalk rainbow. If `prefixColor` is set, that color is used instead.
- **Input:** `handleInput: true` by default so stdin can be forwarded to child processes.

Rust should keep this information architecture: Stackrun logs vs prefixed child stdout/stderr. Exact rainbow/chalk parity is not required on day one; readable prefixes and `prefixColor` names are.

## Non-goals

- No HTML dashboard.
- No interactive config wizard in the first Rust phases.
