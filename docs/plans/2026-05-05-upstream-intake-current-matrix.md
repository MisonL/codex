# Current Upstream Intake Matrix

日期: 2026-05-05

## Scope

本文件是 `docs/superpowers/specs/2026-05-05-selective-upstream-intake-design.md` 和 `docs/superpowers/plans/2026-05-05-selective-upstream-intake.md` 的执行矩阵。它用于把 `openai/codex` 最新上游差异拆成可决策、可验证、可回滚的吸纳批次。

本轮不直接合并代码，不修改业务实现。

## Baseline Evidence

Commands:

```bash
git fetch origin main --prune
git fetch upstream main --prune
git rev-parse origin/main upstream/main
git merge-base origin/main upstream/main
git rev-list --left-right --count upstream/main...origin/main
git diff --shortstat upstream/main..origin/main
```

Observed:

```text
origin/main    0106975ae1e16a5005a7ceee2c329c0e2dadb90e
upstream/main  70807730f5a8e093d5182089ad5a4b1b4355f9fa
merge-base     25fa9741660dfc95fffb23b67e52af4f56e30187
ahead counts   upstream 1835, fork 207
tree diff      3587 files changed, 349658 insertions, 607077 deletions
```

Directory overlap by changed file count:

```text
606 codex-rs/app-server-protocol
595 codex-rs/core
467 codex-rs/tui
251 web
141 codex-rs/app-server
 77 sdk
 70 codex-rs/utils
 65 codex-rs/serve
 52 .github
 48 codex-rs/exec-server
 46 codex-rs/tools
 45 codex-rs/windows-sandbox-rs
 45 codex-rs/memories
 39 codex-rs/hooks
 39 codex-rs/config
 36 codex-rs/codex-api
 35 codex-rs/state
 35 codex-rs/rollout-trace
 35 codex-rs/protocol
 34 codex-rs/core-plugins
```

Implication: most useful intake work crosses `codex-rs/core`, `app-server-protocol`, `tui`, and `app-server`, which are fork deep-change zones. Direct merge is not an acceptable control input.

## Decision Legend

| Decision | Meaning | Execution rule |
| --- | --- | --- |
| P0-direct | Security or runtime stability fix with clear value and bounded conflict | Implement in first batch with focused tests |
| P1-facade | Valuable but touches fork frozen boundaries | Add or reuse compatibility facade before behavior intake |
| P2-scheduled | UX, CI, or observability value without urgent risk | Batch by module after P0/P1 |
| P3-defer | Potentially valuable but changes policy, cost, or core architecture | Requires separate design and owner decision |
| N/A | Upstream feature has no current fork landing zone | Record only |
| reject | Conflicts with fork product or safety policy | Do not intake unless product direction changes |

## Frozen Boundaries

| Boundary | Freeze reason | Allowed intake shape |
| --- | --- | --- |
| `web/` and `codex-rs/serve` | Fork product surface | Protocol/event fixes only |
| agent teams and multi-agent runtime | Fork orchestration route | Additive event/state/tool fixes only |
| hooks, skills, superpowers | Fork workflow route | Lifecycle, observability, and safety fixes only |
| app-server v2 protocol | Shared wire contract | Optional fields, new methods, generated schema updates |
| thread/session state | Shared state fact source | `ThreadRepository` facade before source migration |
| plugin/marketplace/MCP | Fork extensibility route | `PluginFacade` or launcher adapter before UI/API changes |
| sandbox/network/approval policy | Security default | Only tightening or explicitly gated behavior |
| hodexctl/release/install | Fork distribution | CI hygiene only; no asset naming breakage |

## P0 Direct Intake Candidates

| Commit | Area | Functional value | Frozen boundary touched | Conflict | Decision | Verification |
| --- | --- | --- | --- | --- | --- | --- |
| `d85783901c` | network-proxy | Cover DNS timeout blocking | network policy | Medium | P0-direct, implemented 2026-05-06 | `cargo test -p codex-network-proxy` |
| `4fd7dfe223` | memories/MCP | Reject symlink traversal in local memory backend | memories state | Medium | N/A in current fork: `codex-rs/memories/mcp` crate is absent | `git ls-tree -r --name-only HEAD \| rg '^codex-rs/memories/mcp/'` has no hits; `cargo test -p codex-core memories` |
| `35aaa5d9fc` | app-server transport | Bound websocket request sends with idle timeout | app-server transport | Medium | P0-direct, implemented 2026-05-06 | `cargo test -p codex-api responses_websocket` |
| `9e905528bb` | login/auth | Fix custom CA login behind TLS-inspecting proxies | auth transport | Low | P0-direct, implemented 2026-05-06 | `cargo test -p codex-client --test ca_env`; `cargo test -p codex-client`; `cargo test -p codex-login` |
| `5d5500650b` | Windows PTY | Preserve ConPTY ownership during teardown | Windows process lifecycle | Medium | Fork-equivalent PTY fix implemented 2026-05-06; Windows sandbox ConPTY bridge absent in current fork | `git show 5d5500650b`; `git log --all -S '_input: FileDescriptor' -- codex-rs/utils/pty/src/win/psuedocon.rs`; `cargo test -p codex-utils-pty`; `cargo check -p codex-utils-pty --target x86_64-pc-windows-msvc`; `cargo test -p codex-windows-sandbox`; `cargo check -p codex-windows-sandbox --target x86_64-pc-windows-msvc` |
| `5b80f87c97` | linux-sandbox | Fall back when system bwrap lacks permissions | sandbox runtime | High | N/A in current fork: no system bwrap launcher; bwrap path executes vendored bubblewrap directly | `git show 5b80f87c97`; `git ls-tree -r --name-only HEAD \| rg '^codex-rs/linux-sandbox/src/(launcher\|vendored_bwrap\|linux_run_main\|bwrap)\.rs$'`; `rg 'find_system_bwrap\|SystemBwrap\|exec_system_bwrap' codex-rs/linux-sandbox codex-rs`; `cargo test -p codex-linux-sandbox` (macOS cfg-gated 0 tests); `cargo check -p codex-linux-sandbox --target x86_64-unknown-linux-gnu` blocked by missing OpenSSL/pkg-config cross sysroot |
| `dca105cf99` | hooks/context | Spill large hook outputs from context | hooks context | Medium | P0-direct, implemented 2026-05-06 | `cargo test -p codex-core --test all hooks`; `just fmt`; `just fix -p codex-core` |
| `127434cd8b` | TUI/startup | Bound startup terminal probes | TUI startup | Low | P0-direct | `cargo test -p codex-tui` targeted tests |
| `2817866a32` | config/core | Reduce `ConfigBuilder::build` stack usage | config builder | Low | P0-direct, implemented 2026-05-06 | `cargo test -p codex-core --lib config`; `cargo check -p codex-core --lib` |
| `5744b85b9a` | dependency/security | Fix cargo deny | dependency gate | Low | Already covered by current fork commit `0106975ae1` | `cargo deny check`, GitHub `cargo-deny` |

## P1 Facade Intake Candidates

| Commit | Area | Functional value | Frozen boundary touched | Conflict | Decision | Verification |
| --- | --- | --- | --- | --- | --- | --- |
| `33d24b0df5` | app-server/thread | Migrate more thread history reads to ThreadStore | thread state | High | P1-facade | thread read/list tests |
| `707e51bd8b` | app-server/thread | Route metadata updates through ThreadStore | thread state | High | P1-facade | metadata update tests |
| `541e99cf09` | app-server/thread | Always return limited thread history | protocol/history | High | P1-facade | app-server protocol and history tests |
| `e4d6675632` | app-server/thread | Migrate loaded thread/read history to ThreadStore | thread state | High | P1-facade | thread read/resume tests |
| `127be0612c` | app-server/thread | Migrate thread turns list to thread store | thread state | High | P1-facade | `thread/turns/list` tests |
| `9d579813bb` | provider/model | Add model service tiers metadata | model metadata | Medium | P1-facade | provider/model metadata tests |
| `d927f61208` | provider/compaction | Add remote compaction v2 Responses client path | provider runtime | Medium | P1-facade | compact/provider tests |
| `0d418f478d` | auth | Rename agent identity login surface to access token | auth/API terminology | Medium | P1-facade | login/account tests; docs check |
| `b9e8df47da` | MCP/tooling | Use MCP server instructions in deferred namespace descriptions | MCP/tool schema | Medium | P1-facade | MCP tool spec tests |
| `c8c30d9d75` | MCP/tooling | Emit MCP tool calls as turn items | app-server events | High | P1-facade | app-server item/event tests |
| `83a4e3b66b` | MCP apps | Persist MCP Apps tool call end event | app-server events | High | P1-facade | event persistence tests |
| `0035d7bd18` | exec-server | Add stdio exec-server listener | exec runtime | High | P1-facade | exec-server tests; no sandbox default relaxation |
| `610eefb86b` | plugins | Marketplace upgrade flow | plugin manager | High | P1-facade | plugin list/install/upgrade tests |
| `a8db4af5c3` | plugins | Remove remote plugin uninstall prefix gate | plugin policy | Medium | P1-facade | plugin uninstall tests |
| `48791920a8` | plugins | Track local paths for shared plugins | plugin store | Medium | P1-facade | plugin store tests |
| `96d2ea9058` | plugins/skills | Add remote plugin skill read API | app-server protocol | High | P1-facade | protocol schema and skill read tests |
| `f48b777717` | multi-agent | Support template interpolation in multi-agent usage hints | agent teams | Medium | P1-facade | multi-agent prompt/template tests |
| `be71b6fcd1` | environment | Use selected turn environments for runtime context | environment context | High | P1-facade | turn context tests |
| `443f6b831e` | elicitation | Use 2025-06-18 elicitation capability shape | protocol | High | P1-facade | protocol schema and elicitation tests |
| `aed74e5ee4` | image/view | Emit image view as core item | tool item schema | Medium | P1-facade | view image and app-server event tests |

## P2 Scheduled Intake Candidates

| Commit | Area | Functional value | Frozen boundary touched | Conflict | Decision | Verification |
| --- | --- | --- | --- | --- | --- | --- |
| `36912ce3de` | TUI | Shared paste burst interval on Windows | TUI input | Low | P2-scheduled | `cargo test -p codex-tui` |
| `87d2235b54` | TUI | Modified backspace/delete keys | TUI input | Low | P2-scheduled | key input tests |
| `48402be6fa` | TUI | Improve keymap coverage | TUI tests | Low | P2-scheduled | TUI keymap tests |
| `cc16995cc6` | TUI/GitHub | PR summary statusline items | TUI statusline | Medium | P2-scheduled | statusline snapshots |
| `94800ecbbf` | TUI | Keymap debug inspector | TUI UX | Medium | P2-scheduled | TUI tests and snapshots |
| `ff66b3c7eb` | TUI | Restore alt-enter newline alias | TUI composer | Low | P2-scheduled | composer tests |
| `6784db51c0` | TUI/IDE | Add `/ide` context support | TUI context | Medium | P2-scheduled | IDE context tests |
| `a93c89f497` | TUI/theme | Color statusline from active theme | TUI theme | Medium | P2-scheduled | snapshots |
| `d898cc8f3f` | TUI/goals | Format multi-day goal durations | TUI goals | Low | P2-scheduled | goal/status tests |
| `b6f81257f8` | TUI | Add vim composer mode | TUI composer | Medium | P2-scheduled | composer mode tests |
| `30de54da36` | Bazel/CI | Run sharded rust integration tests | CI | Medium | P2-scheduled | GitHub Bazel workflow |
| `cd2760fc08` | Bazel/CI | Cross-compile Windows Bazel clippy | CI | Medium | P2-scheduled | GitHub Bazel workflow |
| `466798aa83` | Bazel/CI | Cross-compile Windows Bazel tests | CI | Medium | P2-scheduled | GitHub Bazel workflow |
| `c39824c2fd` | PR babysitter | Improve CI diagnostics and guardrails | `.codex` skill | Low | P2-scheduled | script tests if present |

## P3 Deferred or Reject Candidates

| Commit | Area | Functional value | Frozen boundary touched | Conflict | Decision | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| `4950e7d8a6` | exec/security | Unsandboxed process exec API | sandbox/approval | High | P3-defer | Requires explicit safety design |
| `5c1ec8f4fd` | TUI/policy | Retire `/approvals`, rename `/autoreview` to `/approve` | command UX and approval model | High | P3-defer | May conflict with fork approval semantics |
| `1b900bee8a` | approval policy | Unify skip-review handling for `approval_mode = approve` | approval semantics | High | P3-defer | Needs policy owner decision |
| `9b8d585075` | environment config | Add Codex environment config | runtime context | High | P3-defer | Needs environment model alignment |
| `41e171fcf2` | app-server | Move transport into dedicated crate | architecture | High | P3-defer | Structural refactor, no direct user feature |
| `972b819213` | app-server transport | Protocol v3 segmentation for remote control | app-server transport | High | P3-defer | No direct landing without remote-control decision |
| `85203d8872` | image generation | Default image generation on | cost/safety | High | reject by default | Conflicts with conservative defaults |
| `53b1570367` | image output | Default high detail image output | cost/safety | High | reject by default | Conflicts with cost policy |
| `9aaa5d9358` | network/sandbox | Bypass managed network for escalated exec | network policy | High | reject | Permission relaxation |
| `67849d950d` | docs/spec cleanup | Remove local docs and specs | docs/superpowers | High | reject | Conflicts with fork planning workflow |
| `3d1d164aee` | goal/behavior | Remove no-tool goal continuation suppression | agent behavior | Medium | P3-defer | Needs fork long-running orchestration review |

## Area-Level Control Matrix

| Area | Current fork position | Upstream intake posture | First batch |
| --- | --- | --- | --- |
| Security/runtime | Preserve conservative defaults | Directly absorb tightening and stability fixes | P0 |
| Thread/app-server | Deep fork changes, Web UI depends on it | Facade first, then optional fields/new methods | P1 |
| Provider/model | Fork multi-provider path | Centralize capabilities, preserve Anthropic | P1 |
| Plugin/MCP | Fork plugin manager and skills route | Facade and launcher adapter first | P1 |
| TUI UX | User-visible but less urgent | Batch low-risk shortcuts/statusline/composer fixes | P2 |
| CI/Bazel | Fork release and CI differ | Absorb hygiene without changing release assets | P2 |
| Policy defaults | Conservative by design | Defer or reject relaxations | P3 |

## Raw Candidate Reproduction

Use this command to regenerate the raw upstream-only commit list:

```bash
git log --reverse --no-merges --format='%H%x09%cs%x09%s' origin/main..upstream/main
```

Use this command to inspect path overlap for a specific commit:

```bash
git show --name-only --format='%H%n%s' <commit>
```

Use this command to compare a selected upstream commit against fork files:

```bash
git diff-tree --no-commit-id --name-only -r <commit>
```

## Next Execution Gate

Start with `P0-direct` only. The first code branch should be named:

```text
intake/p0-security-runtime-<timestamp>
```

Do not start P1 facade work until P0 is merged or explicitly abandoned.
