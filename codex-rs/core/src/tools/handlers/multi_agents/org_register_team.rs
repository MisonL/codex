use super::*;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct OrgRegisterTeamArgs {
    org_id: String,
    team_id: String,
}

#[derive(Debug, Serialize)]
struct OrgRegisterTeamResult {
    org_id: String,
    team_id: String,
    changed: bool,
    owner_thread_id: String,
    leaders: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedOrgEvent {
    id: String,
    created_at: i64,
    org_id: String,
    sequence: u64,
    kind: String,
    actor_thread_id: String,
    causal_parent: Option<String>,
    payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedTeamEvent {
    id: String,
    created_at: i64,
    org_id: String,
    team_id: String,
    sequence: u64,
    kind: String,
    actor_thread_id: String,
    task_id: Option<String>,
    causal_parent: Option<String>,
    payload: Value,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_register_team requires the agent_org experimental feature".to_string(),
        ));
    }

    if matches!(turn.session_source, SessionSource::SubAgent(_)) {
        return Err(FunctionCallError::RespondToModel(
            "org_register_team is president-only and must be called from the main session"
                .to_string(),
        ));
    }

    let args: OrgRegisterTeamArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;
    let team_id = normalized_team_id(&args.team_id)?;
    let codex_home = turn.config.codex_home.as_path();
    let _org_lock = lock_org_config(codex_home, &org_id).await?;
    let _team_lock = lock_team_config(codex_home, &team_id).await?;

    let caller_thread_id = session.conversation_id.to_string();
    let mut org_config = read_persisted_org_config(codex_home, &org_id).await?;
    if org_config.president_thread_id != caller_thread_id {
        return Err(FunctionCallError::RespondToModel(format!(
            "only the org president can register teams for org `{org_id}`"
        )));
    }

    let mut team_config = read_persisted_team_config(codex_home, &team_id).await?;
    if let Some(existing_org_id) = team_config.org_id.as_deref() {
        if existing_org_id != org_id {
            return Err(FunctionCallError::RespondToModel(format!(
                "team `{team_id}` is already registered to org `{existing_org_id}`"
            )));
        }
    }

    let desired_team_ref = PersistedOrgTeamRef {
        team_id: team_id.clone(),
        owner_thread_id: team_config.lead_thread_id.clone(),
        leaders: team_config.leaders.clone(),
    };

    let mut org_changed = false;
    match org_config
        .teams
        .iter_mut()
        .find(|entry| entry.team_id == team_id)
    {
        Some(existing) => {
            if *existing != desired_team_ref {
                *existing = desired_team_ref.clone();
                org_changed = true;
            }
        }
        None => {
            org_config.teams.push(desired_team_ref.clone());
            org_changed = true;
        }
    }

    let mut team_changed = false;
    if team_config.org_id.as_deref() != Some(org_id.as_str()) {
        team_config.org_id = Some(org_id.clone());
        team_changed = true;
    }

    if org_changed {
        write_json_atomic(&org_config_path(codex_home, &org_id), &org_config)
            .await
            .map_err(|err| org_persistence_error("write org config", &org_id, err))?;
    }
    if team_changed {
        write_json_atomic(&team_config_path(codex_home, &team_id), &team_config)
            .await
            .map_err(|err| team_persistence_error("write team config", &team_id, err))?;
    }

    let created_at = now_unix_seconds();
    if org_changed {
        let events_path = org_events_path(codex_home, &org_id);
        let _events_lock = lock_org_events(codex_home, &org_id).await?;
        let sequence = next_jsonl_sequence(&events_path)
            .await
            .map_err(|err| org_persistence_error("compute org event sequence", &org_id, err))?;
        let event = PersistedOrgEvent {
            id: format!("ev-{}", ThreadId::new()),
            created_at,
            org_id: org_id.clone(),
            sequence,
            kind: "org.team.registered".to_string(),
            actor_thread_id: caller_thread_id.clone(),
            causal_parent: None,
            payload: json!({
                "teamId": team_id.clone(),
                "ownerThreadId": desired_team_ref.owner_thread_id,
                "leaders": desired_team_ref.leaders
            }),
        };
        append_jsonl_entry(&events_path, &event)
            .await
            .map_err(|err| org_persistence_error("append org event", &org_id, err))?;
    }
    if team_changed {
        let events_path = team_events_path(codex_home, &team_id);
        let _events_lock = lock_team_events(codex_home, &team_id).await?;
        let sequence = next_jsonl_sequence(&events_path)
            .await
            .map_err(|err| team_persistence_error("compute team event sequence", &team_id, err))?;
        let event = PersistedTeamEvent {
            id: format!("ev-{}", ThreadId::new()),
            created_at,
            org_id: org_id.clone(),
            team_id: team_id.clone(),
            sequence,
            kind: "team.config.updated".to_string(),
            actor_thread_id: caller_thread_id.clone(),
            task_id: None,
            causal_parent: None,
            payload: json!({ "orgId": org_id.clone() }),
        };
        append_jsonl_entry(&events_path, &event)
            .await
            .map_err(|err| team_persistence_error("append team event", &team_id, err))?;
    }

    let content = serde_json::to_string(&OrgRegisterTeamResult {
        org_id,
        team_id,
        changed: org_changed || team_changed,
        owner_thread_id: team_config.lead_thread_id,
        leaders: team_config.leaders,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_register_team result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
