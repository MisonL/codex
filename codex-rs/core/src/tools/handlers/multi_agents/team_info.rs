use super::*;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct TeamInfoArgs {
    team_id: String,
}

#[derive(Debug, Serialize)]
struct TeamInfoMember {
    name: String,
    agent_id: String,
    agent_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct TeamInfoResult {
    team_id: String,
    team_name: String,
    lead_thread_id: String,
    created_at: i64,
    members: Vec<TeamInfoMember>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "team_info requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: TeamInfoArgs = parse_arguments(&arguments)?;
    let team_id = normalized_team_id(&args.team_id)?;
    let config = read_persisted_team_config(turn.config.codex_home.as_path(), &team_id).await?;
    assert_team_member_or_lead(&team_id, &config, session.conversation_id)?;

    let members = config
        .members
        .into_iter()
        .map(|member| TeamInfoMember {
            name: member.name,
            agent_id: member.agent_id,
            agent_type: member.agent_type,
        })
        .collect::<Vec<_>>();

    let content = serde_json::to_string(&TeamInfoResult {
        team_id,
        team_name: config.team_name,
        lead_thread_id: config.lead_thread_id,
        created_at: config.created_at,
        members,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize team_info result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
