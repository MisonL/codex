use std::sync::Arc;

use anyhow::Context;
use anyhow::bail;
use assert_matches::assert_matches;
use codex_core::CodexThread;
use codex_protocol::models::PermissionProfile;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use core_test_support::responses::ResponseMock;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use tokio::time::timeout;

use super::fixture::ToolchainFixture;
use super::script;

#[derive(Debug, Default)]
struct ObservedEvents {
    plan_update: bool,
    exec_begin: bool,
    terminal_interaction: bool,
    view_image: bool,
    patch_success: bool,
    request_permissions: bool,
    request_user_input: bool,
}

impl ObservedEvents {
    fn assert_complete(&self, events: &[&'static str]) {
        assert!(
            self.plan_update,
            "expected update_plan to emit PlanUpdate; observed={self:?}; events={events:?}"
        );
        assert!(
            self.exec_begin,
            "expected exec_command begin event; observed={self:?}; events={events:?}"
        );
        assert!(
            self.terminal_interaction,
            "expected write_stdin to emit terminal interaction; observed={self:?}; events={events:?}"
        );
        assert!(
            self.view_image,
            "expected view_image event; observed={self:?}; events={events:?}"
        );
        assert!(
            self.patch_success,
            "expected apply_patch success event; observed={self:?}; events={events:?}"
        );
        assert!(
            self.request_permissions,
            "expected request_permissions event; observed={self:?}; events={events:?}"
        );
        assert!(
            self.request_user_input,
            "expected request_user_input event; observed={self:?}; events={events:?}"
        );
    }
}

pub(crate) async fn assert_events(
    codex: &Arc<CodexThread>,
    fixture: &ToolchainFixture,
    request_log: &ResponseMock,
) -> anyhow::Result<()> {
    let mut observed = ObservedEvents::default();
    let mut events = Vec::new();
    loop {
        let event = match timeout(Duration::from_secs(10), codex.next_event()).await {
            Ok(Ok(event)) => event.msg,
            Ok(Err(err)) => bail!("stream errored after events: {events:?}; err: {err:?}"),
            Err(_) => {
                bail!(
                    "timeout waiting for event after events: {events:?}; requests: {}",
                    request_log.requests().len()
                );
            }
        };
        events.push(event_name(&event));
        if handle_event(codex, fixture, &mut observed, event).await? {
            break;
        }
    }
    observed.assert_complete(&events);
    Ok(())
}

fn event_name(event: &EventMsg) -> &'static str {
    match event {
        EventMsg::PlanUpdate(_) => "PlanUpdate",
        EventMsg::ExecCommandBegin(_) => "ExecCommandBegin",
        EventMsg::ExecCommandEnd(_) => "ExecCommandEnd",
        EventMsg::ExecCommandOutputDelta(_) => "ExecCommandOutputDelta",
        EventMsg::RequestPermissions(_) => "RequestPermissions",
        EventMsg::RequestUserInput(_) => "RequestUserInput",
        EventMsg::TerminalInteraction(_) => "TerminalInteraction",
        EventMsg::ViewImageToolCall(_) => "ViewImageToolCall",
        EventMsg::PatchApplyEnd(_) => "PatchApplyEnd",
        EventMsg::TurnComplete(_) => "TurnComplete",
        _ => "Other",
    }
}

async fn handle_event(
    codex: &Arc<CodexThread>,
    fixture: &ToolchainFixture,
    observed: &mut ObservedEvents,
    event: EventMsg,
) -> anyhow::Result<bool> {
    match event {
        EventMsg::PlanUpdate(update) => {
            observed.plan_update = true;
            assert_eq!(
                update.explanation.as_deref(),
                Some("Complex toolchain fixture")
            );
            assert_matches!(update.plan[0].status, StepStatus::InProgress);
            Ok(false)
        }
        EventMsg::ExecCommandBegin(begin) if begin.call_id == "exec-call" => {
            observed.exec_begin = true;
            Ok(false)
        }
        EventMsg::RequestPermissions(request) if request.call_id == "permissions-call" => {
            observed.request_permissions = true;
            assert_eq!(request.reason.as_deref(), Some(script::PERMISSION_REASON));
            respond_to_permissions(codex, request.call_id, request.permissions).await?;
            Ok(false)
        }
        EventMsg::RequestUserInput(request) if request.call_id == "user-input-call" => {
            observed.request_user_input = true;
            assert_eq!(request.questions.len(), 1);
            assert_eq!(request.questions[0].id, script::USER_INPUT_QUESTION_ID);
            assert!(request.questions[0].is_other);
            respond_to_user_input(codex, request.turn_id).await?;
            Ok(false)
        }
        EventMsg::TerminalInteraction(interaction) if interaction.call_id == "exec-call" => {
            observed.terminal_interaction = true;
            assert_eq!(interaction.stdin, script::STDIN_CHARS);
            Ok(false)
        }
        EventMsg::ViewImageToolCall(view) if view.call_id == "view-call" => {
            observed.view_image = true;
            assert_eq!(view.path, fixture.asset_path);
            Ok(false)
        }
        EventMsg::PatchApplyEnd(end) if end.call_id == "patch-call" => {
            observed.patch_success = end.success;
            Ok(false)
        }
        EventMsg::TurnComplete(_) => Ok(true),
        _ => Ok(false),
    }
}

async fn respond_to_permissions(
    codex: &Arc<CodexThread>,
    call_id: String,
    permissions: PermissionProfile,
) -> anyhow::Result<()> {
    codex
        .submit(Op::RequestPermissionsResponse {
            id: call_id,
            response: RequestPermissionsResponse {
                permissions,
                scope: PermissionGrantScope::Turn,
            },
        })
        .await
        .context("submit request_permissions response")
        .map(|_| ())
}

async fn respond_to_user_input(codex: &Arc<CodexThread>, turn_id: String) -> anyhow::Result<()> {
    let mut answers = std::collections::HashMap::new();
    answers.insert(
        script::USER_INPUT_QUESTION_ID.to_string(),
        RequestUserInputAnswer {
            answers: vec!["proceed".to_string()],
        },
    );
    codex
        .submit(Op::UserInputAnswer {
            id: turn_id,
            response: RequestUserInputResponse { answers },
        })
        .await
        .context("submit request_user_input response")
        .map(|_| ())
}
