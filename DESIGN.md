# DESIGN.md — visual UI

Stackrun is a **CLI-only** tool. There is no web UI, TUI framework, or graphical surface.

## Output (terminal)

Visual design is log formatting with `[name]` prefixes:

- **Host lines** (stderr, not `tracing`): `[stackrun]` then a space then the sentence. Color only the name token `stackrun` (letter-rainbow over red/green/yellow/blue/magenta/cyan). Brackets stay uncolored. The line body stays default terminal color. `colors: false` prints `[stackrun]` with no ANSI. `prefixLength` does not slice `stackrun`.
- **Human start lines** (host-prefixed): `[stackrun] Starting [name] {run}` plus ` in {cwd}` when set, and `[stackrun] Starting tunnel sibling [prefix] for {name} at {local} …`. Named tunnels append `({public})`. Quick tunnels append `(public host is a new *.trycloudflare.com)` and do not parse child stdout. Nested `[name]` / `[prefix]` in the body stay uncolored.
- **Prefixed child output:** `[name]` then a space then the log line. `color` (or `colors: "auto"`) colors only the `[name]` token; the rest of the line stays default terminal color. `colors: false` prints `[name]` with no ANSI.
- **Prefix length:** default 10 characters; the name *inside* the brackets is sliced to that length (`[verylongna]` for a 12-character name).
- **Tunnel process:** each cloudflared sibling uses its own prefix (`Tunnel` / cyan unless overridden), not the user command’s name. Quick-tunnel URLs stay in that process’s stdout; do not parse or inject them.
- **Input:** `handleInput: true` by default so stdin can be forwarded to child processes.

Rust should keep this information architecture: Stackrun logs vs prefixed child stdout/stderr. Readable prefixes and `color` names are enough.

## Non-goals

- No HTML dashboard and no product web UI.
- No interactive config wizard in the first Rust phases.

GitHub Pages (`docs` branch, generated at publish) is an install landing page plus `install.sh`, not a control surface.
