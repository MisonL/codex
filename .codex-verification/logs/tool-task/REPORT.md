# Codex Tool Coverage Verification

- Binary: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/bin/codex`
- Version: `codex-cli 1.3.0`
- Git HEAD: `fbc9276c1bce3080fa9c0fc767d26ba92506533a`
- Runtime source of truth: `request-01.json tools array`

## Tool Coverage

- `exec_command`: called (test-red, stdin-open, test-red, stdin-open, test-red, stdin-open, test-green, diff-evidence, test-red, stdin-open, test-green, diff-evidence)
- `write_stdin`: called (write-stdin-positive, write-stdin-positive, write-stdin-positive, write-stdin-negative)
- `update_plan`: called (plan-start, plan-start, plan-start, plan-done, plan-start, plan-done)
- `js_repl`: called (js-calc, js-calc, js-calc, js-calc)
- `js_repl_reset`: called (js-reset, js-reset, js-reset, js-reset)
- `request_user_input`: called_negative_nonblocking (request-user-input-invalid)
- `CronCreate`: called (cron-create)
- `CronList`: called (cron-list)
- `CronDelete`: called_negative_missing_id (cron-delete-missing)
- `apply_patch`: called (patch-parser, patch-parser, patch-parser)
- `web_search`: called (<none>)
- `view_image`: called (view-image, view-image, view-image, view-image)

## Coding Task Evidence

- Red test: `python3 -m unittest discover -v` exited 1 before patch.
- Fix: `apply_patch` changed `duration_parser.py`.
- Green test: Codex reran unittest with exit 0.
- Independent recheck: unittest exit code 0 after Codex completed.
- Diff evidence: `final.diff`.

## Evidence Files

- `harness`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/harness/codex_tool_task_harness.py`
- `codex_jsonl`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/codex.stdout.jsonl`
- `codex_stderr`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/codex.stderr.log`
- `exposed_tools`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/exposed-tools.json`
- `request_bodies`: 5 files
- `responses_sse`: 5 files
- `final_diff`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/final.diff`
- `independent_unittest_stdout`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/independent-unittest.stdout.log`
- `independent_unittest_stderr`: `/Volumes/Work/code/stellarlinkco-codex-src/.codex-verification/logs/tool-task/independent-unittest.stderr.log`
