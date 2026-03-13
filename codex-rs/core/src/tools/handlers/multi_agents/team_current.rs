use super::*;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct TeamCurrentArgs {}

#[derive(Debug, Serialize)]
struct TeamCurrentResult {
    team_id: Option<String>,
    team_name: Option<String>,
    role: Option<String>,
    lead_thread_id: Option<String>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "team_current requires the agent_org experimental feature".to_string(),
        ));
    }

    let _: TeamCurrentArgs = parse_arguments(&arguments)?;

    let result = match find_persisted_team_for_thread(
        turn.config.codex_home.as_path(),
        session.conversation_id,
    )
    .await?
    {
        Some((team_id, config, role)) => TeamCurrentResult {
            team_id: Some(team_id),
            team_name: Some(config.team_name),
            role: Some(role.as_str().to_string()),
            lead_thread_id: Some(config.lead_thread_id),
        },
        None => TeamCurrentResult {
            team_id: None,
            team_name: None,
            role: None,
            lead_thread_id: None,
        },
    };

    let content = serde_json::to_string(&result).map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize team_current result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
