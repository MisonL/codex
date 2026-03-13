# Agent Teams v2: 组织层级 + 团队内 Mesh 协作 (设计提案)

日期: 2026-03-13

本文提出 Codex "Agent Teams" 的下一迭代方案, 目标包括:

- 在单个 team 内启用点对点协作 (不再只是 lead <-> member).
- 支持将单个任务直接分派给多个 agent, 无需 leader 预先拆分工作边界.
- 引入清晰的跨 team 通信边界: 跨 team 消息仅允许 team leader 发送.
- 建立显式层级: 面向用户的 "President" (主线 agent thread) 通过各 team leader 管理多个 team.
- 通过强化通用 agent-to-agent 工具 (例如 `send_input`) 的授权, 使边界可被代码强制执行, 阻止 teammate 绕过 org/team 策略.

该提案刻意保持增量式, 复用 `docs/agent-teams.md` 中已经存在的 durable inbox + persisted tasks 原语.

## 0.1 固定决策 (Pinned)

本文为该方案的固定版本. 以下约束不是可选项:

1. **持久化优先是唯一真相**
   - team/org 的成员关系、角色与策略必须从 `$CODEX_HOME` 下持久化的控制面状态读取.
   - 允许存在内存 registry (in-memory registry) 作为缓存, 但不得作为授权真相.

1. **团队内消息是 mesh (范围限制在 `teamId`)**
   - 同一 `teamId` 内, 任意成员可以向任意其他成员 (以及 leaders) 发送消息.
   - President 也可以作为团队 owner/监管者 (owner/supervisor) 向任何 team 的任何人发送消息.

1. **单任务多 assignee 是一等能力**
   - leader 可以将 1 个任务分派给多个 assignee (无需预拆分).
   - 每个 assignee 的状态需要持久化, 并驱动任务完成语义.

1. **跨 team 通信仅 leader 可用 (范围限制在 `orgId`)**
   - 只有 team leaders (以及 President) 可以发送跨 team 消息, 且只能通过 `org_*` 工具.
   - team members 不允许直接向其他 team 发送消息.

1. **边界约束必须在代码层执行 (不是 prompt 约定)**
   - 能绕过策略的通用 agent-to-agent 工具 (至少: `send_input`, `close_agent`, `resume_agent`) 必须对 teammate thread 做限制.
   - teammate 必须使用 `team_*` / `org_*` 工具进行沟通与协作.

1. **默认隔离, 通过 artifact 显式共享**
   - 大体量或需要持久保留的产物必须通过显式 artifact 共享 (不要靠在消息里复制大段内容).

## 0.2 术语与命名

当前 v1 实现里 "lead" 指代 "spawn team 的那个 thread". v2 引入了委派领导, 因此必须消歧:

- **总裁线程 (President thread)**: 面向用户的主线 agent thread (root thread), 负责监管 org.
- **团队所有者 (Team owner)**: 负责团队生命周期 (spawn/close/cleanup) 的 thread. 为兼容性, 在 team config 中持久化为 `leadThreadId`, 默认应被视作 President.
- **团队负责人 (Team leader)**: 团队内的委派 leader, 持久化在 `leaders[]`. leaders 拥有团队控制面权限 (任务、跨 team 沟通、受策略约束的 broadcast 等).
- **团队成员 (Team member)**: 团队内的普通成员 (非 leader).

固定规则:

- `leadThreadId` 表示 **owner/president**, 而不是委派的 team leader.

## 0.3 命名约定

为避免歧义, 本文对不同层采用不同命名约定:

1. **工具 API (Tool APIs)** (`team_*`, `org_*`):
   - 工具参数与工具输出使用 `snake_case` (与现有 v1 工具保持一致).

1. **持久化的控制面状态** (位于 `$CODEX_HOME` 下的文件):
   - JSON/JSONL 字段使用 `camelCase` (与现有 `TeamInboxEntry` 的持久化格式保持一致).

1. **Swarm 信封 (envelope) 元数据**:
   - 字段使用 `camelCase` (`swarmRunId`, `teamId`, `taskId`, `sequence`, `causalParent`).

## 0.4 立场定位 (基于现有 Swarm 架构)

本提案是对 "Agent Teams" 的聚焦增强, 处于更大的多 agent 控制面方向之内, 该方向见:

- `docs/plans/2026-03-06-codex-swarm-architecture.md`

其核心原则保持一致:

- 补一个轻量控制面层, 尽量少改数据面, 不重写执行面.

重要对齐点:

- `2026-03-06` 明确避免 **全局** 点对点 mesh. 本提案通过将 "mesh" 严格限制在单个 `team_id` 范围 (durable inbox + 尽力实时投递), 并通过 `org_*` 工具强制 **leader-only** 跨 team 通信, 来保持这一约束.

本文与更早设计的映射关系:

- 控制面对象:
  - `Org` (President + team leaders) 是提议 `SwarmRun` (kind: `swarm`) 的一个轻量切片.
  - `Team` 仍复用现有 `team_id` 范围的工作流 (kind: `team`), 但补齐缺失语义 (团队内 mesh 消息、leader 委派、多 assignee 任务).
- 任务模型:
  - 多 assignee 任务扩展了早期 `TaskSpec` 的方向, 通过 assignee 级状态跟踪实现, 而不要求 leader 预拆分.
- 可观测性与回放:
  - team/org 消息与任务状态迁移应携带稳定 envelope (`swarmRunId`, `teamId`, `taskId`, `sequence`, `causalParent`), 以保证可审计与可回放.
- 记忆模型:
  - 线程工作记忆默认隔离; 当内容较大或需要持久化时, 通过显式 artifact 发布共享.

## 0.5 控制合同 (CSE) (Pinned)

本节用于让设计可验证, 降低 "看起来没问题" 的漂移.

- **主要设定值:** 团队内点对点协作 + 多 assignee 任务, 同时跨 team 通信保持 leader-only 且不可绕行.
- **验收标准 (必须可在代码中测试):**
  - 任意 teammate 可以在同一 `team_id` 内对任意其他 teammate 执行 `team_message`, 并且会追加一条持久化 inbox 记录.
  - 非 leader 的跨 team 消息必须被拒绝 (仅 leaders/President 可通过 `org_*`).
  - 当目标 thread 已注册到 org/team 时, `send_input` / `close_agent` / `resume_agent` 不得绕过 org/team 策略边界.
  - 多 assignee 任务必须遵守 `claimMode` + `completionMode`, 且 `leader_approves` 必须有显式批准执行器.
- **约束:** durable-first; `$CODEX_HOME` 下的持久化状态是授权真相; 禁止静默回退路径.
- **传感器/证据:** `$CODEX_HOME/teams/*/config.json`, `inbox/*.jsonl`, `tasks/*.json`, `events.jsonl` (以及对应 lock 文件).

## 1. 目标

1. 团队成员可以通过 team-scoped 工具直接互相协作.
1. team leader 可以把 1 个任务分派给多个成员, 由成员自组织拆分.
1. 跨 team 通信被约束为 leaders (以及可选的 President) 才能执行, 且只有一个受控入口/出口.
1. 面向用户的主线 agent 作为 "President", 负责监督各 team leaders 与整体进展.
1. 所有消息仍保持 durable-first (先持久化, 再尽力实时投递).

## 2. 非目标

1. 完整的 "嵌套团队": 允许队员自由 spawn 更多 teams/agents 且不受治理 (后续可以加配额/治理再做).
1. 分布式、多进程控制面. 本提案仍保持类似 v1 的 in-process + 文件持久化.
1. 新的聊天 UI. 本提案核心交付是语义与工具; UI 改进属于后续跟进.

## 3. 当前状态 (v1) 与缺口

在 `docs/agent-teams.md` 中, 当前 Agent Teams 工作流已提供:

- `spawn_team` / `wait_team` / `close_team` / `team_cleanup`
- `$CODEX_HOME/teams/<team_id>/inbox/<thread_id>.jsonl` 形式的 per-thread durable inbox
- `$CODEX_HOME/tasks/<team_id>/*.json` 形式的初始任务持久化
- 任务工具: `team_task_list`, `team_task_claim(_next)`, `team_task_complete`
- lead 驱动的消息: `team_message`, `team_broadcast` (lead -> member) 与 `team_ask_lead` (member -> lead)

面向 "真实团队" 的缺口:

1. 团队内消息实际呈现为围绕 lead 的星型结构.
1. 任务是 1:1 分派, 迫使 lead 预先拆分职责边界.
1. 缺少一等的跨 team 边界. 如果知道 thread id, teammate 可能用通用工具 (`send_input`, `close_agent`, `resume_agent`) 绕过预期的 leader-only 跨 team 边界.
1. leadership 是隐式的 (spawn team 的 thread), 不符合常见 "团队有一个 leader agent" 的心智模型.

## 4. 提议模型

### 4.1 实体

1. **组织 (Org)**
1. **团队 (Team)**
1. **Agent 线程 (Agent thread)** (现有 `ThreadId` / "agent_id")

### 4.2 角色

1. **总裁 (President)**
   - 面向用户的主线 agent thread.
   - 拥有 org, 监督所有 teams.
   - 负责创建 teams 并任命 team leaders.

1. **团队负责人 (Team Leader)**
   - 团队内的 agent thread, 且只属于一个 operational team.
   - 拥有管理该 team 的权限 (消息策略、任务分派、向 President 汇报状态).

1. **团队成员 (Team Member)**
   - 团队内普通 agent thread.
   - 可以在团队内与 peers 直接协作.

### 4.3 信封 (Swarm 风格元数据)

为对齐 `2026-03-06-codex-swarm-architecture.md` 的 `swarm envelope` 方向, 下列元数据应至少出现在持久化状态中, 并尽可能出现在 emitted events 中:

- `swarmRunId`: Org id (President 管理的 "swarm run" 范围; 在 team/org 状态中持久化为 `orgId`)
- `teamId`: team id
- `agentId`: sender/receiver thread id
- `taskId`: 可选; 当消息或状态迁移与某个 task 绑定时设置
- `sequence`: 按 scope 单调递增, 用于确定性回放
  - Team scope: 对 `(orgId, teamId)` 单调递增
  - Org scope: 对 `orgId` 单调递增
- `causalParent`: 可选; 因果链指针 (例如 "该消息响应了 task X 的 claim")

本提案不要求修改现有 `item` 模型; 它要求:

1. 在持久化记录中补齐稳定标识, 让控制面可以在不解析模型输出的前提下被审计与回放.
1. 引入 append-only 控制面事件日志, 以便确定性生成 `sequence`.

固定的持久化增量:

- team 事件日志: `$CODEX_HOME/teams/<team_id>/events.jsonl`
- org 事件日志: `$CODEX_HOME/orgs/<org_id>/events.jsonl`

task JSON 仍然是 "最新快照". 事件日志是回放/审计真相.

### 4.4 持久化 Schema (Pinned)

固定设计依赖持久化的控制面状态. v2 的最小 schema 如下:

兼容性规则:

- v2 reader 必须继续可解析 v1 持久化文件 (缺少 v2 新字段), 通过将新字段视作可选并应用安全默认值实现.

#### 4.4.1 Team config (`$CODEX_HOME/teams/<team_id>/config.json`)

```json
{
  "schemaVersion": 2,
  "teamName": "demo-team",
  "orgId": "org-123",
  "leadThreadId": "thread-president",
  "leaders": ["thread-leader-a"],
  "broadcastPolicy": "leaders_only",
  "createdAt": 1739988000,
  "members": [{ "name": "alice", "agentId": "thread-alice", "agentType": "develop" }]
}
```

#### 4.4.2 Org config (`$CODEX_HOME/orgs/<org_id>/config.json`)

```json
{
  "schemaVersion": 2,
  "orgId": "org-123",
  "presidentThreadId": "thread-president",
  "createdAt": 1739988000,
  "teams": [{ "teamId": "demo-team", "leaders": ["thread-leader-a"] }]
}
```

#### 4.4.3 Team inbox entry (`$CODEX_HOME/teams/<team_id>/inbox/<thread_id>.jsonl`)

```json
{
  "id": "msg-1",
  "createdAt": 1739988001,
  "orgId": "org-123",
  "teamId": "demo-team",
  "fromThreadId": "thread-alice",
  "fromName": "alice",
  "fromRole": "member",
  "toThreadId": "thread-bob",
  "taskId": "task-1",
  "sequence": 42,
  "causalParent": "taskClaim:task-1:thread-alice",
  "inputItems": [],
  "prompt": "..."
}
```

#### 4.4.4 Team task snapshot (`$CODEX_HOME/tasks/<team_id>/<task_id>.json`)

```json
{
  "schemaVersion": 2,
  "id": "task-1",
  "title": "Implement feature X",
  "state": "claimed",
  "dependsOn": [],
  "assignees": [{ "name": "alice", "agentId": "thread-alice" }],
  "assigneeState": { "thread-alice": "claimed" },
  "claimMode": "exclusive",
  "completionMode": "any_assignee",
  "leaseUntil": null,
  "approvedAt": null,
  "approvedByAgentId": null,
  "updatedAt": 1739988002
}
```

说明:

- 当 `completionMode == leader_approves` 时, `approvedAt` / `approvedByAgentId` 必须被写入; 其他模式应保持为 `null`.
- `state` 是派生字段; `assigneeState` 才是完成语义的权威真相.

#### 4.4.5 控制面事件日志 entry (`$CODEX_HOME/teams/<team_id>/events.jsonl`)

```json
{
  "id": "ev-1",
  "createdAt": 1739988001,
  "orgId": "org-123",
  "teamId": "demo-team",
  "sequence": 42,
  "kind": "team.message.appended",
  "actorThreadId": "thread-alice",
  "taskId": null,
  "causalParent": null,
  "payload": {}
}
```

#### 4.4.6 Org inbox entry (`$CODEX_HOME/orgs/<org_id>/inbox/<thread_id>.jsonl`)

```json
{
  "id": "msg-1",
  "createdAt": 1739988001,
  "orgId": "org-123",
  "fromThreadId": "thread-leader-a",
  "fromTeamId": "team-a",
  "fromName": "lead-a",
  "fromRole": "leader",
  "toThreadId": "thread-leader-b",
  "toTeamId": "team-b",
  "sequence": 7,
  "causalParent": null,
  "inputItems": [],
  "prompt": "..."
}
```

#### 4.4.7 Org 控制面事件日志 entry (`$CODEX_HOME/orgs/<org_id>/events.jsonl`)

```json
{
  "id": "ev-1",
  "createdAt": 1739988001,
  "orgId": "org-123",
  "sequence": 7,
  "kind": "org.leader.message.appended",
  "actorThreadId": "thread-leader-a",
  "causalParent": null,
  "payload": { "fromTeamId": "team-a", "toTeamId": "team-b" }
}
```

### 4.5 控制面引导工具 (new)

v2 模型引入了新的持久化字段 (`leaders[]`, `broadcastPolicy`, `orgId`) 与新的持久化资源 (`$CODEX_HOME/orgs/...`). 这要求 President 拥有明确的控制面执行器 (tools), 而不是靠手工编辑文件.

#### 4.5.1 `team_update_config` (new, president-only)

以受控方式更新团队持久化配置 (`$CODEX_HOME/teams/<team_id>/config.json`).

- Inputs (snake_case):
  - `team_id`
  - `leaders` (可选; 成员名或 thread id, 必须校验属于 `members[]`)
  - `broadcast_policy` (可选)
  - `org_id` (可选; 将 team attach/detach 到某个 org)
- Outputs:
  - 更新后的 team 元信息 (至少包含: `team_id`, `org_id`, `leaders`, `broadcast_policy`)

必备属性:

- 授权: 仅 President thread (team owner / `leadThreadId`) 可调用.
- 写入必须原子化, 且需要向 `$CODEX_HOME/teams/<team_id>/events.jsonl` 追加控制面事件.

#### 4.5.2 `team_set_leaders` (可选便利工具)

`team_update_config` 的便利封装, 仅修改 `leaders`.

#### 4.5.3 `org_create` (new, president-only)

在 `$CODEX_HOME/orgs/<org_id>/config.json` 创建 org 配置并初始化:

- `presidentThreadId = caller`
- 空 `teams[]`
- org event log 目录与 inbox 目录

#### 4.5.4 `org_register_team` (new, president-only)

将 team 挂接到 org 并维护引用一致性:

- Writes:
  - 更新 `orgs/<org_id>/config.json`, 写入 team 与其 leaders
  - 更新 `teams/<team_id>/config.json`, 写入 `orgId = <org_id>`
- 必须幂等, 且需要分别追加 org/team 的控制面事件.

### 4.6 幂等性、锁与事件覆盖 (Pinned)

为保证 durable-first 语义在并发与重启场景下正确:

- **原子写:** 对 `config.json` 与 task snapshot 的更新必须采用 write-temp-then-rename (禁止产生半截 JSON).
- **互斥锁:** 所有 JSONL append 面必须使用 per-file lock (v1 已有 inbox lock; events logs 也必须加锁).
- **sequence 分配:** 在持有 해당 scope (team/org) 的 `events.lock` 时分配 `sequence`, 然后将其写入对应的 inbox/event entry.
- **幂等:** task-level 完成迁移与 hooks 必须只触发一次; 重复调用必须显式报错或显式 no-op, 但不得静默 "半成功".

最小事件覆盖 (仅当状态实际变化时才向 `events.jsonl` 追加):

- Team scope:
  - `team.config.updated`
  - `team.message.appended`
  - `team.task.created`
  - `team.task.assignees.updated`
  - `team.task.assignee.claimed`
  - `team.task.assignee.completed`
  - `team.task.approved`
- Org scope:
  - `org.created`
  - `org.team.registered`
  - `org.leader.message.appended`

## 5. 团队内 Mesh 协作

### 5.1 设计原则

如果两个 agent 同属同一个 `team_id`, 他们应能通过 team-scoped 工具沟通, 且该工具必须:

1. 校验成员关系.
1. 将消息持久化写入 receiver 的 inbox (durable-first).
1. 尽力进行实时投递 (best-effort).

### 5.2 工具变更

#### 5.2.1 `team_current` (new)

当前 team-scoped 工具都要求显式传入 `team_id`. 要实现真实的 mesh 协作, teammate 必须能在不依赖 out-of-band 的前提下发现自己的 `team_id`.

`team_current` 返回调用方的当前 team 上下文 (不在 team 内则返回空):

- `team_id`
- `org_id` (可选)
- `role`: `"member" | "leader"` (或空)
- `president_thread_id` (team owner; 为兼容性在配置中持久化为 `leadThreadId`)

固定要求:

- v2 必须提供 `team_current` 或等价的自动注入机制, 使 teammate 不需要 President 人工粘贴 team id 也能调用 `team_*` 工具.

#### 5.2.2 `team_info` (new)

返回团队自组织所需的元信息:

- `team_id`, `org_id` (未注册到 org 时可为空)
- `president_thread_id` (team owner; 为兼容性持久化为 `leadThreadId`)
- `leaders` (thread ids 与 names)
- `members` (thread ids、names、可选 agent roles)
- 可选: 消息策略 (见下文)

这能避免 out-of-band 共享 agent ids, 并让团队内成员自发现 peers.

授权:

- 仅 team 成员/leader 或 President thread (team owner) 可调用.

#### 5.2.3 `team_message` (行为变更)

v1 当前语义基本是 "lead -> member". v2 语义:

1. 任意 team member 或 leader 均可调用 `team_message`.
1. sender 与 receiver 必须同属同一个 `team_id`.
1. 授权与成员查找必须基于持久化 team config (`$CODEX_HOME/teams/<team_id>/config.json`), 不得依赖以 spawning thread 为 key 的 in-memory registry.
1. 持久化 inbox entry (JSONL 用 `camelCase`) 应包含:
   - `fromThreadId`
   - `fromName` (从 team config 解析; 当适用时可为 `"president"`)
   - `fromRole` (`member` / `leader` / `president`)
   - `teamId`
   - `orgId` (可选)
   - `sequence` / `causalParent` (当可用时)
   - `taskId` (可选; 当与某个任务绑定时设置)

这将 team 变为有边界的 mesh, 且不暴露跨 team 消息能力.

#### 5.2.4 `team_broadcast` (策略 + 行为变更)

broadcast 很有用, 但也容易变噪声. v2 提议在 team config 中加入策略开关:

- `broadcastPolicy: "leaders_only" | "all_members"`

默认: `leaders_only`.

若为 `all_members`, 任意成员可 broadcast; 若为 `leaders_only`, 非 leader 必须使用 `team_message` 或通过 leader 协调.

#### 5.2.5 `team_ask_lead` (行为变更)

v1 中 `team_ask_lead` 会向 spawning thread ("lead") 发消息. v2 中 "lead" 应优先解析为委派 leaders:

1. 当 `leaders[]` 非空时, `team_ask_lead` 投递给所有 team leaders.
1. 否则, 投递给 `leadThreadId` (President / team owner).
1. 仍保持 durable-first: 先写 inbox, 再尽力实时投递.

### 5.3 推荐协作协议 (prompt 级)

工具只提供通信能力; 协作质量依赖协作协议. 当某个任务被分配给多个 agent 时, 注入标准 kickoff 信息:

1. 每个 assignee 用 2-4 个要点说明自己的计划与预期产物.
1. assignees 通过 `team_message` 协商边界与依赖关系.
1. 若出现冲突或歧义, 升级给 team leader 裁决.

这能让团队内保持自治, 而不要求 leader 在分派时做微观拆分.

## 6. 多 assignee 任务

### 6.1 问题

leader 应能把 1 个任务直接分派给多个 agent, 期望他们自己协调与拆分, 而不是让 leader 先拆成 N 个小任务.

### 6.2 任务模型 v2 (schema 概念)

将单一 `assignee` 替换为 `assignees`:

- `assignees: [{ name, agentId }]`
- `assigneeState: { "<agentId>": "pending" | "claimed" | "completed" }`
- `claimMode: "shared" | "exclusive"`
- `completionMode: "all_assignees" | "any_assignee" | "leader_approves"`
- `leaseUntil`: 可选; 对齐更早的 `TaskSpec.lease_until` / `Lease` 概念, 用于长任务所有权
- `artifacts`: 可选; assignees 发布的 artifact 引用 (见下文)

默认值:

- 当 `assignees.len() > 1` 时默认 `claimMode: "shared"`, 否则 `exclusive`
- 当 `assignees.len() > 1` 时默认 `completionMode: "all_assignees"`, 否则 `any_assignee`

固定状态规则:

- 为列表/UI 方便可持久化派生字段 `state`, 但必须把 `assigneeState` 视为完成语义的权威真相.

固定不变量:

- `assignees[].agentId` 在同一 task 内必须唯一.
- 当 `assignees.len() > 1` 且 `completionMode == all_assignees` 时必须要求 `claimMode == shared` (否则 task 可能变成不可完成).
- `assignees` 是权威的 "当前分派集合"; `assigneeState` 可以包含历史 assignee 用于审计, 但当 assignee 被移除后不得阻塞 `all_assignees` 的完成判定.

### 6.2.1 完成语义 (Pinned)

`completionMode` 决定 task 在 task-level 上何时被视为 "completed":

1. `all_assignees`
   - 当所有当前 assignees 的 `assigneeState[agentId] == "completed"` 时满足 task-level 完成.
   - 通过 `team_task_assign` 或成员移除来删除 assignee 时, 不得让任务变得不可完成: 被移除的 assignees 必须从 "当前 assignees 集合" 中排除.

1. `any_assignee`
   - 当任意 assignee 变为 `"completed"` 时满足 task-level 完成.
   - 其他 assignees 仍可后续完成以便审计/credit, 但 task-level 的完成迁移 (hooks/events) 必须幂等且只触发一次.
   - task-level 完成后, 新的 claim 必须被拒绝 (避免重复劳动).

1. `leader_approves`
   - assignees 标记自己的 `assigneeState` 为 `"completed"`.
   - 仅当 leader (或 President) 显式批准后, task-level 才满足完成.

固定要求:

- 当 `completionMode == leader_approves` 时, v2 必须提供显式批准执行器 (例如 `team_task_approve`), 禁止把 "批准" 混用到 "complete" 上.

### 6.3 工具变更

#### 6.3.1 `team_task_create` (new)

在 `spawn_team` 之后创建任务:

- `team_id`
- `title`
- `description` (可选)
- `assignees` (一个或多个成员名或 thread id)
- `dependencies` (可选)
- `claim_mode` / `completion_mode` (可选)
- `kickoff: true|false` (可选, 默认 true): 为 true 时, 自动向所有 assignees 发送 kickoff 信息 (协作协议).

授权:

- 仅 team leaders 或 President thread (team owner) 可调用.

#### 6.3.2 `team_task_claim` / `team_task_claim_next` (行为变更)

对 `shared` 任务:

- claim 会将调用方的 `assigneeState` 标记为 `claimed`, 但不会阻塞其他 assignees.
- claim 要求调用方必须在 `assignees` 中 (第一阶段不支持 "代领").
- `team_task_claim_next` 应选择下一条可 claim 的 pending task, 满足:
  - 调用方在 `assignees` 中, 且
  - `assigneeState[caller] == "pending"`, 且
  - dependencies 已满足.

对 `exclusive` 任务:

- 保持现有行为 (只能有一个 claim).

#### 6.3.3 `team_task_complete` (行为变更)

对 `shared` 任务:

- complete 会将调用方的 `assigneeState` 标记为 `completed`.
- complete 要求调用方必须在 `assignees` 中 (完成归因于 assignee).
- task 何时被视为 completed 由 `completionMode` 决定.

对 `exclusive` 任务:

- 保持现有行为.

#### 6.3.4 `team_task_assign` (new)

允许 leaders 在创建后增删 assignees.

授权:

- 仅 team leaders 或 President thread (team owner) 可调用.

#### 6.3.5 `team_task_approve` (new, `leader_approves` 必需)

当 `completionMode == leader_approves` 时, 用于批准任务:

- 仅 team leaders 或 President 可调用.
- 当需要批准时, 通过批准将 task 迁移到 completed.

### 6.4 为什么这能解决 "leader 不必预拆分"

1. leader 将单个 shared task 分派给多个 agent.
1. agents 在 team 内通过 mesh 消息协商边界并自组织拆分.
1. 任务模型通过 per-assignee 状态跟踪进展, 无需拆成 N 个任务.

## 7. 跨 team 通信: 仅 leaders

### 7.1 设计原则

team members 不应直接向其他 teams 发消息. 跨 team 通信需要:

1. 在需要时可用.
1. 只有一个受控入口/出口.
1. 限制为 team leaders (以及可选的 President).

### 7.2 组织层 (new persisted concept)

引入 org registry, 持久化在 `$CODEX_HOME/orgs/<org_id>/...`:

- `config.json`: President thread id, teams 列表, 每个 team 的 leader thread ids
- org 范围的 durable inbox (对每个 leader), 语义与 team inbox 相同 (durable-first)

org 层即为边界强制执行的机制基础.

### 7.3 Org 工具 (new)

1. `org_info`: 列出 org 内 teams 与 leaders.
1. `org_leader_message`: leader -> leader 消息, 依据 org config 校验.
1. `org_inbox_pop` / `org_inbox_ack`: 读取与 ack org 范围消息.

授权:

- `org_leader_message` 仅允许:
  - President thread, 或
  - org 内任一 team 的 leader thread
- receiver 必须是:
  - org 内另一个 team 的 leader, 或
  - President thread

### 7.4 边界强制执行 (required)

要让 "跨 team 仅 leaders" 可被强制执行 (而不是 prompt 约定), 必须阻止通过通用工具绕行:

1. 限制 teammate threads 使用通用 agent-to-agent 工具 (至少: `send_input`, `close_agent`, `resume_agent`).
1. 确保 teammates 拥有可用的替代表面:
   - team 内: `team_message` / `team_broadcast` (策略约束) + inbox 工具.
   - 跨 team: 仅 team leaders 使用 `org_leader_message`, 然后通过 `team_*` 转发给成员.

固定策略:

- 通用工具不得绕过 org/team 策略边界.
- 授权检查必须使用 `$CODEX_HOME` 下持久化的 org/team 状态, 不得依赖 per-session 的 in-memory registry.
- 为避免每次工具调用都全量扫描, 可以构建 threadId -> teamId 的缓存索引, 但缓存必须从持久化状态派生, 且重启后仍安全 (持久化状态仍是唯一真相).

强制要求的 hardening 行为:

1. **通用工具必须按 target 做授权 (target-based authorization)**
   - 对 `send_input` / `close_agent` / `resume_agent`, 授权必须考虑 **target thread**.
   - 如果 target thread 已注册到任一 team/org, 该工具必须执行与 `team_*` / `org_*` 等价的边界规则, 无论 caller 是否在 in-memory 上下文里被认为 "在 team 内".

1. **teammate threads 禁止嵌套 spawn team/agent**
   - team members (包括委派 leaders) 不得通过 `spawn_agent` / `spawn_team` 作为绕行路径.
   - 若未来支持嵌套 spawn, 必须显式并保留 org/team scope (禁止非 President threads 创建 "脱离治理" 的 agents).

## 8. 团队内领导委派

为对齐 "团队有一个 leader agent" 的心智模型:

1. 在 team config 中加入 `leaders: [thread_id]`.
1. 将 team leaders 视为以下控制面动作的特权 actor:
   - broadcast 策略
   - task create/assign
   - 向 President 汇报状态

spawning thread (President) 仍是 owner, 负责 cleanup 与审计, 但不需要微观管理团队日常运作.

## 9. Artifacts (显式共享, 非共享上下文)

为对齐 "默认隔离, 通过 artifact 共享" 的方向:

1. team 消息应尽量短, 以协作为主.
1. 非 trivial 的产物 (计划、总结、patch 集、评审、表格等) 应通过显式 artifact 发布并以 id 引用.

后续可跟进的控制面工具 (第一阶段非必需), 用于让 artifact 更易用:

- `team_artifact_publish`: 在 team scope 创建 artifact
- `team_artifact_read`: 读取 artifact
- `team_artifact_list`: 列出某 task/team 的 artifacts

这些与 `2026-03-06-codex-swarm-architecture.md` 中的 `Artifact` 对象方向一致.

## 10. 端到端示例流程

### 10.1 President 创建两个 teams 并任命 leaders

1. `spawn_team` 创建 Team A, 成员包含 `lead-a`.
1. `spawn_team` 创建 Team B, 成员包含 `lead-b`.
1. President 更新每个 team config, 将 `lead-a` 与 `lead-b` 标记为 leaders (机制: `team_set_leaders` / `team_update_config`, 写入 `$CODEX_HOME/teams/<team_id>/config.json`).
1. President 创建 org 并注册 Team A/B 及其 leaders.

### 10.2 Team A leader 将 1 个任务分派给多个成员

1. 调用 `team_task_create`, 参数 `assignees: ["alice", "bob", "charlie"]`, `kickoff: true`.
1. 每个 assignee claim 任务 (`team_task_claim`), 发布自己的计划并自组织拆分.
1. 完成后各自标记完成 (`team_task_complete`).

### 10.3 Team A leader 与 Team B leader 沟通

1. `org_leader_message` 从 `lead-a` 发给 `lead-b`.
1. `lead-b` 通过 `team_broadcast` 或 `team_message` 将关键信息转达给 Team B 成员.

## 11. TUI UX (用户反馈 + 控制界面)

本节描述 Codex TUI 如何让多 team 工作可见、可控, 同时避免刷屏.

设计目标:

1. **分层信息:** 先总览, 需要时再深钻.
1. **默认低噪声:** 避免把所有内部消息流进主 transcript.
1. **快速导航:** 在 President / leaders / members 之间切换尽量只需 1-2 个动作.
1. **持久化状态为真相:** 面板读 `$CODEX_HOME` 的控制面状态 (而不是解析模型输出).

### 11.1 信息架构

TUI 应呈现清晰层级:

- Org (President scope) -> Teams -> Agents -> Tasks -> Artifacts

其中:

- 主聊天线程是 **总裁 (President)**.
- 每个 team 有一个或多个 **leaders** (agent threads).
- 成员通过 team 内 mesh 消息与 shared tasks 协作.

### 11.2 入口点 (Commands)

TUI 已有 slash commands 与选择视图. 增加 team/org 入口:

- `/org`: 打开 Org 仪表盘 (teams + leaders + 汇总)
- `/org inbox`: 展示 President 的 org inbox (leader updates, 跨 team 协调)
- `/team`: 打开当前 team 仪表盘 (leader/member threads 使用)
- `/team tasks`: 打开任务看板 (当前 team)
- `/team inbox`: 展示当前线程的 team inbox
- `/teams`: 列出 teams 并跳转到某个 team leader thread ("watch leader")

上述命令仅在协作能力启用时可用 (与 `/collab`、`/agent` 的 gating 保持一致).

### 11.3 仪表盘与 Overlays

复用现有全屏 overlay/pager 交互模式 (类似 transcript overlay):

1. **Org 仪表盘 (President)**
   - 表格行: `team name/id`, `leader`, `status`, `members`, `tasks (pending/claimed/completed)`, `unread (org inbox)`
   - 动作:
     - Watch leader thread
     - Message leader (org-scoped)
     - Open team summary (read-only)

1. **Team 仪表盘 (Leader/Member)**
   - 区域:
     - Members 列表 (可选状态点)
     - Task 汇总
     - Inbox 汇总 (unread)
     - 最近 artifacts (按 task)
   - 动作:
     - Watch member thread
     - Open task board
     - Open inbox

1. **任务看板 (Task Board)**
   - 按状态分组: `Pending`, `Claimed`, `Completed`
   - 多 assignee 任务: 显示每个 assignee 的子状态 (claimed/completed) 与完成模式 ("any/all/leader approves")
   - 动作:
     - Claim (self)
     - Complete (self)
     - Open artifacts for task
     - (Leader only) assign/unassign members

1. **Inbox Viewer**
   - 基于 `team_inbox_pop/ack` 与 `org_inbox_pop/ack` (cursor-based)
   - 展示字段:
     - `from` (name + role)
     - `team/org` 上下文
     - `taskId` (若存在)
     - prompt 预览
   - 支持 paging、search, 并提供显式 "Ack all visible" 动作

### 11.4 Transcript: 只显示摘要, 不展示全量内部聊天

主 transcript 应包含:

- 高层编排事件 (spawn/wait/close) (已通过 `Collab*` events 存在).
- 任务生命周期摘要:
  - 任务创建 (team + assignees)
  - 任务完成 (谁完成, task 是否已达到 task-level 完成)
- leader -> President updates (org inbox), 以短 "status cards" 形式摘要展示.

默认情况下, 主 transcript 不应自动展示每条 team 内 peer-to-peer 消息. 这些消息应在 inbox 视图中查看.

### 11.5 状态栏增强 (Optional)

可选增加状态栏项目, 支持 "一眼可控":

- `org`: 当前 org id (或 "none")
- `team`: 当前 team id/name (或 "none")
- `agents`: org 内 (或 team 内) running/total agents
- `unread`: unread inbox count (依据角色区分 org/team)
- `tasks`: 当前 team 的 pending/claimed 汇总

上述值应从持久化状态计算, 并采用轻量刷新/缓存策略.

### 11.6 数据源 (不依赖模型)

为避免 model/tool 耦合, TUI 应通过以下方式查询状态:

1. `$CODEX_HOME` 下的持久化控制面文件 (teams, orgs, tasks, inbox cursors).
1. 或 (长期优先) app-server v2 endpoints: `swarm/read`, `swarm/list`, `swarm/task/list`, `swarm/inbox/pop`.

这与 `2026-03-06` 的方向一致: collab tools 应演化为稳定协议/控制面资源, 而不是 "解析工具输出".

### 11.7 UX 边界场景

1. 若协作能力未启用, 仪表盘应给出明确提示.
1. 若某 agent thread 不存在 (shutdown/not found), 仍保留可见性但标记为 closed (类似现有 agent picker).
1. 若 inbox JSONL 变大, 依赖 cursor-based pop, 避免每次 redraw 全量扫描.

## 12. 增量实现计划

1. 团队内 mesh 消息:
   - 新增 `team_current`.
   - 新增 `team_info`.
   - 更新 `team_message`, 支持 member-to-member, 且成员校验基于持久化 team config.
   - 在 team config 中加入 `broadcastPolicy`, 并在 `team_broadcast` 中执行策略.

1. 多 assignee 任务:
   - 新增 `team_task_create` 与 `team_task_assign`.
   - 为 `leader_approves` 新增 `team_task_approve`.
   - 扩展 task 持久化 schema, 支持多 assignees 与 per-assignee state.
   - 更新 claim/complete 逻辑以匹配新语义.

1. Org 边界:
   - 引入 org 持久化与 `org_*` 工具, 支持 leader-to-leader 消息.
   - 限制通用工具 (`send_input`, `close_agent`, `resume_agent`), 强化边界.
   - 增加 president-only 控制面管理工具:
     - `team_update_config` / `team_set_leaders` 用于 leaders 委派与策略旗标.
     - `org_create` / `org_register_team` (或等价能力) 用于 org bootstrap 与 team 注册.

1. UX 后续:
   - TUI overlays: org/team inbox 与任务状态摘要.

## 13. 兼容性与迁移

1. 尽量保留 v1 工具名; 行为变更应尽量保持可兼容迁移.
1. 持久化 schema 版本化:
   - 在 team config 与 task JSON 中加入/维护 `schemaVersion`.
1. 为 v1 teams 提供迁移路径:
   - v1 team config: `leaders = []` 表示 "无委派 leader", 默认 broadcast/task create 由 President-only 执行.
   - v1 tasks 映射为 v2 tasks: `assignees = [assignee]`, `claimMode = exclusive`, `completionMode = any_assignee`.
