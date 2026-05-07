use std::path::Path;

use codex_hooks::CommandHookConfig;
use codex_hooks::CommandHooksConfig;
use codex_hooks::HookEvent;
use codex_hooks::HookMatcherConfig;
use codex_hooks::HookPayload;
use codex_hooks::HookResponse;
use codex_hooks::HookResult;
use codex_hooks::Hooks;
use codex_hooks::HooksConfig;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

#[cfg(windows)]
fn success_command() -> Vec<String> {
    vec!["cmd".to_string(), "/C".to_string(), "exit /B 0".to_string()]
}

#[cfg(not(windows))]
fn success_command() -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()]
}

fn payload(cwd: &Path, hook_event: HookEvent) -> HookPayload {
    HookPayload {
        session_id: ThreadId::new(),
        transcript_path: None,
        cwd: cwd.to_path_buf(),
        permission_mode: "never".to_string(),
        hook_event,
    }
}

#[tokio::test]
async fn post_compact_hooks_match_trigger_and_dispatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let hooks = Hooks::new(HooksConfig {
        command_hooks: CommandHooksConfig {
            post_compact: vec![
                CommandHookConfig {
                    name: Some("post-compact-manual".to_string()),
                    command: success_command(),
                    matcher: HookMatcherConfig {
                        matcher: Some("^manual$".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                CommandHookConfig {
                    name: Some("post-compact-auto".to_string()),
                    command: success_command(),
                    matcher: HookMatcherConfig {
                        matcher: Some("^auto$".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    });

    let outcomes = hooks
        .dispatch(payload(
            dir.path(),
            HookEvent::PostCompact {
                trigger: "manual".to_string(),
                custom_instructions: None,
            },
        ))
        .await;

    assert_eq!(
        outcomes,
        vec![HookResponse {
            hook_name: "post-compact-manual".to_string(),
            result: HookResult::success(),
        }]
    );
}
