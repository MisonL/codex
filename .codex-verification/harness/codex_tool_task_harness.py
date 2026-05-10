#!/usr/bin/env python3
"""Deterministic local Responses API harness for the compiled Codex binary."""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


PNG_1X1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR42mNk+M8AAwUBAcgnPEAAAAAASUVORK5CYII="
)


def sse(events: list[dict[str, Any]]) -> bytes:
    chunks: list[str] = []
    for event in events:
        kind = event["type"]
        chunks.append(f"event: {kind}\n")
        if len(event) > 1:
            chunks.append(f"data: {json.dumps(event, separators=(',', ':'))}\n\n")
        else:
            chunks.append("\n")
    return "".join(chunks).encode()


def ev_response_created(response_id: str) -> dict[str, Any]:
    return {"type": "response.created", "response": {"id": response_id}}


def ev_completed(response_id: str) -> dict[str, Any]:
    return {
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": 0,
                "input_tokens_details": None,
                "output_tokens": 0,
                "output_tokens_details": None,
                "total_tokens": 0,
            },
        },
    }


def ev_message(item_id: str, text: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": item_id,
            "content": [{"type": "output_text", "text": text}],
        },
    }


def ev_function(call_id: str, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": json.dumps(arguments, separators=(",", ":")),
        },
    }


def ev_custom(call_id: str, name: str, raw_input: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": raw_input,
        },
    }


def ev_local_shell(call_id: str, command: list[str]) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "local_shell_call",
            "call_id": call_id,
            "status": "completed",
            "action": {"type": "exec", "command": command},
        },
    }


def ev_web_search_done(call_id: str, query: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "web_search_call",
            "id": call_id,
            "status": "completed",
            "action": {"type": "search", "query": query},
        },
    }


def tool_names(request_body: dict[str, Any]) -> list[str]:
    names: list[str] = []
    for tool in request_body.get("tools", []) or []:
        name = tool.get("name") or tool.get("type")
        if isinstance(name, str):
            names.append(name)
    return names


def choose_shell_tool(names: set[str]) -> str:
    for name in ("shell_command", "exec_command", "shell", "local_shell"):
        if name in names:
            return name
    raise RuntimeError(f"no shell-capable tool exposed in {sorted(names)}")


def shell_call(
    call_id: str, shell_tool: str, command: list[str] | str
) -> dict[str, Any]:
    if shell_tool == "local_shell":
        if isinstance(command, str):
            command = ["/bin/zsh", "-lc", command]
        return ev_local_shell(call_id, command)
    if shell_tool == "shell_command":
        if isinstance(command, list):
            command = " ".join(subprocess.list2cmdline([part]) for part in command)
        return ev_function(
            call_id, "shell_command", {"command": command, "timeout_ms": 60000}
        )
    if shell_tool == "exec_command":
        if isinstance(command, str):
            command_text = command
        else:
            command_text = " ".join(subprocess.list2cmdline([part]) for part in command)
        return ev_function(
            call_id,
            "exec_command",
            {"cmd": command_text, "yield_time_ms": 1000, "max_output_tokens": 6000},
        )
    if isinstance(command, str):
        command = ["/bin/zsh", "-lc", command]
    return ev_function(call_id, "shell", {"command": command, "timeout_ms": 60000})


def output_text_for(body: dict[str, Any], call_id: str) -> str:
    for item in body.get("input", []) or []:
        if item.get("call_id") != call_id:
            continue
        output = item.get("output")
        if isinstance(output, str):
            return output
        if isinstance(output, list):
            return "\n".join(
                entry.get("text", "") for entry in output if isinstance(entry, dict)
            )
    return ""


def write_fixture(workspace: Path) -> None:
    workspace.mkdir(parents=True, exist_ok=True)
    (workspace / "duration_parser.py").write_text(
        "\n".join(
            [
                "import re",
                "",
                "",
                "UNITS = {",
                "    's': 1,",
                "    'm': 60,",
                "    'h': 3600,",
                "}",
                "",
                "",
                "def parse_duration(value):",
                "    match = re.fullmatch(r'\\s*(\\d+)\\s*([smh])\\s*', value)",
                "    if not match:",
                "        raise ValueError(f'invalid duration: {value!r}')",
                "    amount, unit = match.groups()",
                "    return int(amount) * UNITS[unit]",
                "",
            ]
        ),
        encoding="utf-8",
    )
    (workspace / "test_duration_parser.py").write_text(
        "\n".join(
            [
                "import unittest",
                "",
                "from duration_parser import parse_duration",
                "",
                "",
                "class ParseDurationTests(unittest.TestCase):",
                "    def test_single_unit(self):",
                "        self.assertEqual(parse_duration('45s'), 45)",
                "        self.assertEqual(parse_duration('2m'), 120)",
                "        self.assertEqual(parse_duration('1h'), 3600)",
                "",
                "    def test_mixed_units(self):",
                "        self.assertEqual(parse_duration('1h 30m 5s'), 5405)",
                "        self.assertEqual(parse_duration('2h 15m'), 8100)",
                "",
                "    def test_rejects_invalid_tail(self):",
                "        with self.assertRaises(ValueError):",
                "            parse_duration('1h bananas')",
                "",
                "",
                "if __name__ == '__main__':",
                "    unittest.main()",
                "",
            ]
        ),
        encoding="utf-8",
    )
    (workspace / "evidence.png").write_bytes(PNG_1X1)


class HarnessState:
    def __init__(self, workspace: Path, evidence: Path):
        self.workspace = workspace
        self.evidence = evidence
        self.lock = threading.Lock()
        self.requests: list[dict[str, Any]] = []
        self.exposed_tools: list[str] = []

    def record_request(self, body: dict[str, Any]) -> int:
        with self.lock:
            self.requests.append(body)
            if not self.exposed_tools:
                self.exposed_tools = tool_names(body)
            index = len(self.requests)
            (self.evidence / f"request-{index:02d}.json").write_text(
                json.dumps(body, indent=2, sort_keys=True),
                encoding="utf-8",
            )
            return index


def build_round_events(
    state: HarnessState, index: int, body: dict[str, Any]
) -> list[dict[str, Any]]:
    names = set(state.exposed_tools or tool_names(body))
    shell_tool = choose_shell_tool(names)
    workspace = state.workspace

    if index == 1:
        events = [ev_response_created("resp-1")]
        events.append(
            ev_function(
                "plan-start",
                "update_plan",
                {
                    "explanation": "Run failing test, inspect code, patch parser, rerun tests.",
                    "plan": [
                        {"step": "Inspect fixture", "status": "in_progress"},
                        {"step": "Reproduce failing test", "status": "pending"},
                        {"step": "Patch parser", "status": "pending"},
                        {"step": "Rerun tests and diff", "status": "pending"},
                    ],
                },
            )
        )
        if "list_dir" in names:
            events.append(ev_function("list-dir", "list_dir", {"path": str(workspace)}))
        if "grep_files" in names:
            events.append(
                ev_function(
                    "grep-tests",
                    "grep_files",
                    {"pattern": "mixed_units", "path": str(workspace)},
                )
            )
        if "read_file" in names:
            events.append(
                ev_function(
                    "read-parser",
                    "read_file",
                    {"path": str(workspace / "duration_parser.py")},
                )
            )
        if "js_repl" in names:
            events.append(
                ev_custom(
                    "js-calc", "js_repl", "const seconds = 3600 + 30 * 60 + 5;\nseconds"
                )
            )
        if "js_repl_reset" in names:
            events.append(ev_function("js-reset", "js_repl_reset", {}))
        if "view_image" in names:
            events.append(
                ev_function(
                    "view-image",
                    "view_image",
                    {"path": str(workspace / "evidence.png")},
                )
            )
        events.append(
            shell_call("test-red", shell_tool, "python3 -m unittest discover -v")
        )
        if "write_stdin" in names:
            if shell_tool == "exec_command":
                events.append(
                    ev_function(
                        "stdin-open",
                        "exec_command",
                        {
                            "cmd": 'python3 -u -c \'import time; print("ready", flush=True); time.sleep(10); print("done", flush=True)\'',
                            "tty": True,
                            "yield_time_ms": 1000,
                            "max_output_tokens": 2000,
                        },
                    )
                )
            else:
                events.append(
                    shell_call(
                        "stdin-open",
                        shell_tool,
                        'python3 -u -c \'import time; print("ready", flush=True); time.sleep(10); print("done", flush=True)\'',
                    )
                )
        events.append(ev_completed("resp-1"))
        return events

    if index == 2:
        patch = """*** Begin Patch
*** Update File: duration_parser.py
@@
 def parse_duration(value):
-    match = re.fullmatch(r'\\s*(\\d+)\\s*([smh])\\s*', value)
-    if not match:
+    parts = re.findall(r'(\\d+)\\s*([smh])', value)
+    if not parts:
         raise ValueError(f'invalid duration: {value!r}')
-    amount, unit = match.groups()
-    return int(amount) * UNITS[unit]
+    consumed = ''.join(f'{amount}{unit}' for amount, unit in parts)
+    compact = re.sub(r'\\s+', '', value)
+    if consumed != compact:
+        raise ValueError(f'invalid duration: {value!r}')
+    return sum(int(amount) * UNITS[unit] for amount, unit in parts)
*** End Patch
"""
        events = [ev_response_created("resp-2")]
        stdin_output = output_text_for(body, "stdin-open")
        match = re.search(r"session ID (\d+)", stdin_output)
        if "write_stdin" in names and match:
            events.append(
                ev_function(
                    "write-stdin-positive",
                    "write_stdin",
                    {
                        "session_id": int(match.group(1)),
                        "chars": "",
                        "yield_time_ms": 1000,
                        "max_output_tokens": 2000,
                    },
                )
            )
        if "apply_patch" in names:
            events.append(ev_custom("patch-parser", "apply_patch", patch))
        else:
            events.append(
                shell_call(
                    "patch-parser-shell",
                    shell_tool,
                    f"apply_patch <<'PATCH'\n{patch}\nPATCH",
                )
            )
        events.append(ev_completed("resp-2"))
        return events

    if index == 3:
        events = [ev_response_created("resp-3")]
        events.append(
            shell_call("test-green", shell_tool, "python3 -m unittest discover -v")
        )
        events.append(
            shell_call(
                "diff-evidence",
                shell_tool,
                "git diff -- duration_parser.py test_duration_parser.py || true",
            )
        )
        events.append(
            ev_function(
                "plan-done",
                "update_plan",
                {
                    "explanation": "Parser patched and tests rerun.",
                    "plan": [
                        {"step": "Inspect fixture", "status": "completed"},
                        {"step": "Reproduce failing test", "status": "completed"},
                        {"step": "Patch parser", "status": "completed"},
                        {"step": "Rerun tests and diff", "status": "completed"},
                    ],
                },
            )
        )
        events.append(ev_completed("resp-3"))
        return events

    if index == 4:
        events = [ev_response_created("resp-4")]
        events.append(
            ev_web_search_done(
                "web-search-check", "Codex harness deterministic web search event"
            )
        )
        events.append(
            ev_function(
                "cron-create",
                "CronCreate",
                {"schedule": "*/10 * * * *", "prompt": "check deterministic fixture"},
            )
        )
        events.append(ev_function("cron-list", "CronList", {}))
        events.append(
            ev_function("cron-delete-missing", "CronDelete", {"id": "missing-task"})
        )
        events.append(
            ev_function(
                "request-user-input-invalid",
                "request_user_input",
                {
                    "questions": [
                        {
                            "id": "scope",
                            "header": "Scope",
                            "question": "This negative call must fail before waiting.",
                            "options": [],
                        }
                    ]
                },
            )
        )
        events.append(
            ev_function(
                "write-stdin-negative",
                "write_stdin",
                {"session_id": 999999, "chars": "", "yield_time_ms": 1000},
            )
        )
        events.append(ev_completed("resp-4"))
        return events

    return [
        ev_response_created(f"resp-{index}"),
        ev_message(
            f"msg-{index}",
            "Completed deterministic coding task: reproduced failing unittest, patched duration_parser.py, reran unittest, captured git diff, and exercised remaining non-interactive tool entries.",
        ),
        ev_completed(f"resp-{index}"),
    ]


def make_handler(state: HarnessState) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, fmt: str, *args: Any) -> None:
            with (state.evidence / "server.log").open("a", encoding="utf-8") as fh:
                fh.write(fmt % args + "\n")

        def do_GET(self) -> None:
            if self.path == "/v1/models":
                payload = {
                    "models": [
                        {
                            "slug": "gpt-5",
                            "display_name": "GPT 5 Harness",
                            "description": None,
                            "default_reasoning_level": "medium",
                            "supported_reasoning_levels": [],
                            "shell_type": "unified_exec",
                            "visibility": "none",
                            "supported_in_api": True,
                            "priority": 99,
                            "service_tiers": [],
                            "availability_nux": None,
                            "upgrade": None,
                            "base_instructions": "You are a coding agent.",
                            "model_messages": None,
                            "supports_reasoning_summaries": False,
                            "default_reasoning_summary": "auto",
                            "support_verbosity": False,
                            "default_verbosity": None,
                            "apply_patch_tool_type": "freeform",
                            "web_search_tool_type": "text",
                            "truncation_policy": {"mode": "bytes", "limit": 100000},
                            "supports_parallel_tool_calls": False,
                            "supports_image_detail_original": False,
                            "context_window": 272000,
                            "max_context_window": 272000,
                            "auto_compact_token_limit": None,
                            "effective_context_window_percent": 95,
                            "experimental_supported_tools": [
                                "grep_files",
                                "read_file",
                                "list_dir",
                                "test_sync_tool",
                            ],
                            "input_modalities": ["text", "image"],
                            "prefer_websockets": False,
                        }
                    ]
                }
                encoded = json.dumps(payload).encode()
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)
                return
            self.send_error(404)

        def do_POST(self) -> None:
            if self.path != "/v1/responses":
                self.send_error(404)
                return
            length = int(self.headers.get("content-length", "0"))
            raw = self.rfile.read(length)
            body = json.loads(raw.decode())
            index = state.record_request(body)
            events = build_round_events(state, index, body)
            encoded = sse(events)
            (state.evidence / f"response-{index:02d}.sse").write_bytes(encoded)
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("cache-control", "no-cache")
            self.send_header("content-length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

    return Handler


def run(args: argparse.Namespace) -> int:
    evidence = Path(args.evidence).resolve()
    workspace = Path(args.workspace).resolve()
    codex_bin = Path(args.codex_bin).resolve()
    if evidence.exists():
        shutil.rmtree(evidence)
    if workspace.exists():
        shutil.rmtree(workspace)
    evidence.mkdir(parents=True)
    write_fixture(workspace)
    subprocess.run(["git", "init", "-q"], cwd=workspace, check=True)
    subprocess.run(
        ["git", "config", "user.email", "codex-harness@example.test"],
        cwd=workspace,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Codex Harness"], cwd=workspace, check=True
    )
    subprocess.run(["git", "add", "."], cwd=workspace, check=True)
    subprocess.run(
        ["git", "commit", "-q", "-m", "Initial fixture"], cwd=workspace, check=True
    )

    state = HarnessState(workspace, evidence)
    server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(state))
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    codex_home = Path(tempfile.mkdtemp(prefix="codex-home-", dir=str(evidence)))
    env = os.environ.copy()
    env.update(
        {
            "CODEX_HOME": str(codex_home),
            "OPENAI_API_KEY": "dummy",
            "OPENAI_BASE_URL": f"http://127.0.0.1:{port}/v1",
            "RUST_LOG": "codex_core=info,codex_exec=info",
        }
    )
    command = [
        str(codex_bin),
        "exec",
        "--json",
        "--enable",
        "apply_patch_freeform",
        "--enable",
        "js_repl",
        "--enable",
        "default_mode_request_user_input",
        "--skip-git-repo-check",
        "--dangerously-bypass-approvals-and-sandbox",
        "--model",
        "gpt-5",
        "--cd",
        str(workspace),
        "Fix duration_parser.py so all unittest tests pass. Use the available tools to inspect, reproduce, patch, rerun, and show diff evidence.",
    ]
    (evidence / "command.json").write_text(
        json.dumps(command, indent=2), encoding="utf-8"
    )
    proc = subprocess.run(
        command,
        cwd=workspace,
        env=env,
        text=True,
        capture_output=True,
        input="",
        timeout=180,
    )
    server.shutdown()
    thread.join(timeout=5)

    (evidence / "codex.stdout.jsonl").write_text(proc.stdout, encoding="utf-8")
    (evidence / "codex.stderr.log").write_text(proc.stderr, encoding="utf-8")
    (evidence / "exit-code.txt").write_text(str(proc.returncode), encoding="utf-8")
    (evidence / "exposed-tools.json").write_text(
        json.dumps(state.exposed_tools, indent=2), encoding="utf-8"
    )
    subprocess.run(
        ["python3", "-m", "unittest", "discover", "-v"],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    ).stdout
    final_test = subprocess.run(
        ["python3", "-m", "unittest", "discover", "-v"],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )
    (evidence / "independent-unittest.stdout.log").write_text(
        final_test.stdout, encoding="utf-8"
    )
    (evidence / "independent-unittest.stderr.log").write_text(
        final_test.stderr, encoding="utf-8"
    )
    (evidence / "independent-unittest-exit-code.txt").write_text(
        str(final_test.returncode), encoding="utf-8"
    )
    diff = subprocess.run(
        ["git", "diff", "--", "duration_parser.py", "test_duration_parser.py"],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )
    (evidence / "final.diff").write_text(diff.stdout, encoding="utf-8")
    return proc.returncode or final_test.returncode


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--codex-bin", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--evidence", required=True)
    return run(parser.parse_args())


if __name__ == "__main__":
    sys.exit(main())
