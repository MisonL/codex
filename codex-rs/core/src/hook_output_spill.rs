use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use tokio::fs;
use tracing::warn;
use uuid::Uuid;

use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::formatted_truncate_text;

const HOOK_OUTPUTS_DIR: &str = "hook_outputs";
const HOOK_OUTPUT_TOKEN_LIMIT: usize = 2_500;

pub(crate) async fn spill_hook_contexts(
    codex_home: &Path,
    thread_id: ThreadId,
    contexts: Vec<String>,
) -> Vec<String> {
    let mut spilled = Vec::with_capacity(contexts.len());
    for context in contexts {
        spilled.push(spill_hook_context(codex_home, thread_id, context).await);
    }
    spilled
}

async fn spill_hook_context(codex_home: &Path, thread_id: ThreadId, context: String) -> String {
    if approx_token_count(&context) <= HOOK_OUTPUT_TOKEN_LIMIT {
        return context;
    }

    let path = hook_output_path(codex_home, thread_id);
    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent).await
    {
        warn!(
            path = %parent.display(),
            error = %err,
            "failed to create hook output directory"
        );
        return truncated_hook_context(&context);
    }

    if let Err(err) = fs::write(&path, &context).await {
        warn!(
            path = %path.display(),
            error = %err,
            "failed to spill hook output"
        );
        return truncated_hook_context(&context);
    }

    spilled_hook_context_preview(&context, &path)
}

fn hook_output_path(codex_home: &Path, thread_id: ThreadId) -> PathBuf {
    codex_home
        .join(HOOK_OUTPUTS_DIR)
        .join(thread_id.to_string())
        .join(format!("{}.txt", Uuid::new_v4()))
}

fn spilled_hook_context_preview(context: &str, path: &Path) -> String {
    let footer = format!("\n\nFull hook output saved to: {}", path.display());
    let footer_tokens = approx_token_count(&footer);
    let preview_limit = HOOK_OUTPUT_TOKEN_LIMIT.saturating_sub(footer_tokens);
    format!(
        "{}{footer}",
        formatted_truncate_text(context, TruncationPolicy::Tokens(preview_limit))
    )
}

fn truncated_hook_context(context: &str) -> String {
    formatted_truncate_text(context, TruncationPolicy::Tokens(HOOK_OUTPUT_TOKEN_LIMIT))
}
