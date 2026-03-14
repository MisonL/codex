use super::*;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct TeamUpdateConfigArgs {
    team_id: String,
    leader_names: Option<Vec<String>>,
    broadcast_policy: Option<PersistedBroadcastPolicy>,
}

#[derive(Debug, Serialize)]
struct TeamUpdateConfigResult {
    team_id: String,
    leader_names: Vec<String>,
    leader_thread_ids: Vec<String>,
    broadcast_policy: String,
}

fn normalize_leader_names(leader_names: Vec<String>) -> Result<Vec<String>, FunctionCallError> {
    let mut normalized_names = Vec::with_capacity(leader_names.len());
    let mut seen_names = HashSet::new();
    for leader_name in leader_names {
        let leader_name = leader_name.trim();
        if leader_name.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "leader_names must not contain blank names".to_string(),
            ));
        }
        if !seen_names.insert(leader_name.to_string()) {
            return Err(FunctionCallError::RespondToModel(format!(
                "leader `{leader_name}` is specified more than once"
            )));
        }
        normalized_names.push(leader_name.to_string());
    }
    Ok(normalized_names)
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "team_update_config requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: TeamUpdateConfigArgs = parse_arguments(&arguments)?;
    let team_id = normalized_team_id(&args.team_id)?;
    let codex_home = turn.config.codex_home.as_path();
    let _lock = lock_team_config(codex_home, &team_id).await?;
    let mut config = read_persisted_team_config(codex_home, &team_id).await?;
    let caller_thread_id = session.conversation_id.to_string();
    if caller_thread_id != config.lead_thread_id {
        return Err(FunctionCallError::RespondToModel(format!(
            "only the team owner can update config for team `{team_id}`"
        )));
    }

    if let Some(leader_names) = args.leader_names {
        let leader_names = normalize_leader_names(leader_names)?;
        let mut leader_thread_ids = Vec::with_capacity(leader_names.len());
        for leader_name in &leader_names {
            let member = config
                .members
                .iter()
                .find(|member| member.name == *leader_name)
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(format!(
                        "leader `{leader_name}` is not a member of team `{team_id}`"
                    ))
                })?;
            leader_thread_ids.push(member.agent_id.clone());
        }
        config.leaders = leader_thread_ids;
    }

    if let Some(broadcast_policy) = args.broadcast_policy {
        config.broadcast_policy = broadcast_policy;
    }

    write_json_atomic(&team_config_path(codex_home, &team_id), &config)
        .await
        .map_err(|err| team_persistence_error("write team config", &team_id, err))?;

    let leader_names = config
        .leaders
        .iter()
        .filter_map(|leader_thread_id| {
            config
                .members
                .iter()
                .find(|member| member.agent_id == *leader_thread_id)
                .map(|member| member.name.clone())
        })
        .collect::<Vec<_>>();
    let content = serde_json::to_string(&TeamUpdateConfigResult {
        team_id,
        leader_names,
        leader_thread_ids: config.leaders,
        broadcast_policy: config.broadcast_policy.as_str().to_string(),
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize team_update_config result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
