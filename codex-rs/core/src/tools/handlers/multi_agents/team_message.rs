use super::*;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct TeamMessageArgs {
    team_id: String,
    member_name: String,
    message: Option<String>,
    items: Option<Vec<UserInput>>,
    #[serde(default)]
    interrupt: bool,
}

#[derive(Debug, Serialize)]
struct TeamMessageResult {
    team_id: String,
    member_name: String,
    agent_id: String,
    submission_id: String,
    delivered: bool,
    inbox_entry_id: String,
    error: Option<String>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamMessageArgs = parse_arguments(&arguments)?;
    let team_id = normalized_team_id(&args.team_id)?;
    let input_items = parse_collab_input(args.message, args.items)?;
    let prompt = input_preview(&input_items);
    let (receiver_thread_id, sender_name) = if turn.config.features.enabled(Feature::AgentOrg) {
        let config = read_persisted_team_config(turn.config.codex_home.as_path(), &team_id).await?;
        let sender_role =
            assert_persisted_team_participant(&team_id, &config, session.conversation_id)?;
        let member = config
            .members
            .iter()
            .find(|member| member.name == args.member_name.trim())
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "member `{}` not found in team `{team_id}`",
                    args.member_name.trim()
                ))
            })?;
        (agent_id(&member.agent_id)?, Some(sender_role.as_str()))
    } else {
        let team = get_team_record(session.conversation_id, &team_id)?;
        let member = find_team_member(&team, &team_id, &args.member_name)?;
        (member.agent_id, Some("lead"))
    };
    let inbox_entry_id = inbox::append_inbox_entry(
        turn.config.codex_home.as_path(),
        &team_id,
        receiver_thread_id,
        session.conversation_id,
        sender_name,
        &input_items,
        &prompt,
    )
    .await?;

    let delivery = send_input_to_member(
        &session,
        &turn,
        call_id,
        receiver_thread_id,
        input_items,
        prompt,
        args.interrupt,
    )
    .await;

    let (delivered, submission_id, error) = match delivery {
        Ok(submission_id) => (true, submission_id, None),
        Err(err) => (false, String::new(), Some(err.to_string())),
    };

    let content = serde_json::to_string(&TeamMessageResult {
        team_id,
        member_name: args.member_name.trim().to_string(),
        agent_id: receiver_thread_id.to_string(),
        submission_id,
        delivered,
        inbox_entry_id,
        error,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize team_message result: {err}"))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
