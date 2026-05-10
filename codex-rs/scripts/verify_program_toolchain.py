#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import Any

from program_toolchain_verify.fixture import FINAL_MESSAGE
from program_toolchain_verify.fixture import MODEL
from program_toolchain_verify.fixture import PROMPT
from program_toolchain_verify.fixture import assert_result
from program_toolchain_verify.fixture import independent_retest
from program_toolchain_verify.fixture import repo_paths
from program_toolchain_verify.fixture import run_shell
from program_toolchain_verify.fixture import run_unit_tests
from program_toolchain_verify.fixture import workspace_text_diff
from program_toolchain_verify.fixture import write_catalog
from program_toolchain_verify.fixture import write_config
from program_toolchain_verify.fixture import write_fixture
from program_toolchain_verify.mock_server import MockState
from program_toolchain_verify.mock_server import ResponsesHandler


def parse_args() -> argparse.Namespace:
    codex_rs, _ = repo_paths()
    parser = argparse.ArgumentParser()
    parser.add_argument("--codex-exec", default=str(codex_rs / "target/debug/codex-exec"))
    parser.add_argument("--timeout", type=int, default=120)
    return parser.parse_args()


def prepare_run_root(codex_rs: Path) -> tuple[Path, Path, Path, Path]:
    run_root = Path(tempfile.mkdtemp(prefix="program-toolchain-", dir=codex_rs / "target"))
    workspace = run_root / "workspace"
    home = run_root / "codex-home"
    catalog = run_root / "model_catalog.json"
    workspace.mkdir()
    write_fixture(workspace)
    return run_root, workspace, home, catalog


def start_server(state: MockState) -> tuple[ThreadingHTTPServer, threading.Thread, str]:
    ResponsesHandler.state = state
    server = ThreadingHTTPServer(("127.0.0.1", 0), ResponsesHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}/v1"
    return server, thread, base_url


def run_codex_exec(
    binary: Path, workspace: Path, home: Path, base_url: str, last_message: Path, timeout: int
) -> subprocess.CompletedProcess[str]:
    cmd = [
        str(binary),
        "--skip-git-repo-check",
        "--ephemeral",
        "--color",
        "never",
        "-o",
        str(last_message),
        "-C",
        str(workspace),
        "-s",
        "danger-full-access",
        "-m",
        MODEL,
        PROMPT,
    ]
    env = os.environ.copy()
    env.update(
        {
            "CODEX_HOME": str(home),
            "CODEX_API_KEY": "dummy",
            "OPENAI_API_KEY": "dummy",
            "OPENAI_BASE_URL": base_url,
        }
    )
    return subprocess.run(cmd, input="", text=True, capture_output=True, env=env, timeout=timeout)


def write_run_artifacts(
    run_root: Path,
    before: Path,
    workspace: Path,
    requests: list[dict[str, Any]],
    summary: dict[str, Any],
) -> None:
    (run_root / "workspace.diff").write_text(workspace_text_diff(before, workspace), encoding="utf-8")
    (run_root / "requests.json").write_text(json.dumps(requests, indent=2), encoding="utf-8")
    (run_root / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")


def verify() -> dict[str, Any]:
    args = parse_args()
    codex_rs, _ = repo_paths()
    binary = Path(args.codex_exec).resolve()
    if not binary.exists():
        raise FileNotFoundError(f"missing compiled codex-exec binary: {binary}")
    run_root, workspace, home, catalog = prepare_run_root(codex_rs)
    baseline = run_unit_tests(workspace)
    if baseline.returncode == 0:
        raise AssertionError("baseline tests unexpectedly passed before codex-exec ran")
    before = run_root / "before"
    shutil.copytree(workspace, before)
    write_catalog(codex_rs, catalog)
    write_config(home, catalog)
    return run_and_assert(binary, run_root, workspace, home, before, baseline, args.timeout)


def run_and_assert(
    binary: Path,
    run_root: Path,
    workspace: Path,
    home: Path,
    before: Path,
    baseline: subprocess.CompletedProcess[str],
    timeout: int,
) -> dict[str, Any]:
    state = MockState(workspace)
    server, thread, base_url = start_server(state)
    started = time.time()
    last_message = run_root / "last_message.txt"
    try:
        proc = run_codex_exec(binary, workspace, home, base_url, last_message, timeout)
    except subprocess.TimeoutExpired as exc:
        summary = timeout_summary(run_root, state, baseline, binary, timeout, exc)
        write_run_artifacts(run_root, before, workspace, state.requests, summary)
        raise AssertionError(
            f"codex-exec timed out after {timeout}s; "
            f"partial artifacts written to {run_root}; "
            f"captured {len(state.requests)} request(s)"
        ) from exc
    finally:
        server.shutdown()
        thread.join(timeout=5)
    if proc.returncode != 0:
        raise AssertionError(f"codex-exec failed with {proc.returncode}\nSTDERR:\n{proc.stderr}")
    return build_summary(
        run_root, before, workspace, last_message, state, proc, started, baseline, binary
    )


def build_summary(
    run_root: Path,
    before: Path,
    workspace: Path,
    last_message: Path,
    state: MockState,
    proc: subprocess.CompletedProcess[str],
    started: float,
    baseline: subprocess.CompletedProcess[str],
    binary: Path,
) -> dict[str, Any]:
    if len(state.requests) != len(state.responders):
        raise AssertionError(f"expected {len(state.responders)} requests, got {len(state.requests)}")
    retest = independent_retest(workspace)
    summary = base_summary(run_root, state, proc, started, baseline, binary, retest)
    try:
        summary.update(assert_result(state.requests, workspace, last_message))
        if retest.returncode != 0:
            raise AssertionError(f"independent retest failed: {retest.stderr}")
    except Exception:
        write_run_artifacts(run_root, before, workspace, state.requests, summary)
        raise
    write_run_artifacts(run_root, before, workspace, state.requests, summary)
    return summary


def timeout_summary(
    run_root: Path,
    state: MockState,
    baseline: subprocess.CompletedProcess[str],
    binary: Path,
    timeout_seconds: int,
    exc: subprocess.TimeoutExpired,
) -> dict[str, Any]:
    return {
        "run_root": str(run_root),
        "binary": str(binary),
        "request_count": len(state.requests),
        "error": "TimeoutExpired",
        "timeout_seconds": timeout_seconds,
        "baseline_returncode": baseline.returncode,
        "baseline_stdout": baseline.stdout,
        "baseline_stderr": baseline.stderr,
        "stdout_before_timeout": decode_timeout_output(exc.stdout),
        "stderr_before_timeout": decode_timeout_output(exc.stderr),
    }


def decode_timeout_output(value: bytes | str | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode(errors="replace")
    return value


def base_summary(
    run_root: Path,
    state: MockState,
    proc: subprocess.CompletedProcess[str],
    started: float,
    baseline: subprocess.CompletedProcess[str],
    binary: Path,
    retest: subprocess.CompletedProcess[str],
) -> dict[str, Any]:
    return {
        "run_root": str(run_root),
        "binary": str(binary),
        "request_count": len(state.requests),
        "elapsed_seconds": round(time.time() - started, 3),
        "baseline_returncode": baseline.returncode,
        "baseline_stdout": baseline.stdout,
        "baseline_stderr": baseline.stderr,
        "stdout": proc.stdout,
        "stderr": proc.stderr,
        "final_message": FINAL_MESSAGE,
        "content_encoding_headers": [
            h.get("content-encoding") for h in state.headers if h.get("content-encoding")
        ],
        "independent_retest_returncode": retest.returncode,
        "independent_retest_stdout": retest.stdout,
        "independent_retest_stderr": retest.stderr,
    }


def main() -> int:
    summary = verify()
    print(json.dumps({"ok": True, "run_root": summary["run_root"], "request_count": summary["request_count"]}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"verify_program_toolchain failed: {exc}", file=sys.stderr)
        raise
