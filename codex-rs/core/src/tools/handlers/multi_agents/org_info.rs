use super::*;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct OrgInfoArgs {
    org_id: String,
}

#[derive(Debug, Serialize)]
struct OrgInfoTeam {
    team_id: String,
    owner_thread_id: String,
    leaders: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OrgPrincipalInfo {
    thread_id: String,
    role: String,
    team_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OrgInfoResult {
    org_id: String,
    org_name: Option<String>,
    president_thread_id: String,
    created_at: i64,
    experience_profile: String,
    teams: Vec<OrgInfoTeam>,
    principals: Vec<OrgPrincipalInfo>,
}

pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    if !turn.config.features.enabled(Feature::AgentOrg) {
        return Err(FunctionCallError::RespondToModel(
            "org_info requires the agent_org experimental feature".to_string(),
        ));
    }

    let args: OrgInfoArgs = parse_arguments(&arguments)?;
    let org_id = normalized_org_id(&args.org_id)?;
    let codex_home = turn.config.codex_home.as_path();
    let config = read_persisted_org_config(codex_home, &org_id).await?;
    assert_persisted_org_principal(codex_home, &org_id, &config, session.conversation_id).await?;

    let mut principals = Vec::new();
    let mut seen_principals = HashSet::new();

    principals.push(OrgPrincipalInfo {
        thread_id: config.president_thread_id.clone(),
        role: CallerOrgRole::President.as_str().to_string(),
        team_id: None,
    });
    seen_principals.insert(config.president_thread_id.clone());

    let mut teams = Vec::new();
    for team_ref in &config.teams {
        let team_config = read_persisted_team_config(codex_home, &team_ref.team_id).await?;
        if team_config.org_id.as_deref() != Some(org_id.as_str()) {
            return Err(FunctionCallError::RespondToModel(format!(
                "org `{org_id}` references team `{}` but team config does not match",
                team_ref.team_id
            )));
        }

        teams.push(OrgInfoTeam {
            team_id: team_ref.team_id.clone(),
            owner_thread_id: team_config.lead_thread_id.clone(),
            leaders: team_config.leaders.clone(),
        });

        if seen_principals.insert(team_config.lead_thread_id.clone()) {
            principals.push(OrgPrincipalInfo {
                thread_id: team_config.lead_thread_id.clone(),
                role: CallerOrgRole::Owner.as_str().to_string(),
                team_id: Some(team_ref.team_id.clone()),
            });
        }
        for leader_thread_id in &team_config.leaders {
            if seen_principals.insert(leader_thread_id.clone()) {
                principals.push(OrgPrincipalInfo {
                    thread_id: leader_thread_id.clone(),
                    role: CallerOrgRole::Leader.as_str().to_string(),
                    team_id: Some(team_ref.team_id.clone()),
                });
            }
        }
    }

    let content = serde_json::to_string(&OrgInfoResult {
        org_id,
        org_name: config.org_name,
        president_thread_id: config.president_thread_id,
        created_at: config.created_at,
        experience_profile: config.experience_profile.as_str().to_string(),
        teams,
        principals,
    })
    .map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize org_info result for call {call_id}: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}
