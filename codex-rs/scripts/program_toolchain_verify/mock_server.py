from __future__ import annotations

import json
import re
import threading
from http.server import BaseHTTPRequestHandler
from pathlib import Path
from typing import Any, Callable

from .fixture import FINAL_MESSAGE
from .protocol import assistant_message
from .protocol import custom_tool_call
from .protocol import function_call
from .protocol import output_text
from .protocol import sse


class MockState:
    def __init__(self, workspace: Path):
        self.workspace = workspace
        self.requests: list[dict[str, Any]] = []
        self.headers: list[dict[str, str]] = []
        self.lock = threading.Lock()
        self.session_id: int | None = None
        self.cron_id: str | None = None
        self.responders: list[Callable[[dict[str, Any]], bytes]] = [
            self.plan,
            self.list_dir,
            self.grep_files,
            self.read_file,
            self.fail_tests,
            self.test_sync,
            self.apply_patch,
            self.pass_tests,
            self.exec_command,
            self.write_stdin,
            self.js_repl,
            self.js_repl_reset,
            self.view_image,
            self.verify_artifact,
            self.cron_create,
            self.cron_list,
            self.cron_delete,
            self.final_message,
        ]

    def next_response(self, body: dict[str, Any], headers: dict[str, str]) -> bytes:
        with self.lock:
            index = len(self.requests)
            self.requests.append(body)
            self.headers.append(headers)
            if index >= len(self.responders):
                raise AssertionError(f"unexpected extra POST {index + 1}")
            return self.responders[index](body)

    def plan(self, _: dict[str, Any]) -> bytes:
        args = {
            "explanation": "Compiled binary program toolchain fixture",
            "plan": [
                {"step": "Reproduce failing unit test", "status": "in_progress"},
                {"step": "Patch ledger math bug", "status": "pending"},
                {"step": "Verify tests and artifacts", "status": "pending"},
            ],
        }
        return sse("resp-1", function_call("plan-call", "update_plan", args))

    def list_dir(self, _: dict[str, Any]) -> bytes:
        args = {"dir_path": str(self.workspace), "limit": 20, "depth": 2}
        return sse("resp-2", function_call("list-call", "list_dir", args))

    def grep_files(self, _: dict[str, Any]) -> bytes:
        args = {
            "pattern": "summarize_entries",
            "path": str(self.workspace),
            "include": "*.py",
            "limit": 10,
        }
        return sse("resp-3", function_call("grep-call", "grep_files", args))

    def read_file(self, _: dict[str, Any]) -> bytes:
        args = {
            "file_path": str(self.workspace / "src" / "ledger_math.py"),
            "offset": 1,
            "limit": 40,
        }
        return sse("resp-4", function_call("read-call", "read_file", args))

    def fail_tests(self, _: dict[str, Any]) -> bytes:
        args = {
            "cmd": "python3 -m unittest discover -s tests -v",
            "login": False,
            "yield_time_ms": 10000,
            "max_output_tokens": 6000,
        }
        return sse("resp-5", function_call("fail-test-call", "exec_command", args))

    def test_sync(self, _: dict[str, Any]) -> bytes:
        args = {"sleep_before_ms": 1, "sleep_after_ms": 1}
        return sse("resp-6", function_call("sync-call", "test_sync_tool", args))

    def exec_command(self, _: dict[str, Any]) -> bytes:
        args = {"cmd": "/bin/bash -i", "tty": True, "yield_time_ms": 50}
        return sse("resp-9", function_call("exec-call", "exec_command", args))

    def write_stdin(self, body: dict[str, Any]) -> bytes:
        text = output_text(body, "exec-call", "function_call_output")
        match = re.search(r"session ID (\d+)", text)
        if not match:
            raise AssertionError(f"could not parse session id from: {text}")
        self.session_id = int(match.group(1))
        chars = (
            "python3 - <<'PY' > src/generated.txt\n"
            "from src.ledger_math import render_summary, summarize_entries\n"
            "summary = summarize_entries([\n"
            "    {'kind': 'base', 'amount': 3},\n"
            "    {'kind': 'bonus', 'amount': 4},\n"
            "])\n"
            "print(render_summary(summary))\n"
            "print('source=compiled-codex-exec')\n"
            "PY\n"
            "exit\n"
        )
        args = {"session_id": self.session_id, "chars": chars, "yield_time_ms": 1000}
        return sse("resp-10", function_call("stdin-call", "write_stdin", args))

    def js_repl(self, _: dict[str, Any]) -> bytes:
        code = (
            'const fs = await import("node:fs/promises"); '
            'const text = await fs.readFile("src/generated.txt", "utf8"); '
            "const net = Number(text.match(/net=(\\d+)/)[1]); "
            "console.log(JSON.stringify({net, doubled: net * 2}));"
        )
        return sse("resp-11", custom_tool_call("js-call", "js_repl", code))

    def js_repl_reset(self, _: dict[str, Any]) -> bytes:
        return sse("resp-12", function_call("js-reset-call", "js_repl_reset", {}))

    def view_image(self, _: dict[str, Any]) -> bytes:
        args = {"path": "assets/evidence.png"}
        return sse("resp-13", function_call("view-call", "view_image", args))

    def apply_patch(self, _: dict[str, Any]) -> bytes:
        patch = (
            "*** Begin Patch\n"
            "*** Update File: src/ledger_math.py\n"
            "@@\n"
            "         if kind == \"base\":\n"
            "             total += amount\n"
            "         elif kind == \"bonus\":\n"
            "-            total += amount\n"
            "+            bonus += amount\n"
            "         else:\n"
            "             raise ValueError(f\"unsupported ledger kind: {kind}\")\n"
            "*** End Patch"
        )
        return sse("resp-7", custom_tool_call("patch-call", "apply_patch", patch))

    def pass_tests(self, _: dict[str, Any]) -> bytes:
        args = {
            "cmd": "python3 -m unittest discover -s tests -v",
            "login": False,
            "yield_time_ms": 10000,
            "max_output_tokens": 6000,
        }
        return sse("resp-8", function_call("pass-test-call", "exec_command", args))

    def verify_artifact(self, _: dict[str, Any]) -> bytes:
        cmd = (
            "printf 'status=done\\ntests=passed\\n' > src/result.txt && "
            "sed -n 's/^net=/net=/p' src/generated.txt >> src/result.txt && "
            "printf 'source=compiled-codex-exec\\n' >> src/result.txt && "
            "printf 'generated:' && cat src/generated.txt && "
            "printf 'result:' && cat src/result.txt"
        )
        args = {"cmd": cmd, "login": False, "yield_time_ms": 10000, "max_output_tokens": 6000}
        return sse("resp-14", function_call("verify-artifact-call", "exec_command", args))

    def cron_create(self, _: dict[str, Any]) -> bytes:
        args = {"schedule": "*/10 * * * *", "prompt": "check ledger status"}
        return sse("resp-15", function_call("cron-create-call", "CronCreate", args))

    def cron_list(self, body: dict[str, Any]) -> bytes:
        text = output_text(body, "cron-create-call", "function_call_output")
        self.cron_id = json.loads(text)["task"]["id"]
        return sse("resp-16", function_call("cron-list-call", "CronList", {}))

    def cron_delete(self, _: dict[str, Any]) -> bytes:
        if not self.cron_id:
            raise AssertionError("CronCreate did not return a task id")
        args = {"id": self.cron_id}
        return sse("resp-17", function_call("cron-delete-call", "CronDelete", args))

    def final_message(self, _: dict[str, Any]) -> bytes:
        return sse("resp-18", assistant_message(FINAL_MESSAGE))


class ResponsesHandler(BaseHTTPRequestHandler):
    state: MockState

    def log_message(self, _: str, *args: Any) -> None:
        return

    def do_POST(self) -> None:
        if self.path != "/v1/responses":
            self.send_error(404, "unexpected path")
            return
        try:
            body = self.read_json_body()
            response = self.state.next_response(body, dict(self.headers.items()))
        except Exception as exc:
            self.send_error(500, str(exc))
            return
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def read_json_body(self) -> dict[str, Any]:
        length = int(self.headers.get("content-length", "0"))
        return json.loads(self.rfile.read(length))
