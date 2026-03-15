use super::*;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct OrgCreateArgs {
    org_id: String,
    org_name: Option<String>,
    experience_profile: Option<PersistedExperienceProfile>,
}

#[derive(Debug, Serialize)]
struct OrgCreateResult {
    org_id: String,
    created: bool,
    org_name: Option<String>,
    president_thread_id: String,
    created_at: i64,
    experience_profile: String,
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

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_create requires the agent_org experimental feature".to_string(),
        ));
    }

    if matches!(turn.session_source, SessionSource::SubAgent(_)) {
        return Err(FunctionCallError::RespondToModel(
            "org_create is president-only and must be called from the main session".to_string(),
        ));
    }

    let args: OrgCreateArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;
    let org_name = optional_non_empty(&args.org_name, "org_name")?;
    let experience_profile = args.experience_profile.unwrap_or_default();
    let codex_home = turn.config.codex_home.as_path();
    let _lock = lock_org_config(codex_home, &org_id).await?;
    let config_path = org_config_path(codex_home, &org_id);

    let caller_thread_id = session.conversation_id.to_string();
    let mut created = false;

    let config = match tokio::fs::read_to_string(&config_path).await {
        Ok(raw) => {
            let config: PersistedOrgConfig = serde_json::from_str(&raw)
                .map_err(|err| org_persistence_error("parse org config", &org_id, err))?;
            if config.president_thread_id != caller_thread_id {
                return Err(FunctionCallError::RespondToModel(format!(
                    "org `{org_id}` is owned by a different president thread"
                )));
            }
            if let Some(expected) = org_name {
                if config.org_name.as_deref() != Some(expected) {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "org `{org_id}` already exists with a different org_name"
                    )));
                }
            }
            if args.experience_profile.is_some() && config.experience_profile != experience_profile
            {
                return Err(FunctionCallError::RespondToModel(format!(
                    "org `{org_id}` already exists with a different experience_profile"
                )));
            }
            config
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            created = true;
            let created_at = now_unix_seconds();
            let config = PersistedOrgConfig {
                schema_version: default_team_schema_version(),
                org_id: org_id.clone(),
                org_name: org_name.map(std::string::ToString::to_string),
                president_thread_id: caller_thread_id.clone(),
                created_at,
                experience_profile,
                teams: Vec::new(),
            };

            write_json_atomic(&config_path, &config)
                .await
                .map_err(|err| org_persistence_error("write org config", &org_id, err))?;

            let inbox_dir = org_dir(codex_home, &org_id).join("inbox");
            tokio::fs::create_dir_all(&inbox_dir)
                .await
                .map_err(|err| org_persistence_error("create org inbox directory", &org_id, err))?;

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
                kind: "org.created".to_string(),
                actor_thread_id: caller_thread_id.clone(),
                causal_parent: None,
                payload: json!({}),
            };
            append_jsonl_entry(&events_path, &event)
                .await
                .map_err(|err| org_persistence_error("append org event", &org_id, err))?;

            config
        }
        Err(err) => return Err(org_persistence_error("read org config", &org_id, err)),
    };

    let content = serde_json::to_string(&OrgCreateResult {
        org_id,
        created,
        org_name: config.org_name,
        president_thread_id: config.president_thread_id,
        created_at: config.created_at,
        experience_profile: config.experience_profile.as_str().to_string(),
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_create result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
