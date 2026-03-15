use super::locks::lock_file_exclusive;
use super::now_unix_seconds;
use super::org_dir;
use crate::function_tool::FunctionCallError;
use codex_protocol::ThreadId;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

const ORG_INBOX_DIR: &str = "inbox";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OrgInboxEntry {
    pub(super) id: String,
    pub(super) created_at: i64,
    pub(super) org_id: String,
    pub(super) from_thread_id: String,
    pub(super) from_team_id: Option<String>,
    pub(super) from_role: String,
    pub(super) to_thread_id: String,
    pub(super) to_team_id: Option<String>,
    pub(super) input_items: Vec<UserInput>,
    pub(super) prompt: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgInboxCursor {
    acked_lines: usize,
    last_entry_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OrgInboxAckToken {
    pub(super) org_id: String,
    pub(super) thread_id: String,
    pub(super) acked_lines: usize,
    pub(super) last_entry_id: Option<String>,
}

fn inbox_dir(codex_home: &Path, org_id: &str) -> PathBuf {
    org_dir(codex_home, org_id).join(ORG_INBOX_DIR)
}

fn inbox_path(codex_home: &Path, org_id: &str, thread_id: ThreadId) -> PathBuf {
    inbox_dir(codex_home, org_id).join(format!("{thread_id}.jsonl"))
}

fn inbox_lock_path(codex_home: &Path, org_id: &str, thread_id: ThreadId) -> PathBuf {
    inbox_dir(codex_home, org_id).join(format!("{thread_id}.lock"))
}

fn inbox_cursor_path(codex_home: &Path, org_id: &str, thread_id: ThreadId) -> PathBuf {
    inbox_dir(codex_home, org_id).join(format!("{thread_id}.cursor.json"))
}

fn inbox_error(
    action: &str,
    org_id: &str,
    thread_id: ThreadId,
    err: impl std::fmt::Display,
) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!(
        "failed to {action} inbox for org `{org_id}` thread `{thread_id}`: {err}"
    ))
}

async fn read_cursor(
    codex_home: &Path,
    org_id: &str,
    thread_id: ThreadId,
) -> Result<OrgInboxCursor, FunctionCallError> {
    let cursor_path = inbox_cursor_path(codex_home, org_id, thread_id);
    let raw = match tokio::fs::read_to_string(&cursor_path).await {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(OrgInboxCursor::default()),
        Err(err) => return Err(inbox_error("read", org_id, thread_id, err)),
    };

    serde_json::from_str(&raw).map_err(|err| inbox_error("parse", org_id, thread_id, err))
}

async fn write_cursor(
    codex_home: &Path,
    org_id: &str,
    thread_id: ThreadId,
    cursor: &OrgInboxCursor,
) -> Result<(), FunctionCallError> {
    let cursor_path = inbox_cursor_path(codex_home, org_id, thread_id);
    super::write_json_atomic(&cursor_path, cursor)
        .await
        .map_err(|err| inbox_error("write", org_id, thread_id, err))
}

pub(super) async fn append_inbox_entry(
    codex_home: &Path,
    org_id: &str,
    receiver_thread_id: ThreadId,
    receiver_team_id: Option<&str>,
    sender_thread_id: ThreadId,
    sender_team_id: Option<&str>,
    sender_role: &str,
    input_items: &[UserInput],
    prompt: &str,
) -> Result<String, FunctionCallError> {
    let inbox_dir = inbox_dir(codex_home, org_id);
    tokio::fs::create_dir_all(&inbox_dir)
        .await
        .map_err(|err| inbox_error("create", org_id, receiver_thread_id, err))?;

    let lock_path = inbox_lock_path(codex_home, org_id, receiver_thread_id);
    let _lock = lock_file_exclusive(&lock_path)
        .await
        .map_err(|err| inbox_error("lock", org_id, receiver_thread_id, err))?;

    let entry = OrgInboxEntry {
        id: ThreadId::new().to_string(),
        created_at: now_unix_seconds(),
        org_id: org_id.to_string(),
        from_thread_id: sender_thread_id.to_string(),
        from_team_id: sender_team_id.map(std::string::ToString::to_string),
        from_role: sender_role.to_string(),
        to_thread_id: receiver_thread_id.to_string(),
        to_team_id: receiver_team_id.map(std::string::ToString::to_string),
        input_items: input_items.to_vec(),
        prompt: prompt.to_string(),
    };

    let mut serialized = serde_json::to_string(&entry)
        .map_err(|err| inbox_error("serialize", org_id, receiver_thread_id, err))?;
    serialized.push('\n');
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(inbox_path(codex_home, org_id, receiver_thread_id))
        .await
        .map_err(|err| inbox_error("open", org_id, receiver_thread_id, err))?;
    file.write_all(serialized.as_bytes())
        .await
        .map_err(|err| inbox_error("append", org_id, receiver_thread_id, err))?;

    Ok(entry.id)
}

pub(super) async fn pop_inbox_entries(
    codex_home: &Path,
    org_id: &str,
    receiver_thread_id: ThreadId,
    limit: usize,
) -> Result<(Vec<OrgInboxEntry>, Option<OrgInboxAckToken>), FunctionCallError> {
    let inbox_dir = inbox_dir(codex_home, org_id);
    tokio::fs::create_dir_all(&inbox_dir)
        .await
        .map_err(|err| inbox_error("create", org_id, receiver_thread_id, err))?;

    let lock_path = inbox_lock_path(codex_home, org_id, receiver_thread_id);
    let _lock = lock_file_exclusive(&lock_path)
        .await
        .map_err(|err| inbox_error("lock", org_id, receiver_thread_id, err))?;

    let cursor = read_cursor(codex_home, org_id, receiver_thread_id).await?;

    let inbox_file =
        match tokio::fs::File::open(inbox_path(codex_home, org_id, receiver_thread_id)).await {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok((Vec::new(), None)),
            Err(err) => return Err(inbox_error("open", org_id, receiver_thread_id, err)),
        };

    let mut reader = BufReader::new(inbox_file).lines();
    let mut index = 0usize;
    let mut entries = Vec::new();
    let mut last_entry_id = None;

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|err| inbox_error("read", org_id, receiver_thread_id, err))?
    {
        if index < cursor.acked_lines {
            index += 1;
            continue;
        }

        let entry: OrgInboxEntry = serde_json::from_str(&line)
            .map_err(|err| inbox_error("parse", org_id, receiver_thread_id, err))?;
        last_entry_id = Some(entry.id.clone());
        entries.push(entry);
        index += 1;

        if entries.len() >= limit {
            break;
        }
    }

    if entries.is_empty() {
        return Ok((entries, None));
    }

    let ack_token = OrgInboxAckToken {
        org_id: org_id.to_string(),
        thread_id: receiver_thread_id.to_string(),
        acked_lines: cursor.acked_lines + entries.len(),
        last_entry_id,
    };

    Ok((entries, Some(ack_token)))
}

pub(super) async fn ack_inbox(
    codex_home: &Path,
    token: &OrgInboxAckToken,
) -> Result<(), FunctionCallError> {
    let receiver_thread_id = super::agent_id(&token.thread_id)?;
    let org_id = token.org_id.as_str();

    let inbox_dir = inbox_dir(codex_home, org_id);
    tokio::fs::create_dir_all(&inbox_dir)
        .await
        .map_err(|err| inbox_error("create", org_id, receiver_thread_id, err))?;

    let lock_path = inbox_lock_path(codex_home, org_id, receiver_thread_id);
    let _lock = lock_file_exclusive(&lock_path)
        .await
        .map_err(|err| inbox_error("lock", org_id, receiver_thread_id, err))?;

    let cursor = OrgInboxCursor {
        acked_lines: token.acked_lines,
        last_entry_id: token.last_entry_id.clone(),
    };
    write_cursor(codex_home, org_id, receiver_thread_id, &cursor).await?;

    Ok(())
}
