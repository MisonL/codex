#![cfg(not(target_os = "windows"))]

mod assertions;
mod fixture;
mod outputs;
mod script;

use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complex_program_toolchain_uses_discovery_edit_execution_and_inspection_tools()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = fixture::test_builder();
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder.build(&server).await?;
    let fixture = fixture::ToolchainFixture::write(cwd.path())?;
    let request_log = mount_sse_sequence(&server, script::responses(&fixture, cwd.path())?).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: script::PROMPT.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: session_configured.model.clone(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            }),
            personality: None,
        })
        .await?;

    assertions::assert_events(&codex, &fixture, &request_log).await?;
    let requests = request_log.requests();
    outputs::assert_advertised_tools(&requests[0])?;
    let final_request = requests
        .last()
        .ok_or_else(|| anyhow::anyhow!("final request recorded"))?;
    outputs::assert_tool_outputs(final_request)?;
    fixture.assert_files()?;

    Ok(())
}
