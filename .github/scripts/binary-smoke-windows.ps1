$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location (Join-Path $RepoRoot 'codex-rs')

$packages = @(
  'codex-cli',
  'codex-app-server',
  'codex-mcp-server',
  'codex-file-search',
  'codex-execpolicy',
  'codex-execpolicy-legacy',
  'codex-stdio-to-uds',
  'codex-responses-api-proxy',
  'codex-tui',
  'codex-apply-patch',
  'codex-windows-sandbox'
)

$buildArgs = @('build', '--all-features', '--bins')
foreach ($package in $packages) {
  $buildArgs += @('-p', $package)
}

& cargo @buildArgs

function Invoke-BinaryHelp {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
  )

  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') $Binary
  Write-Host "==> $Binary --help"
  & $binaryPath --help *> $null
}

function Invoke-StdioToUdsSmoke {
  Write-Host "==> cargo test -p codex-stdio-to-uds --test stdio_to_uds"
  & cargo test -p codex-stdio-to-uds --test stdio_to_uds pipes_stdin_and_stdout_through_socket -- --exact
}

function Invoke-ApplyPatchSmoke {
  $tempRoot = Join-Path $env:RUNNER_TEMP ("apply-patch-" + [System.Guid]::NewGuid().ToString())
  $applyPatchBinary = Join-Path (Join-Path $PWD 'target\debug') 'apply_patch.exe'
  $patch = @'
*** Begin Patch
*** Add File: smoke.txt
+hello
*** End Patch
'@
  New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

  try {
    Write-Host "==> apply_patch.exe <patch>"
    Push-Location $tempRoot
    & $applyPatchBinary $patch *> $null
    Pop-Location

    $smokeFile = Join-Path $tempRoot 'smoke.txt'
    if ((Get-Content $smokeFile -Raw).Trim() -ne 'hello') {
      throw 'apply_patch did not write expected content'
    }
  } finally {
    if ((Get-Location).Path -eq $tempRoot) {
      Pop-Location
    }
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
  }
}

Invoke-BinaryHelp 'codex.exe'
Invoke-BinaryHelp 'codex-app-server.exe'
Invoke-BinaryHelp 'codex-mcp-server.exe'
Invoke-BinaryHelp 'codex-file-search.exe'
Invoke-BinaryHelp 'codex-execpolicy.exe'
Invoke-BinaryHelp 'codex-execpolicy-legacy.exe'
Invoke-StdioToUdsSmoke
Invoke-BinaryHelp 'codex-responses-api-proxy.exe'
Invoke-BinaryHelp 'codex-tui.exe'
Invoke-ApplyPatchSmoke

foreach ($binary in @('codex-windows-sandbox-setup.exe', 'codex-command-runner.exe')) {
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') $binary
  if (!(Test-Path $binaryPath)) {
    throw "$binary was not built"
  }
}

& (Join-Path (Join-Path $PWD 'target\debug') 'codex.exe') --version *> $null
