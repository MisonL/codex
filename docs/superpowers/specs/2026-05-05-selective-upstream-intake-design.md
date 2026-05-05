# Selective Upstream Intake Design

**日期**: 2026-05-05

**范围**: 当前 fork `main` 与 `openai/codex` `upstream/main` 的长期差异治理。

**当前基线**

- Fork head: `0106975ae1 fix cargo deny advisories`
- Upstream head: `70807730f5 tools: remove unused experimental list_dir tool (#21170)`
- Merge base: `25fa9741660dfc95fffb23b67e52af4f56e30187`
- 历史差距: upstream ahead 1835 commits, fork ahead 207 commits
- 树差距: 3587 files changed, 349658 insertions, 607077 deletions

## Control Contract

**Primary setpoint**

在保留 fork 二开路线的前提下，持续吸纳 OpenAI 上游的通用能力、安全修复、稳定性修复、协议兼容增强和低风险体验改进。

**Acceptance**

- Fork 差异化主线不被回退:
  - Web UI and `codex serve`
  - agent teams and multi-agent orchestration
  - hooks, skills, superpowers workflow
  - Anthropic and multi-provider runtime
  - hodexctl and fork release/install chain
  - conservative sandbox and network approval policy
- 每个 intake 批次都有明确 area、风险等级、验证命令、CI 状态和回滚条件。
- 共享协议、状态事实源、权限默认值只能通过显式设计打开，不能在普通同步中顺手改写。

**Guardrails**

- 不直接 `merge upstream/main` 到 `main`。
- 不把上游同功能实现静默覆盖 fork 本地实现。
- 不放宽默认网络、sandbox、approval、安全策略。
- 不默认开启高成本能力，例如 image generation 或 high detail image output。
- 不直接替换本地 app-server、plugin manager、thread storage 主链。

**Rollback trigger**

- 任何变更破坏当前 required GitHub Actions。
- 任何变更导致 `web/`, `codex serve`, agent teams, hooks 主链回退。
- 任何变更修改已有 app-server v2 wire shape 的兼容语义。
- 任何变更把权限策略从 fail-closed 改成隐式允许。
- 任何变更需要一次性重构两个以上共享边界。

## First Principles

本项目真正不能妥协的约束不是“和上游代码长得一样”，而是:

1. **用户可见能力不能回退**
   - Fork 的定位是 Rust-first OpenCode-like agent: Web UI, multi-model, multi-agent, long-running orchestration.
   - 上游同步必须减少维护误差，而不是抹掉 fork 产品差异。

2. **共享契约先于实现**
   - app-server protocol, thread state, plugin store, MCP tool schema, permission policy 都是共享事实源。
   - 这类边界只能通过 facade、可选字段、feature gate、兼容层演化。

3. **安全默认值优先**
   - 上游若放宽 YOLO/network/sandbox 默认值，本 fork 不默认吸纳。
   - 上游若修补 traversal、timeout、certificate、sandbox fallback 等收紧型安全项，应优先吸纳。

4. **复杂性只允许被显式转移**
   - ThreadStore 对齐会把复杂性转移到状态面。
   - Plugin/MCP 对齐会把复杂性转移到控制面 facade。
   - Provider runtime 对齐会把复杂性转移到 provider capability model。
   - 每次转移都必须记录收益、新成本和失效模式。

## Project Control Topology

**总体设计部**

- 维护 fork 差异化边界和 upstream intake 策略。
- 裁决同功能双实现: 保留本地、采用上游、或设计融合层。

**模块 owner 视角**

- `web/` and `codex-rs/serve`: Web UI 和服务入口。
- `codex-rs/core`: agent runtime, tools, provider, permissions, session.
- `codex-rs/app-server*`: v2 protocol, thread/session API, serve integration.
- `codex-rs/tui`: terminal UX and slash commands.
- `codex-rs/core-plugins`, `codex-rs/core-skills`, `codex-rs/codex-mcp`: plugins, skills, MCP.
- `.github/`, `scripts/`, `hodexctl`: release, CI, installer.

**冻结边界**

- app-server v2 wire compatibility.
- thread/session persistence semantics.
- plugin store and marketplace state semantics.
- permission, sandbox, network approval defaults.
- release asset names and installer behavior.

## Fork-Specific Feature Lines

| 功能线 | 当前二开价值 | 上游吸纳策略 |
| --- | --- | --- |
| Web UI / `codex serve` | Fork 核心产品形态 | 吸纳协议/事件修复，不替换 Web 主链 |
| Agent teams / multi-agent | 长任务编排和协作 | 吸纳底层事件、state、tooling 修复；同功能实现需裁决 |
| Hooks / skills / superpowers | Claude Code-like workflow | 吸纳 hook output、lifecycle、skills telemetry；拒绝无关内部 skill |
| Multi-provider / Anthropic | 多模型能力 | 吸纳 provider capability、model metadata、remote compaction 判断 |
| Plugin / Marketplace / MCP | 可扩展能力面 | 先建 facade，再吸纳 marketplace/MCP 互补能力 |
| Sandbox / Network policy | 安全收敛 | 只吸纳收紧型或修复型，不吸纳默认放宽 |
| Hodexctl / release | Fork 分发链 | 保留 fork release 入口，仅吸纳 CI/release hygiene |

## Intake Classification

**P0: 直接推进**

- Security fixes:
  - symlink traversal
  - DNS timeout blocking
  - custom CA login
  - sandbox fallback
  - dependency advisories
- Stability fixes:
  - hook output spill
  - websocket send timeout
  - PTY teardown
  - non-invasive CI flake fixes

**P1: 选择性吸纳**

- app-server/thread capabilities through `ThreadRepository` facade.
- plugin/MCP capabilities through `PluginFacade` and launcher adapter.
- provider/model metadata through `ProviderRuntime`.
- protocol additions using optional fields or new methods.

**P2: 排期吸纳**

- TUI keymap and statusline UX.
- PR/GitHub status display.
- low-risk command polish.
- CI/Bazel improvements that do not rewrite fork release flow.

**P3: 暂缓或不吸纳**

- image generation default-on.
- high detail image output default-on.
- YOLO network enforcement relaxations.
- owner nudge email and upstream business-specific APIs.
- pure large refactors with no user-visible benefit.

## Complexity Transfer Ledger

| 主题 | 复杂性原位置 | 新位置 | 收益 | 新成本 | 失效模式 |
| --- | --- | --- | --- | --- | --- |
| ThreadStore 对齐 | app-server direct thread handling | `ThreadRepository` facade | 状态 API 可分批对齐 | facade 需要维护映射 | facade 与底层状态漂移 |
| Provider runtime 对齐 | scattered provider branches | `ProviderRuntime` capability model | provider 行为集中治理 | 过渡期双入口 | 部分路径绕过 runtime |
| Plugin/MCP 对齐 | manager/store direct use | `PluginFacade` and launcher adapter | 不替换主链也能吸纳能力 | 状态同步复杂 | UI 与 CLI 看到不同状态 |
| Policy 对齐 | hard-coded defaults | explicit profiles and gates | 可提供上游能力但保守默认 | 配置矩阵增多 | 组合策略语义不清 |
| Release 对齐 | fork scripts and GitHub Actions | release hygiene layer | 保留 fork 分发同时吸纳稳定性 | CI 维护成本 | asset 命名或安装路径漂移 |

## Verification Model

每个 intake 批次必须满足:

- `git diff --name-only` 只包含该批次边界内文件。
- 最小相关测试通过。
- 改 Rust 后在 `codex-rs` 跑 `just fmt`。
- 改 dependency 后跑 `just bazel-lock-update` and `just bazel-lock-check`。
- 改 protocol/schema 后跑对应 schema generator and protocol tests。
- 推送后 babysit GitHub Actions 到 required checks 全绿。

## Non-Goals

- 不把 fork 变回 OpenAI 原版。
- 不一次性解决全部 1835 upstream commits。
- 不把旧审计文档中的暂缓项全部自动转成执行项。
- 不在没有 owner 裁决时合并同功能双实现。
