## Hodexctl on Windows PowerShell: install, PATH, and graceful-error verification

This note is for validating the Windows PowerShell UX fix where failing commands
(for example `hodexctl list`) should not print PowerShell stack traces, script
paths, line/char, `CategoryInfo`, or `FullyQualifiedErrorId`.

Expected behavior on failure:

- Print a short, user-friendly English error message.
- Exit code is 1.

### Quick glossary

- Controller script: `scripts/hodexctl/hodexctl.ps1`
- Installer: `scripts/install-hodexctl.ps1`
- Wrapper: `hodexctl.cmd` in the command directory, calls the controller script.

### Install from a fork/branch (single line)

The PowerShell installer supports testing a fork and a branch/tag/commit via
environment variables:

- `HODEXCTL_REPO` = `<owner>/<repo>` (default `stellarlinkco/codex`)
- `HODEX_CONTROLLER_REF` = git ref used to download scripts (default `main`)
- `HODEX_STATE_DIR` = state root (default `%LOCALAPPDATA%\hodex`)
- `HODEX_COMMAND_DIR` = wrapper dir (default `<state_dir>\commands`)
- `HODEXCTL_NO_PATH_UPDATE=1` disables writing the user PATH.

#### Recommended: persistent state dir + PATH update enabled

This is best when you want `hodexctl` to work in new terminals.

```powershell
$env:HODEXCTL_REPO="MisonL/codex"; $env:HODEX_CONTROLLER_REF="fix/hodexctl-ps1-graceful-error"; $env:HODEX_STATE_DIR="$env:LOCALAPPDATA\hodex-test"; Remove-Item Env:HODEXCTL_NO_PATH_UPDATE -ErrorAction SilentlyContinue; irm https://raw.githubusercontent.com/MisonL/codex/fix/hodexctl-ps1-graceful-error/scripts/install-hodexctl.ps1 | iex
```

Notes:

- This is a single line.
- It writes to the user PATH (registry User Path), not the system PATH.
- Windows Terminal often needs a full app restart to pick up PATH changes (a new
  tab is not always enough).

#### Isolated: temp dirs + skip PATH update

Use this when you want zero PATH changes and do not care about new terminals.

```powershell
$env:HODEXCTL_REPO="MisonL/codex"; $env:HODEX_CONTROLLER_REF="fix/hodexctl-ps1-graceful-error"; $env:HODEX_STATE_DIR="$env:TEMP\hodex-test\state"; $env:HODEX_COMMAND_DIR="$env:TEMP\hodex-test\commands"; $env:HODEXCTL_NO_PATH_UPDATE="1"; irm https://raw.githubusercontent.com/MisonL/codex/fix/hodexctl-ps1-graceful-error/scripts/install-hodexctl.ps1 | iex
```

Run via wrapper:

```powershell
& "$env:TEMP\hodex-test\commands\hodexctl.cmd" status
```

### Verify "graceful error" UX

Pick a failure that is guaranteed to fail.

Option A (repo parsing failure):

```powershell
hodexctl list -Repo not/a-repo
$LASTEXITCODE
```

Option B (missing required argument):

```powershell
hodexctl downgrade
$LASTEXITCODE
```

Expected:

- Output is a short English error message.
- Output does NOT contain any of these strings:
  - `At line:`
  - `CategoryInfo`
  - `FullyQualifiedErrorId`
- `$LASTEXITCODE` is `1`.

### "hodexctl is not recognized" troubleshooting

1) If you used `HODEXCTL_NO_PATH_UPDATE=1`, this is expected in new terminals.

- Fix: reinstall without it (recommended), or add the command dir to PATH
  yourself, or run the wrapper directly.

2) If you used `HODEX_STATE_DIR` / `HODEX_COMMAND_DIR` only as session env vars,
new terminals will not inherit them.

- Fix: use the defaults, or pick a persistent directory (for example
  `%LOCALAPPDATA%\hodex-test`) and reinstall.

3) PATH updated, but a new tab/window still does not see it.

- Fix: fully exit the terminal app and reopen it (Windows Terminal is the usual
  culprit).
- Validate in the same session:
  - `Get-Command hodexctl`
  - `hodexctl status`

If PATH is still broken, `hodexctl repair` is the recommended self-heal.

### GitHub API failures (root cause, not the UX fix)

The graceful-error fix does not solve the real reason `list` fails. Common
causes:

- Anonymous GitHub API rate limiting (HTTP 403).
  - Mitigation: set `GITHUB_TOKEN`, or install GitHub CLI and run `gh auth login`.
- `gh` missing / not logged in / no permission.
- Proxy/VPN/TLS interception issues that break GitHub API requests.

