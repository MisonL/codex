use super::*;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct OrgPrincipalMessageArgs {
    org_id: String,
    to_thread_id: String,
    message: Option<String>,
    items: Option<Vec<UserInput>>,
}

#[derive(Debug, Serialize)]
struct OrgPrincipalMessageResult {
    org_id: String,
    from_thread_id: String,
    from_role: String,
    from_team_id: Option<String>,
    to_thread_id: String,
    to_role: String,
    to_team_id: Option<String>,
    delivered_live: bool,
    suppressed_reason: Option<String>,
    submission_id: String,
    inbox_entry_id: String,
    error: Option<String>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_principal_message requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: OrgPrincipalMessageArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;
    let receiver_thread_id = agent_id(&args.to_thread_id)?;
    if receiver_thread_id == session.conversation_id {
        return Err(FunctionCallError::RespondToModel(
            "org_principal_message cannot target the caller thread".to_string(),
        ));
    }

    let input_items = parse_collab_input(args.message, args.items)?;
    let prompt = input_preview(&input_items);

    let codex_home = turn.config.codex_home.as_path();
    let org_config = read_persisted_org_config(codex_home, &org_id).await?;
    let caller_principal =
        assert_persisted_org_principal(codex_home, &org_id, &org_config, session.conversation_id)
            .await?;
    let receiver_principal =
        persisted_org_principal(codex_home, &org_id, &org_config, receiver_thread_id)
            .await?
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "thread `{receiver_thread_id}` is not a principal of org `{org_id}`"
                ))
            })?;

    if let (Some(from_team_id), Some(to_team_id)) = (
        caller_principal.team_id.as_deref(),
        receiver_principal.team_id.as_deref(),
    ) {
        if from_team_id == to_team_id {
            return Err(FunctionCallError::RespondToModel(format!(
                "org_principal_message requires principals in different teams (both in team `{from_team_id}`)"
            )));
        }
    }

    let inbox_entry_id = org_inbox::append_inbox_entry(
        codex_home,
        &org_id,
        receiver_thread_id,
        receiver_principal.team_id.as_deref(),
        session.conversation_id,
        caller_principal.team_id.as_deref(),
        caller_principal.role.as_str(),
        &input_items,
        &prompt,
    )
    .await?;

    let created_at = now_unix_seconds();
    let events_path = org_events_path(codex_home, &org_id);
    let _events_lock = lock_org_events(codex_home, &org_id).await?;
    let sequence = next_jsonl_sequence(&events_path)
        .await
        .map_err(|err| org_persistence_error("compute org event sequence", &org_id, err))?;
    let event = json!({
        "id": format!("ev-{}", ThreadId::new()),
        "createdAt": created_at,
        "orgId": org_id.clone(),
        "sequence": sequence,
        "kind": "org.principal.message.appended",
        "actorThreadId": session.conversation_id.to_string(),
        "causalParent": null,
        "payload": {
            "fromThreadId": session.conversation_id.to_string(),
            "fromTeamId": caller_principal.team_id.clone(),
            "fromRole": caller_principal.role.as_str(),
            "toThreadId": receiver_thread_id.to_string(),
            "toTeamId": receiver_principal.team_id.clone(),
            "toRole": receiver_principal.role.as_str(),
            "inboxEntryId": inbox_entry_id
        }
    });
    append_jsonl_entry(&events_path, &event)
        .await
        .map_err(|err| org_persistence_error("append org event", &org_id, err))?;

    let delivered_live = false;
    let suppressed_reason = Some("durable_only".to_string());
    let submission_id = String::new();

    let content = serde_json::to_string(&OrgPrincipalMessageResult {
        org_id,
        from_thread_id: session.conversation_id.to_string(),
        from_role: caller_principal.role.as_str().to_string(),
        from_team_id: caller_principal.team_id,
        to_thread_id: receiver_thread_id.to_string(),
        to_role: receiver_principal.role.as_str().to_string(),
        to_team_id: receiver_principal.team_id,
        delivered_live,
        suppressed_reason,
        submission_id,
        inbox_entry_id,
        error: None,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_principal_message result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
