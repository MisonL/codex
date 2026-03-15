use super::*;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct OrgInboxAckArgs {
    org_id: String,
    ack_token: String,
}

#[derive(Debug, Serialize)]
struct OrgInboxAckResult {
    org_id: String,
    thread_id: String,
    acked: bool,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_inbox_ack requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: OrgInboxAckArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;

    if args.ack_token.trim().is_empty() {
        let content = serde_json::to_string(&OrgInboxAckResult {
            org_id,
            thread_id: session.conversation_id.to_string(),
            acked: false,
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize org_inbox_ack result for call {call_id}: {err}"
            ))
        })?;
        return Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        });
    }

    let codex_home = turn.config.codex_home.as_path();
    let config = read_persisted_org_config(codex_home, &org_id).await?;
    assert_persisted_org_principal(codex_home, &org_id, &config, session.conversation_id).await?;

    let token: org_inbox::OrgInboxAckToken = serde_json::from_str(&args.ack_token)
        .map_err(|err| FunctionCallError::RespondToModel(format!("ack_token is invalid: {err}")))?;
    if token.org_id != org_id {
        return Err(FunctionCallError::RespondToModel(
            "ack_token org_id mismatch".to_string(),
        ));
    }
    if token.thread_id != session.conversation_id.to_string() {
        return Err(FunctionCallError::RespondToModel(
            "ack_token thread_id mismatch".to_string(),
        ));
    }

    org_inbox::ack_inbox(codex_home, &token).await?;

    let content = serde_json::to_string(&OrgInboxAckResult {
        org_id,
        thread_id: session.conversation_id.to_string(),
        acked: true,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_inbox_ack result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
