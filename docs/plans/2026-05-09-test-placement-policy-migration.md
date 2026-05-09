# Test Placement Policy Migration Plan

Date: 2026-05-09

## Scope

This plan is the follow-up for `DOC-001` and `DEF-001` from
`docs/reviews/CR-CODEX-TUI-CLI-FULL-SURFACE-2026-05-08.md`.

It does not drive a broad source migration by itself. Its purpose is to turn the
test-placement finding from a vague `Deferred` item into a staged policy and
migration decision with explicit gates.

## Baseline Evidence

Commands:

```bash
rg -l "#\[test\]|#\[cfg\(test\)\]" codex-rs/*/src -g "*.rs" | wc -l
rg -n "#\[test\]" codex-rs/*/src -g "*.rs" | wc -l
rg -n "#\[cfg\(test\)\]" codex-rs/*/src -g "*.rs" | wc -l
```

Observed:

```text
source files with test markers: 369
#[test] hits: 3055
#[cfg(test)] hits: 474
```

Largest affected crates by source file count:

```text
143 core
 80 tui
 12 windows-sandbox-rs
 11 network-proxy
 10 app-server
  8 state
  8 protocol
  8 codex-api
  7 shell-command
  7 app-server-protocol
```

Largest affected crates by `#[test]` count:

```text
1131 core
 920 tui
 174 shell-command
 102 app-server-protocol
  92 protocol
  81 network-proxy
  52 linux-sandbox
  49 apply-patch
  45 github-webhook
  38 app-server
```

Largest affected crates by `#[cfg(test)]` count:

```text
170 core
117 tui
 20 state
 17 app-server
 15 network-proxy
 12 windows-sandbox-rs
 12 github-webhook
  9 app-server-protocol
  8 protocol
  8 linux-sandbox
```

## Decision Required

Before moving tests, choose one policy:

1. Strict migration: all Rust tests under `codex-rs/*/src` must move to
   crate-level `tests/` directories.
2. Policy alignment: existing source-tree tests remain allowed, but new
   validation work should prefer integration tests under `tests/` when practical.
3. Hybrid gate: no new source-tree tests for new modules, while existing
   private-unit tests are migrated only when their owning crate is already being
   refactored.

Current recommendation: use option 3 until a repository owner explicitly accepts
the cost of option 1. The current scope is too large to treat as cleanup inside a
CLI/TUI validation loop.

## Migration Slices

Each slice must be a separate branch or commit, with no unrelated behavior
change mixed in.

| Slice | Scope | Rationale | Verification |
| --- | --- | --- | --- |
| TP-001 | `codex-rs/cli` source tests | Small crate, existing `codex-rs/cli/tests` directory, low protocol risk | `cargo test -p codex-cli`; source-marker scan for `codex-rs/cli/src` |
| TP-002 | `codex-rs/exec` source tests | Small crate, existing integration suite, CLI behavior is externally observable | `cargo test -p codex-exec`; source-marker scan for `codex-rs/exec/src` |
| TP-003 | `codex-rs/apply-patch` source tests | Bounded parser/tool crate with existing `tests/` directory | `cargo test -p codex-apply-patch`; source-marker scan for `codex-rs/apply-patch/src` |
| TP-004 | `codex-rs/protocol` or `codex-rs/app-server-protocol` public API tests | Data-shape behavior can usually be asserted through public types | `cargo test -p codex-protocol` or `cargo test -p codex-app-server-protocol`; source-marker scan for selected crate |
| TP-005 | `codex-rs/core` selected public-flow tests only | Largest risk surface; migrate only tests that can use public or existing test-support APIs | Targeted `cargo test -p codex-core ...`; full `cargo test -p codex-core` before closing any core slice |
| TP-006 | `codex-rs/tui` selected snapshot or widget tests only | Large snapshot-heavy surface; migrate only where public harnesses already exist | `cargo test -p codex-tui`; `cargo insta accept -p codex-tui` only for intentional render changes |

## Gate Rules

- Do not move a test if it requires exposing private production APIs only for the
  migration.
- Do not change runtime behavior in the same commit as a test move.
- For each migrated crate, run the crate-specific test command and record the
  source-marker scan result.
- For `core`, `common`, or protocol crates, ask before running the complete
  workspace suite, but record whether it was run or intentionally skipped.
- If a test cannot move without weakening the assertion, leave it in place and
  record the blocker instead of replacing it with a weaker integration test.

## First Safe Task

Start with `TP-001` or `TP-002`. A valid first commit should contain only:

- moved tests for one crate;
- any test-support imports or fixture moves needed by that crate;
- the matching plan checkbox or review note update;
- passing crate-specific tests and source-marker scan evidence.

Do not start with `codex-rs/core` or `codex-rs/tui`; those are the dominant
hit areas and should remain separate refactor projects.
