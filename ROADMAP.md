# Roadmap

Ideas for later. Not a schedule. Pick one, update [SPEC.md](SPEC.md) / [DESIGN.md](DESIGN.md), then implement.

## Next

- [ ] **Per-process IO.** `process.handleInput` is all-or-nothing: inherit stdin to every child, or null. Later: manage stdin/stdout per command (focus one process, mute a stream). CLI-only; no TUI or dashboard.

- [ ] **Clickable tunnel URLs.** Today, trycloudflare URLs stay in cloudflared stdout and are not parsed. Later: when a public URL is known (named `tunnel.public`, or a discovered `*.trycloudflare.com`), print a clean `https://…` line so the terminal can open it. OSC-8 optional. Injecting that URL into env is a follow-on.

## Later

- [ ] Homebrew / scoop / winget
- [ ] Apple notarization
- [ ] Windows process-group parity
