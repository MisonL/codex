# Selective Upstream Intake Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a repeatable selective upstream intake loop that absorbs OpenAI upstream improvements without overwriting this fork's product-specific lines.

**Architecture:** Treat upstream synchronization as a control loop. First classify upstream commits by frozen boundaries and risk, then process changes in isolated worktrees through small PRs. High-coupling areas enter through compatibility facades instead of direct replacement.

**Tech Stack:** Git, GitHub CLI, Rust workspace, Bazel lock checks, app-server protocol v2, TUI, plugin/MCP runtime, GitHub Actions.

---

## Control Contract

- **Primary setpoint:** Reduce useful upstream delta while preserving fork-specific product behavior.
- **Acceptance:** Each intake batch has a documented decision matrix, minimal implementation scope, local verification, pushed branch, and green GitHub Actions.
- **Guardrails:** No direct `upstream/main` merge into `main`; no permission default relaxation; no replacement of Web UI, agent teams, hooks, plugin store, or release chain.
- **Sampling plan:** One batch per control surface: security/runtime, provider/model, app-server/thread, plugin/MCP, TUI UX, CI/release.
- **Rollback trigger:** Wire incompatibility, CI failure not fixed in-batch, product-line regression, or hidden fallback.

## Task 1: Generate Current Upstream Intake Matrix

**Files:**
- Create: `docs/plans/2026-05-05-upstream-intake-current-matrix.md`

- [ ] Step 1: Refresh remotes.

```bash
git fetch origin main --prune
git fetch upstream main --prune
```

Expected:
- `origin/main` and `upstream/main` are current.
- Working tree remains clean.

- [ ] Step 2: Capture current comparison facts.

```bash
git rev-parse origin/main upstream/main
git merge-base origin/main upstream/main
git rev-list --left-right --count upstream/main...origin/main
git diff --shortstat upstream/main..origin/main
```

Expected:
- Output includes current fork head, upstream head, merge base, commit counts, and tree shortstat.

- [ ] Step 3: Build commit candidate list by area.

```bash
git log --reverse --no-merges --format='%H%x09%s' origin/main..upstream/main > /tmp/upstream-candidates.tsv
```

Expected:
- `/tmp/upstream-candidates.tsv` contains upstream commits not present in fork.

- [ ] Step 4: Create matrix document with these columns.

```text
Commit | Area | Functional value | Frozen boundary touched | Conflict level | Decision | Verification
```

Decision values:

```text
P0-direct | P1-facade | P2-scheduled | P3-defer | N/A | reject
```

- [ ] Step 5: Commit the matrix only.

```bash
git add docs/plans/2026-05-05-upstream-intake-current-matrix.md
git commit -m "docs: add current upstream intake matrix"
```

## Task 2: P0 Security and Runtime Batch

**Files:**
- Modify only files identified by the Task 1 matrix.
- Update: `docs/plans/2026-05-05-upstream-intake-current-matrix.md`

- [ ] Step 1: Create an isolated worktree from `origin/main`.

```bash
ts="$(date +%Y%m%d-%H%M%S)"
branch="intake/p0-security-runtime-$ts"
path=".worktrees/$branch"
git worktree add -b "$branch" "$path" origin/main
cd "$path"
```

Expected:
- New branch starts from current `origin/main`.

- [ ] Step 2: Select only P0 commits or semantic patches that are security/runtime fixes.

Allowed examples:

```text
symlink traversal rejection
DNS timeout blocking
custom CA login behind TLS-inspecting proxies
sandbox fallback without permission relaxation
dependency advisory fixes
bounded websocket sends
hook output spill from context
PTY teardown fixes
```

- [ ] Step 3: Apply patches semantically, not mechanically.

Rules:
- Preserve fork behavior when paths overlap.
- Do not introduce hidden fallback.
- Do not relax defaults.
- Add or keep tests that prove the actual failure mode.

- [ ] Step 4: Run focused verification.

Use the relevant subset:

```bash
cd codex-rs
cargo test -p codex-core
cargo test -p codex-network-proxy
cargo test -p codex-login
just fmt
```

Expected:
- Targeted tests pass.
- Formatting passes or modifies only touched Rust files.

- [ ] Step 5: Commit and push.

```bash
git status --short
git add -A
git commit -m "intake: absorb upstream security and runtime fixes"
git push -u origin HEAD
```

- [ ] Step 6: Open PR and watch CI.

```bash
gh pr create --base main --head "$branch" --title "intake: upstream security and runtime fixes" --body "Selective P0 upstream intake. Fork-specific product lines preserved."
gh pr checks --watch
```

Expected:
- Required checks pass before merge.

## Task 3: Provider and Model Metadata Batch

**Files:**
- Expected areas:
  - `codex-rs/core/src/provider_runtime.rs`
  - `codex-rs/codex-api/src/provider.rs`
  - `codex-rs/core/models.json`
  - related provider/model tests

- [ ] Step 1: Create isolated branch `intake/provider-model-*`.
- [ ] Step 2: Pull in model service tiers and provider capability metadata.
- [ ] Step 3: Route provider-specific behavior through `ProviderRuntime` or the current equivalent facade.
- [ ] Step 4: Keep Anthropic and fork multi-provider behavior intact.
- [ ] Step 5: Run targeted provider/model tests.

```bash
cd codex-rs
cargo test -p codex-core remote_models
cargo test -p codex-core client
just fmt
```

- [ ] Step 6: Commit, push, PR, and watch CI.

## Task 4: App-Server and Thread Compatibility Batch

**Files:**
- Expected areas:
  - `codex-rs/app-server-protocol/src/protocol/common.rs`
  - `codex-rs/app-server-protocol/src/protocol/v2.rs`
  - `codex-rs/app-server/src/`
  - `codex-rs/app-server/tests/suite/v2/`
  - `codex-rs/app-server/README.md`

- [ ] Step 1: Create isolated branch `intake/app-server-thread-*`.
- [ ] Step 2: Add or reuse a `ThreadRepository`-style facade before changing thread APIs.
- [ ] Step 3: Only add optional fields or new methods; do not break current v2 request/response shapes.
- [ ] Step 4: Prioritize limited thread history, metadata routing, thread turns pagination, and thread read consistency.
- [ ] Step 5: Regenerate schema if protocol shape changes.

```bash
cd codex-rs
just write-app-server-schema
cargo test -p codex-app-server-protocol
cargo test -p codex-app-server thread
just fmt
```

- [ ] Step 6: Commit, push, PR, and watch CI.

## Task 5: Plugin and MCP Facade Batch

**Files:**
- Expected areas:
  - `codex-rs/core/src/plugins/`
  - `codex-rs/core/src/mcp/`
  - `codex-rs/codex-mcp/src/`
  - `codex-rs/app-server/src/`
  - `codex-rs/tui/src/`

- [ ] Step 1: Create isolated branch `intake/plugin-mcp-facade-*`.
- [ ] Step 2: Define or extend `PluginFacade` over existing manager/store.
- [ ] Step 3: Absorb marketplace list/read/remove/upgrade improvements only through the facade.
- [ ] Step 4: Absorb MCP launcher/tool-call improvements without replacing current MCP runtime in one step.
- [ ] Step 5: Keep `/plugins` current behavior available; new UI affordances must be additive.
- [ ] Step 6: Run plugin/MCP tests.

```bash
cd codex-rs
cargo test -p codex-core plugin
cargo test -p codex-app-server plugin
cargo test -p codex-mcp
just fmt
```

- [ ] Step 7: Commit, push, PR, and watch CI.

## Task 6: TUI UX Batch

**Files:**
- Expected areas:
  - `codex-rs/tui/src/`
  - `codex-rs/tui/tests/`
  - TUI snapshots

- [ ] Step 1: Create isolated branch `intake/tui-ux-*`.
- [ ] Step 2: Select low-risk user-visible improvements only.

Allowed examples:

```text
modified backspace/delete handling
shared paste burst interval
keymap coverage
PR summary statusline items
rename prefill
resume picker shortcut fixes
```

- [ ] Step 3: Avoid plugin UI redesign and major command model changes in this batch.
- [ ] Step 4: Run TUI tests and accept snapshots only for intentional rendering changes.

```bash
cd codex-rs
cargo test -p codex-tui
cargo insta accept -p codex-tui
just fmt
```

- [ ] Step 5: Commit, push, PR, and watch CI.

## Task 7: CI, Bazel, and Release Hygiene Batch

**Files:**
- Expected areas:
  - `.github/workflows/`
  - `.github/actions/`
  - `MODULE.bazel`
  - `MODULE.bazel.lock`
  - `scripts/`
  - release validation scripts

- [ ] Step 1: Create isolated branch `intake/ci-release-hygiene-*`.
- [ ] Step 2: Absorb CI improvements that do not break fork release assets or hodexctl installers.
- [ ] Step 3: Keep fork release asset names and installer URLs stable.
- [ ] Step 4: Run local lock checks if Bazel dependencies change.

```bash
just bazel-lock-update
just bazel-lock-check
```

- [ ] Step 5: Commit, push, PR, and watch CI.

## Task 8: Close the Loop

**Files:**
- Update: `docs/plans/2026-05-05-upstream-intake-current-matrix.md`
- Optional update: `docs/superpowers/specs/2026-05-05-selective-upstream-intake-design.md`

- [ ] Step 1: Mark every candidate as absorbed, selectively absorbed, deferred, rejected, or N/A.
- [ ] Step 2: Record PR numbers and final GitHub Actions outcomes.
- [ ] Step 3: List any remaining same-function double implementations that require owner decision.
- [ ] Step 4: Commit the closure update.

```bash
git add docs/plans/2026-05-05-upstream-intake-current-matrix.md docs/superpowers/specs/2026-05-05-selective-upstream-intake-design.md
git commit -m "docs: close upstream intake planning loop"
```

## Execution Rule

Execute only one task branch at a time. Do not begin the next intake batch until the current branch is merged or explicitly abandoned.
