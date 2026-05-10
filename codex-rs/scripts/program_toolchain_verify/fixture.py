from __future__ import annotations

import json
import struct
import subprocess
import zlib
from difflib import unified_diff
from pathlib import Path
from typing import Any

from .protocol import summarize_outputs
from .protocol import tool_names

MODEL = "program-toolchain-test"
PROMPT = "fix the ledger math package, prove the tests fail first, then pass"
FINAL_MESSAGE = "compiled program coding task complete"
EXPECTED_TOOLS = {
    "exec_command",
    "write_stdin",
    "update_plan",
    "list_dir",
    "grep_files",
    "read_file",
    "test_sync_tool",
    "js_repl",
    "js_repl_reset",
    "view_image",
    "apply_patch",
    "CronCreate",
    "CronList",
    "CronDelete",
    "request_user_input",
    "request_permissions",
    "web_search",
    "image_generation",
}
EXPECTED_OUTPUT_CALLS = [
    "plan-call",
    "list-call",
    "grep-call",
    "read-call",
    "fail-test-call",
    "sync-call",
    "exec-call",
    "stdin-call",
    "js-call",
    "js-reset-call",
    "view-call",
    "patch-call",
    "pass-test-call",
    "verify-artifact-call",
    "cron-create-call",
    "cron-list-call",
    "cron-delete-call",
]


def repo_paths() -> tuple[Path, Path]:
    codex_rs = Path(__file__).resolve().parents[2]
    return codex_rs, codex_rs.parent


def write_fixture(workspace: Path) -> None:
    (workspace / "src").mkdir(parents=True)
    (workspace / "tests").mkdir()
    (workspace / "assets").mkdir()
    (workspace / "src" / "__init__.py").write_text("", encoding="utf-8")
    (workspace / "src" / "ledger_math.py").write_text(ledger_math_source(), encoding="utf-8")
    (workspace / "tests" / "test_ledger_math.py").write_text(test_source(), encoding="utf-8")
    (workspace / "assets" / "evidence.png").write_bytes(valid_png_bytes())


def ledger_math_source() -> str:
    return """from __future__ import annotations


def summarize_entries(entries: list[dict[str, int | str]]) -> dict[str, int]:
    total = 0
    bonus = 0
    for entry in entries:
        amount = int(entry["amount"])
        kind = str(entry.get("kind", "base"))
        if kind == "base":
            total += amount
        elif kind == "bonus":
            total += amount
        else:
            raise ValueError(f"unsupported ledger kind: {kind}")
    return {"total": total, "bonus": bonus, "net": total + bonus}


def render_summary(summary: dict[str, int]) -> str:
    return "\\n".join(
        [
            f"total={summary['total']}",
            f"bonus={summary['bonus']}",
            f"net={summary['net']}",
        ]
    )
"""


def test_source() -> str:
    return """from __future__ import annotations

import unittest

from src.ledger_math import render_summary
from src.ledger_math import summarize_entries


class LedgerMathTest(unittest.TestCase):
    def test_summarize_entries_keeps_base_and_bonus_separate(self) -> None:
        summary = summarize_entries(
            [
                {"kind": "base", "amount": 3},
                {"kind": "bonus", "amount": 4},
            ]
        )
        self.assertEqual(summary, {"total": 3, "bonus": 4, "net": 7})

    def test_render_summary_is_stable(self) -> None:
        text = render_summary({"total": 3, "bonus": 4, "net": 7})
        self.assertEqual(text, "total=3\\nbonus=4\\nnet=7")

    def test_unknown_kind_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported ledger kind"):
            summarize_entries([{"kind": "fee", "amount": 2}])


if __name__ == "__main__":
    unittest.main()
"""


def valid_png_bytes() -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(kind + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", crc)

    width = 2
    height = 2
    rgba = bytes([0, 90, 180, 255] * width * height)
    rows = b"".join(b"\x00" + rgba[row * width * 4 : (row + 1) * width * 4] for row in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


def write_catalog(codex_rs: Path, path: Path) -> None:
    catalog = json.loads((codex_rs / "core" / "models.json").read_text(encoding="utf-8"))
    model = next(m for m in catalog["models"] if m["slug"] == "gpt-5.1-codex")
    model = dict(model)
    model["slug"] = MODEL
    model["display_name"] = MODEL
    model["experimental_supported_tools"] = [
        "test_sync_tool",
        "read_file",
        "grep_files",
        "list_dir",
    ]
    path.write_text(json.dumps({"models": [model]}, indent=2), encoding="utf-8")


def write_config(home: Path, catalog: Path) -> None:
    config = f"""
model = "{MODEL}"
model_catalog_json = "{catalog}"
web_search = "live"
disable_cron = false
suppress_unstable_features_warning = true

[features]
apply_patch_freeform = true
unified_exec = true
js_repl = true
request_permissions = true
request_permissions_tool = true
default_mode_request_user_input = true
image_generation = true
enable_request_compression = false
"""
    home.mkdir()
    (home / "config.toml").write_text(config.lstrip(), encoding="utf-8")


def run_shell(cmd: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["/bin/bash", "-lc", cmd], cwd=cwd, text=True, capture_output=True)


def run_unit_tests(workspace: Path) -> subprocess.CompletedProcess[str]:
    return run_shell("python3 -m unittest discover -s tests -v", workspace)


def assert_result(
    requests: list[dict[str, Any]], workspace: Path, last_message: Path
) -> dict[str, Any]:
    advertised = tool_names(requests[0])
    assert_exact_set("advertised tools", EXPECTED_TOOLS, advertised)
    assert_workspace_files(workspace)
    if last_message.read_text(encoding="utf-8") != FINAL_MESSAGE:
        raise AssertionError("last message file did not contain final assistant message")
    outputs = summarize_outputs(requests)
    assert_exact_set("tool output call ids", set(EXPECTED_OUTPUT_CALLS), set(outputs))
    assert_tool_output_semantics(outputs)
    return {"advertised_tools": sorted(advertised), "tool_outputs": outputs}


def assert_exact_set(name: str, expected: set[str], actual: set[str]) -> None:
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        raise AssertionError(f"{name} mismatch: missing={missing}, unexpected={unexpected}")


def assert_tool_output_semantics(outputs: dict[str, Any]) -> None:
    text = outputs["list-call"]["output"]
    if "ledger_math.py" not in text or "test_ledger_math.py" not in text:
        raise AssertionError("list_dir output did not include coding fixture files")
    if "ledger_math.py" not in outputs["grep-call"]["output"]:
        raise AssertionError("grep_files output did not include ledger_math.py")
    if "total += amount" not in outputs["read-call"]["output"]:
        raise AssertionError("read_file output did not include broken bonus branch")
    if "FAILED" not in outputs["fail-test-call"]["output"]:
        raise AssertionError("initial test run did not fail")
    if outputs["sync-call"]["output"] != "ok":
        raise AssertionError("test_sync_tool output was not ok")
    js_items = outputs["js-call"]["output"]
    if not isinstance(js_items, list) or js_items[0].get("text") != '{"net":7,"doubled":14}':
        raise AssertionError(f"unexpected js_repl output: {js_items!r}")
    view_items = outputs["view-call"]["output"]
    if not isinstance(view_items, list) or view_items[0].get("type") != "input_image":
        raise AssertionError(f"view_image did not return an input_image: {view_items!r}")
    if "M src/ledger_math.py" not in outputs["patch-call"]["output"]:
        raise AssertionError("apply_patch output did not include source modification")
    if "OK" not in outputs["pass-test-call"]["output"]:
        raise AssertionError("post-fix test run did not pass")
    if "net=7" not in outputs["verify-artifact-call"]["output"]:
        raise AssertionError("artifact verification did not include net result")


def assert_workspace_files(workspace: Path) -> None:
    generated = (workspace / "src" / "generated.txt").read_text(encoding="utf-8")
    result = (workspace / "src" / "result.txt").read_text(encoding="utf-8")
    fixed_source = (workspace / "src" / "ledger_math.py").read_text(encoding="utf-8")
    if "bonus += amount" not in fixed_source:
        raise AssertionError("ledger_math.py was not fixed")
    if generated != "total=3\nbonus=4\nnet=7\nsource=compiled-codex-exec\n":
        raise AssertionError(f"unexpected generated.txt: {generated!r}")
    expected = "status=done\ntests=passed\nnet=7\nsource=compiled-codex-exec\n"
    if result != expected:
        raise AssertionError(f"unexpected result.txt: {result!r}")


def independent_retest(workspace: Path) -> subprocess.CompletedProcess[str]:
    return run_shell(
        "python3 -m unittest discover -s tests -v && "
        "grep -qx 'total=3' src/generated.txt && "
        "grep -qx 'bonus=4' src/generated.txt && "
        "grep -qx 'net=7' src/generated.txt && "
        "grep -qx 'source=compiled-codex-exec' src/generated.txt && "
        "grep -qx 'tests=passed' src/result.txt",
        workspace,
    )


def workspace_text_diff(before: Path, after: Path) -> str:
    lines: list[str] = []
    names = sorted({*text_files(before), *text_files(after)})
    for name in names:
        before_path = before / name
        after_path = after / name
        before_text = read_text_if_exists(before_path)
        after_text = read_text_if_exists(after_path)
        if before_text == after_text:
            continue
        lines.extend(
            unified_diff(
                before_text.splitlines(keepends=True),
                after_text.splitlines(keepends=True),
                fromfile=f"before/{name}",
                tofile=f"after/{name}",
            )
        )
    return "".join(lines)


def text_files(root: Path) -> set[Path]:
    paths: set[Path] = set()
    for path in root.rglob("*"):
        if not path.is_file() or "__pycache__" in path.parts:
            continue
        if path.suffix in {".py", ".txt"}:
            paths.add(path.relative_to(root))
    return paths


def read_text_if_exists(path: Path) -> str:
    if not path.exists():
        return ""
    return path.read_text(encoding="utf-8")
