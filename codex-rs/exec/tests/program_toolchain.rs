#![cfg(not(target_os = "windows"))]

use anyhow::Context;
use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;

const PROMPT: &str = "finish the binary toolchain fixture";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compiled_codex_exec_runs_program_toolchain_against_mock_responses() -> anyhow::Result<()> {
    use core_test_support::skip_if_no_network;

    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    write_fixture(test.cwd_path())?;

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            function_sse("resp-1", "inspect-call", "exec_command", inspect_args())?,
            function_sse("resp-2", "write-call", "exec_command", write_args())?,
            sse_apply_patch("resp-3", "patch-call"),
            function_sse("resp-4", "verify-call", "exec_command", verify_args())?,
            responses::sse(vec![
                responses::ev_response_created("resp-5"),
                responses::ev_assistant_message("msg-1", "binary toolchain complete"),
                responses::ev_completed("resp-5"),
            ]),
        ],
    )
    .await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("-s")
        .arg("danger-full-access")
        .arg("-m")
        .arg("gpt-5.1")
        .arg(PROMPT)
        .assert()
        .success()
        .stdout("binary toolchain complete\n");

    assert_eq!(
        fs::read_to_string(test.cwd_path().join("src/generated.txt"))?,
        "sum=7\nsource=compiled-codex-exec\n",
    );
    assert_eq!(
        fs::read_to_string(test.cwd_path().join("src/result.txt"))?,
        "status=done\nsum=7\nverified=binary\n",
    );

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 5);
    assert_initial_prompt(&requests[0]);
    assert_advertised_tools(&requests[0])?;
    assert_function_output(&requests[1].body_json(), "inspect-call", "status=pending")?;
    assert_function_output(
        &requests[2].body_json(),
        "write-call",
        "compiled-codex-exec",
    )?;
    assert_function_output(&requests[3].body_json(), "patch-call", "A src/result.txt")?;
    assert_function_output(&requests[4].body_json(), "verify-call", "verified=binary")?;

    Ok(())
}

fn write_fixture(cwd: &std::path::Path) -> anyhow::Result<()> {
    let src_dir = cwd.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(
        src_dir.join("ledger.txt"),
        "total=3\nbonus=4\nstatus=pending\n",
    )?;
    Ok(())
}

fn function_sse(
    response_id: &str,
    call_id: &str,
    tool_name: &str,
    args: Value,
) -> anyhow::Result<String> {
    Ok(responses::sse(vec![
        responses::ev_response_created(response_id),
        responses::ev_function_call(call_id, tool_name, &serde_json::to_string(&args)?),
        responses::ev_completed(response_id),
    ]))
}

fn sse_apply_patch(response_id: &str, call_id: &str) -> String {
    responses::sse(vec![
        responses::ev_response_created(response_id),
        responses::ev_apply_patch_function_call(call_id, patch_text()),
        responses::ev_completed(response_id),
    ])
}

fn inspect_args() -> Value {
    json!({
        "cmd": "cat src/ledger.txt",
        "yield_time_ms": 1000,
    })
}

fn write_args() -> Value {
    json!({
        "cmd": "total=$(sed -n 's/^total=//p' src/ledger.txt); bonus=$(sed -n 's/^bonus=//p' src/ledger.txt); output=$(printf 'sum=%s\\nsource=compiled-codex-exec\\n' \"$((total + bonus))\"); printf '%s\\n' \"$output\" > src/generated.txt; printf '%s\\n' \"$output\"",
        "yield_time_ms": 1000,
    })
}

fn verify_args() -> Value {
    json!({
        "cmd": "cat src/generated.txt src/result.txt",
        "yield_time_ms": 1000,
    })
}

fn patch_text() -> &'static str {
    "*** Begin Patch\n*** Add File: src/result.txt\n+status=done\n+sum=7\n+verified=binary\n*** End Patch"
}

fn assert_initial_prompt(req: &responses::ResponsesRequest) {
    assert!(
        req.has_message_with_input_texts("user", |texts| texts == [PROMPT.to_string()]),
        "compiled codex-exec should send the user prompt to the mock Responses server",
    );
}

fn assert_advertised_tools(req: &responses::ResponsesRequest) -> anyhow::Result<()> {
    let mut tools = Vec::new();
    for tool in req
        .body_json()
        .get("tools")
        .and_then(Value::as_array)
        .context("request should include tools")?
    {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| tool.get("type").and_then(Value::as_str))
            .with_context(|| format!("tool should include name or type: {tool}"))?;
        tools.push(name.to_string());
    }
    assert!(tools.iter().any(|tool| tool == "exec_command"));
    assert!(tools.iter().any(|tool| tool == "apply_patch"));
    Ok(())
}

fn assert_function_output(body: &Value, call_id: &str, expected: &str) -> anyhow::Result<()> {
    let output = body
        .get("input")
        .and_then(Value::as_array)
        .context("request missing input array")?
        .iter()
        .find(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .and_then(|item| item.get("output"))
        .context("missing function_call_output")?;
    let text = output
        .as_str()
        .or_else(|| output.get("content").and_then(Value::as_str))
        .context("function_call_output did not contain text")?;
    assert!(
        text.contains(expected),
        "expected output for {call_id} to contain {expected:?}, got {text:?}",
    );
    Ok(())
}
