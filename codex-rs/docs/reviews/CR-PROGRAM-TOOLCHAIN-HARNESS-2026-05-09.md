# Program Toolchain Harness Review - 2026-05-09

## Scope

- Added a core integration harness for local program tools in `core/tests/suite/program_toolchain_harness/`.
- Added an exec integration test source in `exec/tests/program_toolchain.rs`.
- Added `scripts/verify_program_toolchain.py` plus helpers in `scripts/program_toolchain_verify/` to drive the compiled `target/debug/codex-exec` binary against a local mock Responses server.
- The scripted verifier creates a temporary fixture workspace, writes a custom `model_catalog.json`, enables the required local-tool feature flags through a temporary `CODEX_HOME/config.toml`, records every `/v1/responses` request, and asserts the resulting files and tool outputs.

## Tool Coverage

Compiled `codex-exec` scripted coverage:

- `update_plan`
- `list_dir`
- `grep_files`
- `read_file`
- `test_sync_tool`
- `exec_command`
- `write_stdin`
- `js_repl`
- `js_repl_reset`
- `view_image`
- `apply_patch`
- `CronCreate`
- `CronList`
- `CronDelete`

Compiled `codex-exec` advertised-only coverage:

- `web_search`
- `image_generation`
- `request_permissions`
- `request_user_input`

Core harness coverage:

- `exec_command`
- `write_stdin`
- `update_plan`
- `request_user_input`
- `request_permissions`
- `CronCreate`
- `CronList`
- `CronDelete`
- `grep_files`
- `read_file`
- `list_dir`
- `test_sync_tool`
- `js_repl`
- `js_repl_reset`
- `view_image`
- `apply_patch`

Remote built-in tools asserted by request shape:

- `web_search`
- `image_generation`

## Verification Results

- `python3 -m py_compile codex-rs/scripts/verify_program_toolchain.py codex-rs/scripts/program_toolchain_verify/*.py`
  - Result: passed.
- `wc -l codex-rs/scripts/verify_program_toolchain.py codex-rs/scripts/program_toolchain_verify/*.py`
  - Result: passed against the AGENTS.md 300-line file limit for each new Python file.
  - File lengths after the latest evidence hardening: entry script 242 lines, `fixture.py` 300 lines, `mock_server.py` 221 lines, `protocol.py` 98 lines.
- `python3 codex-rs/scripts/verify_program_toolchain.py --timeout 90`
  - Result: passed.
  - Compiled binary: `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/debug/codex-exec`.
  - Latest evidence directory: `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/program-toolchain-520gaecv`.
  - Latest elapsed time: 5.735 seconds.
  - Mock request count: 18 POSTs to `/v1/responses`.
  - Tool coverage assertion: `EXPECTED_TOOLS` must exactly match the compiled binary's advertised tools; `EXPECTED_OUTPUT_CALLS` must exactly match the recorded local tool output call ids.
  - Baseline check before Codex run: failed as expected with exit code 1 because the bonus branch returned `{"total": 7, "bonus": 0, "net": 7}` instead of `{"total": 3, "bonus": 4, "net": 7}`.
  - Final stdout and last-message file: `compiled program coding task complete`.
  - Independent retest after Codex run: exit code 0.
  - Workspace diff: unified text diff in `workspace.diff`; it changes `src/ledger_math.py` from `total += amount` to `bonus += amount` in the bonus branch, adds `src/generated.txt`, and adds `src/result.txt`.
  - Recorded advertised tools: `CronCreate`, `CronDelete`, `CronList`, `apply_patch`, `exec_command`, `grep_files`, `image_generation`, `js_repl`, `js_repl_reset`, `list_dir`, `read_file`, `request_permissions`, `request_user_input`, `test_sync_tool`, `update_plan`, `view_image`, `web_search`, `write_stdin`.
  - Recorded tool outputs include `function_call_output` for `update_plan`, `list_dir`, `grep_files`, `read_file`, `fail-test-call`, `test_sync_tool`, `pass-test-call`, `exec_command`, `write_stdin`, `js_repl_reset`, `view_image`, `verify-artifact-call`, `CronCreate`, `CronList`, and `CronDelete`, plus `custom_tool_call_output` for `js_repl` and `apply_patch`.
  - `view_image` returned structured `input_image` content with a data URL after fixing the fixture PNG generator.
  - Generated file: `total=3`, `bonus=4`, `net=7`, `source=compiled-codex-exec`.
  - Result file: `status=done`, `tests=passed`, `net=7`, `source=compiled-codex-exec`.
- `python3 -m unittest discover -s tests -v && grep -qx 'total=3' src/generated.txt && grep -qx 'bonus=4' src/generated.txt && grep -qx 'net=7' src/generated.txt && grep -qx 'source=compiled-codex-exec' src/generated.txt && grep -qx 'tests=passed' src/result.txt`
  - Workdir: `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/program-toolchain-520gaecv/workspace`.
  - Result: passed as an independent retest of the fixture after the compiled `codex-exec` run.
- Earlier `python3 codex-rs/scripts/verify_program_toolchain.py`
  - Result: timed out once at 120 seconds before reaching `apply_patch`; this exposed missing failure artifacts in the verifier.
  - Fix: verifier now writes `requests.json`, `summary.json`, and `workspace.diff` on timeout before raising an error.
  - Follow-up: `python3 codex-rs/scripts/verify_program_toolchain.py --timeout 45` passed with 18 requests.
- Earlier post-format verifier run
  - Result: failed with `initial test run did not fail`.
  - Cause: `fail-test-call` sometimes returned only a still-running `exec_command` session when the login shell printed startup output before the unittest process completed.
  - Fix: the verifier now runs the fail/pass/artifact commands with `login: false` and `yield_time_ms: 10000`.
  - Follow-up: the final `--timeout 90` run above passed with deterministic fail-before-pass output.
- First scripted verifier run
  - Result: exposed a real harness weakness.
  - The verifier passed while `view_image` returned an error text for an invalid PNG fixture. The script was tightened to generate a valid PNG using standard-library PNG chunks and to assert `view_image` returns an `input_image`.
- `just fmt`
  - Result: passed.
- `git diff --check`
  - Result: passed.
- `just fix -p codex-core`
  - Result: did not complete.
  - The command reached `cargo clippy --fix --tests --allow-dirty -p codex-core` and then remained silent for about 55 minutes with the relevant `cargo fix` / rustc processes at 0% CPU.
  - Action taken: the stuck process tree started by this run was terminated, and a follow-up `ps` check confirmed no remaining `cargo-clippy`, `cargo fix`, or `clippy-driver` process from this run.
- Direct compiled binary smoke using `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/debug/codex-exec`
  - Result: passed.
  - Mock request count: 5 POSTs to `/v1/responses`.
  - Final stdout: `binary toolchain complete`.
  - Generated file: `sum=7`, `source=compiled-codex-exec`.
  - Patched file: `status=done`, `sum=7`, `verified=binary`.
- `cargo test -p codex-exec --test all compiled_codex_exec_runs_program_toolchain_against_mock_responses -- --nocapture`
  - First run: failed because the test called `read_file`, which is not registered in the default compiled `codex-exec` path for `gpt-5.1`; output was `unsupported call: read_file`.
  - Follow-up runs: blocked in the `exec/tests/all.rs` link step for more than 4 minutes at low CPU and were stopped.
- `cargo test -p codex-exec --test program_toolchain compiled_codex_exec_runs_program_toolchain_against_mock_responses -- --nocapture`
  - Fresh result: passed.
  - Latest run: test profile finished in 5.09s; the test passed in 5.31s.
  - Earlier full build run: Cargo finished the test profile in 27m 14s, then ran `tests/program_toolchain.rs`; the test passed in 81.32s.
  - One natural-return run before the fix failed because `write-call` redirected all generated content to `src/generated.txt`; the recorded `function_call_output` was empty. The test now prints the same generated content that it writes, so the assertion covers real tool output.
- `cargo test -p codex-core --test all complex_program_toolchain_uses_discovery_edit_execution_and_inspection_tools -- --nocapture`
  - Result: passed.
  - Latest run: test profile finished in 6.82s; the test passed in 5.33s.
  - The core harness covers local tool discovery, request/response event plumbing, scheduling tools, unified exec plus `write_stdin`, `js_repl`, `view_image`, and freeform `apply_patch`.
  - One failing run exposed a missing fixture flag: `exec_command` returned `additional permissions are disabled; enable features.request_permission before using with_additional_permissions`, then `write_stdin` failed with `Unknown process id 1000`.
  - Fix: enable `Feature::RequestPermissions` alongside `Feature::RequestPermissionsTool` in the core fixture, and assert the current `PermissionProfile` JSON shape including explicit `file_system: null` and `macos: null`.

## Findings

- The scripted verifier now uses compiled `codex-exec` with a custom model catalog, so `read_file`, `grep_files`, `list_dir`, and `test_sync_tool` are both advertised and actually called by the compiled binary path.
- `request_permissions` and `request_user_input` are advertised by the compiled binary when the matching feature flags are enabled, but they are not safe to call from `codex-exec` in this script because exec mode has no harness channel to approve or answer those prompts. The core harness covers their full request/response behavior.
- `web_search` and `image_generation` are built-in remote tool specs. The local mock can assert that compiled `codex-exec` advertises them, but cannot execute remote backend side effects locally.
- Cargo integration test verification is slow in this environment because the `program_toolchain` test binary link step took 27m 14s, but the new Rust integration test now completes through `cargo test` and provides direct compiled `codex-exec` evidence.

## Completion Audit

- Re-verify compiled Codex: `summary.json` in `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/program-toolchain-520gaecv` records binary `/Volumes/Work/code/stellarlinkco-codex-src/codex-rs/target/debug/codex-exec`, `request_count: 18`, elapsed time 5.735 seconds, and final output `compiled program coding task complete`.
- Test all AI-callable program tools: `scripts/program_toolchain_verify/fixture.py` requires exact advertised tool equality for `exec_command`, `write_stdin`, `update_plan`, `list_dir`, `grep_files`, `read_file`, `test_sync_tool`, `js_repl`, `js_repl_reset`, `view_image`, `apply_patch`, `CronCreate`, `CronList`, `CronDelete`, `request_user_input`, `request_permissions`, `web_search`, and `image_generation`; the latest verifier passed that exact-set check.
- Exercise executable local tools: `requests.json` in the evidence directory records outputs for `update_plan`, `list_dir`, `grep_files`, `read_file`, `exec_command` fail/pass/artifact checks, `test_sync_tool`, `apply_patch`, interactive `exec_command` plus `write_stdin`, `js_repl`, `js_repl_reset`, `view_image`, `CronCreate`, `CronList`, and `CronDelete`.
- Account for non-executed advertised tools: `request_permissions` and `request_user_input` are covered in the core harness because `codex-exec` cannot answer its own pending client prompts; `web_search` and `image_generation` are remote built-in request specs, verified by request shape rather than local side effects.
- Complete a real coding task: the fixture starts with a Python package bug in `src/ledger_math.py`, unit tests in `tests/test_ledger_math.py`, and an image asset; Codex discovers files, reads the broken branch, reproduces the test failure, applies the source patch, reruns tests, generates an artifact file, verifies it, and emits the final message.
- Preserve failing-before-fix evidence: `summary.json` has `baseline_returncode: 1`; `tool_outputs.fail-test-call.output` records the unittest failure showing `{'total': 7, 'bonus': 0, 'net': 7}` vs `{'total': 3, 'bonus': 4, 'net': 7}`.
- Preserve fix evidence: `workspace.diff` changes the bonus branch from `total += amount` to `bonus += amount`; `tool_outputs.patch-call.output` records `M src/ledger_math.py`.
- Preserve passing-after-fix evidence: `tool_outputs.pass-test-call.output` records the same unittest suite exiting 0 with `OK`; `tool_outputs.verify-artifact-call.output` records `generated:` and `result:` contents.
- Preserve independent retest evidence: running `python3 -m unittest discover -s tests -v && grep ...` from the evidence workspace passed after the Codex run, separately from the model-driven tool call sequence.
- Preserve diff evidence: `workspace.diff` contains only text diffs for `src/ledger_math.py`, added `src/generated.txt`, and added `src/result.txt`; Python cache files are excluded from diff evidence.

## Residual Risk

- The Rust integration test source in `exec/tests/program_toolchain.rs` now completes through `cargo test` in this environment and validates the compiled `codex-exec` five-request mock Responses flow, final stdout, generated file, patched file, and recorded local tool outputs.
- The core harness now also completes through `cargo test` in this environment and validates request-permission round trips plus tool event coverage that `codex-exec` cannot answer itself.
- The scripted verifier is intentionally local: it validates request shape, tool registry, local tool execution, file effects, and final output, but it does not perform live web search or image generation backend calls.
