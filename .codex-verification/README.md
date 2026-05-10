# Codex Verification Evidence

This directory stores a deterministic local verification run for the compiled
Codex binary in this checkout. It is an evidence bundle, not product
documentation or a normal test fixture.

## Scope

- Binary under test: `.codex-verification/bin/codex`
- Reported version: `codex-cli 1.3.0`
- Git HEAD recorded by the run:
  `fbc9276c1bce3080fa9c0fc767d26ba92506533a`
- Harness: `.codex-verification/harness/codex_tool_task_harness.py`
- Main report: `.codex-verification/logs/tool-task/REPORT.md`
- Structured manifest: `.codex-verification/logs/tool-task/manifest.json`

The harness starts a local fake Responses API, runs `codex exec --json`, records
the tool surface from `request-01.json`, and drives a small real coding task in
`.codex-verification/work/duration-fixture`.

## Verified Behavior

The recorded run shows that Codex:

- exposed 12 AI-callable tools in this harness run;
- exercised every exposed tool entry at least once;
- reproduced a failing `python3 -m unittest discover -v` run;
- patched `duration_parser.py` with `apply_patch`;
- reran the same unittest command successfully;
- passed an independent unittest recheck after Codex exited;
- captured final diff evidence.

See `logs/tool-task/REPORT.md` for the human-readable summary and
`logs/tool-task/manifest.json` for exact file paths, tool call statuses, and
limitations.

## Important Limits

- `request_user_input` is covered only by an invalid-options negative call, so
  the non-interactive `codex exec` run does not block waiting for a human.
- `CronDelete` is covered by a missing-id negative call; `CronCreate` and
  `CronList` are covered by session-scoped positive calls.
- `web_search` is covered by a `web_search_call` event from the fake Responses
  stream, not by live internet access.
- The verified runtime tool surface is the `request-01.json` tools array from
  this run. Tools not present there were not AI-callable in this harness run.

## Reproduction Notes

The original command is recorded in `logs/tool-task/command.json`. To rerun the
harness, use the same binary path, workspace path, and evidence path shape, or
point those arguments at a fresh output directory to avoid overwriting this
bundle.
