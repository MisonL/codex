use std::collections::BTreeSet;

use anyhow::Context;
use anyhow::bail;
use core_test_support::responses::ResponsesRequest;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::script;

pub(crate) fn assert_advertised_tools(req: &ResponsesRequest) -> anyhow::Result<()> {
    let advertised = tool_names(req)?;
    for expected in script::LOCAL_PROGRAM_TOOLS
        .iter()
        .chain(script::REMOTE_BUILTIN_TOOLS)
    {
        assert!(
            advertised.contains(*expected),
            "expected advertised tool {expected}; had {advertised:?}"
        );
    }
    Ok(())
}

pub(crate) fn assert_tool_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    assert_discovery_outputs(req)?;
    assert_interactive_outputs(req)?;
    assert_scheduling_outputs(req)?;
    assert_execution_outputs(req)?;
    assert_image_and_patch_outputs(req)?;
    Ok(())
}

fn assert_discovery_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    let (plan_out, plan_success) = function_output(req, "plan-call")?;
    assert_eq!(plan_out, "Plan updated");
    assert_ne!(plan_success, Some(false));

    let (list_out, list_success) = function_output(req, "list-call")?;
    assert_ne!(list_success, Some(false));
    assert!(list_out.contains("src/"));
    assert!(list_out.contains("ledger.txt"));

    let (grep_out, grep_success) = function_output(req, "grep-call")?;
    assert_ne!(grep_success, Some(false));
    assert!(grep_out.contains("ledger.txt"));

    let (read_out, read_success) = function_output(req, "read-call")?;
    assert_ne!(read_success, Some(false));
    assert!(read_out.contains("L1: total=3"));
    assert!(read_out.contains("L2: bonus=4"));
    Ok(())
}

fn assert_interactive_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    let (permissions_out, permissions_success) = function_output(req, "permissions-call")?;
    assert_ne!(
        permissions_success,
        Some(false),
        "request_permissions failed: {permissions_out}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&permissions_out).context("permissions json")?,
        json!({
            "permissions": {
                "file_system": null,
                "macos": null,
                "network": {"enabled": true}
            },
            "scope": "turn"
        })
    );

    let (input_out, input_success) = function_output(req, "user-input-call")?;
    assert_ne!(
        input_success,
        Some(false),
        "request_user_input failed: {input_out}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&input_out).context("user input json")?,
        json!({
            "answers": {
                script::USER_INPUT_QUESTION_ID: { "answers": ["proceed"] }
            }
        })
    );
    Ok(())
}

fn assert_scheduling_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    let (create_out, create_success) = function_output(req, "cron-create-call")?;
    assert_ne!(
        create_success,
        Some(false),
        "CronCreate failed: {create_out}"
    );
    let create_json = serde_json::from_str::<Value>(&create_out).context("create json")?;
    assert_eq!(create_json["task"]["id"], script::CRON_TASK_ID);
    assert_eq!(create_json["task"]["schedule"], "*/10 * * * *");
    assert_eq!(create_json["task"]["prompt"], "check ledger status");

    let (list_out, list_success) = function_output(req, "cron-list-call")?;
    assert_ne!(list_success, Some(false), "CronList failed: {list_out}");
    let list_json = serde_json::from_str::<Value>(&list_out).context("list json")?;
    assert_eq!(list_json["tasks"].as_array().map(Vec::len), Some(1));
    assert_eq!(list_json["tasks"][0]["id"], script::CRON_TASK_ID);

    let (delete_out, delete_success) = function_output(req, "cron-delete-call")?;
    assert_ne!(
        delete_success,
        Some(false),
        "CronDelete failed: {delete_out}"
    );
    let delete_json = serde_json::from_str::<Value>(&delete_out).context("delete json")?;
    assert_eq!(delete_json["result"]["deleted"].as_bool(), Some(true));
    assert_eq!(delete_json["result"]["task"]["id"], script::CRON_TASK_ID);

    let (sync_out, sync_success) = function_output(req, "sync-call")?;
    assert_ne!(
        sync_success,
        Some(false),
        "test_sync_tool failed: {sync_out}"
    );
    assert_eq!(sync_out, "ok");
    Ok(())
}

fn assert_execution_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    let (exec_out, exec_success) = function_output(req, "exec-call")?;
    assert_ne!(exec_success, Some(false));
    assert!(
        exec_out.contains("Process running with session ID 1000")
            || exec_out.contains("Process exited with code 0"),
        "unexpected exec output: {exec_out}"
    );

    let (stdin_out, stdin_success) = function_output(req, "stdin-call")?;
    assert_ne!(stdin_success, Some(false));
    assert!(stdin_out.contains("Process exited with code 0"));

    let (js_out, js_success) = custom_output(req, "js-call")?;
    assert_ne!(js_success, Some(false), "js_repl failed: {js_out}");
    assert!(js_out.contains(r#"{"sum":7,"doubled":14}"#));

    let (js_reset_out, js_reset_success) = function_output(req, "js-reset-call")?;
    assert_ne!(
        js_reset_success,
        Some(false),
        "js_repl_reset failed: {js_reset_out}"
    );
    assert_eq!(js_reset_out, "js_repl kernel reset");
    Ok(())
}

fn assert_image_and_patch_outputs(req: &ResponsesRequest) -> anyhow::Result<()> {
    let view_output = req.function_call_output("view-call");
    let view_items = view_output
        .get("output")
        .and_then(Value::as_array)
        .context("view_image should return content item array")?;
    assert_eq!(view_items.len(), 1);
    assert_eq!(
        view_items[0].get("type").and_then(Value::as_str),
        Some("input_image")
    );

    let (patch_out, patch_success) = function_output(req, "patch-call")?;
    assert_ne!(patch_success, Some(false));
    assert!(patch_out.contains("A src/result.txt"));

    let (verify_out, verify_success) = function_output(req, "verify-call")?;
    assert_ne!(verify_success, Some(false));
    assert!(verify_out.contains("generated:sum=7"));
    assert!(verify_out.contains("result:status=done"));
    Ok(())
}

fn function_output(
    req: &ResponsesRequest,
    call_id: &str,
) -> anyhow::Result<(String, Option<bool>)> {
    let (content, success) = req
        .function_call_output_content_and_success(call_id)
        .with_context(|| format!("function_call_output missing for {call_id}"))?;
    Ok((content.unwrap_or_default(), success))
}

fn custom_output(req: &ResponsesRequest, call_id: &str) -> anyhow::Result<(String, Option<bool>)> {
    let (content, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .with_context(|| format!("custom_tool_call_output missing for {call_id}"))?;
    Ok((content.unwrap_or_default(), success))
}

fn tool_names(req: &ResponsesRequest) -> anyhow::Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for tool in req.body_json()["tools"]
        .as_array()
        .context("tools array should be present")?
    {
        let Some(name) = tool
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| tool.get("type").and_then(Value::as_str))
        else {
            bail!("tool should have a function name or builtin type: {tool}");
        };
        names.insert(name.to_string());
    }
    Ok(names)
}
