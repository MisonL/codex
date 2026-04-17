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
  $tempRoot = Join-Path $env:RUNNER_TEMP ("codex-stdio-to-uds-" + [System.Guid]::NewGuid().ToString())
  $socketPath = Join-Path $tempRoot 'socket'
  $stdoutPath = Join-Path $tempRoot 'stdout.txt'
  $stderrPath = Join-Path $tempRoot 'stderr.txt'
  New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

  $pythonScript = @'
import socket
import sys

socket_path = sys.argv[1]
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(socket_path)
server.listen(1)
connection, _ = server.accept()
received = bytearray()
while True:
    chunk = connection.recv(65536)
    if not chunk:
        break
    received.extend(chunk)
if bytes(received) != b"request":
    raise SystemExit(f"unexpected request: {bytes(received)!r}")
connection.sendall(b"response")
connection.close()
server.close()
'@

  $server = Start-Process -FilePath python -ArgumentList @('-c', $pythonScript, $socketPath) -PassThru -NoNewWindow -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
  try {
    for ($i = 0; $i -lt 50; $i++) {
      if (Test-Path $socketPath) {
        break
      }
      Start-Sleep -Milliseconds 100
    }

    Write-Host "==> codex-stdio-to-uds.exe <socket-path>"
    $output = "request" | & (Join-Path (Join-Path $PWD 'target\debug') 'codex-stdio-to-uds.exe') $socketPath
    Wait-Process -Id $server.Id
    if ($server.ExitCode -ne 0) {
      $stderr = if (Test-Path $stderrPath) { Get-Content $stderrPath -Raw } else { '' }
      throw "stdio-to-uds server failed: $stderr"
    }
    if ($output.Trim() -ne 'response') {
      throw "unexpected stdio-to-uds output: $output"
    }
  } finally {
    try { if ($server -and -not $server.HasExited) { $server.Kill() } } catch {}
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
Invoke-BinaryHelp 'apply_patch.exe'

foreach ($binary in @('codex-windows-sandbox-setup.exe', 'codex-command-runner.exe')) {
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') $binary
  if (!(Test-Path $binaryPath)) {
    throw "$binary was not built"
  }
}

& (Join-Path (Join-Path $PWD 'target\debug') 'codex.exe') --version *> $null
