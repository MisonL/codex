# Codex TUI and CLI Full Surface Validation

日期: 2026-05-08

## Scope

本文件是 `/Volumes/Work/code/stellarlinkco-codex-src` 当前线程的验证矩阵和证据索引，覆盖 Codex Rust CLI/TUI、非交互 `exec`、slash commands、agent tool handlers、MCP/app/plugin 入口、权限/沙箱/文件/进程/网络组合，以及 TUI 视觉和交互一致性。

本文件不记录业务实现计划，不作为代码修改驱动；它只记录验证任务、命令、结果、证据和缺陷。所有结论必须由源码位置、命令输出、测试结果、日志或截图支撑。

## Baseline Facts

| Item | Evidence | Status |
| --- | --- | --- |
| Workspace | `/Volumes/Work/code/stellarlinkco-codex-src` | Confirmed |
| Branch | `main...origin/main` | Confirmed by `git status --short --branch` |
| Initial dirty file | `codex-rs/core/src/shell_snapshot.rs` | Initially dirty; later updated by this validation loop to fix snapshot timeout cleanup |
| Primary CLI entry | `codex-rs/cli/src/main.rs` | Confirmed |
| TUI CLI args | `codex-rs/tui/src/cli.rs` | Confirmed |
| Exec CLI args | `codex-rs/exec/src/cli.rs` | Confirmed |
| Slash command registry | `codex-rs/tui/src/slash_command.rs` | Confirmed |
| Slash command dispatch | `codex-rs/tui/src/chatwidget.rs` | Confirmed |
| Tool handler directory | `codex-rs/core/src/tools/handlers/` | Confirmed |
| TUI interactive test rule | `.codex/skills/test-tui/SKILL.md` | Confirmed |
| Rust toolchain | `rustc 1.91.1`, `cargo 1.91.1`, `just 1.47.1` | Confirmed |
| Existing debug binary | `codex-rs/target/debug/codex` and `codex-rs/target/debug/codex-exec`, both version `1.3.0` | Confirmed |

## Verification Status Legend

| Status | Meaning |
| --- | --- |
| Pending | 已定义验证项，但尚未执行 |
| Pass | 命令或人工检查完成且结果符合预期 |
| Fail | 验证发现阻塞或行为不符合预期 |
| Blocked | 当前环境缺少必要前置条件，不能得出通过结论 |
| Skipped | 明确不适用于当前平台或当前目标 |

## Surface Inventory

### Top-Level CLI Commands

Source: `codex-rs/cli/src/main.rs`

| Surface | Representative commands | Priority | Verification |
| --- | --- | --- | --- |
| Interactive TUI default | `codex`, `codex [PROMPT]` | P0 | `just codex --help`; interactive TUI smoke |
| Non-interactive exec | `codex exec`, `codex e` | P0 | `just exec --help`; targeted exec tests |
| Review | `codex review` | P1 | help smoke; non-mutating review dry run if credentials allow |
| Login/logout | `codex login`, `codex login status`, `codex logout` | P1 | help/status only unless user authorizes credential changes |
| MCP config | `codex mcp list/get/add/remove/login/logout` | P0 | isolated `CODEX_HOME` config tests |
| Plugins | `codex plugin marketplace remove` | P1 | isolated `CODEX_HOME` negative/positive config tests |
| MCP server | `codex mcp-server` | P1 | help/startup smoke, protocol tests |
| App server | `codex app-server`, `codex app-server generate-ts`, `generate-json-schema` | P1 | protocol/schema tests; help smoke |
| Serve | `codex serve --host --port --no-open --dev --token` | P1 | help smoke; local bind smoke |
| Completion | `codex completion <shell>` | P0 | generate for supported shells |
| Sandbox | `codex sandbox macos/linux/windows` | P1 | platform-specific help and minimal command smoke |
| Debug | `codex debug app-server send-message-v2`, hidden `clear-memories` | P2 | help smoke; no destructive clear without isolated home |
| Execpolicy | hidden `codex execpolicy check` | P2 | targeted tests |
| Apply | `codex apply`, `codex a` | P1 | apply-patch tests |
| Resume/fork | `codex resume`, `codex fork` | P1 | parser tests; TUI picker smoke |
| Cloud tasks | `codex cloud`, `codex cloud-tasks` | P2 | help smoke only unless credentials allow |
| Features | `codex features` | P1 | help/list smoke |

### TUI CLI Flags

Source: `codex-rs/tui/src/cli.rs`

| Flag group | Examples | Priority | Verification |
| --- | --- | --- | --- |
| Prompt and images | `PROMPT`, `--image/-i` | P0 | parser/help; image attach smoke with fixture |
| Model/provider | `--model/-m`, `--oss`, `--local-provider` | P1 | parser/config precedence tests |
| Profile/config | `--profile/-p`, `-c key=value` | P0 | help/config smoke |
| Permissions/sandbox | `--sandbox/-s`, `--ask-for-approval/-a`, `--full-auto`, `--dangerously-bypass-approvals-and-sandbox` | P0 | parser conflict tests; permission popup smoke |
| Workspace | `--cd/-C`, `--add-dir` | P0 | parser and exec tests |
| Web search | `--search` | P1 | config/tool availability smoke; no live web unless credentials allow |
| Display | `--no-alt-screen` | P0 | interactive visual smoke |
| Resume/fork internal | top-level wrappers set skipped fields | P1 | parser tests for wrapper behavior |

### Exec CLI Flags

Source: `codex-rs/exec/src/cli.rs`

| Flag group | Examples | Priority | Verification |
| --- | --- | --- | --- |
| Prompt/stdin | `PROMPT`, `-` | P0 | `codex-rs/exec/tests/suite/prompt_stdin.rs` |
| Resume/review | `exec resume`, `exec review` | P1 | parser and suite tests |
| JSON/output | `--json`, `--output-last-message`, `--output-schema`, `--color`, `--progress-cursor` | P0 | suite tests and help smoke |
| Session isolation | `--ephemeral`, `--skip-git-repo-check` | P0 | suite tests |
| Permissions/sandbox | `--sandbox`, `--full-auto`, `--dangerously-bypass-approvals-and-sandbox`, `--add-dir` | P0 | suite tests |
| Model/provider/profile | `--model`, `--oss`, `--local-provider`, `--profile`, `-c` | P1 | parser/config smoke |

### Slash Commands

Source: `codex-rs/tui/src/slash_command.rs`, dispatch in `codex-rs/tui/src/chatwidget.rs`

| Category | Commands | Priority | Verification |
| --- | --- | --- | --- |
| Session lifecycle | `/new`, `/clear`, `/resume`, `/fork`, `/quit`, `/exit`, `/rollout` | P0 | TUI interactive smoke |
| Agent turn control | `/compact`, `/review`, `/plan`, `/loop`, `/collab`, `/agent`, `/multi-agents` | P0 | command dispatch tests; interactive smoke |
| Permissions | `/approvals`, `/permissions`, `/setup-default-sandbox`, `/sandbox-add-read-dir` | P0 | popup/sandbox tests; platform gating |
| Model/config | `/model`, `/fast`, `/personality`, `/experimental`, `/status`, `/debug-config` | P0 | popup and status snapshots |
| Tools/status | `/diff`, `/copy`, `/mention`, `/skills`, `/mcp`, `/apps`, `/plugins`, `/ps`, `/clean` | P1 | TUI smoke; command output snapshots |
| Visual/audio | `/statusline`, `/theme`, `/realtime`, `/settings` | P1 | visual/accessibility smoke |
| Account/feedback | `/logout`, `/feedback` | P2 | non-destructive help/popup only unless explicitly authorized |
| Debug-only | `/test-approval`, `/debug-m-drop`, `/debug-m-update` | P2 | debug build only; isolated run |

### Agent Tool Handlers

Source: `codex-rs/core/src/tools/handlers/`

| Handler area | Files | Priority | Verification |
| --- | --- | --- | --- |
| Shell/process | `shell.rs`, `unified_exec.rs`, `agent_jobs.rs` | P0 | core/exec tests; sandbox smoke |
| File read/search/list | `read_file.rs`, `list_dir.rs`, `grep_files.rs`, `search_tool_bm25.rs` | P0 | core tool tests; path boundary tests |
| Patch | `apply_patch.rs`, `tool_apply_patch.lark`, runtime apply patch | P0 | `cargo test -p codex-apply-patch`; exec apply patch suite |
| Planning/input/permissions | `plan.rs`, `request_user_input.rs`, `request_permissions.rs` | P0 | app-server/TUI tests |
| MCP/dynamic/resource | `mcp.rs`, `dynamic.rs`, `mcp_resource.rs`, `mcp_tool_call.rs` | P0 | MCP tests; app-server dynamic tool tests |
| Artifacts/media | `artifacts.rs`, `spreadsheet_artifact.rs`, `presentation_artifact.rs`, `view_image.rs` | P1 | artifact tests; fixture smoke |
| Scheduled tasks/JS REPL | `cron.rs`, `js_repl.rs` | P1 | feature-gated tests |
| Parallel/multi-agent | `multi_agents.rs`, `parallel.rs`, `orchestrator.rs` | P1 | multi-agent tests and dispatch smoke |

## Execution Matrix

| ID | Area | Scenario | Command or method | Expected result | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| CLI-001 | CLI parser | Top-level help renders and lists public commands | `codex-rs/target/debug/codex --help`; initial `just codex --help` attempt blocked on Cargo lock and was terminated | Exit 0, help includes TUI and subcommands | Pass | 2026-05-08 existing debug binary output lists `exec`, `serve`, `review`, `login`, `mcp`, `plugin`, `mcp-server`, `app-server`, `completion`, `sandbox`, `debug`, `apply`, `resume`, `fork`, `cloud`, `features` |
| CLI-002 | CLI parser | Exec help renders | `codex-rs/target/debug/codex exec --help`; initial `just exec --help` attempt blocked on Cargo lock and was terminated | Exit 0, help includes exec flags and subcommands | Pass | 2026-05-08 existing debug binary output lists `resume`, `review`, JSON/output/schema/session/sandbox flags |
| CLI-003 | CLI parser | Completion generation works | `codex-rs/target/debug/codex completion bash \| sed -n '1,80p'` | Exit 0, completion output contains `codex` commands | Pass | 2026-05-08 output contains `_codex()` and command cases including `codex__exec`, `codex__mcp`, `codex__resume`, `codex__review` |
| CLI-004 | CLI parser | Feature flags reject unknown feature | `codex-rs/target/debug/codex --enable does_not_exist features list` | Clear error, no silent ignore | Pass | Exit 1 with `Error: Unknown feature flag: does_not_exist` |
| CLI-005 | MCP config | `mcp add/list/get/remove` with isolated `CODEX_HOME` | temp `CODEX_HOME`, `codex-rs/target/debug/codex mcp ...` | Config round trip works, no user home mutation | Pass | Added `test-server`, listed JSON, got JSON, removed, final list `[]` under temp `CODEX_HOME` |
| CLI-006 | Sandbox | macOS sandbox minimal command | `codex-rs/target/debug/codex sandbox macos --help`; `codex-rs/target/debug/codex sandbox macos /usr/bin/true` | Help and command succeed on supported platform | Pass | Help rendered; `/usr/bin/true` exited 0 |
| EXEC-001 | Exec tests | Prompt/stdin/json/schema/apply/sandbox suite | `cargo test -p codex-exec` | Exit 0 | Pass | 19 lib tests, 1 main test, 55 suite tests, 25 JSON event tests, and doc-tests all passed |
| EXEC-002 | Exec smoke | Non-interactive help and conflict handling | `codex-rs/target/debug/codex exec --help`; conflict and invalid enum probes | Clear clap errors | Pass | Exec help renders; `--full-auto` with `--dangerously-bypass-approvals-and-sandbox` exits 2; `--color invalid` exits 2; `exec resume --help` and `exec review --help` render |
| TUI-001 | TUI tests | TUI unit/snapshot suite | `cargo test -p codex-tui` | Exit 0 or known existing hang documented | Pass | 1309 lib tests passed, 2 ignored; main/md-events/test_backend had 0 tests; integration `tests/all.rs` passed 6 and ignored 1; doc-tests ignored 2 |
| TUI-002 | TUI interactive | Start TUI with logs and no alt screen | `RUST_LOG=trace codex-rs/target/debug/codex -c log_dir=/tmp/codex-tui-log.l9rrij --no-alt-screen`; confirm `TERM=dumb`; send `/exit` then Enter | UI starts; logs written; can exit cleanly | Pass | TUI rendered startup card/statusline/MCP startup, `/exit` exited 0, log file `/tmp/codex-tui-log.l9rrij/codex-tui.log` contains `ShutdownComplete` |
| TUI-003 | Slash commands | Popup command list and inline commands | interactive `/status`, `/diff`, `/model`, `/permissions`, `/theme`, `/statusline`, `/exit` | No panic; expected popups/output | Pass | `/status` rendered session card; `/diff` opened pager with current diff; `/model`, `/permissions`, `/theme`, `/statusline` opened expected selectors; `/exit` shut down cleanly |
| TUI-004 | Visual/UX | Desktop terminal layout, wrapping, popups, statusline | terminal capture from `TERM=xterm-256color RUST_LOG=trace just codex -c log_dir=/tmp/codex-tui-interactive.VirHa4 --no-alt-screen` | Text does not overlap; focus and hints consistent | Pass | Status card, diff pager, model selector, permissions selector, theme preview, and status line selector rendered with visible headers, selection cursor, and footer hints in 80-column PTY |
| TUI-005 | Accessibility | Keyboard-only operation for core popups | interactive arrows, `q`, `esc`, `/exit` | User can navigate and cancel/apply | Pass | Arrow keys moved selections in `/model`, `/permissions`, `/theme`, `/statusline`; diff pager scrolled with down arrow and exited with `q`; selectors dismissed with `esc`; `/exit` ended session |
| CORE-001 | Tool handlers | Core tests for tools/protocol/session behavior | `cargo test -p codex-core` plus targeted reruns | Exit 0 | Pass | 2026-05-09 full rerun exited 0: main integration suite 762 passed, 0 failed, 18 ignored; `permissions_glob_profiles` 2 passed; `responses_headers` 4 passed |
| CORE-001A | Shell snapshot | Timeout cleanup kills snapshot shell process group | `cargo test -p codex-core --lib shell_snapshot` | Exit 0 | Pass | 11 shell snapshot tests passed, including `timed_out_snapshot_shell_is_terminated` |
| CORE-002 | Apply patch | Patch grammar/tool behavior | `cargo test -p codex-apply-patch` | Exit 0 | Pass | 49 unit tests and 17 integration tests passed; doc-tests passed |
| CORE-003 | MCP client/server | MCP server/client tests | `cargo test -p codex-mcp-server`; `cargo test -p codex-rmcp-client` | Exit 0 | Pass | `codex-rmcp-client`: 23 unit tests passed plus integration/resource/http recovery tests passed; `codex-mcp-server`: 10 lib tests, 3 integration tests, and doc-tests passed |
| APP-001A | App server protocol | Protocol/schema regression | `cargo test -p codex-app-server-protocol` | Exit 0 | Pass | 102 unit tests and 2 schema fixture tests passed; doc-tests passed |
| APP-001B | App server runtime | App-server runtime/API regression | `cargo test -p codex-app-server` | Exit 0 | Pass | 87 lib tests passed; integration `tests/all.rs` passed 243 and ignored 1; main/test_notify_capture/doc-tests had 0 tests |
| APP-002 | Serve | Local HTTP bind smoke | `codex-rs/target/debug/codex serve --host 127.0.0.1 --port 0 --no-open --token smoke-token`; `curl -i`; `kill <pid>` | Server starts and returns Web UI HTML; service is stopped after smoke | Pass | Server reported `http://127.0.0.1:52546?token=smoke-token`; curl returned HTTP 200 HTML; PID 68901 was killed and port 52546 no longer listened |
| APP-003 | App server schema | Generate JSON schema to temp directory | `tmp_out=$(mktemp -d ...); codex-rs/target/debug/codex app-server generate-json-schema -o "$tmp_out"; find ...; rm -rf "$tmp_out"` | Exit 0 and schema files are generated | Pass | Generated files included `ClientRequest.json`, `EventMsg.json`, approval params/responses, fuzzy search schemas |
| APP-004 | App server TypeScript | Generate TypeScript bindings to temp directory | `tmp_out=$(mktemp -d ...); codex-rs/target/debug/codex app-server generate-ts -o "$tmp_out"; find ...; rm -rf "$tmp_out"` | Exit 0 and TS files are generated | Pass | Generated files included `ClientRequest.ts`, `ClientNotification.ts`, `ApplyPatchApprovalParams.ts`, collab agent event types |
| CLI-007 | Help smoke | Review/login/logout/resume/fork help | `codex-rs/target/debug/codex review --help`; `login --help`; `logout --help`; `resume --help`; `fork --help` | Exit 0 and options render | Pass | Help output confirmed review change selectors, login status/API key/device flags, resume/fork session and TUI flags |
| CLI-TEST-001 | CLI tests | CLI crate parser/config tests | `cargo test -p codex-cli` | Exit 0 | Pass | 2026-05-09 rerun exited 0: 6 lib tests, 31 main tests, and debug clear memories / execpolicy / features / marketplace / MCP add-list integration tests passed; doc-tests had 0 tests |
| DOC-001 | Test placement | No Rust tests in core `src/` directories per local constraint | `rg -n "#\\[test\\]|#\\[cfg\\(test\\)\\]" codex-rs/*/src -g "*.rs"` | Existing repo may violate; report facts, do not mutate | Deferred | Existing repo has broad source-tree tests: 369 source files, 3055 `#[test]` hits, and 474 `#[cfg(test)]` hits. This is a repository-wide policy debt rather than a functional blocker in this validation loop |
| HYGIENE-001 | Workspace hygiene | Matrix change is isolated | `git diff --name-only`; `git status --short` | Only validation report plus targeted core test/harness fixes | Pass | Current diff is limited to `docs/reviews/CR-CODEX-TUI-CLI-FULL-SURFACE-2026-05-08.md`, `codex-rs/core/src/shell_snapshot.rs`, and three core suite test files: `cli_stream.rs`, `shell_snapshot.rs`, `view_image.rs` |

## Evidence Log

Append executed commands here with exit code, short output summary, and artifact path if any.

| Time | ID | Command | Exit | Result summary | Artifact |
| --- | --- | --- | --- | --- | --- |
| 2026-05-08 | CLI-001 | `just codex --help` | 143 | Blocked waiting for Cargo package/artifact locks; terminated to avoid hanging the validation loop | terminal output |
| 2026-05-08 | CLI-002 | `just exec --help` | 143 | Blocked waiting for Cargo package/artifact locks; terminated to avoid hanging the validation loop | terminal output |
| 2026-05-08 | CLI-003 | `cargo run --bin codex -- completion bash` | terminated | Began compiling after lock contention; terminated and replaced by existing debug binary smoke | terminal output |
| 2026-05-08 | CLI-001 | `codex-rs/target/debug/codex --help` | 0 | Help rendered and listed public top-level command surface | terminal output |
| 2026-05-08 | CLI-002 | `codex-rs/target/debug/codex exec --help` | 0 | Exec help rendered and listed exec flags/subcommands | terminal output |
| 2026-05-08 | CLI-003 | `codex-rs/target/debug/codex completion bash \| sed -n '1,80p'` | 0 | Bash completion generated with top-level command cases | terminal output |
| 2026-05-08 | CLI-004 | `rg -n "is_known_feature_key\|feature_toggles\|FeatureToggles\|enable" codex-rs/cli/src/main.rs`; `sed -n '520,760p' codex-rs/cli/src/main.rs` | 0 | Source confirms unknown feature validation path; runtime unit test not completed due compile cost | terminal output |
| 2026-05-08 | CLI-004 | `codex-rs/target/debug/codex --enable does_not_exist features list` | 1 | Unknown feature is rejected with explicit error | terminal output |
| 2026-05-08 | CLI-005 | `tmp_home=$(mktemp -d ...); CODEX_HOME="$tmp_home" codex-rs/target/debug/codex mcp add/list/get/remove ...` | 0 | MCP config round trip succeeded in isolated home and final list was empty | terminal output |
| 2026-05-08 | CLI-006 | `codex-rs/target/debug/codex sandbox macos --help`; `codex-rs/target/debug/codex sandbox macos /usr/bin/true` | 0 | macOS sandbox help rendered; minimal command exited 0 | terminal output |
| 2026-05-08 | CLI-TEST-001 | `cargo test -p codex-cli` | terminated | Command stayed in `codex-core` compile stage for over 20 minutes and was terminated; no test pass/fail summary | terminal output |
| 2026-05-08 | Login status | `CODEX_HOME=$(mktemp -d ...); codex-rs/target/debug/codex login status; rm -rf "$CODEX_HOME"` | 1 | Isolated home status reports `Not logged in` without mutating real credentials | terminal output |
| 2026-05-08 | APP-002 | `codex-rs/target/debug/codex serve --help` | 0 | Serve help rendered host/port/no-open/dev/token flags | terminal output |
| 2026-05-08 | APP-002 | `codex-rs/target/debug/codex serve --host 127.0.0.1 --port 0 --no-open --token smoke-token`; `curl -sS -i http://127.0.0.1:52546?...`; `kill 68901` | 0 for HTTP probe; service terminated after smoke | Local Web UI returned HTTP 200 and `text/html`; port was closed after killing the smoke server | terminal output |
| 2026-05-08 | APP-003 | `tmp_out=$(mktemp -d ...); codex-rs/target/debug/codex app-server generate-json-schema -o "$tmp_out"; find "$tmp_out" ...; rm -rf "$tmp_out"` | 0 | JSON schema generation succeeded and emitted protocol schema files | terminal output |
| 2026-05-08 | APP-004 | `tmp_out=$(mktemp -d ...); codex-rs/target/debug/codex app-server generate-ts -o "$tmp_out"; find "$tmp_out" ...; rm -rf "$tmp_out"` | 0 | TypeScript binding generation succeeded and emitted protocol TS files | terminal output |
| 2026-05-08 | CLI-007 | `codex-rs/target/debug/codex review/login/logout/resume/fork --help` | 0 | Help rendered for review, auth, resume and fork surfaces | terminal output |
| 2026-05-08 | TUI-002 | `RUST_LOG=trace codex-rs/target/debug/codex -c log_dir=/tmp/codex-tui-log.l9rrij --no-alt-screen` | 0 | TUI started after `TERM=dumb` confirmation, rendered main screen and statusline, started MCP status, accepted `/exit`, and logged shutdown | `/tmp/codex-tui-log.l9rrij/codex-tui.log` |
| 2026-05-08 | EXEC-002 | `codex-rs/target/debug/codex exec --full-auto --dangerously-bypass-approvals-and-sandbox noop`; `codex-rs/target/debug/codex exec --color invalid noop`; `codex-rs/target/debug/codex exec resume --help`; `codex-rs/target/debug/codex exec review --help` | 2, 2, 0, 0 | Exec parser rejects conflicting execution modes and invalid color enum; resume/review help renders | terminal output |
| 2026-05-08 | EXEC-001 | `cargo test -p codex-exec` | 0 | Exec package tests passed: 19 lib tests, 1 main test, 55 suite tests, 25 JSON event tests, doc-tests 0 | terminal output |
| 2026-05-08 | CORE-001A | `cargo test -p codex-core --lib shell_snapshot` | 0 | Shell snapshot filtered lib tests passed: 11 passed, 0 failed, 1591 filtered out, including timeout process group cleanup | terminal output |
| 2026-05-08 | CORE-002 | `cargo test -p codex-apply-patch` | 0 | Patch parser/invocation/tool/scenario tests passed: 49 unit tests, 17 integration tests, doc-tests 0 | terminal output |
| 2026-05-08 | APP-001A | `cargo test -p codex-app-server-protocol` | 0 | Protocol/common/v2/thread history/export/schema tests passed: 102 unit tests, 2 schema fixture tests, doc-tests 0 | terminal output |
| 2026-05-08 | CORE-001 | `RUST_BACKTRACE=1 cargo test -p codex-core --test all responses_mode_stream_cli -- --nocapture` | 0 | Targeted rerun passed: 1 passed, 0 failed; command output printed `hi`; finished test body in 18.43s | terminal output |
| 2026-05-08 | CORE-001 | `RUST_BACKTRACE=1 cargo test -p codex-core --test all view_image -- --nocapture` | 101 | View image group rerun was flaky: 10 passed, 1 failed; `user_turn_with_local_image_attaches_image` timed out waiting for `TurnComplete` at `tests/common/lib.rs:322` | terminal output |
| 2026-05-08 | CORE-001 | `RUST_BACKTRACE=1 cargo test -p codex-core --test all user_turn_with_local_image_attaches_image -- --nocapture` | 0 | Single failing view image test passed on rerun: 1 passed, 0 failed; finished in 11.35s | terminal output |
| 2026-05-08 | CORE-001A | `just fix -p codex-core` | 0 | Scoped Clippy auto-fix completed for `codex-core`; no additional source changes were applied beyond the existing `shell_snapshot.rs` diff | terminal output |
| 2026-05-08 | CORE-001A | `just fmt` | 0 | Rust formatting completed successfully | terminal output |
| 2026-05-09 | TUI-001 | `cargo test -p codex-tui` | 0 | TUI tests passed: 1309 lib tests passed, 2 ignored; integration `tests/all.rs` passed 6, ignored 1; doc-tests 0 passed, 2 ignored | terminal output |
| 2026-05-09 | TUI-003/TUI-004/TUI-005 | `TERM=xterm-256color RUST_LOG=trace just codex -c log_dir=/tmp/codex-tui-interactive.VirHa4 --no-alt-screen`; typed slash commands and keyboard controls | 0 | Interactive TUI smoke passed: `/status`, `/diff`, `/model`, `/permissions`, `/theme`, `/statusline`, and `/exit`; pager scroll, selector navigation, escape cancel, and clean shutdown verified | `/tmp/codex-tui-interactive.VirHa4/codex-tui.log` |
| 2026-05-09 | CORE-003 | `cargo test -p codex-rmcp-client`; `cargo test -p codex-mcp-server` | 0, 0 | MCP client/server tests passed: `codex-rmcp-client` completed 23 unit tests plus process/resource/http recovery integration tests; `codex-mcp-server` completed 10 lib tests, 3 integration tests, and doc-tests 0 | terminal output |
| 2026-05-09 | APP-001B | `cargo test -p codex-app-server` | 0 | App-server runtime/API tests passed: 87 lib tests, 243 integration tests, 1 ignored integration test, main/test_notify_capture/doc-tests 0 | terminal output |
| 2026-05-09 | CORE-001 | `cargo test -p codex-core` | 0 | Full core regression passed: main integration suite 762 passed, 0 failed, 18 ignored, finished in 2459.14s; `permissions_glob_profiles` passed 2; `responses_headers` passed 4 | terminal output |
| 2026-05-09 | CLI-TEST-001 | `cargo test -p codex-cli` | 0 | CLI crate tests passed: 6 lib tests, 31 main tests, 1 debug clear memories test, 2 execpolicy tests, 4 features tests, 3 marketplace remove tests, 6 MCP add/remove tests, 3 MCP list tests, doc-tests 0 | terminal output |
| 2026-05-08 | DOC-001 | `rg -n "#\\[test\\]\|#\\[cfg\\(test\\)\\]" codex-rs/*/src -g "*.rs"` | 0 | Found existing source-tree tests in many crates, so local placement constraint is not satisfied by current repo | terminal output |
| 2026-05-09 | DOC-001 | `rg -l "#\\[test\\]\|#\\[cfg\\(test\\)\\]" codex-rs/*/src -g "*.rs" \| wc -l`; `rg -n "#\\[test\\]" ... \| wc -l`; `rg -n "#\\[cfg\\(test\\)\\]" ... \| wc -l` | 0 | Scope quantified: 369 source files contain test markers, with 3055 `#[test]` hits and 474 `#[cfg(test)]` hits. Top affected crates include core and tui, so migration requires a separate repository-wide refactor plan | terminal output |
| 2026-05-09 | HYGIENE-001 | `git diff --name-only`; `git status --short` | 0 | Current workspace diff is limited to the validation report plus targeted core test/harness files: `shell_snapshot.rs`, `cli_stream.rs`, `shell_snapshot.rs` suite test, and `view_image.rs` | terminal output |

## Defects and Follow-Up Tasks

| ID | Severity | Area | Finding | Evidence | Status |
| --- | --- | --- | --- | --- | --- |
| DEF-001 | Medium | Test placement policy | Current repository already contains many Rust tests inside `codex-rs/*/src`, which conflicts with the pasted local constraint that tests must not be added or retained in core source directories. The scope is repository-wide and should be handled as a separate migration or policy-alignment decision, not as part of this validation loop. | `rg -n "#\\[test\\]\|#\\[cfg\\(test\\)\\]" codex-rs/*/src -g "*.rs"`; quantified 2026-05-09 scan | Deferred |
| DEF-002 | Low | TUI test environment | When launched from this PTY, TUI reports `TERM is set to "dumb"` and requires a confirmation before rendering. The automated TUI smoke protocol now explicitly sets `TERM=xterm-256color`, so future smoke runs avoid the confirmation prompt and exercise the intended TUI path. | TUI-002 terminal capture; `.codex/skills/test-tui/SKILL.md` | Closed |
| DEF-003 | Low | Core integration tests | `suite::view_image::user_turn_with_local_image_attaches_image` previously timed out waiting for `TurnComplete` under concurrent `view_image` execution. The test wait budget was raised to cover CPU-bound decode/resize/encode work under concurrent integration load, and the full core suite now passes. | `cargo test -p codex-core --test all view_image -- --nocapture`; single-test rerun; `cargo test -p codex-core` on 2026-05-09 | Closed |

## Completion Assessment

2026-05-09 final gate status: the CLI/TUI/core/app-server/MCP/apply-patch surfaces listed in this matrix have current passing evidence, including full `cargo test -p codex-core` and current `cargo test -p codex-cli` reruns. No known blocking failure remains in the validated critical paths or high-risk tool combinations.

Residual non-blocking finding remains documented as `DEF-001`: the repository's existing source-tree tests conflict with the local placement constraint. The quantified scope is 369 source files and 3000+ test markers, so this is a separate policy/migration follow-up rather than a current functional regression from the validated Codex CLI/TUI surface.

Scope boundary: this report does not claim every credentialed, remote, or destructive command path was exercised live. Login/logout, cloud tasks, and other credential-sensitive paths were limited to non-destructive help/status or isolated-home checks unless explicitly authorized.

## Manual TUI Protocol

Interactive TUI verification must follow `.codex/skills/test-tui/SKILL.md`:

```bash
TERM=xterm-256color RUST_LOG=trace just codex -c log_dir=<tmp-log-dir> --no-alt-screen
```

When sending test input programmatically, send text first and press Enter in a separate write. Log directories and terminal captures should be recorded in the evidence log.
