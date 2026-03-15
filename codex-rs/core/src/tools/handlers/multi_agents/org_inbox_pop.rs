use super::*;
use std::sync::Arc;

const DEFAULT_INBOX_POP_LIMIT: usize = 50;
const MAX_INBOX_POP_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
struct OrgInboxPopArgs {
    org_id: String,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct OrgInboxMessage {
    id: String,
    created_at: i64,
    from_thread_id: String,
    from_team_id: Option<String>,
    from_role: String,
    input_items: Vec<UserInput>,
    prompt: String,
}

impl From<org_inbox::OrgInboxEntry> for OrgInboxMessage {
    fn from(value: org_inbox::OrgInboxEntry) -> Self {
        Self {
            id: value.id,
            created_at: value.created_at,
            from_thread_id: value.from_thread_id,
            from_team_id: value.from_team_id,
            from_role: value.from_role,
            input_items: value.input_items,
            prompt: value.prompt,
        }
    }
}

#[derive(Debug, Serialize)]
struct OrgInboxPopResult {
    org_id: String,
    thread_id: String,
    role: String,
    messages: Vec<OrgInboxMessage>,
    ack_token: String,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_inbox_pop requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: OrgInboxPopArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;
    let codex_home = turn.config.codex_home.as_path();
    let config = read_persisted_org_config(codex_home, &org_id).await?;
    let principal =
        assert_persisted_org_principal(codex_home, &org_id, &config, session.conversation_id)
            .await?;

    let limit = args
        .limit
        .unwrap_or(DEFAULT_INBOX_POP_LIMIT)
        .clamp(1, MAX_INBOX_POP_LIMIT);
    let (entries, ack_token) =
        org_inbox::pop_inbox_entries(codex_home, &org_id, session.conversation_id, limit).await?;

    let ack_token = ack_token
        .map(|token| serde_json::to_string(&token))
        .transpose()
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize org_inbox_pop ack_token for call {call_id}: {err}"
            ))
        })?
        .unwrap_or_default();

    let content = serde_json::to_string(&OrgInboxPopResult {
        org_id,
        thread_id: session.conversation_id.to_string(),
        role: principal.role.as_str().to_string(),
        messages: entries.into_iter().map(OrgInboxMessage::from).collect(),
        ack_token,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_inbox_pop result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
