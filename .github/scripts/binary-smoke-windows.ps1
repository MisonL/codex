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

Invoke-BinaryHelp 'codex.exe'
Invoke-BinaryHelp 'codex-app-server.exe'
Invoke-BinaryHelp 'codex-mcp-server.exe'
Invoke-BinaryHelp 'codex-file-search.exe'
Invoke-BinaryHelp 'codex-execpolicy.exe'
Invoke-BinaryHelp 'codex-execpolicy-legacy.exe'
Invoke-BinaryHelp 'codex-stdio-to-uds.exe'
Invoke-BinaryHelp 'codex-responses-api-proxy.exe'
Invoke-BinaryHelp 'codex-tui.exe'
Invoke-BinaryHelp 'apply_patch.exe'

foreach ($binary in @('codex-windows-sandbox-setup.exe', 'codex-command-runner.exe')) {
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') $binary
  if (!(Test-Path $binaryPath)) {
    throw "$binary was not built"
  }
}

& (Join-Path (Join-Path $PWD 'target\debug') 'codex.exe') --version *> $null
