---
name: test-tui
description: Guide for testing Codex TUI interactively
---

You can start and use Codex TUI to verify changes. 

Important notes:

Start interactively.
Always set `TERM=xterm-256color` when driving the TUI from an automated PTY. If `TERM=dumb`, the CLI intentionally asks for confirmation before starting and non-interactive smoke tests can hang or exercise the wrong prompt path.
Always set RUST_LOG="trace" when starting the process.
Pass `-c log_dir=<some_temp_dir>` argument to have logs written to a specific directory to help with debugging.
When sending a test message programmatically, send text first, then send Enter in a separate write (do not send text + Enter in one burst).
Use `just codex` target to run - `TERM=xterm-256color RUST_LOG=trace just codex -c ...`
