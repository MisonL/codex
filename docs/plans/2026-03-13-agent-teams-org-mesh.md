# Agent Organization (Agent Org): 组织层级 + 团队内 Mesh 协作 + 招募系统 (设计提案)

日期: 2026-03-13

本文将 "Agent Teams v2" 正式定名为 **Agent Organization**（简称 **Agent Org**）。
目标是在 `docs/plans/2026-03-06-codex-swarm-architecture.md` 的 swarm 控制面方向上,
增强现有 v1 `docs/agent-teams.md` 的 team 机制, 让它更像一个可治理的组织 (org), 而不仅是临时的一对一协作.

该方案的目标包括:

- 团队内启用点对点协作 (mesh, 范围严格限制在单个 `teamId` 内; 不再只是 owner/lead <-> member).
- 支持将单个任务直接分派给多个 agent, 无需 leader 预先拆分工作边界.
- 引入清晰的跨 team 通信边界: 跨 team 消息入口/出口收敛, 默认只允许 teams 的 principals (owners/leaders) 互通, 再由 leader 向下传达.
- 建立显式层级: 面向用户的 "President" (主线 agent thread) 作为 org 的总裁角色, 通过 team owner/leader 管理多个 teams.
- 引入 Agent Org 的组织元数据 (使命、愿景、文化价值观、背景环境), 以及成员 profile 与招募模板, 支持 leader/owner/president 按需招募补充人力.
- 通过强化通用 agent-to-agent 工具 (例如 `send_input`) 的授权, 使边界可被代码强制执行, 阻止 teammate 绕过 org/team 策略.

重要约束:

- Agent Org 是实验性功能, 必须在 `/experimental` 下以 `agent_org` 开关启用, 默认关闭; 关闭时全部行为保持 v1 不变.
- 该提案刻意保持增量式, 复用 `docs/agent-teams.md` 中已经存在的 durable inbox + persisted tasks 原语, 并以 durable-first 作为唯一真相.

## 0.1 固定决策 (Pinned)

本文为该方案的固定版本. 以下约束不是可选项:

1. **持久化优先是唯一真相**
   - team/org 的成员关系、角色与策略必须从 `$CODEX_HOME` 下持久化的控制面状态读取.
   - 允许存在内存 registry (in-memory registry) 作为缓存, 但不得作为授权真相.

1. **实验性开关是硬边界**
   - 所有 Agent Org v2 语义与新工具必须受 `agent_org` feature gate 控制.
   - `agent_org` 未启用时, 必须保持 v1 行为完全不变 (包含工具授权与持久化格式).

1. **团队内消息是 mesh (范围限制在 `teamId`)**
   - 同一 `teamId` 内, 任意成员可以向任意其他成员 (以及 leaders) 发送消息.
   - President 在某个 team 内的权限取决于其在该 team 的角色 (owner/leader/member); 对其他 teams 的沟通必须收敛到 principals (owner/leader) 通道, 不提供 President -> member 的跨 team 直连.

1. **单任务多 assignee 是一等能力**
   - leader 可以将 1 个任务分派给多个 assignee (无需预拆分).
   - 每个 assignee 的状态需要持久化, 并驱动任务完成语义.

1. **跨 team 通信仅 principals 可用 (范围限制在 `orgId`)**
   - 只有 team leaders/team owners (以及 President) 可以发送跨 team 消息, 且只能通过 `org_*` 工具.
   - team members 不允许直接向其他 team 发送消息.

1. **边界约束必须在代码层执行 (不是 prompt 约定)**
   - 能绕过策略的通用 agent-to-agent 工具 (至少: `send_input`, `close_agent`, `resume_agent`) 必须对 teammate thread 做限制.
   - teammate 必须使用 `team_*` / `org_*` 工具进行沟通与协作.

1. **默认隔离, 通过 artifact 显式共享**
   - 大体量或需要持久保留的产物必须通过显式 artifact 共享 (不要靠在消息里复制大段内容).

1. **profile/文化等元数据不参与授权**
   - 角色与权限只来自持久化控制面 (org/team config).
   - `AgentProfile`、org 文化、招募模板等只用于协作语境与提示构造, 不得被用作授权依据, 也不得允许通过 profile "自封 leader".

1. **招募是受控的 spawn, 不是放开 `spawn_*`**
   - leader/owner/president 的 "招募" 必须通过 `team_*` / `org_*` 的受控工具完成.
   - 禁止通过开放 `spawn_agent` / `spawn_team` 给 teammate 来实现招募, 以避免绕行与审计缺失.

## 0.2 术语与命名

当前 v1 实现里 "lead" 指代 "spawn team 的那个 thread". Agent Org 引入 owner/leader 层级与 profile/招募, 因此必须消歧:

- **Agent Org (Org)**: 一个可治理的多 team 组织边界, 包含 org 元数据、team 注册表、跨 team 通信边界与审计事件.
- **总裁线程 (President thread)**: 面向用户的主线 agent thread (root thread), 负责监管 org.
- **团队所有者 (Team owner)**: 负责团队生命周期与团队治理的 thread. 为兼容性, 在 team config 中持久化为 `leadThreadId`. team owner 可以是 President, 也可以是 President 招募并委派的中层 owner.
- **团队负责人 (Team leader)**: 团队内的委派 leader, 持久化在 `leaders[]`. leaders 拥有团队控制面权限 (任务、跨 team 沟通、受策略约束的 broadcast、招募团队成员等).
- **团队成员 (Team member)**: 团队内的普通成员 (非 leader).
- **组织元数据 (Org metadata)**: org 的使命、愿景、文化价值观、背景环境 (默认 "当下真实世界"), 后续可通过 "Vibe"(待规划) 扩展更多环境要素.
- **成员 profile (AgentProfile)**: 每个成员可自定义的人设/属性 (姓名、性别、年龄、教育、工龄、技能、兴趣、特长、健康等). 该信息不用于授权.
- **招募模板 (Recruitment template)**: 可复用的 profile + 角色 + 模型偏好等组合, 供 leader/owner/president 批量或差异化招募.

固定规则:

- `leadThreadId` 表示 **team owner**, 而不是委派的 team leader.

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

- `2026-03-06` 明确避免 **全局** 点对点 mesh. 本提案通过将 "mesh" 严格限制在单个 `team_id` 范围 (durable inbox + 尽力实时投递), 并通过 `org_*` 工具强制 **仅 principals** 跨 team 通信, 来保持这一约束.

本文与更早设计的映射关系:

- 控制面对象:
  - `Org` (President + team owners/leaders) 是提议 `SwarmRun` (kind: `swarm`) 的一个轻量切片.
  - `Team` 仍复用现有 `team_id` 范围的工作流 (kind: `team`), 但补齐缺失语义 (团队内 mesh 消息、leader 委派、多 assignee 任务).
- 任务模型:
  - 多 assignee 任务扩展了早期 `TaskSpec` 的方向, 通过 assignee 级状态跟踪实现, 而不要求 leader 预拆分.
- 可观测性与回放:
  - team/org 消息与任务状态迁移应携带稳定 envelope (`swarmRunId`, `teamId`, `taskId`, `sequence`, `causalParent`), 以保证可审计与可回放.
- 记忆模型:
  - 线程工作记忆默认隔离; 当内容较大或需要持久化时, 通过显式 artifact 发布共享.

## 0.5 控制合同 (CSE) (Pinned)

本节用于让设计可验证, 降低 "看起来没问题" 的漂移.

- **主要设定值 (Primary Setpoint):**
  - `agent_org` 启用时: 团队内 mesh 协作 + 多 assignee 任务 + 仅 principals 跨 team 边界 + 受控招募与 profile/org 元数据语境注入.
  - `agent_org` 未启用时: v1 行为必须完全不变 (包含工具授权与持久化格式兼容).
- **验收标准 (Acceptance, 必须可在代码中测试):**
  - `agent_org` 默认关闭, 且未启用时不暴露 v2 新工具 (至少: `org_update_config`, `org_profile_update_self`, `team_template_upsert`, `team_recruit`).
  - 任意 teammate 可以在同一 `team_id` 内对任意其他 teammate 执行 `team_message`, 且会追加 1 条 durable inbox 记录 (先持久化, 再尽力投递).
  - 非 principal 的跨 team 消息必须被拒绝 (仅 principals/President 可通过 `org_*`).
  - 当目标 thread 已注册到 org/team 时, `send_input` / `close_agent` / `resume_agent` 不得绕过 org/team 策略边界.
  - 多 assignee 任务必须遵守 `claimMode` + `completionMode`, 且 `leader_approves` 必须有显式批准执行器.
  - `team_recruit` 必须写入持久化成员引用 (team config + inbox), 并为新成员初始化 profile (若提供), 且追加控制面事件.
- **护栏指标 (Guardrails):**
  - 授权护栏: 禁止出现 "跨 team 直连 member" 与 "member 调用 org_*" 的可达路径.
  - 状态护栏: `config.json` / task snapshot 必须原子写; JSONL append 必须加锁; `sequence` 必须单调递增且可回放.
  - 成本护栏: 招募/广播必须可配额; 超额必须显式失败, 不得静默.
- **采样与验证计划 (Sampling Plan):**
  - L0: 仅 `agent_org` 关闭的回归覆盖 (确保 v1 无漂移).
  - L1: `agent_org` 打开时的最小端到端集成测试覆盖 (team mesh / principals channel / recruit / multi-assignee task).
  - L2: 长链路验证 (TUI overlays + app-server v2 资源化接口) 作为后续阶段门禁, 不作为早期必要条件.
- **恢复目标 (Recovery Target):**
  - 任何阶段出现越权、持久化破坏或不可回放状态时, 允许通过关闭 `agent_org` 立即回到 v1 行为 (不依赖手工修文件).
- **回滚触发 (Rollback Triggers):**
  - 发现跨 team 越权消息可达, 或持久化 schema 产生不可逆破坏.
  - 事件序列不单调/不可回放, 或出现 "半成功" 的状态迁移.
- **硬约束 (Constraints):** durable-first; `$CODEX_HOME` 下的持久化状态是授权真相; 禁止静默回退路径.
- **传感器/证据 (Sensors):** `$CODEX_HOME/teams/*/config.json`, `inbox/*.jsonl`, `$CODEX_HOME/orgs/*/config.json`, `profiles/*.json`, `recruitment/templates/*.json`, `tasks/*.json`, `events.jsonl` (以及对应 lock 文件).

## 0.6 第一性原理: 组织作为可控系统 (Pinned)

现代顶尖软件公司的组织设计并不神秘, 其核心是解决三个不可避免的问题:

1. **并行化:** 单个个体无法覆盖全部工作, 必须用团队与分工提高吞吐.
1. **可治理:** 并行会引入沟通与协调成本, 必须用边界与流程降低熵增.
1. **可持续:** 交付不是终点, 运行、故障与复盘要求系统可观测、可恢复、可审计.

Agent Org 的顶层设计按控制论拆成三面:

- **控制面 (Control Plane):** org/team 配置、角色与授权、预算/配额、招募与任命、任务分派与审批.
- **数据面 (Data Plane):** 具体的协作执行 (消息投递、任务 claim/complete、artifact 发布与读取).
- **状态面 (State Plane):** durable-first 的事实源 (configs/inbox/tasks/events/profiles/templates).

对应到产品原语, 最小且可组合的一组对象是:

- **边界:** `Org` (跨 team 边界与治理) + `Team` (团队内协作边界)
- **身份:** `ThreadId` (最小执行单元, 也是授权主体)
- **工作:** `Task` (可分派、可回放的工作项; 支持多 assignee)
- **沟通:** `InboxEntry` (可审计的消息投递记录; team scope 与 org scope)
- **产物:** `Artifact` (显式共享, 替代默认共享上下文)
- **审计:** `events.jsonl` + `sequence` + `causalParent` (确定性回放与责任归因)

组织的基本闭环在本系统中映射为:

- Plan: 通过任务与 artifact (PRD/RFC/计划) 把目标显式化
- Build: 通过多 assignee 协作并产出 patch/test 产物
- Ship: 通过 leader/owner 的显式批准将变更收口 (不依赖隐式约定)
- Run: 通过事件/任务表达 incident 与修复, 并用 principals channel 协调跨 team
- Learn: 通过复盘 artifact 固化经验, 回写模板/策略 (而不是只靠聊天记忆)

## 1. 目标

1. 团队成员可以通过 team-scoped 工具直接互相协作.
1. team leader 可以把 1 个任务分派给多个成员, 由成员自组织拆分.
1. 跨 team 通信被约束为 principals (owners/leaders, 以及可选的 President) 才能执行, 且只有一个受控入口/出口.
1. 面向用户的主线 agent 作为 "President", 负责监督各 team leaders 与整体进展.
1. leader/owner/president 可按需招募下级角色 (成员/leader/owner), 并支持保存可复用的招募模板与批量差异化招募.
1. 所有成员可维护自己的 `AgentProfile`, 并在启用 Agent Org 时将 org/team/profile 元数据注入协作语境以提升一致性 (但不影响授权).
1. 所有消息仍保持 durable-first (先持久化, 再尽力实时投递).

## 2. 非目标

1. 完整的 "嵌套团队": 允许队员自由 spawn 更多 teams/agents 且不受治理 (后续可以加配额/治理再做).
1. 分布式、多进程控制面. 本提案仍保持类似 v1 的 in-process + 文件持久化.
1. "Vibe" 的完整世界观系统. 本提案只提供 org 背景环境的默认值与可扩展的持久化接口.
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
1. 缺少一等的跨 team 边界. 如果知道 thread id, teammate 可能用通用工具 (`send_input`, `close_agent`, `resume_agent`) 绕过预期的 "仅 principals" 跨 team 边界.
1. leadership 是隐式的 (spawn team 的 thread), 不符合常见 "团队有一个 leader agent" 的心智模型.

## 4. 提议模型

### 4.1 实体

1. **组织 (Org)**
1. **团队 (Team)**
1. **Agent 线程 (Agent thread)** (现有 `ThreadId` / "agent_id")

### 4.2 角色

1. **总裁 (President)**
   - 面向用户的主线 agent thread.
   - 作为 org 的 owner, 维护 org 元数据、边界策略与最终裁决.
   - 可以按需招募 team owners, 创建/注册 teams, 并任命/调整 team owners 与 team leaders.

1. **团队所有者 (Team Owner)**
   - 负责某个 team 的生命周期与治理配置 (成员清单、leaders、策略、招募模板等).
   - 为兼容性, team owner 在 team config 中持久化为 `leadThreadId` (字段名沿用 v1).
   - 可以按需招募 team leaders, 并对 team 内任务与协作边界负责.

1. **团队负责人 (Team Leader)**
   - 团队内的委派 leader, 持久化在 `leaders[]`.
   - 拥有团队控制面权限 (消息策略、多 assignee 任务分派、跨 team 沟通、招募团队成员等).
   - 向 team owner 汇报; 必要时由 team owner 向 President 升级.

1. **团队成员 (Team Member)**
   - 团队内普通 agent thread.
   - 可以在团队内与 peers 直接协作, 并维护自己的 `AgentProfile`.

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
  "leadThreadId": "thread-owner-a",
  "leaders": ["thread-leader-a"],
  "broadcastPolicy": "leaders_only",
  "createdAt": 1739988000,
  "members": [
    { "name": "alice", "agentId": "thread-alice", "agentType": "develop" },
    { "name": "bob", "agentId": "thread-bob", "agentType": "develop" }
  ]
}
```

#### 4.4.2 Org config (`$CODEX_HOME/orgs/<org_id>/config.json`)

```json
{
  "schemaVersion": 2,
  "orgId": "org-123",
  "orgName": "demo-org",
  "presidentThreadId": "thread-president",
  "createdAt": 1739988000,
  "environment": { "vibeId": "real_world_now" },
  "mission": "用可治理的多 agent 协作交付真实软件",
  "vision": "让组织可以快速扩张与自我修复",
  "values": ["事实优先", "边界清晰", "可审计", "可回滚"],
  "teams": [
    {
      "teamId": "demo-team",
      "ownerThreadId": "thread-owner-a",
      "leaders": ["thread-leader-a"]
    }
  ]
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

说明:

- `fromRole` 表示发送方在 org 范围内的 principal 角色, 取值应至少包含: `leader` / `owner` / `president`.

#### 4.4.7 Org 控制面事件日志 entry (`$CODEX_HOME/orgs/<org_id>/events.jsonl`)

```json
{
  "id": "ev-1",
  "createdAt": 1739988001,
  "orgId": "org-123",
  "sequence": 7,
  "kind": "org.principal.message.appended",
  "actorThreadId": "thread-leader-a",
  "causalParent": null,
  "payload": { "fromTeamId": "team-a", "toTeamId": "team-b" }
}
```

#### 4.4.8 成员 profile (`$CODEX_HOME/orgs/<org_id>/profiles/<thread_id>.json`)

```json
{
  "schemaVersion": 1,
  "orgId": "org-123",
  "threadId": "thread-alice",
  "updatedAt": 1739988002,
  "profile": {
    "displayName": "Alice",
    "gender": "female",
    "age": 28,
    "education": "本科",
    "yearsOfExperience": 5,
    "jobTitle": "前端工程师",
    "skills": ["React", "TypeScript"],
    "interests": ["设计系统"],
    "strengths": ["UI 细节打磨"],
    "health": "良好",
    "extra": { "hobby": "攀岩" }
  }
}
```

说明:

- `profile` 仅用于协作语境与提示构造, 不参与授权.
- 默认规则: 成员只能修改自己的 profile; leader/owner/president 只能在招募时创建/初始化下级 profile (后续变更由本人更新).

#### 4.4.9 招募模板 (Org/Team) (`$CODEX_HOME/{orgs/<org_id>|teams/<team_id>}/recruitment/templates/<template_id>.json`)

Team scope 与 Org scope 的模板 schema 一致, 仅存放路径不同.

```json
{
  "schemaVersion": 1,
  "templateId": "frontend-ui-v1",
  "scope": "team_member",
  "createdAt": 1739988000,
  "createdByThreadId": "thread-leader-a",
  "spawn": {
    "agentType": "develop",
    "modelProvider": null,
    "model": null
  },
  "profile": {
    "displayName": "UI 工程师",
    "skills": ["UI", "组件库", "交互细节"],
    "strengths": ["审美一致性", "可用性优化"],
    "extra": { "focus": "TUI/CLI UX" }
  }
}
```

说明:

- `scope` 约束模板可被谁使用: `team_member` / `team_leader` / `team_owner`.
- 模板的 `spawn.*` 只是 spawn 偏好, 不得作为授权依据.
- 模板创建/更新必须追加控制面事件, 以便审计与回放.

### 4.5 控制面引导工具 (new)

Agent Org 引入了新的持久化字段与资源 (org metadata、`leaders[]`, `broadcastPolicy`, `orgId`, profiles, recruitment templates 等).
这些对象属于控制面/状态面, 不能依赖手工编辑文件, 必须通过受控工具写入并追加审计事件.

硬约束:

- 下列 `team_*` / `org_*` 控制面工具必须受 `agent_org` feature gate 约束.

#### 4.5.1 `team_update_config` (new, owner/president-only)

以受控方式更新团队持久化配置 (`$CODEX_HOME/teams/<team_id>/config.json`).

- Inputs (snake_case):
  - `team_id`
  - `leaders` (可选; 成员名或 thread id, 必须校验属于 `members[]`)
  - `broadcast_policy` (可选)
  - `org_id` (可选; 将 team attach/detach 到某个 org)
- Outputs:
  - 更新后的 team 元信息 (至少包含: `team_id`, `org_id`, `leaders`, `broadcast_policy`)

必备属性:

- 授权: 默认仅 team owner (`leadThreadId`) 可调用; President thread 作为 org owner 允许 override.
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

#### 4.5.5 `org_update_config` (new, president-only)

以受控方式更新 org 持久化配置 (`$CODEX_HOME/orgs/<org_id>/config.json`) 的元数据字段:

- `org_name` (可选)
- `environment` (可选; 默认 `{ vibeId: "real_world_now" }`)
- `mission` / `vision` / `values` (可选)

固定要求:

- 写入必须原子化, 并追加 `org.config.updated` 事件.

#### 4.5.6 `org_profile_update_self` (new, self-only)

允许调用方更新自己的 `AgentProfile` (`$CODEX_HOME/orgs/<org_id>/profiles/<caller_thread_id>.json`).

- 授权: 仅允许修改自己的 profile; 禁止修改他人 profile.
- 写入必须原子化, 并追加 `org.profile.updated` 事件 (payload 至少包含 `threadId`).

#### 4.5.7 `team_template_upsert` / `org_template_upsert` (new)

以受控方式创建/更新招募模板:

- Team scope: `$CODEX_HOME/teams/<team_id>/recruitment/templates/<template_id>.json`
  - 授权: team leaders / team owner
- Org scope: `$CODEX_HOME/orgs/<org_id>/recruitment/templates/<template_id>.json`
  - 授权: President thread (org owner)

固定要求:

- 模板更新必须追加事件 (`team.template.upserted` / `org.template.upserted`).
- 模板内容不得改变授权边界; 只作为 spawn 偏好与 profile 初始化输入.

#### 4.5.8 `team_recruit` / `org_recruit` (new)

受控招募下级角色, 并在持久化控制面中注册 (避免通过开放 `spawn_*` 绕行).

- `team_recruit`
  - 授权: team leaders / team owner
  - 作用: 招募 team members (可选扩展: 招募/任命 leaders)
- `org_recruit`
  - 授权: President thread (org owner)
  - 作用: 招募 team owners (以及必要的跨 team 管理角色)

Inputs (snake_case, 建议形态):

- `team_recruit`:
  - `team_id`
  - `recruits`: 批量招募请求数组, 支持差异化
    - `template_id` (可选)
    - `quantity` (可选, 默认 1)
    - `spawn_overrides` (可选): `agent_type`/`model_provider`/`model`
    - `profile_overrides` (可选): 仅覆盖部分 profile 字段
- `org_recruit`:
  - `org_id`
  - `recruits`: 同上, 但 `scope` 默认为 `team_owner`

Outputs (最小):

- `recruited`: 每个新招募的 thread 的 `{ name, agent_id, role, template_id? }`

固定要求:

- 招募必须是可审计的控制面动作, 追加事件 (`team.member.recruited`, `team.leader.recruited`, `org.owner.recruited` 等).
- 招募成功后必须更新 team/org config 的成员引用, 并确保 inbox/任务等路径可用.

### 4.6 幂等性、锁与事件覆盖 (Pinned)

为保证 durable-first 语义在并发与重启场景下正确:

- **原子写:** 对 `config.json` 与 task snapshot 的更新必须采用 write-temp-then-rename (禁止产生半截 JSON).
- **互斥锁:** 所有 JSONL append 面必须使用 per-file lock (v1 已有 inbox lock; events logs 也必须加锁).
- **sequence 分配:** 在持有对应 scope (team/org) 的 `events.lock` 时分配 `sequence`, 然后将其写入对应的 inbox/event entry.
- **幂等:** task-level 完成迁移与 hooks 必须只触发一次; 重复调用必须显式报错或显式 no-op, 但不得静默 "半成功".

最小事件覆盖 (仅当状态实际变化时才向 `events.jsonl` 追加):

- Team scope:
  - `team.config.updated`
  - `team.message.appended`
  - `team.template.upserted`
  - `team.member.recruited`
  - `team.leader.recruited`
  - `team.task.created`
  - `team.task.assignees.updated`
  - `team.task.assignee.claimed`
  - `team.task.assignee.completed`
  - `team.task.approved`
- Org scope:
  - `org.created`
  - `org.config.updated`
  - `org.team.registered`
  - `org.template.upserted`
  - `org.profile.updated`
  - `org.owner.recruited`
  - `org.principal.message.appended`

### 4.7 元数据流 (Metadata Flow)

Agent Org 在持久化控制面中引入了多类元数据, 需要明确它们如何流动, 以及哪些流动会影响运行时行为:

1. Org metadata
   - 来源: `org_update_config`
   - 载体: `$CODEX_HOME/orgs/<org_id>/config.json` + `events.jsonl`
   - 用途: 构造 org-level 协作语境 (使命/愿景/文化/环境), 在招募与初始化阶段注入到下级 agent 的基础指令中

1. Team control-plane metadata
   - 来源: `team_update_config`
   - 载体: `$CODEX_HOME/teams/<team_id>/config.json` + `events.jsonl`
   - 用途: 授权真相 (members/leaders/owner), 团队内消息策略, 任务分派边界

1. AgentProfile
   - 来源: `org_profile_update_self` (self-only) 与招募初始化
   - 载体: `$CODEX_HOME/orgs/<org_id>/profiles/<thread_id>.json` + `events.jsonl`
   - 用途: 构造个体协作语境与角色分工, 不参与授权

1. Recruitment templates
   - 来源: `team_template_upsert` / `org_template_upsert`
   - 载体: `$CODEX_HOME/{teams/<team_id>|orgs/<org_id>}/recruitment/templates/*.json` + `events.jsonl`
   - 用途: 批量/差异化招募时的 spawn 偏好与 profile 初始化输入

关键路径 (从控制面到运行时) 必须保持单向可审计:

```text
template/profile/org/team 更新 (tools)
  -> 写入 config/templates/profiles (原子) + 追加 events (append-only)
  -> (可选) 触发受控招募 recruit
       -> spawn agent thread
       -> 更新 team/org 引用
       -> best-effort 投递初始化消息
```

### 4.8 模块依赖关系与边界

本提案的改动面主要落在控制面与状态面, 数据面尽量不改, 执行面不重写:

- Feature gate (控制面入口)
  - `agent_org` feature flag 作为硬边界, 控制新工具暴露与 v2 行为切换
  - `/experimental` 作为用户显式开关入口, 默认关闭
- Tool handlers (控制面执行器)
  - `team_*` / `org_*` 工具负责写入持久化真相与追加审计事件
  - 与通用 agent-to-agent 工具的授权耦合点必须显式收敛 (防绕行)
- Persistence (状态面事实源)
  - `$CODEX_HOME/teams/...`, `$CODEX_HOME/orgs/...`, `$CODEX_HOME/tasks/...` 为事实源
  - lock + write-temp-then-rename 保证并发与崩溃安全
- Live delivery (执行面)
  - 仍复用 `AgentControl` 的 spawn/send/shutdown 等能力
  - durable-first: 先写入 inbox, 再尽力实时投递; 投递失败不得丢持久化消息
- UI/Protocol (观测面)
  - 短期: UI 可直接读取 `$CODEX_HOME` 的控制面状态做渲染 (cursor-based)
  - 长期: 对齐 `2026-03-06` 的方向, 将其演进为 app-server v2 资源接口, 避免 "解析工具输出"

冻结边界 (本提案不触碰):

- 不引入全局 peer-to-peer mesh; mesh 严格限制在单个 `teamId`
- 不改变 `thread/turn/item` 作为运行主语的模型
- 不把 profile/文化等元数据作为授权依据

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
- `role`: `"member" | "lead"` (或空; `lead` 表示团队 owner, 对应 `leadThreadId`)
- `lead_thread_id` (团队 owner; 为兼容性在配置中持久化为 `leadThreadId`)
- `org_president_thread_id` (可选; 当 `org_id` 存在时从 org config 读取)

固定要求:

- v2 必须提供 `team_current` 或等价的自动注入机制, 使 teammate 不需要 President 人工粘贴 team id 也能调用 `team_*` 工具.
- v2 必须提供 `team_current` 或等价的自动注入机制, 使 teammate 不需要依赖 out-of-band 的上级转述也能发现自己的 team 上下文.

#### 5.2.2 `team_info` (new)

返回团队自组织所需的元信息:

- `team_id`, `org_id` (未注册到 org 时可为空)
- `lead_thread_id` (团队 owner; 为兼容性持久化为 `leadThreadId`)
- `org_president_thread_id` (可选)
- `leaders` (thread ids 与 names; 当 team config 尚未引入 `leaders[]` 时可为空)
- `members` (thread ids、names、可选 agent roles)
- 可选: 消息策略 (见下文)

这能避免 out-of-band 共享 agent ids, 并让团队内成员自发现 peers.

授权:

- 仅 team 成员/leader、team owner 或 org president 可调用.

#### 5.2.3 `team_message` (行为变更)

v1 当前语义基本是 "lead -> member". v2 语义:

1. 任意 team member 或 leader 均可调用 `team_message`.
1. sender 与 receiver 必须同属同一个 `team_id`.
1. 授权与成员查找必须基于持久化 team config (`$CODEX_HOME/teams/<team_id>/config.json`), 不得依赖以 spawning thread 为 key 的 in-memory registry.
1. 持久化 inbox entry (JSONL 用 `camelCase`) 应包含:
   - `fromThreadId`
   - `fromName` (从 team config 解析)
   - `fromRole` (`member` / `leader` / `owner` / `president`)
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

## 7. 跨 team 通信: 仅 principals (owners/leaders)

### 7.1 设计原则

team members 不应直接向其他 teams 发消息. 跨 team 通信需要:

1. 在需要时可用.
1. 只有一个受控入口/出口.
1. 限制为 team leaders/team owners (以及可选的 President).

### 7.2 组织层 (new persisted concept)

引入 org registry, 持久化在 `$CODEX_HOME/orgs/<org_id>/...`:

- `config.json`: org metadata + presidentThreadId + teams 列表 (teamId/ownerThreadId/leaders[])
- org 范围的 durable inbox (对每个 principal: owner/leader/president), 语义与 team inbox 相同 (durable-first)

org 层即为边界强制执行的机制基础.

### 7.3 Org 工具 (new)

1. `org_info`: 列出 org 内 teams 与 principals (owners/leaders/president).
1. `org_principal_message`: principal -> principal 消息, 依据 org config 校验.
1. `org_inbox_pop` / `org_inbox_ack`: 读取与 ack org 范围消息.

授权:

- `org_principal_message` 仅允许:
  - President thread, 或
  - org 内任一 team 的 leader thread, 或
  - org 内任一 team 的 owner thread
- receiver 必须是:
  - org 内另一个 team 的 leader/owner, 或
  - President thread

### 7.4 边界强制执行 (required)

要让 "跨 team 仅 principals" 可被强制执行 (而不是 prompt 约定), 必须阻止通过通用工具绕行:

1. 限制 teammate threads 使用通用 agent-to-agent 工具 (至少: `send_input`, `close_agent`, `resume_agent`).
1. 确保 teammates 拥有可用的替代表面:
   - team 内: `team_message` / `team_broadcast` (策略约束) + inbox 工具.
   - 跨 team: 仅 principals 使用 `org_principal_message`, 然后通过 `team_*` 转发给成员.

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

为对齐 "团队是一个组织单元" 的心智模型, 并落地 President -> Owner -> Leader -> Member 的层级治理:

1. `leadThreadId` 表示 team owner (字段名沿用 v1; 语义升级为 owner, 不再混用为委派 leader).
1. 在 team config 中加入 `leaders: [thread_id]`.
1. 将 team leaders 视为以下控制面动作的特权 actor:
   - broadcast 策略
   - task create/assign
   - 跨 team 沟通 (org 边界内)
   - 招募团队成员
1. 将 team owner 视为以下控制面动作的特权 actor:
   - team 生命周期与治理配置
   - 任命/调整 team leaders
   - (可选) 招募 team leaders

默认行为:

- v1 `spawn_team` 的 spawning thread 仍是初始 team owner.
- 启用 Agent Org 后, President 可通过受控工具将 team owner 委派给招募得到的 owner thread, 以实现组织扩张与分层管理.

## 9. Artifacts (显式共享, 非共享上下文)

为对齐 "默认隔离, 通过 artifact 共享" 的方向:

1. team 消息应尽量短, 以协作为主.
1. 非 trivial 的产物 (计划、总结、patch 集、评审、表格等) 应通过显式 artifact 发布并以 id 引用.

推荐的 artifact kind 口径 (用于组织化产物, 不参与授权):

- `prd`: 需求描述与验收口径
- `rfc`: 设计提案与取舍
- `adr`: 架构决策记录 (含备选方案与被拒绝原因)
- `plan`: 执行计划与拆解
- `patch`: 代码变更产物 (可对应 PR 或 patchset)
- `review`: 评审意见 (设计/代码/上线)
- `release_plan`: 发布/灰度/回滚计划
- `runbook`: 运行手册与应急预案
- `postmortem`: 事故复盘
- `summary`: 周报/里程碑摘要

后续可跟进的控制面工具 (第一阶段非必需), 用于让 artifact 更易用:

- `team_artifact_publish`: 在 team scope 创建 artifact
- `team_artifact_read`: 读取 artifact
- `team_artifact_list`: 列出某 task/team 的 artifacts

这些与 `2026-03-06-codex-swarm-architecture.md` 中的 `Artifact` 对象方向一致.

## 10. 端到端示例流程

### 10.1 President 创建 org 并扩张两个 teams

1. President `org_create` 创建 org (默认环境 `real_world_now`).
1. President `org_recruit` 招募两个 team owners: `owner-a`, `owner-b`.
1. `owner-a` 调用 `spawn_team` 创建 Team A (spawning thread 即 Team A 的 `leadThreadId`).
1. `owner-b` 调用 `spawn_team` 创建 Team B.
1. President `org_register_team` 注册 Team A/B.
1. `owner-a` 与 `owner-b` 通过 `team_update_config` 委派各自的 team leaders.

### 10.2 Team A leader 将 1 个任务分派给多个成员

1. Team A leader `team_template_upsert` 保存常用招募模板 (例如 UI/PM/DEV/QA).
1. Team A leader `team_recruit` 按需批量招募: 1 UI, 1 PM, 2 DEV, 1 QA (模板 + overrides).
1. 调用 `team_task_create`, 参数 `assignees: ["ui-1", "pm-1", "dev-1", "dev-2", "qa-1"]`, `kickoff: true`.
1. 每个 assignee claim 任务 (`team_task_claim`), 发布自己的计划并自组织拆分.
1. 完成后各自标记完成 (`team_task_complete`).

### 10.3 Team A leader 与 Team B leader 沟通

1. `org_principal_message` 从 `lead-a` 发给 `lead-b`.
1. `lead-b` 通过 `team_broadcast` 或 `team_message` 将关键信息转达给 Team B 成员.

### 10.4 从需求到合并: 现代研发主路径 (示例)

1. President/Owner 为该项目建立最小 setpoint 与护栏 (质量/成本/边界), 并通过 org 元数据固化 (mission/values + vibe).
1. PM (team member) 发布 `prd` artifact, 明确验收标准与约束.
1. Tech lead (team leader) 发布 `rfc` artifact, 组织评审并记录取舍; 涉及跨 team 依赖时, 通过 principals channel 与相关 leaders 对齐.
1. leader 通过 `team_task_create` 将同一工作项分派给多名 assignees (DEV/QA/Design 等), 由成员自组织拆分并通过 team mesh 协商依赖.
1. assignees 发布 `patch` / `review` / `summary` artifacts, leader 汇总并执行必要审批 (例如 `leader_approves` 完成模式).
1. 完成后输出面向 President 的简短 `summary` 作为 status card (避免主 transcript 被内部细节淹没).

### 10.5 故障应急与复盘: 运行闭环 (示例, 仅规划)

1. President 接到故障信号后, 指派相关 team owners/leaders 建立处理边界 (谁决策, 谁执行, 谁同步对外).
1. 通过 `org_principal_message` 在 principals channel 中发起跨 team 协调, 每个 leader 再在 team 内广播/分派任务.
1. 关键动作一律以 task + artifact 表达:
   - `runbook` (止血步骤与回滚策略)
   - `patch` (修复)
   - `postmortem` (复盘与行动项)
1. 复盘结论回写到模板/策略:
   - 更新招募模板 (例如补齐 SRE/QA 画像)
   - 更新团队协作协议 (kickoff 模板与 Definition of Done)
   - 更新 org values/guardrails (例如对 broadcast、招募、审批的治理口径)

## 11. TUI UX (用户反馈 + 控制界面)

本节描述 Codex TUI 如何让多 team 工作可见、可控, 同时避免刷屏.

设计目标:

1. **分层信息:** 先总览, 需要时再深钻.
1. **默认低噪声:** 避免把所有内部消息流进主 transcript.
1. **快速导航:** 在 President / owners / leaders / members 之间切换尽量只需 1-2 个动作.
1. **持久化状态为真相:** 面板读 `$CODEX_HOME` 的控制面状态 (而不是解析模型输出).

### 11.1 信息架构

TUI 应呈现清晰层级:

- Org (President scope) -> Teams -> Agents -> Tasks -> Artifacts

其中:

- 主聊天线程是 **总裁 (President)**.
- 每个 team 有一个 **owner** 与一个或多个 **leaders** (agent threads).
- 成员通过 team 内 mesh 消息与 shared tasks 协作.

### 11.2 入口点 (Commands)

TUI 已有 slash commands 与选择视图. 增加 team/org 入口:

- `/org`: 打开 Org 仪表盘 (teams + leaders + 汇总)
- `/org inbox`: 展示 President 的 org inbox (leader updates, 跨 team 协调)
- `/team`: 打开当前 team 仪表盘 (leader/member threads 使用)
- `/team tasks`: 打开任务看板 (当前 team)
- `/team inbox`: 展示当前线程的 team inbox
- `/teams`: 列出 teams 并跳转到某个 team leader thread ("watch leader")

上述命令至少要求启用 `multi_agent`; 其中 org/profile/招募相关入口仅在 `agent_org` 启用时可用.

### 11.3 仪表盘与 Overlays

复用现有全屏 overlay/pager 交互模式 (类似 transcript overlay):

1. **Org 仪表盘 (President)**
   - 表格行: `team name/id`, `owner`, `leaders`, `status`, `members`, `tasks (pending/claimed/completed)`, `unread (org inbox)`
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

按风险与耦合从低到高, 落地模块依赖关系如下:

- `codex-rs/core`: feature gate、工具授权、持久化读写、事件日志 (本提案主落点, 必须先做)
- `codex-rs/protocol` / `codex-rs/app-server(-protocol)`: envelope 补齐与资源化 API (用于多观察者订阅/重连回放, 可后置)
- `codex-rs/tui`: org/team 仪表盘与 inbox/task overlays (可后置, 先保证控制面与工具语义)
- `docs/`: 行为口径与迁移说明 (每次新增/变更 API 必须同步)

1. Feature gate (硬前置):
   - 新增 `agent_org` feature flag (Stage::Experimental), 默认关闭.
   - 所有 v2 新工具与行为变更必须在 `agent_org` 启用时才生效; 未启用时 v1 行为必须完全不变.

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
   - 增加 owner/president 控制面管理工具:
     - `team_update_config` / `team_set_leaders` 用于 leaders 委派与策略旗标, 并支持 President override.
     - `org_create` / `org_register_team` (或等价能力) 用于 org bootstrap 与 team 注册.

1. Org 元数据 + profile + 招募模板:
   - 扩展 org config: `environment`/`mission`/`vision`/`values`.
   - 新增 `org_update_config` 与 `org_profile_update_self`.
   - 新增 `team_template_upsert` / `org_template_upsert`.

1. 受控招募:
   - 新增 `team_recruit` / `org_recruit`, 在 tool 内部调用 spawn 能力并同步写入持久化真相与审计事件.
   - 招募时允许基于模板 + overrides 初始化 profile, 并将新 thread 注册到 team/org.
   - 可选扩展: `team_set_owner` / `team_transfer_ownership`, 以支持 President 招募 team owners 并委派团队所有权.

1. UX 后续:
   - TUI overlays: org/team inbox 与任务状态摘要.

## 13. 兼容性与迁移

1. `agent_org` 未启用时, 必须保持 v1 行为完全不变 (包含工具授权与持久化格式).
1. 尽量保留 v1 工具名; v2 行为变更必须通过 feature gate 与 schemaVersion 迁移显式引入.
1. 持久化 schema 版本化:
   - 在 team config 与 task JSON 中加入/维护 `schemaVersion`.
1. 为 v1 teams 提供迁移路径:
   - v1 team config: `leaders = []` 表示 "无委派 leader", 默认 broadcast/task create 由 President-only 执行.
   - v1 tasks 映射为 v2 tasks: `assignees = [assignee]`, `claimMode = exclusive`, `completionMode = any_assignee`.

## 14. 产品功能规划 (完整清单)

本节将本提案收敛为可实施的产品功能清单, 以便后续按阶段交付. 术语沿用前文 (President/Owner/Leader/Member; Org/Team; profile/template).

### 14.1 角色与权限矩阵 (产品口径)

固定原则:

- 授权只看持久化控制面状态 (org/team config), 不看 prompt, 不看 profile/template.
- 所有可越权的通用工具必须做 target-based authorization, 并在 `agent_org` 启用后强制边界.

能力矩阵 (最小集合):

1. President
   - Org: 创建/更新 org 元数据, 注册/解绑 team, 招募 owners, 跨 team 协调
   - Team: 可 override team owner 的治理动作 (用于紧急处置与组织扩张)
1. Team owner (`leadThreadId`)
   - Team: 配置治理 (leaders/broadcastPolicy), 任命 leaders, 招募成员, 任务分派与审批
1. Team leader (`leaders[]`)
   - Team: 团队内 mesh 协作, 广播 (受 broadcastPolicy 约束), 多 assignee 任务创建/分派/审批, 招募成员, 跨 team (principal) 沟通
1. Team member
   - Team: 团队内 mesh 协作, 任务 claim/complete, 产物发布 (artifact), 自己 profile 更新, 向 leader 升级 (team_ask_lead)

### 14.2 Org 元数据与 Vibe (世界观环境)

产品目标:

- Org 需要一个稳定的 "组织身份" 与 "运行环境" 作为协作语境锚点, 且可扩展.
- 默认环境为 "当下真实世界" (`vibeId = "real_world_now"`), 未来通过 "Vibe" 功能扩展更多环境要素.

最小可用字段:

- `environment.vibeId` (默认 `real_world_now`)
- `mission` / `vision` / `values[]`
- `orgName` (可选)

Vibe 扩展接口 (仅规划, 本期不实现):

```json
{
  "vibeId": "real_world_now",
  "era": "present",
  "locale": "default",
  "constraints": ["no_magic", "no_time_travel"],
  "style": { "tone": "professional", "riskPreference": "balanced" }
}
```

固定约束:

- Vibe 只用于语境与提示构造, 不得改变授权边界.
- Vibe/使命/价值观的注入必须可追溯 (体现在控制面事件里, 并可在调试输出中定位来源).

### 14.3 成员 Profile (可自定义属性)

产品目标:

- 让每个成员拥有可持久化的人设/属性, 用于协作语境与角色分工.
- 支持成员自维护, 支持招募时初始化, 支持模板复用.

Profile 字段范围 (建议口径: "可协作的人设与能力画像", 不参与授权):

- 基础身份 (可选): `displayName`, `gender`, `age`, `education`, `yearsOfExperience`, `jobTitle`
- 职业能力: `skills[]`, `strengths[]`
- 兴趣倾向: `interests[]`
- 健康字段: 仅允许自由文本 (`health`), 不做结构化推断, 不参与任何授权或调度
- 扩展字段: `extra` (任意键值, 用于容纳未来更多自定义属性)

权限规则:

- 默认仅允许 self update.
- leader/owner/president 仅允许在招募时创建或初始化 profile, 后续更新由本人完成.

### 14.4 招募系统 (Recruitment)

产品目标:

- leader/owner/president 能 "按需补充人力", 而无需开放 `spawn_*` 给 teammate 绕行.
- 支持临时招募与长期编制, 支持保存模板, 支持批量与差异化.

招募对象与 scope:

- `team_recruit`: 招募 team members (以及可选: 由 owner/President 任命 leaders)
- `org_recruit`: 招募 team owners (以及 org 级治理角色)

模板:

- Team scope templates: 让 leader 快速补齐成员 (UI/PM/DEV/QA 等)
- Org scope templates: 让 President 快速补齐 owners (中层管理)
- 模板需要支持:
  - `spawn` 偏好 (role/model/worktree/background 等)
  - `profile` 初始化 (可部分覆盖)
  - `quantity` + `overrides` (差异化招募)

无模板招募 (必须支持):

- leader/owner 可以在招募请求里直接提供 "成员画像草案" (例如需要 UI, 需要 PM, 需要两个开发, 需要测试), 工具将其持久化为 profile 的初始化内容 (必要字段缺失时允许为空).
- 若需要复用, 可将该画像草案提升为模板并通过 `team_template_upsert` / `org_template_upsert` 保存.

临时招募 (仅规划, 本期不实现):

- 支持为招募成员设置 `leaseUntil` 或 `temporary: true`, 以便任务结束后可被显式 demobilize:
  - `team_member_remove` / `org_owner_remove` (受权限约束)
  - 必须持久化并追加事件, 不得仅停留在内存

### 14.5 团队内协作 (Team Mesh)

产品目标:

- team 是一个完整协作单元, 成员之间可直接沟通与协作, 不再是围绕 lead 的星型结构.

最小能力:

- `team_current`: 让 teammate 自发现团队上下文 (teamId/orgId/角色)
- `team_info`: 让 teammate 在授权范围内读取团队元信息 (用于构造消息路由与 UI)
- `team_message`: member <-> member, leader <-> member, owner <-> member (均在 team scope 内)
- `team_broadcast`: 受 `broadcastPolicy` 约束的广播
- `team_inbox_pop` / `team_inbox_ack`: durable-first 的消息接收与确认

### 14.6 跨 Team 协作 (Org Principals Channel)

产品目标:

- 跨 team 的入口/出口必须收敛, 默认只允许 principals 互通, 再由 leader 向下传达.

最小能力:

- `org_create` / `org_update_config` / `org_info`
- `org_register_team` / `org_unregister_team` (或等价)
- `org_principal_message` (principal -> principal)
- `org_inbox_pop` / `org_inbox_ack`

边界强制:

- team members 不得使用 `org_*` 工具发送跨 team 消息.
- 通用工具对 target thread 的授权不得绕行 org/team 边界.

### 14.7 多 Assignee 任务 (单任务多人协作)

产品目标:

- leader 能将同一任务直接分派给多人, 由多人自组织拆分, 而不是 leader 先做边界切分.

最小能力:

- `team_task_create`: 支持 `assignees[]`, `claimMode`, `completionMode`
- `team_task_claim`: 支持 shared claim 或 exclusive claim
- `team_task_complete`: 记录 per-assignee 完成, 推导 task-level 完成
- `team_task_assign`: 动态增删 assignees
- `team_task_approve`: 当 `completionMode = leader_approves` 时启用

固定约束:

- per-assignee state 是权威真相, task-level `state` 只能派生.
- 任何状态迁移必须追加控制面事件, 并可从事件回放重建.

### 14.8 工程产物与评审 (Artifacts + Reviews)

产品目标:

- 用 artifact 将 "决策/评审/验收" 从聊天中抽离, 形成可回放的事实源.
- 让 review 成为可授权、可审计、可拒绝的控制面动作, 不依赖 prompt 约定.

最小能力 (仅规划):

- Team scope:
  - `team_artifact_publish` / `team_artifact_read` / `team_artifact_list`
  - `team_artifact_review_request` (leader/owner 发起, 指定 reviewers)
  - `team_artifact_review_submit` (reviewer 提交 `approve|reject` + notes)
- Org scope (跨 team 的 RFC/复盘等):
  - `org_artifact_publish` / `org_artifact_read` / `org_artifact_list`
  - `org_artifact_review_request` / `org_artifact_review_submit` (仅 principals)

固定约束:

- review 不改变授权边界, 但可以作为 task 完成/发布的前置条件 (与 `leader_approves` completionMode 对齐).
- review 结果必须写入控制面事件日志, 并可从 events 回放重建.

### 14.9 团队章程与工作协议 (Team Charter + Working Agreement)

产品目标:

- 让 "团队负责什么" 和 "如何协作" 变成持久化、可引用的组织知识, 便于新人/新招募成员快速对齐.

最小字段 (建议, 仅用于语境注入, 不参与授权):

- Team charter:
  - `mission` (团队使命)
  - `owned_areas[]` (负责的业务域/系统/组件, 自由文本)
  - `interfaces[]` (对外接口/边界, 自由文本)
  - `oncall` (自由文本: 轮值/响应口径/升级路径)
- Working agreement:
  - `definition_of_done` (DoD, 最小完成定义)
  - `review_policy` (哪些变更必须 review, 由谁 review)
  - `artifact_policy` (哪些信息必须以 artifact 形式沉淀, 禁止只在聊天里)

落地方式 (增量):

- 初期作为 artifact + config 引用: `teamCharterArtifactId`, `workingAgreementArtifactId`.
- 后续在 `team_update_config` 中支持更新这些引用, 并以事件日志审计.

### 14.10 变更管理 (Release / Rollout / Rollback) (仅规划)

产品目标:

- 将 "发布/上线/回滚" 从隐式聊天变成可审计的控制面动作, 支持灰度与回滚条件收敛.

最小能力 (仅规划):

- `team_release_plan_create`: 生成 `release_plan` artifact, 绑定 task, 记录风险与回滚触发.
- `team_release_approve`: owner/leader 显式批准执行 (带事件日志).
- `team_release_abort`: 显式终止并记录原因 (带事件日志).

固定约束:

- release gate 不得隐式通过; 失败必须显式、可定位、可回放.

### 14.11 运行与事故 (Incident + Postmortem) (仅规划)

产品目标:

- 支持现代公司的运行闭环: 发现 -> 止血 -> 修复 -> 复盘 -> 行动项.

最小能力 (仅规划):

- `org_incident_create` / `org_incident_update` / `org_incident_close` (仅 principals)
- incident 关联 artifacts: `runbook`, `patch`, `postmortem`
- incident 关联 tasks: 可为多个 teams 创建任务并追踪完成

固定约束:

- incident 的跨 team 协调仍遵循 principals channel; 不新增 member 级跨 team 通道.

### 14.12 Backlog、里程碑与跨 Team 项目 (仅规划)

产品目标:

- 贴近真实研发组织的 "计划-执行-复盘" 节奏, 需要可持久化的 backlog 与跨 team 项目视图, 而不是只靠即时聊天记忆.

最小能力 (仅规划):

- Team backlog:
  - tasks 增加 `priority` / `labels[]` / `milestone` 等字段 (元数据, 不参与授权)
  - `team_task_list` 支持按 priority/milestone 过滤与排序 (避免大团队任务淹没)
- Org initiative (跨 team 项目):
  - `org_initiative_create` / `org_initiative_update` / `org_initiative_close` (仅 principals)
  - initiative 绑定多个 team tasks/artifacts, 并提供只读汇总视图供 President 监督

固定约束:

- initiative 是 "索引与汇总层", 不能绕过 team 的授权与边界; 细节仍在 team scope 内完成.

## 15. 非功能需求 (NFR) 与治理

### 15.1 可恢复与可审计

- durable-first: 先持久化, 再尽力投递.
- append-only events: 必须能从 `events.jsonl` 重建关键状态 (team/org 关键对象).
- 幂等: 所有写操作必须支持幂等或显式拒绝重复, 不得产生半成功状态.

### 15.2 成本与配额 (仅规划)

Agent Org 会把 "招募" 变成一等能力, 需要配额与预算以防失控:

- org/team 级:
  - `maxAgents`, `maxTeams`, `maxRecruitPerHour`
  - `maxParallelTools` (与 `2026-03-06` 的 Budget 对齐)
- 运行时:
  - 超额时必须显式失败, 或进入显式可见的 degrade 模式 (不得静默)

### 15.3 隐私与数据最小化

Profile 可能包含敏感信息, 产品必须坚持最小化原则:

- profile/template 的默认建议是 "职业画像", 避免收集无必要的个人信息.
- 必须允许成员随时更新自己的 profile 内容.
- 控制面日志不得复制 profile 全量内容; 只记录变更元信息 (例如 threadId, templateId, 变更时间戳).

### 15.4 质量门禁与可复现 (仅规划)

- Agent Org 不替代 CI, 但必须提供可挂接的门禁点:
  - task-level 完成前的 validator (例如要求存在 `patch` artifact, 或存在 review 结果)
  - `leader_approves` completionMode 的显式批准工具
- Definition of Done 必须可被组织固化 (team working agreement), 且在 kickoff 时对 assignees 可见.
- 所有门禁失败必须显式失败并可定位, 禁止静默放行或假成功.

### 15.5 安全与授权 (Pinned)

- profile/模板/文化等元数据不参与授权, 且不得产生 "自封角色" 的可达路径.
- 边界强制必须是代码层 target-based authorization, 不能依赖 prompt 约定.
- 控制面写接口必须校验 caller 身份与角色, 并记录审计事件 (谁在什么时候改了什么).

### 15.6 可观测性与可调试性

- 必须能从 `$CODEX_HOME` 的持久化状态回答 "谁在什么 team/org 里, 拥有什么角色, 做了哪些控制面动作".
- 必须提供最小自描述工具面 (例如 `team_current` / `team_info` / `org_info`), 避免 out-of-band 粘贴.
- 对隐私字段做最小化记录: 事件日志记录引用与摘要, 不复制大字段.

## 16. 风险、边界场景与开放问题

必须显式记录并在实现前给出决策:

1. 多 org: 一个 thread 是否允许同时属于多个 org
1. 成员迁移: thread 是否允许在 teams 之间移动, 迁移时 inbox/tasks 如何处理
1. 归档与清理: team/org 的删除语义, events/inbox 的保留策略
1. 命名冲突: 批量招募时 displayName/name 重复如何处理 (自动编号或显式拒绝)
1. 招募失败回滚: 部分成员 spawn 成功, 部分失败时的事务语义 (必须可预测且可审计)
1. 角色变更: leader 任命/撤销的生效时序, 与正在进行的任务/消息如何一致
1. 深度上限: 子代理深度上限触发时, 哪些工具仍需保留以保证可观测与自描述
1. 噪声与滥用: team mesh/broadcast 产生刷屏时如何治理 (默认策略、限频、冷却窗口)
1. 事件日志增长: events/inbox 的 compaction/归档策略, 以及 cursor 的稳定性
1. 产物保密: artifacts 的访问控制与跨 team 引用边界 (尤其是包含敏感信息时)
1. 多 assignee 卡死: assignee 被移除/线程关闭/长时间无心跳时, 任务如何自动恢复与重新分派
1. 招募失控: 配额/预算不足时的显式失败语义, 以及对正在运行任务的影响
1. 真实世界一致性: "离线测试全绿" 与 "真实运行流程" 的差距如何通过 gate 明确表达与收敛

本提案后续每次实现一个新控制面工具, 都必须对上述问题给出本期的明确决策与可验证行为.
