use std::path::Path;

use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use serde_json::Value;
use serde_json::json;

use super::fixture::ToolchainFixture;

pub(crate) const PROMPT: &str = "finish the ledger task using the available local program tools";
pub(crate) const STDIN_CHARS: &str =
    "cat > src/generated.txt <<'EOF'\nsum=7\nsource=exec-write-stdin\nEOF\nexit\n";
pub(crate) const USER_INPUT_QUESTION_ID: &str = "confirm_toolchain";
pub(crate) const CRON_TASK_ID: &str = "00000001";
pub(crate) const PERMISSION_REASON: &str = "Confirm network permission request plumbing";
pub(crate) const LOCAL_PROGRAM_TOOLS: &[&str] = &[
    "exec_command",
    "write_stdin",
    "update_plan",
    "request_user_input",
    "CronCreate",
    "CronList",
    "CronDelete",
    "request_permissions",
    "grep_files",
    "read_file",
    "list_dir",
    "test_sync_tool",
    "view_image",
    "js_repl",
    "js_repl_reset",
    "apply_patch",
];
pub(crate) const REMOTE_BUILTIN_TOOLS: &[&str] = &["web_search", "image_generation"];

const JS_CODE: &str = "// codex-js-repl: timeout_ms=5000\nconst fs = await import(\"node:fs/promises\"); const text = await fs.readFile(\"src/generated.txt\", \"utf8\"); const sum = Number(text.match(/sum=(\\d+)/)[1]); console.log(JSON.stringify({sum, doubled: sum * 2}));";
const PATCH: &str = "*** Begin Patch\n*** Add File: src/result.txt\n+status=done\n+sum=7\n+doubled=14\n*** End Patch";

pub(crate) fn responses(fixture: &ToolchainFixture, cwd: &Path) -> anyhow::Result<Vec<String>> {
    Ok(vec![
        function_sse("resp-1", "plan-call", "update_plan", plan_args())?,
        function_sse("resp-2", "list-call", "list_dir", list_args(cwd))?,
        function_sse("resp-3", "grep-call", "grep_files", grep_args(cwd))?,
        function_sse("resp-4", "read-call", "read_file", read_args(fixture))?,
        function_sse(
            "resp-5",
            "permissions-call",
            "request_permissions",
            request_permissions_args(),
        )?,
        function_sse(
            "resp-6",
            "user-input-call",
            "request_user_input",
            request_user_input_args(),
        )?,
        function_sse(
            "resp-7",
            "cron-create-call",
            "CronCreate",
            cron_create_args(),
        )?,
        function_sse("resp-8", "cron-list-call", "CronList", json!({}))?,
        function_sse(
            "resp-9",
            "cron-delete-call",
            "CronDelete",
            cron_delete_args(),
        )?,
        function_sse("resp-10", "sync-call", "test_sync_tool", test_sync_args())?,
        function_sse("resp-11", "exec-call", "exec_command", exec_args())?,
        function_sse("resp-12", "stdin-call", "write_stdin", stdin_args())?,
        custom_sse("resp-13", "js-call", "js_repl", JS_CODE),
        function_sse("resp-14", "js-reset-call", "js_repl_reset", json!({}))?,
        function_sse("resp-15", "view-call", "view_image", view_args())?,
        sse(vec![
            ev_response_created("resp-16"),
            ev_apply_patch_function_call("patch-call", PATCH),
            ev_completed("resp-16"),
        ]),
        function_sse("resp-17", "verify-call", "exec_command", verify_args())?,
        sse(vec![
            ev_response_created("resp-18"),
            ev_assistant_message("msg-1", "toolchain complete"),
            ev_completed("resp-18"),
        ]),
    ])
}

fn function_sse(
    response_id: &str,
    call_id: &str,
    tool_name: &str,
    args: Value,
) -> anyhow::Result<String> {
    Ok(sse(vec![
        ev_response_created(response_id),
        ev_function_call(call_id, tool_name, &serde_json::to_string(&args)?),
        ev_completed(response_id),
    ]))
}

fn custom_sse(response_id: &str, call_id: &str, tool_name: &str, input: &str) -> String {
    sse(vec![
        ev_response_created(response_id),
        ev_custom_tool_call(call_id, tool_name, input),
        ev_completed(response_id),
    ])
}

fn plan_args() -> Value {
    json!({
        "explanation": "Complex toolchain fixture",
        "plan": [
            {"step": "Inspect task files", "status": "in_progress"},
            {"step": "Calculate and write answer", "status": "pending"},
            {"step": "Verify outputs", "status": "pending"}
        ],
    })
}

fn list_args(cwd: &Path) -> Value {
    json!({"dir_path": cwd, "limit": 20, "depth": 2})
}

fn grep_args(cwd: &Path) -> Value {
    json!({"pattern": "status=pending", "path": cwd, "include": "*.txt", "limit": 10})
}

fn read_args(fixture: &ToolchainFixture) -> Value {
    json!({"file_path": fixture.src_dir.join("ledger.txt"), "offset": 1, "limit": 5})
}

fn request_permissions_args() -> Value {
    json!({
        "reason": PERMISSION_REASON,
        "permissions": {
            "network": {"enabled": true}
        }
    })
}

fn request_user_input_args() -> Value {
    json!({
        "questions": [{
            "id": USER_INPUT_QUESTION_ID,
            "header": "Confirm",
            "question": "Continue the local toolchain fixture?",
            "options": [{
                "label": "Proceed (Recommended)",
                "description": "Continue the scripted validation."
            }, {
                "label": "Stop",
                "description": "Cancel this scripted validation."
            }]
        }]
    })
}

fn cron_create_args() -> Value {
    json!({"schedule": "*/10 * * * *", "prompt": "check ledger status"})
}

fn cron_delete_args() -> Value {
    json!({"id": CRON_TASK_ID})
}

fn test_sync_args() -> Value {
    json!({"sleep_before_ms": 1, "sleep_after_ms": 1})
}

fn exec_args() -> Value {
    json!({"cmd": "/bin/bash -i", "tty": true, "yield_time_ms": 50})
}

fn stdin_args() -> Value {
    json!({"session_id": 1000, "chars": STDIN_CHARS, "yield_time_ms": 1000})
}

fn view_args() -> Value {
    json!({"path": "assets/evidence.png"})
}

fn verify_args() -> Value {
    json!({
        "cmd": "printf 'generated:' && cat src/generated.txt && printf 'result:' && cat src/result.txt",
        "yield_time_ms": 250,
    })
}
