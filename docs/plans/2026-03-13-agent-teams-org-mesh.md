# Agent Organization (Agent Org): 组织层级 + 团队内 Mesh 协作 + 招募系统 (设计提案)

日期: 2026-03-13
最后更新: 2026-03-14

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

1. **双节奏与背压是硬约束 (Agent-time vs Human-time)**
   - Agent 的产出速度与工作时长 (7x24) 远超人类, 但评审/审批/发布等门禁常由 principals 或人类承担, 存在不可消除的时滞.
   - 因此所有可能产生高频输出的路径 (artifacts/reviews/status/broadcast) 必须提供背压机制:
     - 限频/配额 (超额显式失败或显式进入可见的降级状态, 禁止静默丢弃)
     - 汇总/摘要 (用 artifact 显式沉淀, inbox 只投递引用)
     - 冷却窗口 (避免短时间内的重复请求与门禁振荡)

1. **成员归属默认单值 (one thread, one team, one org)**
   - 默认一个 thread 同一时刻只能属于一个 team, 用于保证 `team_current` 与工具授权语义确定.
   - team 只能挂接到一个 org (`teams/<team_id>/config.json.orgId` 单值); thread 的 org 归属由其 team 的 `orgId` 派生.
   - President/owners 的跨 team 能力来自 principals channel 与 override 权限, 不是 "同时加入多个 team" 的成员身份.

## 0.2 术语与命名

当前 v1 实现里 "lead" 指代 "spawn team 的那个 thread". Agent Org 引入 owner/leader 层级与 profile/招募, 因此必须消歧:

- **Agent Org (Org)**: 一个可治理的多 team 组织边界, 包含 org 元数据、team 注册表、跨 team 通信边界与审计事件.
- **总裁线程 (President thread)**: 面向用户的主线 agent thread (root thread), 负责监管 org.
- **团队所有者 (Team owner)**: 负责团队生命周期与团队治理的 thread. 为兼容性, 在 team config 中持久化为 `leadThreadId`. team owner 可以是 President, 也可以是 President 招募并委派的中层 owner.
- **团队负责人 (Team leader)**: 团队内的委派 leader, 持久化在 `leaders[]`. leaders 拥有团队控制面权限 (任务、跨 team 沟通、受策略约束的 broadcast、招募团队成员等).
- **团队成员 (Team member)**: 团队内的普通成员 (非 leader).
- **组织元数据 (Org metadata)**: org 的使命、愿景、文化价值观、背景环境 (默认 "当下真实世界"). Vibe 扩展暂不规划.
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
  - L2: 长链路验证 (TUI overlays + app-server v2 资源化接口) 作为后续阶段门禁, 不作为早期必要条件 (路线图见 17.2).
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

## 0.7 Agent 世界差异与治理策略 (Pinned)

Agent Org 复用了人类组织的层级与流程, 但必须针对 Agent 世界的物理差异做优化, 否则会在规模化时出现 "高吞吐 + 高噪声 + 高振荡":

1. **产出极快**
   - 关键矛盾从 "产出不足" 变成 "审计、门禁与注意力不足".
   - 若没有背压, 会快速堆积 artifacts/reviews/inbox, 并放大存储与成本.

1. **7x24 持续运行**
   - 系统没有天然的 "下班" 作为缓冲, 需要主动的限频、冷却与汇总机制.
   - 需要将 "人类在线窗口" 与 "系统持续推进" 解耦: 可以推进到明确的 "待评审" / "待批准" 状态, 但禁止隐式越过门禁.

1. **门禁时滞不可消除**
   - principals 的裁决、外部 CI、真实环境验证都具有时滞, 快速执行器会在时滞下产生超调与振荡.
   - 设计必须显式处理 anti-windup: 当门禁队列已饱和时, 禁止继续叠加同类请求, 需要合并、降噪或排队.

对应的产品原则:

- 把 "快" 用在产出与检验上, 把 "慢" 固化为显式门禁与可审计裁决.
- 用 artifacts 承载事实, 用 reviews 承载门禁, 用 status/digest 承载对外汇报, 避免用高频消息代替共享事实源.

## 0.8 组织工程操作系统 (Harness-first + CSE) (Pinned)

目标:

- 让每个 agent 在执行任务时默认以 harness 为主: 先证据后结论, 以真实工具链输出作为事实依据.
- 让 principals (owner/leader/president) 在 harness 之上承担控制器职责: 明确 setpoint/护栏/时滞/回滚触发, 避免在 7x24 高吞吐下失控.

角色要求:

- **Team member:**
  - 必须内化 `$harness-engineering` 的执行方式 (证据驱动、预算化探索、阈值触发求助、可追溯交付).
  - 不要求对系统级控制拓扑做裁决, 但必须提供可复现证据, 避免把决策压力无证据地上抛.

- **Team leader / Team owner / President:**
  - 必须同时内化 `$harness-engineering` 与 `$cybernetic-systems-engineering`.
  - 对本 scope 的 setpoint 与边界负责:
    - leader/owner: team scope 的任务门禁、review 规则、背压策略与恢复.
    - president: org scope 的跨 team 边界、路由策略、配额预算与升级路径.

植入机制 (不依赖授权, 但必须默认存在):

- 通过 artifacts 固化并版本化两份 playbook:
  - `harness_playbook` (所有成员)
  - `cse_playbook` (principals)
  - 最小内容要求 (harness_playbook):
    - 成功定义与边界 (交付物/验收/不可改范围)
    - 里程碑与证据类型 (harness_run/patch/test_plan/digest)
    - 探索预算与尝试预算 (何时必须换策略/升级)
    - 求助阈值与结构化求助模板 (选项型提问)
  - 最小内容要求 (cse_playbook):
    - Control Contract 模板 (setpoint/验收/护栏/时滞/回滚触发)
    - 多模型分析口径 (数据流/状态机/排队与时滞)
    - 分层验证与 gate 边界 (L0/L1/L2)
    - 复杂性转移账本与升级路径 (跨模块裁决)
- `team_onboard` / `org_onboard` 生成的 onboarding packet artifact 必须包含上述 playbook 的引用, 作为入职/刷新对齐的可审计事实源.
- 当 `team_task_create` 创建任务并分派 assignees 时, kickoff 信息必须引用 harness playbook, 并在 `completionMode=leader_approves` 或存在 review gate 时, 引用 cse playbook (用于解释门禁与证据要求).

## 0.9 组织控制回路与关键变量 (CSE) (Pinned)

Agent Org 的核心不是 "像公司", 而是把多 agent 协作变成一个可控系统. 因此必须显式定义闭环:

1. 任务交付回路 (Plan/Build/Ship)
   - 参考输入: task 的目标与验收 (title/description + working agreement/DoD)
   - 输出: artifacts (patch/test_plan/harness_run) + review 结论 + task approve
   - 主要时滞: reviews/human gate/真实环境验证
   - anti-windup 执行器:
     - `completionMode=leader_approves` + `team_task_approve` 显式收口
     - `maxOpenReviews` + `review_bundle` 限制门禁队列膨胀

1. 沟通降噪回路 (Message/Digest)
   - 参考输入: 让信息在团队内可达且不刷屏
   - 输出: durable inbox + (尽力)实时投递 + digest/status 汇总
   - 扰动: 7x24 高频产出导致的消息洪峰
   - 执行器:
     - `quietHours` + `priority` 抑制普通实时投递
     - 限频/配额触发时, 明确要求以 digest/rollup artifact 汇总再投递引用

1. 人力供给回路 (Staffing/Recruit/Onboard/Offboard)
   - 参考输入: 任务需求与当前产能缺口
   - 输出: 新成员加入 (profile + playbooks onboarding) 或显式移除
   - 扰动: 深度上限/预算上限/并行改动互相覆盖
   - 执行器:
     - `team_recruit` / `org_recruit` 受控招募 + 配额/预算
     - 默认工作区隔离 (worktree) + 证据可追溯 (harness_run)

1. 组织记忆回路 (Artifacts/Events Retention)
   - 参考输入: 可审计、可回放、可检索
   - 输出: append-only events + 可引用 artifacts
   - 扰动: 高吞吐导致存储增长
   - 执行器:
     - 可审计的归档/压缩 (tombstone/segment compaction), 禁止静默删除

固定要求:

- 对任何 "离线通过" 的证据, 必须显式标注未覆盖项 (uncovered) 与可接受风险, 以避免把时滞与环境差异隐藏到聊天结论里 (见 14.16).

## 1. 目标

1. 团队成员可以通过 team-scoped 工具直接互相协作.
1. team leader 可以把 1 个任务分派给多个成员, 由成员自组织拆分.
1. 跨 team 通信被约束为 principals (owners/leaders, 以及可选的 President) 才能执行, 且只有一个受控入口/出口.
1. 面向用户的主线 agent 作为 "President", 负责监督各 team leaders 与整体进展.
1. leader/owner/president 可按需招募下级角色 (成员/leader/owner), 并支持保存可复用的招募模板与批量差异化招募.
1. 所有成员可维护自己的 `AgentProfile`, 并在启用 Agent Org 时将 org/team/profile 元数据注入协作语境以提升一致性 (但不影响授权).
1. 所有消息仍保持 durable-first (先持久化, 再尽力实时投递).

## 2. 非目标

1. 完整的 "嵌套团队": v2 第一阶段不做; 路线图见 17.4 (嵌套团队必须带治理与配额, 不允许自由无治理 spawn).
1. 分布式、多进程控制面. 本提案仍保持类似 v1 的 in-process + 文件持久化.
1. "Vibe" 的完整世界观系统暂不规划. 本提案只提供 org 背景环境的默认值与持久化字段.
1. 新的聊天 UI: v2 第一阶段不做; 路线图见 17.3.

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
  "playbooks": {
    "harnessPlaybookArtifactId": "art-harness-1",
    "csePlaybookArtifactId": "art-cse-1"
  },
  "quietHours": {
    "enabled": false,
    "timezone": "local",
    "ranges": [{ "start": "22:00", "end": "08:00" }]
  },
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
  "playbooks": {
    "harnessPlaybookArtifactId": "art-harness-1",
    "csePlaybookArtifactId": "art-cse-1"
  },
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
  "priority": "normal",
  "taskId": "task-1",
  "sequence": 42,
  "causalParent": "taskClaim:task-1:thread-alice",
  "delivery": { "deliveredLive": true, "suppressedReason": null },
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
  "priority": "normal",
  "sequence": 7,
  "causalParent": null,
  "delivery": { "deliveredLive": true, "suppressedReason": null },
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

#### 4.4.10 Artifacts 元数据 (Team/Org) (仅规划)

Artifacts 的目标是让 "共享" 显式化: 不共享上下文, 只共享可引用的产物.

推荐 `kind` (非穷举):

- 工程决策: `rfc`, `adr`
- 交付证据: `patch`, `test_plan`, `release_plan`
- 可复现执行: `harness_run`
- 运行知识: `runbook`, `postmortem`
- 双节奏汇总: `digest`, `status_report`, `review_bundle`
- 升级求助: `help_request`
- 执行协议: `harness_playbook`, `cse_playbook`

- Team scope:
  - 元数据: `$CODEX_HOME/teams/<team_id>/artifacts/<artifact_id>.json`
  - 内容: `$CODEX_HOME/teams/<team_id>/artifacts/content/<artifact_id>.<ext>`
- Org scope:
  - 元数据: `$CODEX_HOME/orgs/<org_id>/artifacts/<artifact_id>.json`
  - 内容: `$CODEX_HOME/orgs/<org_id>/artifacts/content/<artifact_id>.<ext>`

最小元数据 schema:

```json
{
  "schemaVersion": 1,
  "artifactId": "art-1",
  "scope": "team",
  "orgId": "org-123",
  "teamId": "demo-team",
  "taskId": "task-1",
  "kind": "rfc",
  "title": "Design: Artifacts + Reviews",
  "summary": "Short summary for indexing",
  "createdAt": 1739988000,
  "createdByThreadId": "thread-alice",
  "createdByName": "alice",
  "visibility": "team_members",
  "contentRef": "artifacts/content/art-1.md",
  "contentMimeType": "text/markdown",
  "contentDigest": "sha256:...",
  "sizeBytes": 1234,
  "rollupOfArtifactIds": null,
  "supersedesArtifactId": null
}
```

固定约束:

- artifact 默认不可变; 修改应通过发布新 artifact 并用 `supersedesArtifactId` 形成版本链, 禁止原地覆写导致审计断裂.
- 对于高频产出, 推荐使用 "汇总 artifact" (例如 `kind=digest|status_report|review_bundle`) 承载多个细碎产物的摘要与引用:
  - `rollupOfArtifactIds` 用于索引与可追溯 (内容仍在 `contentRef` 中表达)
  - inbox 仅投递汇总 artifact 引用, 避免刷屏
- `visibility` 是控制面字段 (影响读取授权), 不得由 profile/模板/prompt 推导:
  - team scope 默认 `team_members`
  - org scope 默认 `principals_only` (避免绕过 principals channel)
  - 推荐枚举值 (按 scope):
    - team: `team_members` | `team_leaders` | `team_owner`
    - org: `org_members` | `principals_only`
  - `org_members` 的定义: 任一已注册到该 org 的 team 的 members/leaders/owner 均视为 org member (由 team configs 派生).
- 完整性校验 (Pinned):
  - `contentDigest` 是事实源的一部分. `*_artifact_read(include_content=true)` 必须在读取时校验 digest/size, 校验失败必须显式报错, 禁止返回疑似被篡改的内容.
- 跨 team 共享必须显式发生在 org scope:
  - 禁止其他 team 的成员直接读取 team scope artifact (即便知道 artifact id).
  - 若需要跨 team 共享, 必须通过 `org_artifact_publish` 重新发布为 org scope artifact (并由 `visibility` 控制读取授权).

#### 4.4.11 Reviews (Team/Org) (仅规划)

Review 是控制面门禁: 产物是否可被采纳/合并/发布, 必须显式记录并可回放.

- Team scope: `$CODEX_HOME/teams/<team_id>/reviews/<review_id>.json`
- Org scope: `$CODEX_HOME/orgs/<org_id>/reviews/<review_id>.json`

最小 review schema:

```json
{
  "schemaVersion": 1,
  "reviewId": "rev-1",
  "artifactId": "art-1",
  "scope": "team",
  "orgId": "org-123",
  "teamId": "demo-team",
  "status": "open",
  "priority": 0,
  "policy": { "minApprovals": 1 },
  "requestedAt": 1739988001,
  "requestedByThreadId": "thread-leader-a",
  "supersedesReviewId": null,
  "reviewers": ["thread-qa-1", "thread-leader-a"],
  "evidenceArtifactIds": ["art-harness-run-1"],
  "decisions": [
    {
      "threadId": "thread-qa-1",
      "decision": "approve",
      "decidedAt": 1739989000,
      "notes": "LGTM"
    }
  ],
  "expiresAt": null,
  "closedAt": null,
  "closedReason": null
}
```

固定约束:

- review 的 `approve/reject` 必须可被工具强制执行 (例如作为 task approve/complete 的前置条件), 不得只停留在聊天结论.
- review 允许过期/撤回/重开, 但任何状态迁移必须追加事件日志, 并可回放重建.
- Review 在 Agent 世界中是稀缺资源 (门禁吞吐远小于产出吞吐), 因此必须具备队列语义:
  - `priority` 用于排序与背压 (高优先级优先被处理)
  - 达到配额/队列上限时必须显式拒绝新增 review request (或要求改为 review_bundle/digest), 禁止无限堆积与静默降级
- 状态机 (Pinned):
  - `status` 推荐枚举: `open` | `approved` | `rejected` | `expired` | `cancelled`
  - `team_review_submit` 可能触发 `open -> approved|rejected` 的状态迁移 (取决于 policy 与 decision).
  - `expiresAt` 到期后可触发 `open -> expired` (过期不等于通过; 门禁评估必须视为不满足).
  - `team_review_cancel` 触发 `open -> cancelled` (用于 superseded artifact、误触发请求、或背压下的合并).
- anti-windup (Pinned):
  - 同一 `(scope, artifactId)` 同时最多允许存在 1 个 `status=open` 的 review.
  - 当已存在 open review 时, `*_review_request` 必须显式拒绝或返回已存在的 `reviewId` (由实现选其一, 但必须可审计且确定).

#### 4.4.12 Onboarding 记录 (Team) (仅规划)

Onboarding 的目标是降低组织扩张时的对齐成本, 让 "入职包" 可审计、可重放、可复用.

- `$CODEX_HOME/teams/<team_id>/onboarding/<thread_id>.json`

最小 schema:

```json
{
  "schemaVersion": 1,
  "orgId": "org-123",
  "teamId": "demo-team",
  "threadId": "thread-alice",
  "onboardedAt": 1739988000,
  "onboardedByThreadId": "thread-leader-a",
  "onboardingArtifactId": "art-onboarding-1",
  "starterTaskIds": ["task-1"],
  "ackedAt": null
}
```

固定约束:

- onboarding 的 "内容" 应通过 artifact 表达 (避免塞进长消息); inbox 只发送引用与必要提醒.
- `ackedAt` 仅表示收悉 (例如通过 `team_inbox_ack` 或专用 ack), 不代表完成训练/理解.

#### 4.4.13 Offboarding 记录 (Team) (仅规划)

Offboarding 的目标是让成员移除/调岗成为可恢复的控制面动作, 不留下权限与任务的悬挂状态.

- `$CODEX_HOME/teams/<team_id>/offboarding/<thread_id>.json`

最小 schema:

```json
{
  "schemaVersion": 1,
  "orgId": "org-123",
  "teamId": "demo-team",
  "threadId": "thread-alice",
  "removedAt": 1739990000,
  "removedByThreadId": "thread-leader-a",
  "reason": "temporary_contract_end",
  "closeAttempted": true,
  "closeError": null,
  "affectedTaskIds": ["task-1"]
}
```

固定约束:

- offboarding 必须先收敛授权真相 (更新 config), 再做 best-effort live close; live close 失败不得回滚授权收敛, 但必须显式记录错误.

#### 4.4.14 Ownership map (Org) (仅规划)

Ownership map 的目标是把 "责任边界" 固化成可审计、可解释的路由依据, 用于建议分派而非授权.

- `$CODEX_HOME/orgs/<org_id>/ownership/map.json`
- `$CODEX_HOME/orgs/<org_id>/ownership/map.lock`

最小 schema:

```json
{
  "schemaVersion": 1,
  "orgId": "org-123",
  "updatedAt": 1739988000,
  "updatedByThreadId": "thread-president",
  "rules": [
    {
      "ruleId": "r-core",
      "patternType": "path_glob",
      "pattern": "codex-rs/core/**",
      "teamId": "core-team",
      "priority": 100
    },
    {
      "ruleId": "r-docs",
      "patternType": "path_glob",
      "pattern": "docs/**",
      "teamId": "docs-team",
      "priority": 10
    }
  ]
}
```

匹配与解释 (Pinned):

- 输入可包含多个候选 path/module; 对每个输入独立匹配, 输出 "命中规则列表" + "最终选择".
- 选择规则:
  1. `priority` 高者优先
  1. 同 priority 时, 更具体者优先 (例如 pattern 字符串更长)
  1. 再同则按 `ruleId` 稳定排序, 保证确定性

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
  - `playbooks` (可选):
    - `harness_playbook_artifact_id` (可选)
    - `cse_playbook_artifact_id` (可选; 仅对 principals 生效)
  - `quiet_hours` (可选):
    - `enabled` (可选)
    - `timezone` (可选; 默认 local)
    - `ranges` (可选): `{ start: "HH:MM", end: "HH:MM" }[]`
- Outputs:
  - 更新后的 team 元信息 (至少包含: `team_id`, `org_id`, `leaders`, `broadcast_policy`, `playbooks`)

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
- 初始化 org-level playbooks:
  - 发布 `kind=harness_playbook` artifact (用于所有成员, `visibility=org_members`)
  - 发布 `kind=cse_playbook` artifact (用于 principals, `visibility=principals_only`)
  - 将其 artifact id 写入 org config 的 `playbooks.*`

#### 4.5.4 `org_register_team` (new, president-only)

将 team 挂接到 org 并维护引用一致性:

- Writes:
  - 更新 `orgs/<org_id>/config.json`, 写入 team 与其 leaders
  - 更新 `teams/<team_id>/config.json`, 写入 `orgId = <org_id>`
- 必须幂等, 且需要分别追加 org/team 的控制面事件.

#### 4.5.4b `org_unregister_team` (new, president-only)

将 team 从 org 脱离并维护引用一致性:

- Inputs:
  - `org_id`
  - `team_id`
- Writes:
  - 更新 `orgs/<org_id>/config.json`, 将 `teams[]` 中对应条目移除
  - 更新 `teams/<team_id>/config.json`, 写入 `orgId = null`
- 必须幂等, 且需要分别追加 org/team 的控制面事件.

固定要求:

- `org_unregister_team` 不得删除 team 的 inbox/tasks/artifacts; 只改变 org 归属与跨 team 沟通边界.
- 若 team owner/leader 仍需与 org 内 principals 协调, 必须先重新 register; 禁止通过残留的 org inbox 条目绕过边界.

#### 4.5.5 `org_update_config` (new, president-only)

以受控方式更新 org 持久化配置 (`$CODEX_HOME/orgs/<org_id>/config.json`) 的元数据字段:

- `org_name` (可选)
- `environment` (可选; 默认 `{ vibeId: "real_world_now" }`)
- `mission` / `vision` / `values` (可选)
- `playbooks` (可选):
  - `harness_playbook_artifact_id` (可选)
  - `cse_playbook_artifact_id` (可选)

固定要求:

- 写入必须原子化, 并追加 `org.config.updated` 事件.
- 输出应包含更新后的 `playbooks` 引用 (当存在更新时), 以便调试与审计.

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

- `recruited`: 每个新招募成功的 thread 的 `{ name, agent_id, role, template_id? }`
- `failed`: 每个失败条目的 `{ index, template_id?, error }` (仅规划)

固定要求:

- 招募必须是可审计的控制面动作, 追加事件 (`team.member.recruited`, `team.leader.recruited`, `org.owner.recruited` 等).
- 招募成功后必须更新 team/org config 的成员引用, 并确保 inbox/任务等路径可用.
- 招募成功后必须默认触发 onboarding 对齐 (最少投递 playbooks 引用):
  - 所有新招募成员必须植入 `$harness-engineering` 执行协议.
  - 新招募 principals (owner/leader/president) 必须额外植入 `$cybernetic-systems-engineering` 控制协议.
- 批量招募不是全有或全无事务:
  - 允许部分成功, 但必须在输出中显式标注失败条目与错误原因.
  - 不得为 "看起来成功" 而静默忽略失败或悄悄重试; 需要重试必须由调用方显式触发.
- 命名必须可确定:
  - team config 内的 `members[].name` 必须唯一; 若模板或 overrides 生成重复 name, 工具必须自动去冲突 (例如追加序号) 或显式拒绝, 且行为可审计.

#### 4.5.9 `team_artifact_publish` / `team_artifact_read` / `team_artifact_list` (new)

Artifacts 属于状态面事实源, 必须通过工具写入并追加事件:

- Team scope: 写入 `$CODEX_HOME/teams/<team_id>/artifacts/...`
  - 授权: team members/leaders/owner
- Org scope: 写入 `$CODEX_HOME/orgs/<org_id>/artifacts/...`
  - 授权:
    - publish: principals-only
    - read/list: 由 artifact `visibility` 决定:
      - `org_members`: 任一 org member 可读
      - `principals_only`: principals-only
- 必须执行大小与频率配额, 超额显式失败; 高频产出应优先通过 digest/rollup artifact 汇总再投递引用.

#### 4.5.10 `team_review_request` / `team_review_submit` / `team_review_cancel` / `team_review_read` / `team_review_list` (new)

Reviews 是控制面门禁:

- Team scope: `$CODEX_HOME/teams/<team_id>/reviews/...`
  - 授权:
    - request: team leader/owner (President override)
    - submit: 指定 reviewer
    - cancel: team leader/owner (President override)
    - read/list: team members/leaders/owner (按 artifact visibility 约束)
- Org scope: `$CODEX_HOME/orgs/<org_id>/reviews/...`
  - 授权: principals-only
- 必须执行 review 队列背压与配额 (例如 `maxOpenReviews`), 队列饱和时 `*_review_request` 必须显式失败或要求改为 review_bundle/digest.

#### 4.5.11 `team_onboard` / `team_member_remove` (new)

Onboarding/Offboarding 是成员状态迁移, 既影响语境也影响授权边界:

- `team_onboard`
  - 授权: team leader/owner (President override)
  - 必须写入 onboarding 记录并追加事件
- `team_member_remove`
  - 授权: team leader/owner (President override)
  - 必须先收敛授权真相 (更新 config), 再做 best-effort close, 并写入 offboarding 记录与事件

#### 4.5.12 `org_ownership_map_upsert` / `org_route_task_suggest` (new)

Ownership routing 属于 org 控制面/状态面:

- `$CODEX_HOME/orgs/<org_id>/ownership/map.json`
- 授权: principals-only
- 必须追加 `org.ownership_map.upserted` 事件, 并保证匹配的确定性与可解释性.

### 4.6 幂等性、锁与事件覆盖 (Pinned)

为保证 durable-first 语义在并发与重启场景下正确:

- **原子写:** 对 `config.json` 与 task snapshot 的更新必须采用 write-temp-then-rename (禁止产生半截 JSON).
- **互斥锁:** 所有 JSONL append 面必须使用 per-file lock (v1 已有 inbox lock; events logs 也必须加锁).
- **sequence 分配:** 在持有对应 scope (team/org) 的 `events.lock` 时分配 `sequence`, 然后将其写入对应的 inbox/event entry.
- **幂等:** task-level 完成迁移与 hooks 必须只触发一次; 重复调用必须显式报错或显式 no-op, 但不得静默 "半成功".
- **cursor 稳定性:** 所有 list/read API (inbox/events/artifacts/reviews) 应优先使用 `sequence` 作为 cursor, 避免依赖文件遍历顺序导致的不确定性.

最小事件覆盖 (仅当状态实际变化时才向 `events.jsonl` 追加):

- Team scope:
  - `team.config.updated`
  - `team.message.appended`
  - `team.template.upserted`
  - `team.member.recruited`
  - `team.member.onboarded`
  - `team.member.removed`
  - `team.leader.recruited`
  - `team.artifact.published`
  - `team.review.requested`
  - `team.review.submitted`
  - `team.review.cancelled`
  - `team.task.created`
  - `team.task.assignees.updated`
  - `team.task.assignee.claimed`
  - `team.task.assignee.completed`
  - `team.task.approved`
- Org scope:
  - `org.created`
  - `org.config.updated`
  - `org.team.registered`
  - `org.team.unregistered`
  - `org.artifact.published`
  - `org.review.requested`
  - `org.review.submitted`
  - `org.review.cancelled`
  - `org.template.upserted`
  - `org.profile.updated`
  - `org.owner.recruited`
  - `org.ownership_map.upserted`
  - `org.principal.message.appended`

### 4.7 元数据流 (Metadata Flow)

Agent Org 在持久化控制面中引入了多类元数据, 需要明确它们如何流动, 以及哪些流动会影响运行时行为:

1. Org metadata
   - 来源: `org_update_config`
   - 载体: `$CODEX_HOME/orgs/<org_id>/config.json` + `events.jsonl`
   - 用途:
     - 构造 org-level 协作语境 (使命/愿景/文化/环境)
     - 固化 org-level playbooks (harness/cse) 的引用, 并在招募与 onboarding 阶段注入到下级 agent 的执行协议中

1. Team control-plane metadata
   - 来源: `team_update_config`
   - 载体: `$CODEX_HOME/teams/<team_id>/config.json` + `events.jsonl`
   - 用途:
     - 授权真相 (members/leaders/owner), 团队内消息策略, 任务分派边界
     - team-level playbooks 覆盖 (用于将 org 级执行协议调整为更贴近该团队的 harness/CSE 口径)

1. AgentProfile
   - 来源: `org_profile_update_self` (self-only) 与招募初始化
   - 载体: `$CODEX_HOME/orgs/<org_id>/profiles/<thread_id>.json` + `events.jsonl`
   - 用途: 构造个体协作语境与角色分工, 不参与授权

1. Recruitment templates
   - 来源: `team_template_upsert` / `org_template_upsert`
   - 载体: `$CODEX_HOME/{teams/<team_id>|orgs/<org_id>}/recruitment/templates/*.json` + `events.jsonl`
   - 用途: 批量/差异化招募时的 spawn 偏好与 profile 初始化输入

1. Artifacts
   - 来源: `team_artifact_publish` / `org_artifact_publish`
   - 载体: `$CODEX_HOME/{teams/<team_id>|orgs/<org_id>}/artifacts/*.json` + `content/*` + `events.jsonl`
   - 用途: 显式共享产物 (PRD/RFC/patch/runbook/postmortem 等), 替代复制粘贴与隐式共享上下文

1. Reviews
   - 来源: `team_review_request` / `team_review_submit` / `team_review_cancel` / `team_review_read` / `team_review_list` (及 org 等价接口)
   - 载体: `$CODEX_HOME/{teams/<team_id>|orgs/<org_id>}/reviews/*.json` + `events.jsonl`
   - 用途: 显式门禁与审计, 为 task approve/complete 提供可验证证据

1. Onboarding/Offboarding
   - 来源: `team_onboard` / `team_member_remove` (以及 org 等价接口)
   - 载体: `$CODEX_HOME/teams/<team_id>/{onboarding|offboarding}/*.json` + `events.jsonl`
   - 用途: 组织扩张时的快速对齐 + 权限与任务的可恢复收口

1. Ownership map
   - 来源: `org_ownership_map_upsert`
   - 载体: `$CODEX_HOME/orgs/<org_id>/ownership/map.json` + `events.jsonl`
   - 用途: 路由建议与责任可见性, 减少 President 人工 triage 成本

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
- `role`: `"member" | "leader" | "owner"` (或空; `owner` 对应 `leadThreadId`)
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
- 可选: playbooks 引用 (harness/cse), 供成员自助读取执行协议与门禁口径
- 可选: quiet hours 配置 (用于决定是否应发送 urgent 与是否应优先做 digest)

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
   - `priority` (`normal` / `urgent`, 默认 `normal`)
   - `delivery.deliveredLive` / `delivery.suppressedReason` (用于解释 quiet hours/限频导致的实时投递抑制)

quiet hours 与优先级 (Agent 世界优化, Pinned):

- team config 可选 `quietHours`. 当启用且当前处于 quiet hours:
  - `priority=normal`: 必须先落 inbox, 但允许抑制实时投递 (`delivery.suppressedReason="quiet_hours"`).
  - `priority=urgent`: 允许绕过抑制并尝试实时投递, 但必须受限频与角色约束.
- `priority=urgent` 默认仅 principals 可用 (team leader/owner, 以及 team 内的 President). 非 principals 调用必须显式失败, 禁止滥用.

背压与 digest (Agent 世界优化, Pinned):

- `team_message` 不得静默丢弃:
  - 若触发限频/配额, 允许先落 inbox 并抑制实时投递 (`delivery.suppressedReason="rate_limited"`).
  - 若已达到持久化写入配额上限, 必须显式失败并返回可追溯错误 (例如 `rate_limit_exceeded`), 禁止在 7x24 下悄悄丢消息.
- 当出现持续的 `rate_limited` 抑制时, 发送方应改用 `kind=digest` artifact 汇总细碎产物, 然后投递引用 (避免刷屏与放大存储).

这将 team 变为有边界的 mesh, 且不暴露跨 team 消息能力.

#### 5.2.4 `team_broadcast` (策略 + 行为变更)

broadcast 很有用, 但也容易变噪声. v2 提议在 team config 中加入策略开关:

- `broadcastPolicy: "leaders_only" | "all_members"`

默认: `leaders_only`.

若为 `all_members`, 任意成员可 broadcast; 若为 `leaders_only`, 非 leader 必须使用 `team_message` 或通过 leader 协调.

quiet hours 与优先级 (Agent 世界优化, Pinned):

- broadcast 属于高噪声路径, 默认必须受 `quietHours` 约束:
  - quiet hours 内的普通 broadcast 允许被抑制实时投递 (仍需 durable-first 落 inbox).
  - `priority=urgent` 允许绕过抑制, 但默认仅 principals 可用, 且必须受强限频.

背压与 digest (Agent 世界优化, Pinned):

- broadcast 必须执行更严格的限频/配额, 超额必须显式失败或显式抑制并要求 digest:
  - 若选择抑制实时投递, 仍需 durable-first 落 inbox, 并写入 `delivery.suppressedReason`.
  - 若要求 digest, 返回的错误必须是可操作的 (例如提示发布 `kind=digest` 汇总 artifact 并投递引用), 禁止静默降级.

#### 5.2.5 `team_ask_lead` (行为变更)

v1 中 `team_ask_lead` 会向 spawning thread ("lead") 发消息. v2 中 "lead" 应优先解析为委派 leaders:

1. 当 `leaders[]` 非空时, `team_ask_lead` 投递给所有 team leaders.
1. 否则, 投递给 `leadThreadId` (President / team owner).
1. 仍保持 durable-first: 先写 inbox, 再尽力实时投递.

### 5.3 推荐协作协议 (prompt 级)

工具只提供通信能力; 协作质量依赖协作协议. 当某个任务被分配给多个 agent 时, 注入标准 kickoff 信息与执行协议:

1. 每个 assignee 用 2-4 个要点说明自己的计划与预期产物.
1. assignees 通过 `team_message` 协商边界与依赖关系.
1. 若出现冲突或歧义, 升级给 team leader 裁决.

成员侧必须内化 `$harness-engineering` 的最小行为:

1. 先定义成功与边界, 再动手 (避免无界重试与无证据结论).
1. 每轮迭代必须产出证据 (命令输出、测试结果、日志或可复现步骤), 以 artifact 或结构化消息引用.
1. 给自己设置探索预算与尝试预算; 同类失败 2-3 次必须换策略或升级求助, 禁止无证据重试.
1. 求助必须给出 2-3 个可选方案与取舍, 不是只抛出问题.

principals (owner/leader/president) 在 harness 之上必须内化 `$cybernetic-systems-engineering` 的控制责任:

1. 在 kickoff 或 gate 处明确本次 setpoint/验收/护栏/时滞与回滚触发 (见 0.5 控制合同).
1. 当存在 review/审批门禁时, 必须明确 "证据要求" 与 "不可越过的边界", 避免高吞吐下的隐式放行.
1. 当出现背压或队列饱和时, 必须做合并与降噪决策 (digest/review_bundle/status rollup), 禁止让系统在 7x24 下无限堆积.

这能让团队内保持自治, 且把证据与门禁显式化, 不要求 leader 在分派时做微观拆分.

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
- `staffing` (可选): 允许在分派任务时按需补充人力 (复用 `team_recruit` 语义)
  - `mode`: `best_effort | require_all` (可选, 默认 `best_effort`)
  - `recruits[]`: 与 `team_recruit.recruits[]` 同形态 (template_id/quantity/spawn_overrides/profile_overrides)
  - `temporary` (可选, 默认 false): 是否以 "临时编制" 招募 (仅规划; 见 14.4)
- `dependencies` (可选)
- `claim_mode` / `completion_mode` (可选)
- `kickoff: true|false` (可选, 默认 true): 为 true 时, 自动向所有 assignees 发送 kickoff 信息 (协作协议).
- `kickoff_template_id` (可选): kickoff 信息应优先通过模板引用生成 (见 14.18.7)
  - 默认: `assignees.len() > 1` 用 `kickoff-multi-assignee`, 否则用 `kickoff-simple`

授权:

- 仅 team leaders 或 President thread (team owner) 可调用.

执行语义 (Pinned):

- 若提供 `staffing`, 工具必须先做输入合法性校验 (title/assignees/dependencies 等) 再触发招募, 避免 "招到了人但 task 创建失败" 的不可控副作用.
- kickoff 信息应引用 kickoff 模板并仅追加本任务 delta, 避免每次重复注入大段协作协议导致 token 膨胀与口径漂移 (见 14.18.7).
- 招募与 task 创建的关系:
  - 招募是控制面动作, 不提供隐式回滚.
  - 若 `mode=require_all` 且存在任何 recruit 失败, 必须显式失败并返回失败原因 (已成功招募的成员仍会保留, 需要显式 remove 才会回收).
  - 若 `mode=best_effort`, 允许部分 recruit 成功, 但必须在输出中显式返回 `recruited`/`failed`, 且仅将成功招募的成员加入 `assignees`.
- 当 `staffing.temporary=true` 时, 必须在招募事件与 offboarding 记录中可追溯该成员的临时属性, 以便任务结束后显式 demobilize (仅规划; 见 14.4).

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
- 必须写入 `approvedAt` / `approvedByAgentId` 并追加 `team.task.approved` 事件.

证据口径 (Pinned):

- `team_task_approve` 必须支持携带或引用门禁证据, 避免仅靠聊天结论:
  - 允许输入 `review_id` (来自 `team_review_*`) 作为审批证据.
  - 允许输入 `artifact_ids[]` (例如 `patch` / `harness_run` / `test_plan`) 作为证据集合.
- 若 team working agreement 要求 review 或 harness 证据, 但调用未提供满足要求的证据, 必须显式失败.

真实世界一致性口径 (Pinned):

- `team_task_approve` 必须显式处理 evidence 的未覆盖项:
  - 允许输入 `accepted_uncovered[]` 作为风险接受清单 (例如 `windows_compat`, `real_db_schema`).
  - 当 evidence (例如引用的 `harness_run`) 声明了 `uncovered[]` 且不为空时:
    - 若未提供 `accepted_uncovered[]`, 必须显式失败 (避免隐式把离线通过当真实通过).
    - 若提供, `accepted_uncovered[]` 必须覆盖全部 `uncovered[]` 且必须写入 task snapshot 或事件日志 (可审计); 否则视为未收敛风险并失败.

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
1. `org_artifact_read` / `org_artifact_list`: 读取 org scope artifacts (按 `visibility` 控制读取授权).
1. `org_principal_message`: principal -> principal 消息, 依据 org config 校验.
1. `org_inbox_pop` / `org_inbox_ack`: 读取与 ack org 范围消息.

授权:

- `org_info` / `org_inbox_pop` / `org_inbox_ack`: principals-only
- `org_artifact_read/list`:
  - `visibility=org_members`: 任一 org member 可读
  - `visibility=principals_only`: principals-only
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
- `onboarding`: 入职包 (组织/团队必读 + 当前任务入口)
- `digest`: 摘要汇总 (双节奏收敛)
- `status_report`: 状态汇报 (时间窗口语义)
- `review_bundle`: 面向门禁的批量证据包 (降低 review 队列压力)
- `harness_run`: 可复现执行证据 (命令/退出码/耗时/环境/git)
- `help_request`: 阻塞求助与裁决选项 (带证据)
- `harness_playbook`: harness-first 执行协议
- `cse_playbook`: principals 的 CSE 控制协议
- `summary`: 周报/里程碑摘要

后续可跟进的控制面工具 (第一阶段非必需; 路线图见 17.1), 用于让 artifact 更易用:

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
1. assignees 发布 `patch` / `harness_run` / `test_plan` 等证据 artifacts; leader 发起 `team_review_request` 并在满足门禁后通过 `team_task_approve` 显式收口 (对齐 `leader_approves` 完成模式).
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
- `codex-rs/protocol` / `codex-rs/app-server(-protocol)`: envelope 补齐与资源化 API (用于多观察者订阅/重连回放, 可后置; 路线图见 17.2)
- `codex-rs/tui`: org/team 仪表盘与 inbox/task overlays (可后置, 先保证控制面与工具语义; 路线图见 17.2)
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

1. Artifacts + Reviews (仅规划到接口与持久化口径):
   - 新增 team/org scope 的 artifact publish/read/list.
   - 新增 review request/submit, 并与 `leader_approves` completionMode 对齐为显式门禁.

1. Team charter + working agreement (仅规划到配置引用口径):
   - 通过 artifact 引用 `teamCharterArtifactId` / `workingAgreementArtifactId`.
   - 新增或扩展 `team_update_config` 以更新引用, 并追加事件日志.

1. Release / Incident / Initiative (仅规划到最小对象与事件口径):
   - release plan artifact + approve/abort 事件.
   - incident 对象 + 关联 tasks/artifacts, 仍遵循 principals channel.
   - initiative 对象用于跨 team 汇总, 不绕过 team 边界.

1. UX 后续:
   - TUI overlays 与资源化接口的规划见 17.2.

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
- 默认环境为 "当下真实世界" (`vibeId = "real_world_now"`). Vibe 扩展暂不规划.

最小可用字段:

- `environment.vibeId` (默认 `real_world_now`)
- `mission` / `vision` / `values[]`
- `orgName` (可选)

固定约束:

- Vibe 只用于语境与提示构造, 不得改变授权边界.
- Vibe/使命/价值观的注入必须可追溯 (体现在控制面事件里, 并可在调试输出中定位来源).

价值观的落地口径 (Pinned):

- `事实优先` / `证据驱动`:
  - 关键结论必须引用证据 (harness_run/patch/review 等), 禁止用 "看起来没问题" 代替事实.
- `边界清晰`:
  - team 的 charter/working agreement 必须显式; 跨 team 仅 principals 通道.
- `可审计`:
  - 控制面动作必须可追溯 (谁在什么时候改了什么), 不允许隐式状态漂移.
- `可回滚`:
  - 发布与关键变更必须有回滚触发口径与 go/no-go 裁决入口.
- `可复现`:
  - 组织认可的验证必须可复现, cadence/digest 只引用可复现证据, 不复制大段输出.

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
- 默认预置一组可编辑模板, 用于降低首次组建成本 (见 14.18.6)
- 模板需要支持:
  - `spawn` 偏好 (role/model/worktree/background 等)
  - `profile` 初始化 (可部分覆盖)
  - `quantity` + `overrides` (差异化招募)

Agent 世界优化 (Pinned):

- 默认应为每个新招募成员启用独立工作区隔离 (例如 worktree), 以避免多 agent 并行改动同一工作目录导致的互相覆盖与不可复现.
- worktree 是执行环境隔离, 不影响授权; 其元信息应在 harness run 证据中可追溯 (例如记录执行命令的 cwd 与 git 信息).

无模板招募 (必须支持):

- leader/owner 可以在招募请求里直接提供 "成员画像草案" (例如需要 UI, 需要 PM, 需要两个开发, 需要测试), 工具将其持久化为 profile 的初始化内容 (必要字段缺失时允许为空).
- 若需要复用, 可将该画像草案提升为模板并通过 `team_template_upsert` / `org_template_upsert` 保存.

任务分派时招募 (增强, 仅规划):

- 允许 leader 在 `team_task_create.staffing` 中声明本次任务需要的额外人力, 工具按需调用 `team_recruit` 完成招募与 onboarding, 并将成功招募的成员自动加入 `assignees`.
- 目标是让 leader 只表达 "我需要什么样的人和数量", 而不是在任务分派前手工做多轮招募与对齐.
- 固定约束:
  - 招募仍是控制面动作, 不提供隐式回滚.
  - 招募失败必须显式返回; `require_all` 模式下必须显式失败并携带失败原因.
  - 该机制不得允许绕过配额/预算, 也不得绕过 `team_recruit` 的审计事件与 onboarding 注入.

临时招募 (仅规划, 本期不实现):

- 目标: 将 "临时补人" 变成可审计、可回收、可控噪声的组织动作, 避免 team 在 7x24 高吞吐下无限膨胀.
- 最小口径:
  - `temporary: true` 表示该成员默认不进入常驻编制, 仅用于补齐某个时间窗/任务集的能力缺口.
  - `leaseUntil` 表示该成员的临时任期边界 (到期时间点).
- 到期收口语义 (Pinned):
  - 到期后不得继续分派新的 tasks 或新的跨 team 动作 (必须显式失败并给出原因), 但允许其完成已分派事项并提交证据产物.
  - team owner/leader 必须显式裁决:
    - 续约: 延长 `leaseUntil` 并记录理由 (例如任务仍未收口, 或临时成员能力仍必要).
    - demobilize: 通过显式移除完成 offboarding 收口 (任务重分派/显式取消/权限收敛).
  - 禁止静默自动移除:
    - 即使到期, 也不得在无审计与无裁决的情况下自动删除成员引用.
- 低噪声可观测性:
  - 临时成员清单与到期时间必须可被汇总引用 (例如在 team status digest 中给出 delta 与到期预警), 避免刷屏式提醒.

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
- team members 仅允许调用 `org_artifact_read` / `org_artifact_list` 读取 org scope artifacts (且必须满足 `visibility=org_members`).
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
  - `team_review_request` (leader/owner 发起, 指定 reviewers)
  - `team_review_submit` (reviewer 提交 `approve|reject` + notes)
  - `team_review_cancel` (leader/owner 取消 open review, 用于合并/降噪)
  - `team_review_read` / `team_review_list` (review 队列可观测)
- Org scope (跨 team 的 RFC/复盘等):
  - `org_artifact_publish` / `org_artifact_read` / `org_artifact_list`
  - `org_review_request` / `org_review_submit` (仅 principals)
  - `org_review_cancel` (仅 principals)
  - `org_review_read` / `org_review_list` (仅 principals)

接口草案 (snake_case, 仅规划):

- `team_artifact_publish`
  - Inputs:
    - `team_id`
    - `task_id` (可选)
    - `kind` (例如 `rfc` / `adr` / `patch` / `postmortem`)
    - `title`
    - `summary` (可选, 用于索引)
    - `content` (建议纯文本; 大内容应写入文件后用 `content_path` 引用)
    - `content_path` (可选, 与 `content` 二选一)
    - `visibility` (可选; 默认 team scope 为 `team_members`)
    - `supersedes_artifact_id` (可选; 表示发布新版本)
  - Outputs:
    - `artifact_id`, `content_digest`, `size_bytes`, `content_ref`
  - Auth:
    - team members/leaders/owner 均可发布 (配额由治理层控制)

- `team_artifact_read`
  - Inputs: `team_id`, `artifact_id`, `include_content` (可选, 默认 false), `max_bytes` (可选)
  - Outputs: artifact 元数据 + (可选) 内容片段/全文
  - Auth: team members/leaders/owner 可读

- `team_artifact_list`
  - Inputs: `team_id`, `task_id` (可选), `kind` (可选), `cursor`/`limit` (可选)
  - Outputs: artifacts 列表 + `next_cursor`
  - Auth: team members/leaders/owner 可读

- `team_review_request`
  - Inputs:
    - `team_id`, `artifact_id`
    - `reviewers` (thread ids)
    - `policy` (例如 `min_approvals`)
    - `evidence_artifact_ids` (可选; 例如 `harness_run` / `test_plan` / `patch` / `review_bundle`)
    - `priority` (可选; 默认 0, 用于队列排序)
    - `expires_at` (可选)
  - Outputs: `review_id`, `status`
  - Auth: team leader/owner (以及 President override)

- `team_review_submit`
  - Inputs: `team_id`, `review_id`, `decision` (`approve|reject`), `notes` (可选)
  - Outputs: `review_id`, `status`, `decisions`
  - Auth: 仅被指定的 reviewer 可提交

- `team_review_cancel`
  - Inputs: `team_id`, `review_id`, `reason` (可选)
  - Outputs: `review_id`, `status`
  - Auth: team leader/owner (以及 President override)

- `team_review_read`
  - Inputs: `team_id`, `review_id`
  - Outputs: review 记录 (含 policy/reviewers/decisions/status 等)
  - Auth: team members/leaders/owner 可读 (按 artifact `visibility` 约束)

- `team_review_list`
  - Inputs: `team_id`, `status` (可选), `artifact_id` (可选), `cursor`/`limit` (可选)
  - Outputs: reviews 列表 + `next_cursor`
  - Auth: team members/leaders/owner 可读

Org scope 等价接口:

- `org_artifact_*` / `org_review_*` 与 team scope 形态一致, 但 scope=org 且默认 `visibility=principals_only`.
  - `org_artifact_read/list` 的授权由 `visibility` 决定:
    - `org_members`: 任一 org member 可读
    - `principals_only`: principals-only
  - `org_artifact_publish` 始终 principals-only (跨 team 发布属于治理动作).
- `org_review_*` 仅 principals 可 request/submit/read/list; 不向 org members 暴露 review 队列 (避免绕过 principals channel 的治理边界).

固定约束:

- review 不改变授权边界, 但可以作为 task 完成/发布的前置条件 (与 `leader_approves` completionMode 对齐).
- task 集成口径: `team_task_approve` (或 task validator) 必须能够引用 `review_id` 作为证据, 并在持久化状态中校验 review 已满足 policy.
- artifacts/reviews 必须 durable-first, 并在 team/org `events.jsonl` 追加事件, 可从 events 回放重建.
- 任何 "通过/拒绝" 必须可被工具强制执行; 禁止只在聊天里说 "approved".
- 大体量内容必须走 artifact contentRef, 避免塞进消息与主 transcript.
- Agent 世界优化 (Pinned):
  - Review 吞吐天然小于产出吞吐, 必须提供背压: 当队列/配额已满时, `*_review_request` 必须显式失败 (或要求改用 `kind=review_bundle` 的汇总 artifact), 禁止无限堆积.
  - `expires_at` 只用于避免陈旧审批, 过期不等于通过; 过期的 review 在门禁评估时必须视为不满足 policy.

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
  - `harness_policy` (证据要求、预算与求助阈值的口径; 对齐 harness playbook)
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

#### 14.12.1 Initiative 组织形式 (仅组织形式)

跨 team 项目在组织形态上不应被实现为 "把所有人塞进一个大 team", 而应是一个 principals-only 的协调层:

- Initiative 的目标是对齐方向、收敛依赖、收口风险与裁决, 而不是替代各 team 的执行自治.

默认角色:

1. Initiative Owner (principal)
   - 责任: 对整体目标、范围与里程碑负责, 处理跨 team 冲突与升级.
   - 典型来源: President 或被 President 委派的 owner.

1. Workstream Lead (principal 或 team leader)
   - 责任: 对某个工作流 (workstream) 的交付负责, 将跨 team 目标落到本 team 的可执行任务集.
   - 典型来源: 参与 team 的 owner 或 leader.

1. Participating Teams
   - 每个 team 只对自己边界内的交付负责, 并按约定节奏输出可复用的状态汇总.

默认结构:

- 一个 initiative 由 1 个 owner + N 个 workstreams 组成.
- 每个 workstream 绑定 1 个主责 team (以及可选的协作 teams), 避免责任扩散.

默认产出 (建议以模板引用为主):

- Initiative charter: 目标、范围、非目标、里程碑、参与 teams, 关键风险与升级路径.
- Dependency map: 关键依赖与阻塞点的汇总.
- Checkpoint digest: 按 `cadence-initiative-checkpoint` 输出阶段性收敛状态.

### 14.13 Onboarding/Offboarding (入职/离职/调岗) (仅规划)

产品目标:

- 让新招募成员在进入组织/团队时获得一致的 "入职包" (org mission/values/vibe + team charter + working agreement + 当前任务), 降低沟通成本.
- 让离职/调岗成为可审计的控制面动作, 并明确任务、消息与权限边界的收口语义.

最小能力 (仅规划):

- Onboarding:
  - `team_onboard`: 向新成员投递标准 onboarding packet (引用 org/team 元数据与必要 artifacts), 并可选自动创建 "新成员首个任务".
  - `org_onboard`: 仅 President/owners 用于招募并初始化 team owners.
- Offboarding:
  - `team_member_remove` / `org_owner_remove`: 从持久化 config 移除引用, 并追加事件日志.
  - 当成员被移除后:
    - 该 thread 立即失去对应 team/org scope 工具授权 (强制边界).
    - 未完成 tasks 必须进入显式状态 (reassign 或 cancel), 禁止静默丢弃.

默认集成:

- `team_recruit` / `org_recruit` 成功后应默认触发一次 `*_onboard(mode=full)`, 避免 "成员已存在但未对齐" 的隐式状态.

接口草案 (snake_case, 仅规划):

- `team_onboard`
  - Inputs:
    - `team_id`
    - `member_name` 或 `member_agent_id` (二选一)
    - `starter_task_ids` (可选)
    - `mode`: `full | refresh` (可选, 默认 `full`)
  - Outputs:
    - `onboarding_artifact_id`
    - `onboarding_record_path`
    - `persisted_inbox`: true/false
    - `delivered_live`: true/false
  - Persistence:
    - 通过 `team_artifact_publish` 创建 `kind=onboarding` 的入职包 artifact (引用 org/team 元数据 + team charter/working agreement artifacts + playbooks).
      - 所有成员必须包含 `harnessPlaybookArtifactId` 引用 (优先 team config, 其次 org config).
      - 若被 onboard 的 thread 属于 principals (team owner/leader 或 President), 则追加 `csePlaybookArtifactId` 引用 (优先 team config, 其次 org config).
    - 写入 `$CODEX_HOME/teams/<team_id>/onboarding/<thread_id>.json` 记录.
    - 向成员 inbox 追加一条短消息, 仅包含 artifact 引用与必要提醒.
  - Events:
    - `team.member.onboarded` (payload: threadId, onboardingArtifactId)
  - Auth:
    - team leader/owner (President 可 override)

- `team_member_remove`
  - Inputs:
    - `team_id`
    - `member_name` 或 `member_agent_id` (二选一)
    - `reason` (可选)
    - `close_member` (可选, 默认 true): 是否尝试 live close
    - `task_policy` (可选):
      - `reassign_to` (可选): 将受影响 tasks 直接重新分派给某个成员
      - `cancel_if_unassigned` (可选, 默认 false): 若移除后出现 "无 assignee" 是否允许显式 cancel
  - Outputs:
    - `removed`: true/false
    - `affected_task_ids[]`
    - `close_attempted`: true/false
    - `close_error` (可选)
  - 任务收口语义 (Pinned):
    - 先从 team config 移除成员引用并落事件 (授权立即收敛).
    - 对所有受影响 tasks:
      - 多 assignee: 从 `assignees[]` 移除该成员; 保留审计需要的历史 assigneeState 但不得阻塞完成判定.
      - 移除后若 `assignees[]` 为空:
        - 若提供 `reassign_to`, 直接重新分派.
        - 否则保持为显式 "未分派" 状态, 禁止静默完成.
    - live close 仅 best-effort; 失败不得回滚授权收敛, 但必须写入 offboarding 记录.
  - Persistence:
    - 写入 `$CODEX_HOME/teams/<team_id>/offboarding/<thread_id>.json` 记录.
  - Events:
    - `team.member.removed` (payload: threadId, reason, affectedTaskIds)
  - Auth:
    - team leader/owner (President 可 override)

Org scope 等价接口 (仅规划):

- `org_onboard` / `org_owner_remove` 用于 team owners 的入职/移除, 并负责 org config 的 principals 引用一致性.
  - `org_onboard` 必须默认植入 harness + cse playbooks (owners 属于 principals).

固定约束:

- onboarding/offboarding 必须 durable-first + 可回放; 禁止只靠 prompt 提示而不落审计事件.
- Agent 世界优化 (Pinned):
  - `team_onboard(mode=refresh)` 必须幂等且低噪声: 默认只投递 onboarding artifact 引用, 禁止重复刷屏与重复创建 starter tasks.
  - 7x24 下的成员流动可能更频繁, `team_member_remove` 必须在无人工介入的情况下把权限与任务状态收口到可恢复、可审计的显式状态.

入职包最小化 (Pinned):

- onboarding packet 必须是 "引用集合", 不复制大段制度与输出:
  - Org: mission/values + (可选)环境设定引用
  - Org Directory: team/council/guild/initiative 的索引入口
  - Team: team charter + working agreement
  - Playbooks: harness (所有成员) + cse (principals)
  - 当前任务入口: starter tasks (可选)
- 入职包应分层以减少 token 与噪声:
  - L0 必读: 最少引用集合 (默认不超过 8-12 条)
  - L1 角色相关: 仅在角色变化或任务需要时追加
  - L2 附录: 历史/背景材料只作为可选引用, 禁止自动注入

过期与刷新策略 (Pinned):

- refresh 的目标是对齐 "引用的版本", 而不是重复刷屏:
  - 默认只投递引用 delta (新增/替换/废弃) 与需要关注的变更摘要, 禁止重发全文.
- 触发 refresh 的典型条件:
  - 角色变化 (member -> leader/owner, 或反向)
  - team charter/working agreement 的引用版本变化
  - org/team playbooks 的引用版本变化
  - 成员长时间离线后重新加入 (例如超过可配置的天数阈值)
- refresh 的低噪声护栏:
  - 对同一成员在同一时间窗内的重复 refresh 应合并为一次投递 (rollup), 其余变化累积到下一次.
  - ack 仅表示收悉不代表理解完成; 若需要验证理解, 应以显式 starter task 的完成作为对齐证据.

### 14.14 组织节奏与状态汇报 (Operating Rhythm) (仅规划)

产品目标:

- 贴近现代研发组织的节奏化协作: 用低噪声、结构化的 status report 替代高频同步, 让 President/owners 能稳定掌控进展.
- 适配 Agent 世界的双节奏: Agent-time 的高频产出通过摘要/汇总收敛到 Human-time 的可读节奏, 避免 7x24 刷屏与门禁振荡.

最小能力 (仅规划):

- `team_status_submit`: member/leader 提交结构化进展 (生成 `summary` artifact + 写入 team inbox)
- `org_status_submit`: leader/owner 向 President 汇报 (生成 `summary` artifact + 写入 org inbox)
- `org_status_rollup`: 将各 team 的 status 汇总成 President 仪表盘视图 (只读)

固定约束:

- status report 必须限频与可追溯 (含时间戳、关联 task/artifact 引用), 避免 inbox 被刷屏.
- status report 必须是 "时间窗口" 语义 (例如包含 `since`), 以适配 7x24 的连续推进; 禁止只靠零散消息让 principals 自行拼接上下文.
- quiet hours 内默认只允许低噪声路径:
  - 普通 `team_broadcast` / 高频 `team_message` 允许被抑制实时投递 (仍需 durable-first).
  - `team_status_submit` / `org_status_submit` 应作为 quiet hours 内的默认对外输出, 以摘要+引用收敛信息带宽.

冷却窗口与自动 rollup (Anti-chatter) (仅规划):

Agent 世界的主要风险不是 "做得不够快", 而是 "控制输入过于频繁导致系统振荡". 因此需要一套明确的冷却与汇总策略, 把高频噪声收敛为可审计的 digest.

- Primary setpoint:
  - principals 能在有限的人类窗口内完成裁决, 而不是被 7x24 的消息洪峰淹没.
- Sensors (建议观测信号):
  - `inboxAppendRate`, `broadcastRate`, `statusSubmitRate`
  - `reviewBacklogDepth`, `oldestReviewAge`
  - 同类失败次数 (例如连续 gate 失败、重复求助)
- Actuators (可施加的控制输入, 必须显式且可追溯):
  - rate limit: 超额时显式失败并返回可机器识别错误码, 禁止静默吞错.
  - live suppression: 允许抑制实时投递但仍 durable-first 落 inbox, 并记录 `suppressedReason`.
  - rollup/digest: 将被抑制或高频事件合并为 `digest`/`status_report` 类 artifact, 仅投递引用.
- 滞回与冷却 (避免阈值附近抖动):
  - 进入抑制态与退出抑制态必须使用不同阈值 (hysteresis), 防止反复横跳.
  - 同一类控制动作必须有冷却窗口, 冷却期内禁止重复触发同类抑制/放开.
- 默认 rollup 触发条件 (可配置):
  - quiet hours 内的非 urgent 消息默认进入 rollup 路径.
  - 当队列信号超阈值 (例如 reviewBacklogDepth 或 oldestReviewAge) 时, 默认切换为 "只允许 digest/status_submit" 的低噪声模式, 直到指标恢复到退出阈值以下.

### 14.15 代码所有权与自动路由 (Code Ownership Routing) (仅规划)

产品目标:

- 贴近真实公司的责任边界: 当组织规模扩大时, 通过 "所有权映射" 自动把工作项路由到负责的 team, 减少 President 人工分派成本.

最小能力 (仅规划):

- `org_ownership_map_upsert` (仅 principals): 维护 path pattern -> teamId 的映射 (概念对齐 CODEOWNERS, 但不引入强制 gate)
- `org_ownership_map_read`: 只读查看映射
- `org_route_task_suggest`: 对给定文件路径/模块名, 建议 owning team (用于分派辅助, 不参与授权)

接口草案 (snake_case, 仅规划):

- `org_ownership_map_upsert`
  - Inputs:
    - `org_id`
    - `rules`: 规则数组 (支持批量 upsert)
      - `rule_id` (可选; 为空则生成)
      - `pattern_type`: `path_glob | module | crate` (最小先做 `path_glob`)
      - `pattern`
      - `team_id`
      - `priority` (可选, 默认 0)
  - Outputs:
    - `updated_rule_ids[]`
    - `map_digest` (用于审计与回滚定位)
  - Auth:
    - principals-only (President/owner/leader)
  - Events:
    - `org.ownership_map.upserted` (payload: ruleIds, mapDigest)

- `org_ownership_map_read`
  - Inputs: `org_id`, `cursor`/`limit` (可选)
  - Outputs: `rules[]` + `next_cursor` + `map_digest`
  - Auth: principals-only (默认; 避免成员绕过 leader 查看跨 team 组织结构)

- `org_route_task_suggest`
  - Inputs:
    - `org_id`
    - `inputs`: 路由输入数组
      - `path` (可选) / `module` (可选) / `crate` (可选)
  - Outputs:
    - `suggestions[]`:
      - `input`
      - `team_id` (可空; 未命中时为空)
      - `explain`: `matched_rules[]` + `selected_rule_id`
      - `principals` (可选): 该 team 的 owner/leaders (用于 President 发起 principals channel 协调)
  - Auth: principals-only

固定约束:

- ownership map 仅用于路由建议与可见性, 不作为授权依据, 也不改变 "跨 team 仅 principals" 的通信边界.
- Agent 世界优化 (Pinned):
  - `org_route_task_suggest` 必须是无副作用的纯读取工具, 支持批量输入以适配高吞吐 triage.
  - 误路由的纠偏必须显式 (principals 更新 ownership map 并留痕), 禁止通过隐式 prompt 约定让错误在 7x24 下持续放大.

### 14.16 Harness-first 证据与可复现 (仅规划)

产品目标:

- 将 "我做完了" 变成可审计、可复现的证据集合, 适配 agent 的极速产出与 7x24 持续运行.
- 降低人类/principals 的 review 负担: 只看结构化摘要与关键证据引用, 不靠翻聊天记录.

最小能力 (仅规划):

- 证据以 artifact 表达, 禁止把命令输出与日志长期塞进消息:
  - `kind=harness_run`: 记录一次关键执行的证据 (命令、退出码、耗时、环境、git 信息、输出摘要)
  - `kind=patch`: 记录变更摘要与引用 (例如 diff/patch 文件路径或摘要)
  - `kind=test_plan`: 记录验证口径与覆盖边界 (尤其是 schema-sensitive 风险)
  - `kind=digest`: 将高频产出在时间窗口内汇总为低噪声摘要

推荐 `harness_run` 内容口径 (示例, 仅规划):

```json
{
  "schemaVersion": 1,
  "runId": "run-1",
  "createdAt": 1739989000,
  "actorThreadId": "thread-alice",
  "platform": { "os": "macos", "arch": "arm64" },
  "git": { "branch": "feat/x", "commit": "abc123" },
  "gateLevel": "L1",
  "commands": [
    { "cmd": "rg -n \"foo\" src/", "exitCode": 0, "durationMs": 120 }
  ],
  "tests": [
    { "cmd": "cargo test -p codex-core", "exitCode": 0, "durationMs": 42000 }
  ],
  "uncovered": ["real_db_schema", "windows_compat"],
  "summary": "What changed, what passed, what is not covered"
}
```

固定约束:

- `harness_run` 必须明确 "覆盖了什么" 与 "没有覆盖什么", 禁止用 "看起来没问题" 代替证据.
- `harness_run.gateLevel` 必须明确该证据对应的 gate 层级 (L0/L1/L2), 避免把离线通过误写为真实环境通过.
- `team_review_request` 在 `completionMode=leader_approves` 或 working agreement 要求 review 时, 必须能引用相关 `harness_run` 与 `patch` 产物作为门禁证据来源.
- 大输出必须摘要化并用 digest/引用表达; 超额必须显式失败或显式要求 rollup, 禁止静默吞并.

`uncovered[]` 建议枚举 (非穷举, 需要稳定字符串以便审计与门禁校验):

- `macos_compat`
- `linux_compat`
- `windows_compat`
- `real_db_schema`
- `external_service_dependency`
- `human_review_required`

### 14.17 阻塞与求助升级 (Help Escalation) (仅规划)

产品目标:

- 在 7x24 高吞吐下, 将 "我卡住了" 变成低噪声、结构化、可审计的求助, 而不是在聊天里反复打断.
- 让 leader/owner/president 的裁决可复用: 同类阻塞可以通过 playbooks/模板固化, 降低重复沟通成本.

最小能力 (仅规划):

- 用 artifact 固化求助上下文:
  - `kind=help_request` artifact 内容必须包含:
    - 当前阻塞 (一句话)
    - 已验证事实 (证据引用)
    - 根因假设 (含不确定点)
    - 可选决策 A/B/C (收益/风险/耗时)
    - 需要确认的单一问题
- 用 scope 内消息通道升级:
  - team 内: `team_ask_lead` 或 `team_message` 引用 help_request artifact
  - 跨 team: principals 通过 `org_principal_message` 引用 help_request artifact

固定约束:

- 求助必须遵循阈值触发: 同类失败 2-3 次或超出预算必须升级; 在阈值前禁止刷屏式求助.
- 求助必须带证据引用 (至少 1 个 harness_run/patch/test_plan 或等价证据), 禁止空喊 "失败了".

### 14.18 预置模板库与引用优先 (Organization Templates) (仅规划)

产品目标:

- Agent Org 启用后, 用户无需从零搭建组织, 但也不应被强制自动创建一堆 teams.
- 因此默认提供一套可复用的 "组织模板库" (templates library):
  - 用户可自行按需实例化 teams
  - 或直接对话 President, 由 President 基于目标/约束规划并组装 teams
- 通过 "引用优先" 减少 token 消耗与口径漂移:
  - 组织知识与协作协议以模板引用为主, 日常沟通只追加差异化补充 (delta), 避免重复粘贴大段制度说明.

固定原则 (Pinned):

- 引用大于复制: 任何高频出现的组织知识 (团队结构、板块职责、协作约定、kickoff 口径) 必须以模板可引用的形式表达, 不应在每个任务/消息里重复注入.
- 口径可版本化: 模板更新应被视为组织变更, 必须可追溯, 避免不同成员收到不同版本导致执行漂移.
- 模板不参与授权: 模板仅定义组织形式与协作语境, 不得改变角色与权限边界.

#### 14.18.1 模板分层 (减少耦合, 提升复用)

按稳定性从高到低, 模板分四层:

1. Org 模板 (最稳定)
   - 定义 org 的身份锚点与默认协作风格: 使命/愿景/价值观/默认环境 (Vibe) 与默认工作方式摘要.
   - 默认预置的 org 模板 (可编辑) 建议口径:
     - environment: `vibeId = real_world_now`
     - values (默认): `事实优先`, `证据驱动`, `边界清晰`, `可审计`, `可回滚`, `可复现`
     - working style (默认): 异步优先, 引用优先 (模板与产物引用大于复制粘贴), 风险显式化 (未覆盖项必须标注)

1. Team 模板 (稳定)
   - 定义 team 的组织形态: 使命/边界/板块划分/leader 结构/推荐编制.

1. 板块 (Subgroup) Leader 模板 (中等稳定)
   - 定义某个板块 leader 的职责、交付物类型、与其他板块的接口.
   - 目标是让 "owner 全权 + leader 分板块" 可规模化复用.

1. 任务 kickoff 模板 (最易变)
   - 定义一次任务启动时必须对齐的最小内容 (目标/验收/协作方式/证据口径引用).
   - 允许按 team 或 initiative 做轻量定制, 但默认通过引用复用.

#### 14.18.1b 模板组装模型 (Component-based Assembly)

为支持用户 "自己组装所需要的团队", Agent Org 的组织形式默认采用组件化组装模型:

- 一个 team 实例由多类模板组合而成:
  - team template (组织形态骨架)
  - subgroup leader templates (板块与职责)
  - recruitment template pack (人员画像与招募偏好)
  - kickoff template pack (任务启动口径)
  - async cadence pack (团队节奏与低噪声汇总口径)
- 组装时只需描述差异化配置 (delta):
  - 例如是否启用 UX/UI 或 Docs/Enablement 板块
  - 例如本 team 需要偏前端/偏后端/偏验证的编制比例
  - 例如该 team 的主要交付风险类型 (兼容性、迁移、性能等)

#### 14.18.2 默认预置的 Team 模板库 (不开箱自动生成 teams)

Agent Org 默认预置以下 team 模板, 但不自动实例化为真实 team:

1. 产品交付团队 (Product Delivery / E2E Squad)
   - 使命: 围绕单一业务域或系统切片端到端交付.
   - owner: 对该 team 的目标、边界、资源与裁决全权负责.
   - 板块 leaders (默认 3 个硬板块 + 可选):
     - Spec & Acceptance: 需求澄清、验收口径、优先级与范围收敛
     - Build & Architecture: 架构裁剪、实现路径、集成策略与冲突裁决
     - Verification & Evidence: 验证策略、证据口径、质量收口与风险标注
     - 可选: UX/UI, Docs/Enablement
   - 推荐编制: 多名实现型成员 + 至少 1 名验证型成员; 需要体验/文档时再扩展板块与成员.

1. 平台与工具团队 (Platform & Tooling Squad)
   - 使命: 为多个 teams 提供共享工程底座与自动化能力, 降低整体摩擦.
   - 板块 leaders (建议):
     - Toolchain: 构建/测试/脚手架/依赖治理
     - Observability & Performance: 性能剖析、可观测与基准口径
     - Integration: 跨模块集成、兼容性与演进策略

1. 质量与发布团队 (Quality & Release Squad)
   - 使命: 将交付正确性制度化, 降低高吞吐下的门禁振荡与返工.
   - 板块 leaders (建议):
     - Test Strategy: 测试分层与回归矩阵
     - Review & Gate: 评审口径、风险分层、放行规则
     - Release: 发布节奏、回滚策略、变更口径

1. 可靠性与事故团队 (Reliability & Incident Squad) (可选实例化)
   - 使命: 事故响应、复盘、行动项闭环, 保障 7x24 演进的可恢复性.
   - 板块 leaders (建议):
     - Incident Command: 指挥与跨 team 协调
     - Runbook: 运行手册与处置路径沉淀
     - Postmortem: 复盘与行动项治理

#### 14.18.3 President 的组装口径 (组织形式层)

当用户要求 President "规划并组装" 时, President 的输出应是组织形式层面的明确方案:

- 需要实例化哪些 teams (基于上面的模板库)
- 每个 team 的 owner 与板块 leaders 配置
- 每个 team 的推荐编制 (按能力缺口建议招募, 可先从最小编制开始)
- 跨 team 的 initiative 结构 (如果需要跨团队项目), 以及各 team 的交付边界

#### 14.18.4 预置的板块 Leader 模板库 (仅组织形式)

为支持 "owner 全权 + leader 分板块" 的可复用组织形态, 默认预置一组板块 leader 模板 (可被不同 teams 复用):

1. Spec & Acceptance Leader
   - 职责: 把目标收敛成可执行的范围与验收口径, 管理范围变更与优先级.
   - 对外接口:
     - 向 owner 提供: 当前范围、风险与取舍建议
     - 向 Build/Verification 板块提供: 验收标准与不可改边界

1. Build & Architecture Leader
   - 职责: 形成可落地的技术路径与集成策略, 协调实现侧分工与冲突裁决.
   - 对外接口:
     - 向 Platform 提出能力缺口与复用需求
     - 向 Verification 提供关键风险点与需要覆盖的路径

1. Verification & Evidence Leader
   - 职责: 定义验证口径与证据要求, 负责质量收口与风险标注, 防止高吞吐下的假收敛.
   - 对外接口:
     - 向 owner 提供: 风险清单与可接受/不可接受项
     - 向 Spec/Build 提供: 证据与验证边界的反馈, 促使范围/实现收敛

1. UX/UI Leader (可选)
   - 职责: 交互与体验一致性, 适配目标用户与使用场景.
   - 对外接口: 与 Spec 对齐体验验收口径, 与 Build 对齐实现约束.

1. Docs/Enablement Leader (可选)
   - 职责: 文档、手册、示例与对外说明, 降低后续维护与扩张成本.
   - 对外接口: 与 Verification 对齐 "可复现" 所需的最小文档与手册.

对于平台/质量/可靠性等团队, 也可复用更专门的板块 leader 模板:

- Toolchain Leader, Observability & Performance Leader, Integration Leader
- Test Strategy Leader, Review & Gate Leader, Release Leader
- Incident Commander, Runbook Leader, Postmortem Leader

#### 14.18.5 组织蓝图 (Blueprint) (默认预置)

除模板库外, 默认预置少量 "组织蓝图" 作为快速组装方案 (仍不自动实例化). 蓝图只是一组对 team/leader 模板的组合引用, 用于减少讨论成本与 token 消耗:

1. 单 team 蓝图 (小规模/单仓)
   - 1 个产品交付团队 (启用 3 个硬板块 leaders)

1. 双 team 蓝图 (中规模/核心 + 平台)
   - 1-2 个产品交付团队
   - 1 个平台与工具团队

1. 多 team 蓝图 (大规模/门禁与运行重要)
   - 多个产品交付团队
   - 1 个平台与工具团队
   - 1 个质量与发布团队
   - 0-1 个可靠性与事故团队

#### 14.18.5b 按项目类型的默认蓝图选择建议 (仅组织形式)

为减少 President 在 intake 后的探索成本, 预置一组 "项目类型 -> 默认蓝图" 的选择建议. 这些建议不替代实际约束, 只提供开箱可用的默认值.

固定原则 (Pinned):

- 默认从单 team 蓝图启动, 除非项目天然需要共享底座或运行治理.
- 若 "人类 gate" 是主要时滞来源, 优先增强 Verify/Gate/Release 的组织能力, 而不是继续增加实现成员.
- 当交付与平台摩擦都很高时, 与其把所有角色塞进一个大 team, 不如实例化平台与工具 team.

项目类型的默认建议:

1. 库 (Library)
   - 默认蓝图: 单 team
   - 强化板块: Verification & Evidence (必选), Release (可选, 可用 Quality/Release team 或板块 leader 承担)
   - 典型扩张触发:
     - 兼容性矩阵变大/发布频繁 -> 实例化质量与发布 team 或启用 Release leader
     - 构建与依赖治理成为瓶颈 -> 增加平台与工具 team

1. CLI / 开发者工具
   - 默认蓝图: 单 team 或 双 team (取决于工具链复杂度)
   - 强化板块: Docs/Enablement (常见), Verification & Evidence (必选)
   - 典型扩张触发:
     - 多平台兼容/安装渠道复杂 -> 质量与发布 team 提前介入

1. 服务 / API (长期运行)
   - 默认蓝图: 双 team (产品交付 + 平台与工具) 更常见
   - 强化板块: Observability & Performance (平台侧常见), Incident (按需)
   - 典型扩张触发:
     - 事故频繁/回滚与复盘无法闭环 -> 实例化可靠性与事故 team

1. 应用 (Web/桌面/移动)
   - 默认蓝图: 单 team
   - 强化板块: UX/UI (按需启用), Verification & Evidence (必选)
   - 典型扩张触发:
     - 体验迭代密集 -> 启用 UX/UI leader + 前端成员模板
     - review 门禁成为瓶颈 -> 质量与发布 team 或 Review & Gate leader 提前介入

1. 大型迁移/重构/跨域演进
   - 默认蓝图: initiative + 多 team (视依赖图)
   - 组织形态: 用 initiative 协调层收敛依赖与裁决, 不合并成单一大 team

#### 14.18.6 默认预置的招募模板包 (Recruitment Template Pack)

为降低用户首次组建组织的成本, Agent Org 默认预置一组可编辑、可复用的招募模板:

- 原则:
  - 默认偏向 "职业画像 + 协作偏好", 避免收集无必要的个人信息.
  - 模板用于招募与语境注入, 不参与授权.

Team scope (常用成员画像):

1. `member-dev-generalist`
   - 通用开发成员, 适配大多数实现任务.
1. `member-product-manager`
   - 偏 PRD/RFC、范围收敛与优先级, 适配 Spec & Acceptance 板块的交付与对齐.
1. `member-dev-frontend-ui`
   - 偏 UI/交互/组件化, 适配 UX 板块.
1. `member-ux-designer`
   - 偏用户视角与交互设计, 适配 UX/UI 板块, 常与 `member-dev-frontend-ui` 配对.
1. `member-dev-backend`
   - 偏接口/存储/集成, 适配 Build 板块.
1. `member-qa-verification`
   - 偏验证策略与证据整理, 适配 Verification 板块.
1. `member-sre-reliability`
   - 偏可观测/运行/故障处置与 runbook, 适配 Platform 或 Reliability 相关板块.
1. `member-security-privacy`
   - 偏威胁模型、依赖供应链风险与数据最小化, 适配安全与隐私相关变更的执行支持.
1. `member-tech-writer`
   - 偏文档与可复现说明, 适配 Docs/Enablement 板块.

Team scope (常用 leaders 画像):

1. `leader-spec-acceptance`
   - 适配 Spec & Acceptance 板块 leader.
1. `leader-build-architecture`
   - 适配 Build & Architecture 板块 leader.
1. `leader-verification-evidence`
   - 适配 Verification & Evidence 板块 leader.
1. `leader-ux-ui` (可选)
   - 适配 UX/UI 板块 leader.
1. `leader-docs-enablement` (可选)
   - 适配 Docs/Enablement 板块 leader.

Team scope (平台/质量/可靠性 leaders 画像):

1. `leader-toolchain`
1. `leader-observability-performance`
1. `leader-integration`
1. `leader-test-strategy`
1. `leader-review-gate`
1. `leader-release`
1. `leader-incident-commander`
1. `leader-runbook`
1. `leader-postmortem`

Org scope (常用 principals 画像):

1. `owner-product-delivery`
1. `owner-platform-tooling`
1. `owner-quality-release`
1. `owner-reliability-incident`

#### 14.18.7 默认预置的任务 kickoff 模板包 (Kickoff Template Pack)

为减少高频协作的重复说明与 token 消耗, 默认预置少量 kickoff 模板, 在创建任务或分派时可引用并追加 delta:

1. `kickoff-simple`
   - 单人或低耦合任务的最小对齐口径 (目标/验收/边界/证据引用).
1. `kickoff-multi-assignee`
   - 多人协作任务的自组织协议:
     - 每个 assignee 给出计划与产物
     - 对齐接口与依赖
     - 冲突升级路径 (leader/owner)
1. `kickoff-leader-approves`
   - 带显式批准门禁的任务口径:
     - 明确证据要求与未覆盖项的风险标注
1. `kickoff-incident`
   - 事故/紧急任务口径:
     - 明确时滞预算、止血优先级、回滚触发与信息汇总口径

#### 14.18.8 默认预置的组织节奏模板 (Async Cadence Pack) (仅组织形式)

现代组织的 "节奏" 在 Agent 世界需要异步化与低噪声化. Agent 的产出速度与并行度远高于人类, 若仍沿用人类组织的同步会议节奏, 会把系统推向高频沟通振荡与 token 浪费. 因此默认预置一组组织节奏模板, 用于 teams/initiative 的可引用协作约定.

固定原则 (Pinned):

- cadence 的产物应当是可引用的 digest, 默认只写 delta, 禁止重复粘贴手册全文.
- cadence 的目标不是 "报喜", 而是提供可控的闭环观测:
  - 当前进展与偏差 (error) 是什么
  - 最大的阻塞与风险在哪里
  - 哪些事项需要 principals 做裁决, 需要哪些证据
- cadence 默认异步发布, 不要求同步会议; 若必须同步, 也只围绕 digest 中的 asks/risk/decision 进行.
- cadence 必须适配人类参与窗口:
  - Agent 可 7x24 执行, 但 "需要人类决策/批准" 的事项必须在窗口内被显式提起并形成可引用条目.
- cadence 必须低噪声:
  - 没有状态迁移与无新增风险时, 不发布或只累积到下一次 digest.
  - 任何 cadence 产物都必须有体量上限, 超出需拆为附件 artifact 并在 digest 中引用.

通用字段约定 (所有 cadence 共用的最小字段):

- `asOf`: 截止时间点 (用于明确 delta 的边界).
- `since`: 本次 digest 覆盖的起点 (用于对齐 "发生了什么变化").
- `owner`: 本 cadence 的责任角色 (team owner / 板块 leader / Incident Commander 等).
- `audience`: 默认接收范围 (leaders, owner, President, councils 等).
- `inputRefs`: 引用的 artifacts/reviews/PRs/issues/threads (只列标识与链接, 不复制内容).
- `asks`: 需要 principals 决策/介入的事项 (最多 3 条, 超出需拆分或进入下一轮).
- `risks`: 风险与未覆盖项; 若为空必须显式写 `none`.
- `next`: 下一步动作 (最多 3 条, 需可执行且可验证).

默认预置的 cadence 模板:

1. `cadence-team-status` (团队状态 digest)
   - 适用范围: 单个 team (owner 维度) 的持续交付状态汇总.
   - 默认触发:
     - 每 24h 至少 1 次; 或当出现重大状态迁移时立即追加 (例如 milestone 完成、关键 blocker 出现、风险升级).
   - 最小字段补充:
     - `progressDelta`: 本周期完成的里程碑/交付项 (仅列 ID 或标题).
     - `blockers`: 当前阻塞 (必须标注 owner 与需要的外部输入).
     - `reviewQueue`: review 拥堵信号 (仅列汇总计数与最老项年龄).
     - `evidenceRefs`: 本周期新增的关键证据引用 (tests/logs/bench/PR).

1. `cadence-review-queue` (门禁与 review 队列 digest)
   - 适用范围: team 或 Quality & Gate 板块的 review/gate 队列治理.
   - 默认触发:
     - 人类窗口开始前 1 次; 或当队列跨越阈值时追加 (例如 backlog 激增、最老项超龄、发布阻塞出现).
   - 最小字段补充:
     - `queueSummary`: 总数、阻塞发布数、最老项年龄.
     - `p0Items`: 需要优先处理的前 N 项 (建议 N<=5, 只列 ID 与阻塞原因).
     - `policyAsks`: 是否需要调整门禁口径/批准策略 (若无则写 `none`).
   - 低噪声约束:
     - 不输出全量列表, 只输出汇总 + 前 N 项 + asks.
     - 队列优先级调整必须基于阈值与理由, 避免高频重排导致振荡.

1. `cadence-initiative-checkpoint` (跨 team initiative 检查点 digest)
   - 适用范围: initiative 的 principals-only 协调层 (不取代各 team 内部执行自治).
   - 默认触发:
     - 每个里程碑完成/失败时; 或固定周期检查点 (例如每 3-7 天, 由 initiative charter 约定).
   - 最小字段补充:
     - `milestone`: 当前里程碑与完成度 (只写结论与 delta).
     - `workstreams`: 各 workstream 的状态与阻塞 (每条最多 1-2 行).
     - `dependencyDelta`: 依赖图变化 (新增/解除/升级的依赖).
     - `decisionRequests`: 需要 councils/President 裁决的事项 (必须给出备选方案与引用证据).

1. `cadence-incident-updates` (事故更新 digest)
   - 适用范围: incident 的对外信息一致性与复盘可追溯性.
   - 默认触发:
     - 由严重度驱动的固定更新频率, 且每次必须声明下一次更新时间点.
   - 最小字段补充:
     - `severity`: 严重度 (例如 P0/P1/P2).
     - `impact`: 影响范围与用户可见症状 (只写事实, 禁止推测).
     - `mitigation`: 当前采取的止血措施与效果.
     - `nextUpdateAt`: 下一次对外更新时间点.
     - `postmortemRefs`: 复盘与行动项的引用入口 (若尚未创建则写 `pending`).

#### 14.18.9 President 的组装问诊模板 (Intake) (仅组织形式)

当用户选择 "对话 President 让他规划并组装" 时, President 应优先使用一组最小问诊问题收敛组织方案, 避免长对话消耗 token:

1. 项目类型与交付形态:
   - 是单仓库还是多仓库, 是库/CLI/服务/应用中的哪类
1. 风险偏好与门禁强度:
   - 是否允许快速试错, 是否必须严格门禁 (例如发布、兼容性、真实环境)
1. 规模与并行度:
   - 需要同时推进多少条工作流 (feature/refactor/bugfix/incident)
1. 人类参与窗口:
   - 是否有人类 reviewer/审批者, 大致可用的时间窗
1. 强制约束:
   - 时间窗口、不可改边界、合规/隐私要求

输出要求:

- President 必须在组织形式层输出蓝图:
  - 选择的组织蓝图 (14.18.5) 或自定义组装
  - 实例化哪些 teams, 每个 team 的 owner 与板块 leaders
  - 每个 team 的最小编制与可选扩编方向 (通过招募模板引用)

#### 14.18.10 默认编制建议 (Staffing Heuristics) (仅组织形式)

目标:

- 让用户在不理解全部组织细节的前提下, 也能用最小编制启动并可控扩张.
- 在 Agent 世界里, "加人" 的边际收益与边际成本都很高:
  - 收益: 并行度上升, 交付变快
  - 成本: token 与协作噪声上升, 口径漂移风险上升
- 因此默认策略是 "小核心 + 弹性扩编":
  - 常驻编制只保留闭环所需的关键角色
  - 任务高峰通过模板化招募短期补齐能力缺口

通用原则 (Pinned):

- 每个产品交付团队至少要覆盖三类闭环职责: Spec, Build, Verify.
  - 如果团队规模较小, 允许 owner/leader 角色叠加, 但职责仍需显式.
- 当出现明显的返工、门禁拥堵或风险收口失败时, 优先补齐 Verify 与 Spec 的板块能力, 再扩实现成员.
- UI/Docs 等板块默认可插拔: 需要时通过板块 leader + 成员模板组合启用, 不需要时保持为引用缺省.

建议的最小编制 (Starter) 与扩编方向:

1. 单 team 蓝图 (小规模/单仓)
   - 1 个产品交付团队
   - owner: `owner-product-delivery` (可临时兼任 `leader-build-architecture`)
   - leaders:
     - `leader-spec-acceptance` (建议)
     - `leader-verification-evidence` (建议)
   - members:
     - 2x `member-dev-generalist`
     - 1x `member-qa-verification`
   - 可选扩编:
     - UI 密集: `leader-ux-ui` + `member-dev-frontend-ui`
     - 文档要求高: `leader-docs-enablement` + `member-tech-writer`

1. 双 team 蓝图 (中规模/核心 + 平台)
   - 1-2 个产品交付团队 + 1 个平台与工具团队
   - 平台与工具团队最小编制:
     - owner: `owner-platform-tooling` (可兼任 Toolchain 板块 leader)
     - members: 1-2x `member-dev-generalist`
   - 可选扩编:
     - 性能/观测痛点明显: 增加 Observability & Performance 板块 leader 与对应成员

1. 多 team 蓝图 (大规模/门禁与运行重要)
   - 多个产品交付团队 + 平台与工具 + 质量与发布 + (可选)可靠性与事故
   - 质量与发布团队最小编制:
     - owner: `owner-quality-release`
     - leaders: Test Strategy 与 Review & Gate (可由同一 leader 兼任)
     - members: 1x `member-qa-verification` (必要时扩)
   - 可靠性与事故团队最小编制 (按需实例化):
     - owner: `owner-reliability-incident`
     - leaders: Incident Command 与 Postmortem (可叠加), Runbook (可选)
     - members: 0-1x `member-dev-generalist` (用于 runbook/修复协作支持)

#### 14.18.11 模板继承、版本与变更治理 (Template Lifecycle) (仅组织形式)

产品目标:

- 模板库的价值来自 "可复用" 与 "一致口径". 若模板更新导致漂移, 将直接放大协作噪声.
- 因此需要在组织形式层明确模板的继承层级、版本策略与变更治理口径.

固定原则 (Pinned):

- 默认 pin 版本: team/initiative 默认引用模板的稳定版本, 不自动跟随更新, 避免 7x24 下的隐式漂移.
- 变更显式化: 模板变更应被视为组织变更, 需要被明确告知并可追溯.
- 继承只用于减少重复: 上层模板提供默认值, 下层只覆盖必要差异 (delta).

默认继承层级:

1. Org 模板
   - 提供组织的默认价值观、工作方式与环境设定.
1. Team 模板
   - 继承 org 的默认工作方式, 并定义该 team 的结构骨架与板块配置.
1. 板块 leader 模板
   - 在 team 内提供板块职责与接口口径.
1. Kickoff/Cadence 模板
   - 提供高频协作的最小协议, 任务与汇总只追加 delta.

默认变更治理口径 (仅组织形式):

- Org 模板与 Councils 相关的关键口径变更:
  - 默认由 President 发起并裁决; 若影响架构/门禁/发布, 应进入对应 Council 收敛.
- Team 模板与 Team Handbook 的变更:
  - 默认由该 team owner 裁决, 并对团队成员可见.
- Kickoff/Cadence 等高频模板:
  - 优先保持稳定; 若需变更, 应先在小范围试行并以引用方式逐步推广.

#### 14.18.12 Team Charter 与 Working Agreement 模板包 (仅组织形式)

用户启用 Agent Org 后, 若没有可复用的章程与工作协议模板, 很容易退化为每个任务重复解释 "我们怎么协作". 因此默认预置少量可编辑模板, 让 team owner 能快速固化团队边界与执行口径.

1. `team-charter-default`
   - 用途: 定义 team 的使命、边界与对外接口, 避免 scope creep 与跨 team 耦合失控.
   - 最小字段:
     - `mission`: 团队使命 (一句话)
     - `scope`: 负责的领域/模块/交付物范围
     - `nonGoals`: 明确不负责的事项 (边界)
     - `interfaces`: 与其他 teams 的主要接口与依赖 (只列引用)
     - `qualityBar`: 最低质量口径 (对齐 DoD)
     - `escalation`: 升级路径 (owner -> councils -> President)

1. `team-working-agreement-default`
   - 用途: 定义团队协作与门禁口径, 使 7x24 高吞吐下仍可低噪声收敛.
   - 固定原则 (Pinned):
     - 异步优先, 引用优先, 证据优先
     - 禁止静默降级, 失败必须显式可定位
   - 最小字段:
     - `definitionOfDone`: DoD (必须可验证, 且可引用到模板/证据类型)
     - `reviewPolicy`: review 与批准口径 (何时需要 leader/owner approve, 何时可自收口)
     - `wipLimits`: WIP 与背压口径 (例如 review 队列阈值、任务并行上限)
     - `cadenceRefs`: 采用哪些 cadence 模板 (见 14.18.8)
     - `quietHours`: 低噪声窗口 (用于抑制刷屏与无意义状态更新)
     - `helpEscalation`: 求助阈值与升级口径 (对齐 14.17)

1. 可选变体 (按风险偏好裁剪):
   - `team-working-agreement-fast-lane`: 低风险变更的快速通道口径
   - `team-working-agreement-strict-gate`: 高风险/发布/兼容性场景的严格门禁口径

### 14.19 组织治理结构 (Councils) (仅组织形式)

产品目标:

- 在现代顶级软件公司里, 跨团队耦合不可避免, 需要稳定的裁决与升级结构.
- 在 Agent 世界里, 产出速度远高于裁决速度, 没有治理结构会导致 "局部很快, 全局振荡".
- 因此 Agent Org 默认提供一组 principals-only 的治理结构 (Councils), 用于收敛跨 team 的关键决策.

固定原则 (Pinned):

- Council 是组织形式, 不是新的通信通道:
  - 仍遵循 "跨 team 仅 principals" 的边界.
- Council 的价值在于 "明确谁对什么误差负责":
  - 避免每个 team 各自优化导致全局耦合失控.
- Council 输出必须是可复用的组织知识:
  - 例如架构决策、门禁口径、发布裁决、事故复盘与行动项 (以模板引用为主, 只追加 delta).

默认 Councils (可按 org 规模裁剪):

1. Architecture Council (架构与边界裁决)
   - 触发: 跨 team 接口/契约变化, 共享边界解冻, 迁移与重构的关键切换点.
   - 成员: President + Platform owner + 受影响的 team owners (必要时加 Quality owner).
   - 责任: 决定边界是否允许打开、兼容窗口、迁移策略与回退条件.

1. Quality & Gate Council (质量与门禁口径)
   - 触发: DoD 变更、证据口径调整、风险分层口径变更、门禁振荡与背压策略调整.
   - 成员: Quality/Release owner + 相关 team owners/leaders + President (按需).
   - 责任: 统一门禁口径, 防止高吞吐下的假收敛与放大风险.

1. Security & Privacy Council (安全与隐私) (可选)
   - 触发: 认证授权、密钥/凭证、依赖供应链安全、涉及敏感数据 (PII/密文/访问日志) 的变更, 或对威胁模型有显著影响的架构调整.
   - 成员: President + (可选)安全负责人 + Platform owner + 受影响的 team owners.
   - 责任: 收敛威胁模型与数据最小化口径, 明确安全门禁与回滚条件, 并把结论回写为可引用条目 (handbook/templates).
   - 最小输出 (Pinned):
     - 变更范围与数据触达清单 (只列引用与结论)
     - 威胁模型摘要与主要缓解措施 (含残余风险)
     - 门禁结论: approve / changes_required / reject (并给出回滚触发口径)
     - 可引用的决策记录条目 (ADR/RFC 引用, 只写 delta)
   - 与其他治理结构的接口:
     - 若涉及共享边界/架构切换点, 同步进入 Architecture Council
     - 若涉及放行策略调整, 同步进入 Quality & Gate Council

1. Release Council (发布裁决) (可与 Quality 合并)
   - 触发: 重大发布、回滚阈值调整、发布窗口与节奏冲突.
   - 成员: Release leader/owner + 相关 team owners + President (按需).
   - 责任: 做 go/no-go 裁决与回滚预案收口.

1. Incident Council (事故指挥与复盘)
   - 触发: 严重事故/持续性故障/跨 team 影响的 brownout.
   - 成员: Incident Commander + 受影响 team owners + President (按需).
   - 责任: 止血优先级、跨 team 协调、复盘与行动项闭环.

#### 14.19.1 CSE Owner Matrix 映射与升级路径 (仅组织形式)

为对齐 `$cybernetic-systems-engineering` 的 "最小 owner matrix", Agent Org 的组织形式默认映射为:

- 总体设计部 (整体裁决与边界冻结):
  - President + Architecture Council
- 模块 owner (模块边界内的交付与回归):
  - 产品交付团队 owner
- 共享边界 owner (共享接口/共享状态/统一门禁口径):
  - 平台与工具团队 owner
  - 质量与发布团队 owner
- 事故与恢复 owner (运行风险与复盘闭环):
  - 可靠性与事故团队 owner (或 Incident Commander)

默认升级路径 (按风险从低到高):

1. 模块内局部改动
   - 由对应产品交付团队 owner/leader 在 team 边界内裁决与推进.
1. 跨 team 但不改共享契约
   - 通过 principals channel 或 initiative 结构对齐依赖与里程碑, 不改变各 team 的执行自治.
1. 触碰共享接口/共享状态/统一门禁
   - 升级到 Architecture Council 或 Quality & Gate Council 裁决后再推进.
1. 触碰冻结边界或发布窗口
   - 默认需要 President 裁决并明确回退条件.

### 14.20 组织演化与扩张 (Scaling & Reorg) (仅组织形式)

产品目标:

- Agent Org 应允许组织结构随项目规模与风险变化而演化, 而不是一次性固化.
- 组织演化应遵循第一性原理: 优先解决当前主要瓶颈, 避免无意义的层级膨胀.

固定原则 (Pinned):

- 组织结构的变化应当是显式决策:
  - 新 team 的创建、职责边界的变化、owner/leaders 的更替都应可追溯.
- 组织演化应优先使用模板与蓝图, 以降低变更成本与沟通噪声.

默认演化触发器 (仅组织形式口径):

1. 交付团队过载 -> 拆分或新增产品交付团队
   - 典型信号: 同一 team 同时承担过多不相干目标, 优先级冲突频繁, 跨域耦合导致决策拥堵.

1. 工具链摩擦成为主要瓶颈 -> 实例化平台与工具团队
   - 典型信号: 大量时间消耗在重复的构建/测试/脚手架/依赖治理问题上, 交付队列被工具问题阻塞.

1. 门禁振荡/返工显著 -> 实例化质量与发布团队
   - 典型信号: review 队列拥堵, "离线通过但真实风险未收敛" 反复出现, 发布决策缺乏统一口径.

1. 事故与运行风险上升 -> 实例化可靠性与事故团队
   - 典型信号: incident 频繁或影响面扩大, 回滚与复盘行动项无法闭环, 运行知识无法沉淀复用.

1. 跨 team 依赖增多 -> 建立 initiative 结构而不是把 teams 混成一个大 team
   - 典型信号: 多个 teams 需要同步推进同一个目标, 但各自仍应保持边界与自治.

### 14.21 Handbook-first 组织手册体系 (仅组织形式)

产品目标:

- 现代顶级软件公司在规模化时, 组织知识必须 "手册化", 否则会被聊天与口头约定稀释并快速漂移.
- 在 Agent 世界里, 高吞吐会放大漂移与噪声, 因此需要把组织知识收敛成可引用的手册体系:
  - 用引用替代复制粘贴, 降低 token 消耗
  - 用版本化与可追溯的手册替代临时对话, 避免口径不一致

固定原则 (Pinned):

- Handbook 只定义组织形式与协作口径, 不参与授权.
- Handbook 的最小单位是 "可引用条目", 日常协作只追加 delta, 不重复注入全文.

建议的手册分层:

1. Org Handbook
   - 组织身份: mission/vision/values/environment (Vibe)
   - 组织结构: 预置模板库、蓝图、Councils、升级路径
   - 默认工作方式: 异步优先、引用优先、风险显式化的口径摘要

1. Team Handbook
   - team charter + working agreement
   - 板块与 leaders: 责任边界与接口
   - 招募模板引用与常用编制建议
   - team cadence 引用 (status/review/incident 等)

### 14.22 现代软件公司工作流映射 (Operating Model) (仅组织形式)

产品目标:

- 让 Agent Org 的组织形式贴近现代软件公司真实工作流, 同时针对 Agent 世界的高速与 7x24 做优化.
- 将 "从想法到交付" 固化为可复用的组织流程骨架, 使不同 teams 的协作方式一致且可扩展.

默认工作流骨架 (从目标到交付):

1. Align (目标与验收收敛)
   - 主责: Spec & Acceptance Leader
   - 输出: 可执行的范围与验收口径, 并对范围变更保持收敛

1. Design & Plan (设计与计划)
   - 主责: Build & Architecture Leader
   - 触发升级: 跨 team 依赖或共享边界变化时进入 Architecture Council

1. Build (实现与集成)
   - 主责: Build & Architecture Leader + 实现成员
   - 约束: 在 team 边界内自组织协作, 跨 team 依赖通过 principals/initiative 收敛

1. Verify (验证与证据收口)
   - 主责: Verification & Evidence Leader
   - 目标: 将风险与未覆盖项显式化, 防止假收敛

1. Gate (门禁与批准)
   - 主责: team owner 或 leaders
   - 触发升级: 门禁口径变化或振荡进入 Quality & Gate Council

1. Release (发布裁决与回滚预案)
   - 主责: Release Council (或 Quality/Release team)
   - 目标: 对 go/no-go 与回滚触发形成统一口径

1. Run & Learn (运行闭环与复盘)
   - 主责: Incident Council (按需) + 相关 team owners
   - 输出: 复盘与行动项回写手册/模板, 形成组织记忆

### 14.23 组织目录 (Org Directory) (仅组织形式)

产品目标:

- 为用户与 President 提供低 token 的组织索引视图, 让任何新会话可在一次引用中获取组织结构与关键入口.
- 降低 out-of-band 粘贴与重复背景注入, 避免每次对话重复解释 teams/councils/initiatives.

固定原则 (Pinned):

- Directory 只存索引与引用, 不复制大文本.
- Directory 不参与授权, 只作为 handbooks 的 "目录页".
- Directory 的每次变更都应可追溯 (变更原因与责任人), 避免口径漂移.

建议的最小字段:

- Org:
  - `orgId`, `name`, `missionRef`, `visionRef`, `valuesRef`
  - `environment` (默认: real_world)
  - `president` (threadId 或 agentId 的引用)
- Councils:
  - `councilId`, `name`, `charterRef`, `members` (principals-only)
  - `escalationRef` (升级路径引用)
- Guilds:
  - `guildId`, `name`, `charterRef`, `members` (leaders-only)
  - `outputRefs` (常用模板/手册条目的引用)
- Teams:
  - `teamId`, `name`, `owner`, `leaders[]`
  - `charterRef`, `teamHandbookRef`
  - `blueprintRef`, `pinnedTemplateRefs[]` (仅列引用与版本)
- Initiatives:
  - `initiativeId`, `owner`, `workstreams[]`, `participatingTeams[]`
  - `charterRef`, `checkpointCadenceRef` (引用 14.18.8 的条目)
- Default Policies:
  - `broadcastPolicyRef`, `reviewGatePolicyRef`, `staffingHeuristicsRef`

维护与变更治理 (Pinned):

- 目录 owner: 默认由 President 负责维护与发布, 确保全局索引口径唯一.
- 变更输入来源:
  - team owners: team 结构、leaders、team handbook/charter 引用变化
  - council/guild 负责人: charter 与成员变化、输出条目变化
  - initiative owner: initiative 生命周期与 workstreams 变化
- 更新触发 (建议):
  - team/council/guild/initiative 的创建或关闭
  - principals 变更 (owner/leaders/chairs)
  - 关键引用变化 (charter/working agreement/cadence/template pinned version)
- 发布与低噪声:
  - Directory 的更新应批量化并以 digest 方式对外可见, 禁止每次微小变更都刷屏式广播.
  - 每次更新必须可追溯: 至少记录变更原因与责任人, 并能回溯到被替换的上一版本引用.
- 一致性约束:
  - Directory 只存索引与引用; 控制面 durable 真相来自 org/team configs 与事件日志.
  - 当 Directory 与 durable 真相冲突时, 以 durable 真相为准, 并触发一次显式的 Directory 修复更新.

### 14.24 Guilds/Chapters (跨团队知识对齐结构) (仅组织形式)

产品目标:

- Council 用于裁决, 但大量日常一致性工作 (模板沉淀、标准演进、最佳实践对齐) 不应该都升级到 Council.
- 因此引入 Guilds/Chapters 作为跨 team 的 leaders-only 对齐结构:
  - 只做标准与模板的沉淀与传播, 不取代各 team 的执行自治
  - 将高频协作口径固化为可引用条目, 进一步降低 token 与漂移

固定原则 (Pinned):

- 仅 leaders 参与 (leaders-only), 不打破 "跨 team 仅 principals" 的边界.
- Guild 不做最终裁决:
  - 变更共享边界/门禁/发布窗口等重大事项仍需进入对应 Council 或 President.
- 输出必须可复用:
  - 默认产物是模板/手册条目的变更提案与已采纳版本 (以引用为主, 只写 delta).

运行细则 (Pinned):

- Guild charter 的最小字段:
  - `scope`: Guild 覆盖的领域边界
  - `chair`: 负责人 (leader)
  - `members`: leaders-only 成员清单
  - `cadenceRef`: 采用的 checkpoint cadence (建议复用 14.18.8 的 digest 口径)
  - `outputs`: 该 Guild 维护的模板/手册条目清单 (以引用为主)
  - `decisionBoundary`: 哪些事项只能提案不能裁决 (需升级到 Council/President)
- 产出形态:
  - 默认产出是 "提案 + 引用" 而不是长讨论:
    - template/handbook 的 delta 提案
    - 必要时附 ADR/RFC 引用 (解释取舍与被拒绝方案)
- 提案到采纳的收敛流程 (避免漂移与振荡):
  - propose: 提案形成可引用条目, 明确收益/新成本/失效模式
  - pilot: 小范围试行并收集证据 (避免一次性全 org 推广)
  - adopt: 进入 Council/President 或对应 owner 的显式采纳裁决
  - pin: 模板/手册版本 pin, 并更新 Directory 的 `outputRefs`
  - deprecate: 旧口径进入废弃窗口, 给出迁移与回滚口径
- 低噪声要求:
  - Guild 的对外输出必须批量化并以 digest 方式发布, 禁止频繁广播式扩散.

默认预置的 Guilds (可按规模裁剪):

1. Spec Guild
   - 范围: Spec/验收口径、范围收敛、需求拆解模板.
   - 输出: spec 与验收模板的演进条目.

1. Build Guild
   - 范围: 架构与实现模式、集成策略、常见重构与迁移路径.
   - 输出: 可复用的设计与集成模板条目 (非裁决).

1. Verification Guild
   - 范围: 测试分层、证据口径、回归矩阵与 flake 治理.
   - 输出: 验证策略模板、harness_run 证据口径条目.

1. Tooling Guild
   - 范围: 构建/测试/脚手架与开发体验治理.
   - 输出: 工具链最佳实践与复用模板.

1. Release Guild
   - 范围: 发布节奏、回滚预案模板、变更通告口径.
   - 输出: release plan 模板条目与 go/no-go 口径更新提案.

1. Incident Guild
   - 范围: runbook、事故沟通模板、复盘行动项口径.
   - 输出: incident update/postmortem 模板条目.

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
  - `maxArtifactsPerHour`, `maxArtifactBytes` (限制产物洪峰)
  - `maxOpenReviews`, `maxReviewRequestsPerHour` (限制门禁队列膨胀)
  - `maxInboxAppendsPerMinute`, `maxBroadcastPerHour`, `maxStatusSubmitsPerDay` (限制刷屏与噪声)
- 运行时:
  - 超额时必须显式失败, 或进入显式可见的 degrade 模式 (不得静默)
  - 超额不得破坏既有真相:
    - 默认只限制新的控制面动作 (例如新的招募/新的 broadcast/新的 review request).
    - 不得因超额而隐式移除既有成员、撤销既有任务或修改既有 artifacts/reviews.

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
- playbooks (harness/cse) 仅用于执行协议与协作语境注入, 不参与授权, 不得作为角色或越权的依据.
- 边界强制必须是代码层 target-based authorization, 不能依赖 prompt 约定.
- 控制面写接口必须校验 caller 身份与角色, 并记录审计事件 (谁在什么时候改了什么).

### 15.6 可观测性与可调试性

- 必须能从 `$CODEX_HOME` 的持久化状态回答 "谁在什么 team/org 里, 拥有什么角色, 做了哪些控制面动作".
- 必须提供最小自描述工具面 (例如 `team_current` / `team_info` / `org_info`), 避免 out-of-band 粘贴.
- 对隐私字段做最小化记录: 事件日志记录引用与摘要, 不复制大字段.

### 15.7 时滞预算与长任务治理 (Delay Budget) (仅规划)

Agent 世界的产出很快, 但验证、评审与外部门禁具有时滞. 若不治理时滞, 系统会在 7x24 下堆积队列并产生假收敛.

固定要求:

- 关键验证必须可计量:
  - `harness_run` 证据必须记录命令与测试的 `durationMs` 与退出码, 便于统计与背压.
- 长任务必须显式预算化:
  - 对高成本命令 (全量测试、慢构建、外部 gate) 必须有显式超时与中止语义, 禁止无界运行导致队列阻塞.
  - principals 在 kickoff/gate 时必须明确本次 delay budget (见 0.5 控制合同).
- 背压触发必须可观测:
  - 当 review 队列饱和、测试超时、或连续失败触发求助阈值时, 必须形成可引用的 artifact/digest 或结构化求助消息, 使决策可审计.

### 15.8 归档、保留与压缩 (Retention / Compaction) (仅规划)

Agent 世界的高吞吐会快速增长 events/inbox/artifacts. 若无保留策略, durable-first 会变成不可控的存储与索引成本.

固定要求:

- 任何清理都必须可审计:
  - 禁止静默删除 artifacts/reviews/onboarding/offboarding 记录.
  - 若引入 GC/归档, 必须追加事件或写入 tombstone, 使回放与责任链不被破坏.
- 压缩必须保持可回放的最小真相:
  - events.jsonl 可进行段落归档与摘要索引, 但必须保留可恢复的 sequence 边界与关键状态迁移证据.
- 保留策略必须可配置且默认保守:
  - org/team 级别可设置最大体量与保留窗口; 超额必须显式失败或显式归档, 禁止静默丢数据.

最小 tombstone 口径 (仅规划):

- 当 artifact content 需要被归档或移除时, 必须保留最小可审计信息, 并允许元数据只读查询:
  - `*_artifact_read(include_content=false)` 仍可返回元数据
  - `*_artifact_read(include_content=true)` 必须显式返回 "content 已归档/不可用" 的错误, 并给出可追溯信息
- tombstone 记录建议形态:

```json
{
  "schemaVersion": 1,
  "artifactId": "art-1",
  "archivedAt": 1739990000,
  "archivedByThreadId": "thread-owner-a",
  "contentDigest": "sha256:...",
  "sizeBytes": 1234,
  "reason": "retention_policy",
  "storageRef": "artifacts/archive/2026-03/art-1.tgz"
}
```

events compaction 口径 (仅规划):

- 若对 `events.jsonl` 做分段/压缩, 必须追加可审计事件 (例如 `team.events.compacted` / `org.events.compacted`), 并记录:
  - `fromSequence` / `toSequence`
  - `compactionDigest` (对压缩结果的摘要)
  - `storageRef` (归档位置)

### 15.9 深度上限与资源耗尽 (Depth / Budget Exhaustion) (仅规划)

Agent 世界的组织扩张与并行执行最终会触达运行时硬约束 (子代理深度上限、并发工具上限、预算上限). 达到上限时系统必须保持可控与可诊断.

固定要求:

- 达到上限必须显式失败:
  - 任何招募/派生 (如 `team_recruit` / `org_recruit`) 若因深度上限或预算被拒绝, 必须返回可机器识别的错误码与解释, 禁止静默成功或吞错.
- 读路径必须仍可用:
  - 即使无法继续招募, 也必须保留最小可观测与可收口能力 (例如 inbox pop/ack, task list/assign/complete, artifact read/list).
- 必须触发结构化求助:
  - 当达到上限导致关键任务无法推进时, 必须通过 `help_request` artifact + scope 内升级通道让 principals 做裁决 (收缩范围、调整配额、或延后).

## 16. 风险、边界场景与开放问题

必须显式记录并在实现前给出决策:

1. 多 org: 决策: v2 不支持. 一个 thread 同一时刻只能属于一个 org (由其 team 的 `orgId` 派生; 见 0.1).
1. 成员迁移: 决策: v2 支持, 但必须显式拆为 "先 offboarding 再 onboarding":
   - `team_member_remove` 负责收敛旧 team 的授权与任务分派 (避免悬挂)
   - `team_onboard` 负责注入新 team 的 onboarding packet 与 playbooks
1. 归档与清理: 决策: v2 不支持 hard delete, 只做可审计的归档/保留策略 (见 15.8).
1. 命名冲突: 决策: `members[].name` 必须唯一. 批量招募时若冲突, 默认自动编号去冲突 (或由策略显式拒绝), 且输出必须可审计.
1. 招募失败回滚: 决策: 批量招募允许部分成功, 无隐式回滚. 成功项 durable-first 落盘并返回, 失败项显式返回错误; 如需回收必须显式 `team_member_remove` / `org_owner_remove`.
1. 角色变更: 决策: durable-first 立即生效. `team_update_config(leaders=...)` 落盘并追加事件后, 授权检查以新 config 为准; 实时投递仅 best-effort, 不得影响授权收敛.
1. 深度上限: 决策: 达到深度/预算上限时必须显式失败招募, 但必须保留读路径与收口工具 (见 15.9).
1. 噪声与滥用: 决策: `broadcastPolicy=leaders_only` 默认启用, 配合限频配额 + quiet hours 抑制 + urgent 仅 principals. 更细粒度冷却窗口与 rollup 触发条件见 14.14.
1. 双节奏: 决策: digest 只作为显式 artifact (例如 `kind=digest|review_bundle|status_report`) 发布, v2 不做自动摘要; 当限频/配额触发时, 工具必须返回可操作的错误或抑制原因并引导使用 digest/rollup 收敛.
1. 事件日志增长: 决策: cursor 使用 `sequence` 保证稳定; 支持可审计的 compaction (见 15.8 的 `*.events.compacted` 口径), 禁止静默删除.
1. 产物保密: 决策: 通过 `visibility` 控制读取授权; team scope artifact 禁止跨 team 直接读取, 跨 team 共享必须 republish 到 org scope.
1. 产物一致性: 决策: read 时必须校验 `contentDigest`/`sizeBytes` 并显式报错; 当 contentRef 丢失或 content 已归档时, `*_artifact_read(include_content=true)` 必须显式失败并提供可追溯信息 (tombstone 或等价记录, 见 15.8).
1. Review 振荡: 决策: review 状态机 + anti-windup (同一 artifact 同时最多 1 个 open review) + 显式 cancel. override/重开通过创建新 review 并设置 `supersedesReviewId` 表达, 禁止隐式覆盖历史.
1. Onboarding 语义: 决策: `team_onboard(mode=refresh)` 必须幂等且低噪声; ack 仅表示收悉不代表理解完成; 入职包最小化与过期/刷新策略见 14.13.
1. Ownership routing 漂移: 决策: 先只支持 `path_glob`, 且匹配顺序确定 (priority -> 更具体 -> ruleId). 回滚通过 `map_digest` 定位历史版本; 误路由通过 principals 更新 map 显式纠偏.
1. 多 assignee 卡死: 决策: v2 先用显式控制面动作收口.
   - assignee 被移除: `team_member_remove` 或 `team_task_assign` 显式移除 assignee, 不得阻塞完成判定 (见 14.13 与 6.2.1).
   - 长时间无响应: 先通过 `leaseUntil` + principals 裁决执行重新分派/移除; 自动心跳与自动恢复见 17.5.
1. 招募失控: 决策: 配额/预算在工具入口同步校验; 超额只拒绝新增动作, 不影响既有成员/任务/产物, 回收必须显式 remove.
1. 真实世界一致性: 决策: 用 `harness_run.uncovered[]` 显式表达未覆盖项, 并要求 `team_task_approve` 显式输入 `accepted_uncovered[]` 作为风险接受清单; 未提供则审批失败 (见 6.3.5 与 14.16).

本提案后续每次实现一个新控制面工具, 都必须对上述问题给出本期的明确决策与可验证行为.

## 17. 后续路线图 (Roadmap) (仅规划)

本节将 v2 第一阶段不做但已规划的能力收敛为可验证的路线图, 以避免需求长期悬空或在实现时失去边界.

固定原则 (Pinned):

- 路线图能力仍必须受 `agent_org` 实验开关与子开关控制, 默认不开启.
- 任何新增能力必须写清控制闭环:
  - setpoint 是什么
  - 用哪些可观测信号判断偏差
  - 允许施加哪些控制输入, 以及冷却/滞回
- 任何自动化动作必须可审计、可回滚、可禁止; 禁止静默自动化.

### 17.1 Artifact 控制面增强 (Artifact Control Plane) (仅规划)

Primary setpoint:

- artifact 成为稳定的组织记忆与协作载体: 可检索、可复用、可引用, 让消息变短而不是变多.

最小能力 (仅规划):

- 查询与索引:
  - `*_artifact_query` 支持按 scope/kind/taskId/作者/时间窗/label 查询, 并提供 cursor 分页 (避免全量扫描).
  - `*_artifact_pin` / `*_artifact_unpin`: 将关键 artifacts 固化为 handbooks/directory 的引用入口.
- 组织化聚合:
  - `review_bundle`/`digest` 作为一等 artifact: 仅聚合引用与摘要, 不复制大段原文.
  - 支持将多个 artifacts 组合为 bundle 并作为门禁证据输入, 降低 review 队列压力.
- 语义一致性:
  - artifact 引用必须可追溯版本与来源 (谁创建/何时/基于哪些 inputs), 以避免双真相.

Acceptance (可验证):

- 在 team/org scope 下, 基于 cursor 的 query 能在大体量 artifacts 里稳定返回结果, 且不会触发全量扫描.
- pin/unpin 会产生可审计事件, 且 Directory/Handbook 的引用可被回放重建.
- visibility 约束在 query/read/pin 中一致执行, 禁止通过 query 绕过跨 team 边界.

Guardrails:

- durable truth 仍以 configs/events 为准; artifact 索引属于派生视图, 允许重建但不允许成为单点真相.
- 与 15.8 保留策略一致: content 归档后仍允许元数据查询, include_content=true 必须显式失败并可追溯.

主要传感器 (Sensors):

- `artifactCount`, `artifactBytes`, `readLatencyMs`, `queryLatencyMs`
- `pinCount`, `bundleCount`, `archivedCount`
- 未命中率 (query miss) 与重复发布率 (同一内容重复发布的比例)

主要控制输入 (Actuators):

- 索引更新与派生视图重建 (显式工具触发或后台任务, 但必须可审计)
- bundle/digest 的发布频率与体量上限 (与配额/限频联动)

复杂性转移账本 (示例):

- 复杂性原位置: 依赖聊天上下文与复制粘贴
- 新位置: artifact 索引与引用治理
- 收益: token 下降、口径更稳定、可审计与可回放
- 新成本: 索引维护、查询性能与权限一致性
- 失效模式: 索引漂移导致 "找不到/找错" 或绕过 visibility 的泄露风险

### 17.2 L2 长链路验证与资源化接口 + TUI overlays (仅规划)

Primary setpoint:

- 在长链路与多观察者场景下, org/team 的关键状态可被稳定读取、回放与重连, 成为 L2 门禁的可观测基础.

范围说明:

- L0/L1 仍以工具语义与最小集成测试为主.
- L2 主要解决 "观测与回放" 的时滞与稳定性问题: UI/协议不是主链功能, 但决定规模化可治理性.

最小能力 (仅规划):

- app-server v2 资源化接口:
  - 提供 org/team/task/inbox/artifact/review/directory 的只读查询接口与 cursor 分页.
  - 提供基于 `sequence` 的增量拉取或订阅 (支持断线重连与回放).
- TUI overlays:
  - Org Dashboard: teams 状态汇总、decision asks、队列背压信号.
  - Team Dashboard: inbox/task/artifact/review 的聚合视图 (引用为主).
  - Artifact Viewer: 渲染与引用跳转, 支持 bundle/digest 展开.

Acceptance (可验证):

- 多观察者可在同一 org/team 上并发读取与回放, 且 cursor/sequence 单调, 重连不会丢事件或重复应用导致错乱.
- 大体量 inbox/artifacts 下, UI 不进行全量扫描, 只做增量读取与分页.
- 未启用 `agent_org` 时不暴露相关接口或保持 v1 行为不变.

Guardrails:

- 不向 v1 增加 API 面; 资源化接口只在 v2/experimental 下出现.
- UI 是传感器, 不能成为控制面绕行入口: 任何控制动作仍必须走受控工具并落审计事件.

主要传感器 (Sensors):

- 回放一致性:
  - `sequenceGapCount` (增量拉取/订阅是否出现缺口)
  - `duplicateApplyCount` (是否出现重复应用导致状态错乱的迹象)
  - `replayDurationMs` (回放耗时)
- 查询与渲染:
  - `queryLatencyMs` (各资源 read/list 的耗时)
  - `renderLatencyMs` (TUI redraw/overlay 渲染耗时)
  - `pageScanBytes` (分页/增量是否意外退化为全量扫描)
- 重连与可用性:
  - `reconnectSuccessRate`, `subscriptionDropRate`
  - `staleReadRate` (读取到陈旧派生视图的比例, 需带时间戳口径)

主要控制输入 (Actuators):

- 分页与增量参数:
  - cursor/limit/pageSize 的默认值与上限
  - 基于 `sequence` 的增量窗口大小与回放批次大小
- 缓存与派生视图:
  - 缓存的 TTL/新鲜度声明与强制刷新入口 (必须可审计)
  - 派生视图的重建策略 (显式触发或后台任务, 但必须可追溯)
- UI 降载:
  - 在大体量数据下强制分页/折叠, 禁止一次性渲染全量列表

Known delays / Delay budget:

- L2 允许引入缓存与派生视图, 但必须声明新鲜度与回放一致性边界.
- 慢回路验证必须预算化: 全量刷新/回放需要超时与中止语义, 禁止无界运行导致队列阻塞.

### 17.3 新的聊天 UI (仅规划)

Primary setpoint:

- 降低人类决策时滞: 将高吞吐协作的关键信号收敛为可读、可追溯、可点击的结构化界面, 而不是靠长对话拼接上下文.

最小能力 (仅规划):

- 多视图而非单流:
  - Conversation: 与 President/teams 的对话流 (只保留必要文本).
  - Directory 面板: org 目录入口 (teams/councils/guilds/initiatives).
  - Work 面板: tasks/reviews/artifacts 的聚合视图与引用跳转.
- 低噪声优先:
  - 将状态更新默认导向 `status_report`/`digest` 视图, 聊天只展示引用与关键 asks.
  - 明确展示抑制/限频状态与原因, 避免 "消息丢了" 的误判.
- 边界可见:
  - UI 必须显式显示 scope 与角色, 并在越权/不可读时给出可解释的错误 (而不是空白).

Acceptance (可验证):

- 在不粘贴背景的情况下, 用户可通过 Directory 直接定位到 team/initiative 的关键入口 (charter/working agreement/status/review queue).
- UI 不改变授权边界: 不出现跨 team 读取成员 inbox/artifacts 的可达路径.

Guardrails:

- UI 不引入新的隐式状态: 任何显示都可追溯到 durable 事实源或 artifact 引用.
- UI 不将大段输出复制进聊天; 只显示摘要并提供引用跳转.

主要传感器 (Sensors):

- `timeToDecisionMs` (从 ask 生成到被批准/裁决的耗时; 仅基于事件/任务状态)
- `contextPasteRate` (需要用户反复粘贴背景的次数, 目标下降)
- `suppressionRate` / `rollupRate` (消息抑制与汇总比例, 目标可解释且稳定)
- `permissionDeniedRate` (越权读取/操作的被拒绝次数, 目标: 可解释且逐步下降)

主要控制输入 (Actuators):

- 默认视图路由:
  - quiet hours 内默认落在 digest/status 视图, 聊天仅展示引用与 asks
- 汇总与折叠策略:
  - 对高频事件启用 rollup, 并声明冷却窗口与滞回 (与 14.14 对齐)
- 可见性护栏:
  - 当 scope/角色不足时, 强制显示可解释错误与升级路径, 禁止静默空白

### 17.4 嵌套团队 (Team-of-Teams) (仅规划)

Primary setpoint:

- 当组织扩张到 President 无法直接治理全部 teams 时, 允许分层管理而不破坏边界与审计口径.

定义与边界:

- 嵌套团队用于表达持久化的治理层级 (长期组织结构), 不取代 initiative (临时跨 team 协调层).
- 仍坚持跨 team 仅 principals 通道; parent/child 关系不提供 member 跨 team 直连.

最小能力 (仅规划):

- 组织结构:
  - `parentTeamId`/`childrenTeamIds` 作为 team 的组织索引字段, 用于 Directory 与治理升级路径.
  - 允许非 President 的 principals 在受控策略下创建 child team, 但必须:
    - 受配额/深度上限约束
    - 自动注册到 org
    - 写入可审计事件 (谁创建/为何/边界是什么)
- 层级治理:
  - parent team owner 可作为 child team 的 sponsor:
    - 负责提案与资源协调
    - 通过 principals channel 与 child owner 对齐
  - 任何越权治理动作必须显式且可审计 (例如 sponsor 请求 President override), 禁止暗箱操作.
- 生命周期:
  - child team 的创建/关闭/迁移必须更新 Directory, 并提供可回放的层级变更事件.

Acceptance (可验证):

- 层级结构可从 events 回放重建, Directory 与 durable 真相一致.
- 层级存在时, 仍无法通过 parent/child 关系绕过 visibility/跨 team 边界.
- 深度/配额达到上限时, child team 创建必须显式失败并可诊断, 不产生半成功状态.

Guardrails:

- 不允许 "自由无治理 spawn": 创建 child team 必须通过受控工具, 并受限于 `maxTeams`/`maxTeamDepth`/`maxChildTeamsPerTeam`.
- 防止控制器冲突: President 与 sponsor 的治理权限必须有明确仲裁与升级路径, 不能同时修改同一 child team 的关键配置而无审计.

主要传感器 (Sensors):

- `teamDepth` / `childTeamCount` (层级与规模是否触发深度上限)
- `sponsorOverrideRate` (sponsor 触发 President override 的频率, 过高表示边界或仲裁失效)
- `directoryDriftCount` (Directory 与 durable 真相不一致的次数, 目标趋近 0)

主要控制输入 (Actuators):

- 配额与护栏:
  - `maxTeamDepth`, `maxChildTeamsPerTeam`, `maxTeams` (超额必须显式失败)
- 组织结构变更策略:
  - 创建/关闭/迁移 child team 的审批与冷却窗口 (避免频繁重组导致治理振荡)
- 仲裁升级:
  - sponsor 无法裁决时升级到 President/Architecture Council 的明确入口

复杂性转移账本 (示例):

- 复杂性原位置: President 集中治理所有 teams
- 新位置: 层级结构与 sponsor 治理协议
- 收益: 扩张能力提升, 人类裁决时滞下降
- 新成本: 层级漂移治理、权限仲裁与组织目录一致性
- 失效模式: sponsor 越权或层级混乱导致责任扩散与边界被误解

### 17.5 自动心跳与自动恢复 (Auto Heartbeat / Auto Recovery) (仅规划)

Primary setpoint:

- 在 7x24 高吞吐下, 系统能把 "无响应/卡死/队列堆积" 收敛为低噪声、可审计、可恢复的控制动作, 而不是靠人工盯盘.

基本原则 (Pinned):

- 自动化默认非破坏性:
  - 优先触发观测与求助 (status/digest/help_request), 而不是直接 close/remove/reassign.
- 破坏性动作必须显式授权:
  - 重新分派、移除成员、关闭 agent 等必须由 principals 批准或由策略明确允许, 且必须可审计.
- anti-chatter:
  - 同一对象 (task/assignee) 的自动动作必须有冷却与退避, 禁止刷屏式 ping.

最小能力 (仅规划):

- Heartbeat 信号:
  - 从 durable 事实源派生 liveness: `lastInboxAckAt`, `lastStatusSubmitAt`, `lastHarnessRunAt`, `lastTaskActivityAt`.
  - 允许 principals 为任务或临时成员设置 `leaseUntil` 作为时滞预算与超时边界.
- 恢复动作 (按风险从低到高):
  1. 生成 `help_request` artifact 并升级到 leader/owner (带证据引用与可选决策).
  1. 请求 assignee 发布 `status_report` 或最小证据 (harness_run/patch), 超时则进入下一步.
  1. 由 principals 裁决 reassign/cancel/demobilize, 并写入可回放事件.

主要传感器 (Sensors):

- liveness 派生信号:
  - `lastInboxAckAt`, `lastStatusSubmitAt`, `lastHarnessRunAt`, `lastTaskActivityAt`
- 队列与时滞信号:
  - `oldestTaskIdleMs`, `oldestReviewAge`, `reviewBacklogDepth`
  - `helpRequestCount` (按时间窗聚合, 用于识别恢复风暴)

主要控制输入 (Actuators):

- 低风险控制输入:
  - 生成 `help_request` + 升级通道引用 (默认优先)
  - 请求 `status_report`/证据产物 (harness_run/patch) 并设置超时
- 高风险控制输入 (必须显式批准或策略允许):
  - reassign/cancel/demobilize/close (每次必须写入事件与受影响对象清单)
- 抑振手段:
  - 对同一对象的恢复动作设置冷却窗口与退避 (anti-chatter)
  - 当执行器已饱和 (例如连续失败或队列打满) 时停止继续加压, 转为求助与裁决 (anti-windup)

Known delays / Delay budget:

- 人类窗口是主要时滞:
  - 自动恢复默认只能推进到 "待裁决/待批准" 状态, 禁止越权自作主张.
- 每个自动动作必须预算化:
  - 对同一对象的自动动作在时间窗内必须有上限, 超额必须降级为 rollup/digest, 禁止刷屏.

Acceptance (可验证):

- 当任务或成员超过 `leaseUntil` 或超过无响应阈值时, 系统会生成可引用的求助与决策请求, 且不会在 quiet hours 内刷屏.
- 对同一对象的自动动作具备退避与冷却窗口, 不会形成控制振荡.
- 任何 reassign/remove/close 都有可追溯事件与受影响对象清单, 且不会产生半成功状态.

Guardrails:

- 自动恢复不得绕过权限与边界, 不得跨 team 对 member 执行控制动作.
- 自动恢复不得删除证据与历史; 只允许追加事件与显式状态迁移.
