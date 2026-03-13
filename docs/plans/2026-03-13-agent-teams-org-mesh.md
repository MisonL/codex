# Agent Teams v2: Org Hierarchy + Mesh Collaboration (Design Proposal)

Date: 2026-03-13

This document proposes a next iteration of Codex "Agent Teams" that:

- Enables peer-to-peer collaboration *within* a team (not just lead <-> member).
- Supports assigning a *single* task to *multiple* agents without pre-splitting work.
- Adds a clear communication boundary between teams: cross-team messaging is restricted to team leaders.
- Establishes an explicit hierarchy: a user-facing "President" (mainline agent thread) manages multiple teams via their leaders.

The proposal is intentionally incremental and reuses the existing durable inbox + persisted tasks primitives described in `docs/agent-teams.md`.

## 0. Positioning (Built on Existing Swarm Architecture)

This proposal is a focused "Agent Teams" enhancement that sits inside the broader multi-agent control-plane direction described in:

- `docs/plans/2026-03-06-codex-swarm-architecture.md`

In particular, it follows the same core principle:

- Add a small control plane layer, minimize changes to the data plane, and avoid rewriting the execution plane.

How this document maps to the earlier design:

- Control-plane objects:
  - `Org` (President + team leaders) is a lightweight slice of the proposed `SwarmRun` (kind: `swarm`).
  - `Team` remains the existing `team_id`-scoped workflow (kind: `team`), but we add missing semantics (mesh messaging, leader delegation, multi-assignee tasks).
- Task model:
  - Multi-assignee tasks extend the earlier `TaskSpec` idea by tracking per-assignee state, without forcing the leader to pre-split work.
- Observability and replay:
  - Team/org messages and task transitions should carry a stable envelope (`swarmRunId`, `teamId`, `taskId`, `sequence`, `causalParent`) so the system remains auditable and replayable.
- Memory model:
  - Keep thread work memory isolated by default; share via explicit, published artifacts when content is large or should be durable.

## 1. Goals

1. Team members can directly coordinate with each other using team-scoped tools.
1. A team leader can assign one task to multiple members and let the members self-organize.
1. Cross-team communication is constrained to team leaders (and optionally the President), with a single controlled ingress/egress.
1. The user-facing mainline agent acts as a "President" who supervises team leaders and overall progress.
1. All messaging remains durable-first (persist, then best-effort live delivery).

## 2. Non-goals

1. Full "nested teams" where a teammate can freely spawn more teams/agents without any governance. (This can be added later with quotas.)
1. A distributed, multi-process control plane. This proposal stays in-process and file-persisted like v1.
1. A brand new chat UI. The core deliverable is semantics and tools; UI improvements are follow-ons.

## 3. Current State (v1) and Gaps

In `docs/agent-teams.md`, the current Agent Teams workflow provides:

- `spawn_team` / `wait_team` / `close_team` / `team_cleanup`
- Durable per-thread inbox under `$CODEX_HOME/teams/<team_id>/inbox/<thread_id>.jsonl`
- A persisted initial task per spawned member under `$CODEX_HOME/tasks/<team_id>/*.json`
- Task operations: `team_task_list`, `team_task_claim(_next)`, `team_task_complete`
- Lead-driven messaging: `team_message`, `team_broadcast` (lead -> member), and `team_ask_lead` (member -> lead)

Observed gaps for "real teams":

1. Intra-team messaging is effectively star-shaped around the lead thread.
1. Tasks are 1:1 assigned, which forces the lead to pre-split responsibilities.
1. There is no first-class cross-team boundary. A teammate could use generic tooling (`send_input`) to message anyone if it knows ids.
1. Leadership is implicit (the spawning thread) and does not map to the common "team leader agent" mental model.

## 4. Proposed Model

### 4.1 Entities

1. **Organization (Org)**
1. **Team**
1. **Agent thread** (existing `ThreadId` / "agent_id")

### 4.2 Roles

1. **President**
- The user-facing mainline agent thread.
- Owns the Org and oversees all teams.
- Responsible for creating teams and appointing team leaders.

1. **Team Leader**
- An agent thread that is a member of exactly one operational team.
- Has privileges to manage that team (messaging policy, task assignment, status reporting).

1. **Team Member**
- A normal agent thread in a team.
- Can coordinate directly with peers in the same team.

### 4.3 Envelope (Swarm-style Metadata)

To align with the `swarm envelope` direction from `2026-03-06-codex-swarm-architecture.md`, the following metadata should be present (at least in persisted state, and ideally also in emitted events):

- `swarmRunId`: the Org id (President-managed "swarm run" scope)
- `teamId`: the team id
- `agentId`: sender/receiver thread id
- `taskId`: optional; set when the message or state transition is tied to a task
- `sequence`: monotonic sequence per `(swarmRunId, teamId)` for deterministic replay
- `causalParent`: optional causal link (for example: "this message was sent in response to task X claim")

This proposal does not require changing the existing `item` model; it only requires enriching persisted records and collab events with stable identifiers.

## 5. Mesh Collaboration Inside a Team

### 5.1 Design Principle

If two agents are members of the same `team_id`, they should be able to communicate through a team-scoped tool that:

1. Validates membership.
1. Persists the message to the receiver inbox (durable-first).
1. Attempts real-time delivery (best-effort).

### 5.2 Tool Changes

#### 5.2.1 `team_info` (new)

Return team metadata needed for self-organization:

- `team_id`, `org_id`
- `leaders` (thread ids and names)
- `members` (thread ids, names, agent roles if available)
- Optional: messaging policy (see below)

This prevents "out-of-band" sharing of agent ids and enables agents to discover peers in-team.

#### 5.2.2 `team_message` (behavior change)

Current v1 semantics are effectively "lead -> member". v2 semantics:

1. Any team member or leader may call `team_message`.
1. Sender and receiver must both be members of the same `team_id`.
1. The persisted inbox entry should include:
- `from_thread_id`
- `from_name` (resolved from team config; `"president"` when applicable)
- `from_role` (member/leader/president)
- `team_id`
- Optional `task_id` when the message is about a task

This turns the team into a mesh, without exposing cross-team messaging.

#### 5.2.3 `team_broadcast` (policy + behavior change)

Broadcast is useful but can become noisy. v2 proposes a policy flag in team config:

- `broadcast_policy: "leaders_only" | "all_members"`

Default: `leaders_only`.

If `all_members`, any member can broadcast; if `leaders_only`, non-leaders must use `team_message` or ask the leader.

### 5.3 Recommended Collaboration Protocol (prompt-level)

Tools enable messaging; prompts/instructions make it effective. When a task is assigned to multiple agents, inject a standard kickoff message:

1. Each assignee posts what they plan to do in 2-4 bullets.
1. Assignees negotiate boundaries and dependencies via `team_message`.
1. If there is conflict or ambiguity, escalate to the Team Leader.

This keeps autonomy inside the team without requiring the leader to micromanage upfront.

## 6. Multi-assignee Tasks

### 6.1 Problem

A leader should be able to assign one task to multiple agents, expecting them to coordinate and self-split, instead of pre-splitting into N tasks.

### 6.2 Task Model v2 (schema concept)

Replace single `assignee` with `assignees`:

- `assignees: [{ name, agent_id }]`
- `assignee_state: { "<agent_id>": "pending" | "claimed" | "completed" }`
- `claim_mode: "shared" | "exclusive"`
- `completion_mode: "all_assignees" | "any_assignee" | "leader_approves"`
- `lease_until`: optional; aligns with the earlier `TaskSpec.lease_until` / `Lease` concepts for long-running ownership
- `artifacts`: optional; artifact references published by assignees (see below)

Defaults:

- `claim_mode: "shared"` when `assignees.len() > 1`, else `exclusive`
- `completion_mode: "all_assignees"` when `assignees.len() > 1`, else `any_assignee`

### 6.3 Tool Changes

#### 6.3.1 `team_task_create` (new)

Create a task after `spawn_team`:

- `team_id`
- `title`
- `description` (optional)
- `assignees` (one or more member names or thread ids)
- `dependencies` (optional)
- `claim_mode` / `completion_mode` (optional)
- `kickoff: true|false` (optional, default true): when true, automatically send a kickoff message to all assignees with the collaboration protocol.

#### 6.3.2 `team_task_claim` / `team_task_claim_next` (behavior change)

For `shared` tasks:

- Claiming marks the caller's `assignee_state` as `claimed` but does not block other assignees.

For `exclusive` tasks:

- Preserve current behavior (exactly one claim).

#### 6.3.3 `team_task_complete` (behavior change)

For `shared` tasks:

- Completing marks the caller's `assignee_state` as `completed`.
- Task is considered completed when `completion_mode` is satisfied.

For `exclusive` tasks:

- Preserve current behavior.

#### 6.3.4 `team_task_assign` (new)

Allow leaders to add/remove assignees after creation.

### 6.4 Why This Solves "Leader Doesn't Need to Pre-split"

1. The leader assigns a single shared task to multiple agents.
1. Agents coordinate in-team (mesh messaging) and decide boundaries themselves.
1. The task model tracks per-assignee progress without requiring N separate tasks.

## 7. Cross-team Communication: Leaders Only

### 7.1 Design Principle

Team members should not directly message other teams. Cross-team communication should:

1. Be possible when needed.
1. Have a single controlled ingress/egress.
1. Be restricted to team leaders (and optionally the President).

### 7.2 Organization Layer (new persisted concept)

Introduce an Org registry persisted under `$CODEX_HOME/orgs/<org_id>/...`:

- `config.json`: President thread id, list of teams, per-team leader thread ids
- Org-scoped durable inbox per leader (same durable-first semantics)

This Org layer is the boundary enforcement mechanism.

### 7.3 Org Tools (new)

1. `org_info`: list teams and leaders in the org.
1. `org_leader_message`: leader -> leader message, validated by org config.
1. `org_inbox_pop` / `org_inbox_ack`: receive and ack org-scoped messages.

Authorization:

- `org_leader_message` may only be called by:
  - the President thread, or
  - a thread listed as a leader for some team in the org.
- The receiver must be:
  - a leader of another team in the org, or
  - the President thread.

### 7.4 Enforcing the Boundary (optional hardening)

To prevent bypassing the boundary via generic tools:

1. Restrict `send_input` for teammate threads.
1. Provide a team-scoped alternative (`team_message`) that supports peer messaging only within team.

This is a policy decision; it can be introduced as a configuration toggle if needed.

## 8. Leadership Delegation Inside a Team

To match the mental model of "a team has a leader agent":

1. Add `leaders: [thread_id]` to the persisted team config.
1. Treat team leaders as privileged actors for:
- broadcast policy
- task create/assign
- status reporting to the President

The spawning thread (President) remains the owner for cleanup and auditing, but does not need to micromanage team operations.

## 9. Artifacts (Explicit Sharing, Not Shared Context)

To stay consistent with the earlier "default isolation, share via artifact" guidance:

1. Team messages should be short and coordination-oriented.
1. Non-trivial outputs (plans, summaries, patch sets, reviews, tables) should be published as explicit artifacts and referenced by id.

Follow-on control-plane tools (not required for the first milestone) that would make this practical:

- `team_artifact_publish`: create an artifact in the team scope.
- `team_artifact_read`: read an artifact.
- `team_artifact_list`: list artifacts for a task/team.

These map directly to the `Artifact` object described in `2026-03-06-codex-swarm-architecture.md`.

## 10. Example End-to-end Flow

### 9.1 President creates two teams and appoints leaders

1. `spawn_team` creates Team A with members including `lead-a`.
1. `spawn_team` creates Team B with members including `lead-b`.
1. President updates each team config to mark `lead-a` and `lead-b` as leaders (mechanism: `team_set_leader` tool or a `leaders` field in `spawn_team` args).
1. President creates an Org and registers Team A/B and their leaders.

### 9.2 Team A leader assigns one task to multiple members

1. `team_task_create` with `assignees: ["alice", "bob", "charlie"]` and `kickoff: true`.
1. Each assignee claims the task (`team_task_claim`), posts their plan, and self-splits work.
1. Each marks completion (`team_task_complete`) as they finish.

### 9.3 Team A leader communicates with Team B leader

1. `org_leader_message` from `lead-a` to `lead-b`.
1. `lead-b` forwards relevant details to Team B members via `team_broadcast` or `team_message`.

## 11. TUI UX (User Feedback + Control Surfaces)

This section describes how the Codex TUI can make multi-team work feel visible and controllable without flooding the transcript.

Design goals:

1. **Layered information:** at-a-glance summaries first, drill-down when needed.
1. **Low noise by default:** avoid streaming every internal message into the main transcript.
1. **Fast navigation:** switching focus between President / leaders / members should be 1-2 actions.
1. **Durable state as source of truth:** dashboards should read from persisted control-plane state (not from model output).

### 11.1 Information Architecture

The TUI should provide a clear hierarchy:

- Org (President scope) -> Teams -> Agents -> Tasks -> Artifacts

Where:

- The main chat thread is the **President**.
- Each team has one or more **leaders** (agent threads).
- Members collaborate in-team via mesh messaging + shared tasks.

### 11.2 Entry Points (Commands)

The TUI already has slash commands and selection views. Add team/org entry points as slash commands:

- `/org`: open Org dashboard (teams + leaders + rollups)
- `/org inbox`: show President's org inbox (leader updates, cross-team coordination)
- `/team`: open current team dashboard (for leader/member threads)
- `/team tasks`: open task board (current team)
- `/team inbox`: show current thread's team inbox
- `/teams`: list teams and jump to a team leader thread ("watch leader")

These commands should be available only when collaboration features are enabled (same gating model as `/collab` and `/agent`).

### 11.3 Dashboards and Overlays

Use the existing full-screen overlay/pager pattern (like the transcript overlay) to implement:

1. **Org Dashboard (President)**
- Table rows: `team name/id`, `leader`, `status`, `members`, `tasks (pending/claimed/completed)`, `unread (org inbox)`
- Actions:
  - Watch leader thread
  - Message leader (org-scoped)
  - Open team summary (read-only)

1. **Team Dashboard (Leader/Member)**
- Sections:
  - Members list with status dots
  - Task rollup
  - Inbox rollup (unread)
  - Recent artifacts (by task)
- Actions:
  - Watch a member thread
  - Open task board
  - Open inbox

1. **Task Board**
- Group tasks by state: `Pending`, `Claimed`, `Completed`
- For multi-assignee tasks: show per-assignee sub-state (claimed/completed) and completion mode ("any/all/leader approves")
- Actions:
  - Claim (self)
  - Complete (self)
  - Open artifacts for task
  - (Leader only) assign/unassign members

1. **Inbox Viewer**
- Backed by `team_inbox_pop/ack` and `org_inbox_pop/ack` (cursor-based).
- Show messages with:
  - `from` (name + role)
  - `team/org` context
  - `taskId` (if present)
  - prompt preview
- Provide paging, search, and an explicit "Ack all visible" action.

### 11.4 Transcript: Summaries, Not Full Internal Chat

The main transcript should contain:

- High-level orchestration events (spawn/wait/close) (already present via `Collab*` events).
- Task lifecycle summaries:
  - Task created (team + assignees)
  - Task completed (who completed, whether task is now fully completed)
- Leader-to-President updates (org inbox), summarized as short "status cards".

It should *not* automatically display every intra-team peer message by default. Those belong in inbox views.

### 11.5 Status Line Enhancements (Optional)

Add new optional status line items to support "at a glance" operation:

- `org`: current org id (or "none")
- `team`: current team id/name (or "none")
- `agents`: running/total agents in org (or in team)
- `unread`: unread inbox count (org/team depending on role)
- `tasks`: pending/claimed rollup for current team

These should be computed from persisted state and cached with lightweight refresh intervals.

### 11.6 Data Sources (No Model Required)

To avoid model/tool coupling, the TUI should query state via:

1. Persisted control-plane files under `$CODEX_HOME` (teams, orgs, tasks, inbox cursors).
1. Or (preferred long-term) app-server v2 endpoints for `swarm/read`, `swarm/list`, `swarm/task/list`, `swarm/inbox/pop`.

This aligns with the `2026-03-06` plan: collab tools become stable protocol/control-plane resources, not "tool output parsing".

### 11.7 UX Edge Cases

1. If collaboration features are disabled, the dashboards should show a friendly prompt to enable them.
1. If an agent thread is missing (shutdown/not found), keep it visible but marked closed (like the existing agent picker).
1. If inbox JSONL grows large, rely on cursor-based pop and avoid full-file scans on every redraw.

## 12. Incremental Implementation Plan

1. Mesh messaging:
- Add `team_info`.
- Update `team_message` to allow member-to-member messaging with membership validation.
- Add `broadcast_policy` to team config and enforce in `team_broadcast`.

1. Multi-assignee tasks:
- Add `team_task_create` and `team_task_assign`.
- Extend persisted task schema to support multiple assignees and per-assignee state.
- Update claim/complete logic accordingly.

1. Org boundary:
- Introduce org persistence and `org_*` tools for leader-to-leader messaging.
- Optionally restrict `send_input` for teammate threads to harden boundaries.

1. UX follow-ons:
- TUI overlays for org/team inboxes and task state summaries.

## 13. Compatibility and Migration

1. Keep v1 tool names where possible; change behavior in a backward-compatible way where feasible.
1. Version persisted schemas:
- `schemaVersion` in team config and task json.
1. Provide a migration path for v1 teams:
- v1 team config: `leaders = []` implies "no delegated leader" and defaults to President-only broadcast/task creation.
- v1 tasks map to v2 tasks with `assignees = [assignee]`.
