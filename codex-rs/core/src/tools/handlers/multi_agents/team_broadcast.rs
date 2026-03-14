use super::*;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct TeamBroadcastArgs {
    team_id: String,
    message: Option<String>,
    items: Option<Vec<UserInput>>,
    #[serde(default)]
    interrupt: bool,
}

#[derive(Debug, Serialize)]
struct TeamBroadcastSent {
    member_name: String,
    agent_id: String,
    submission_id: String,
    inbox_entry_id: String,
}

#[derive(Debug, Serialize)]
struct TeamBroadcastFailed {
    member_name: String,
    agent_id: String,
    inbox_entry_id: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct TeamBroadcastResult {
    team_id: String,
    sent: Vec<TeamBroadcastSent>,
    failed: Vec<TeamBroadcastFailed>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamBroadcastArgs = parse_arguments(&arguments)?;
    let team_id = normalized_team_id(&args.team_id)?;
    let input_items = parse_collab_input(args.message, args.items)?;
    let prompt = input_preview(&input_items);
    let mut sent = Vec::new();
    let mut failed = Vec::new();
    let sender_thread_id = session.conversation_id;
    let (targets, sender_name) = if turn.config.features.enabled(Feature::AgentOrg) {
        let config = read_persisted_team_config(turn.config.codex_home.as_path(), &team_id).await?;
        let sender_role =
            assert_persisted_team_participant(&team_id, &config, session.conversation_id)?;
        if !config.broadcast_policy.allows(sender_role) {
            return Err(FunctionCallError::RespondToModel(format!(
                "broadcast policy `{}` does not allow role `{}` in team `{team_id}`",
                config.broadcast_policy.as_str(),
                sender_role.as_str()
            )));
        }
        (
            config
                .members
                .into_iter()
                .filter_map(|member| {
                    let receiver_thread_id = agent_id(&member.agent_id).ok()?;
                    (receiver_thread_id != sender_thread_id).then_some(TeamMember {
                        name: member.name,
                        agent_id: receiver_thread_id,
                        agent_type: member.agent_type,
                    })
                })
                .collect::<Vec<_>>(),
            Some(sender_role.as_str()),
        )
    } else {
        (
            get_team_record(session.conversation_id, &team_id)?.members,
            Some("lead"),
        )
    };

    for member in &targets {
        let member_call_id = format!("{call_id}:{}", member.name);
        let inbox_entry_id = match inbox::append_inbox_entry(
            turn.config.codex_home.as_path(),
            &team_id,
            member.agent_id,
            session.conversation_id,
            sender_name,
            &input_items,
            &prompt,
        )
        .await
        {
            Ok(entry_id) => entry_id,
            Err(err) => {
                failed.push(TeamBroadcastFailed {
                    member_name: member.name.clone(),
                    agent_id: member.agent_id.to_string(),
                    inbox_entry_id: String::new(),
                    error: err.to_string(),
                });
                continue;
            }
        };

        match send_input_to_member(
            &session,
            &turn,
            member_call_id,
            member.agent_id,
            input_items.clone(),
            prompt.clone(),
            args.interrupt,
        )
        .await
        {
            Ok(submission_id) => sent.push(TeamBroadcastSent {
                member_name: member.name.clone(),
                agent_id: member.agent_id.to_string(),
                submission_id,
                inbox_entry_id,
            }),
            Err(err) => failed.push(TeamBroadcastFailed {
                member_name: member.name.clone(),
                agent_id: member.agent_id.to_string(),
                inbox_entry_id,
                error: err.to_string(),
            }),
        }
    }

    let content = serde_json::to_string(&TeamBroadcastResult {
        team_id,
        sent,
        failed,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize team_broadcast result: {err}"))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
