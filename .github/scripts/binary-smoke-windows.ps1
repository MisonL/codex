$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location (Join-Path $RepoRoot 'codex-rs')

$packages = @(
  'codex-cli',
  'codex-app-server',
  'codex-mcp-server',
  'codex-file-search',
  'codex-exec',
  'codex-execpolicy',
  'codex-execpolicy-legacy',
  'codex-stdio-to-uds',
  'codex-responses-api-proxy',
  'codex-tui',
  'codex-apply-patch',
  'codex-core',
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

function Invoke-FileSearchSmoke {
  $tempRoot = Join-Path $env:RUNNER_TEMP ("file-search-" + [System.Guid]::NewGuid().ToString())
  New-Item -ItemType Directory -Path (Join-Path $tempRoot 'src') -Force | Out-Null
  New-Item -ItemType Directory -Path (Join-Path $tempRoot 'target') -Force | Out-Null
  Set-Content -Path (Join-Path $tempRoot 'src\alpha_result.txt') -Value "alpha" -NoNewline
  Set-Content -Path (Join-Path $tempRoot 'target\alpha_ignored.txt') -Value "alpha" -NoNewline
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') 'codex-file-search.exe'
  try {
    Write-Host "==> codex-file-search.exe --json -C <dir> alpha --exclude target/**"
    $output = & $binaryPath --json -C $tempRoot alpha --exclude 'target/**'
    $matches = @(
      $output |
        Where-Object { $_ -and $_.Trim().Length -gt 0 } |
        ForEach-Object { $_ | ConvertFrom-Json }
    )
    $normalizedPaths = @($matches | ForEach-Object { $_.path -replace '\\', '/' })
    if ($normalizedPaths -notcontains 'src/alpha_result.txt') {
      throw "expected src/alpha_result.txt in file search output"
    }
    if ($normalizedPaths -contains 'target/alpha_ignored.txt') {
      throw "excluded file appeared in file search output"
    }
  } finally {
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
  }
}

function Invoke-ExecPolicySmoke {
  $tempRoot = Join-Path $env:RUNNER_TEMP ("execpolicy-" + [System.Guid]::NewGuid().ToString())
  $rulesPath = Join-Path $tempRoot 'policy.rules'
  New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
  Set-Content -Path $rulesPath -Value @'
prefix_rule(
    pattern = ["git", "push"],
    decision = "forbidden",
)
'@
  try {
    Write-Host "==> codex-execpolicy.exe check"
    $binaryPath = Join-Path (Join-Path $PWD 'target\debug') 'codex-execpolicy.exe'
    $output = & $binaryPath check --rules $rulesPath git push origin main | Out-String
    if ($output -notlike '*"decision":"forbidden"*') {
      throw "expected forbidden decision from codex-execpolicy"
    }
    if ($output -notlike '*"matchedPrefix":["git","push"]*') {
      throw "expected matched prefix in codex-execpolicy output"
    }
  } finally {
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
  }
}

function Invoke-ExecPolicyLegacySmoke {
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') 'codex-execpolicy-legacy.exe'
  Write-Host "==> codex-execpolicy-legacy.exe check-json"
  $output = & $binaryPath check-json '{"program":"pwd","args":[]}' | Out-String
  if ($output -notlike '*"result":"safe"*') {
    throw "expected safe result from codex-execpolicy-legacy"
  }
}

function Invoke-ConfigSchemaSmoke {
  $tempRoot = Join-Path $env:RUNNER_TEMP ("config-schema-" + [System.Guid]::NewGuid().ToString())
  $outPath = Join-Path $tempRoot 'config.schema.json'
  New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
  try {
    Write-Host "==> codex-write-config-schema.exe --out <file>"
    $binaryPath = Join-Path (Join-Path $PWD 'target\debug') 'codex-write-config-schema.exe'
    & $binaryPath --out $outPath
    $schema = Get-Content -Path $outPath -Raw | ConvertFrom-Json
    if ($schema.type -ne 'object') {
      throw "config schema should have top-level type=object"
    }
  } finally {
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
  }
}

function Invoke-ResponsesApiProxySmoke {
  $tempRoot = Join-Path $env:RUNNER_TEMP ("responses-proxy-" + [System.Guid]::NewGuid().ToString())
  $upstreamScript = Join-Path $tempRoot 'upstream.py'
  $upstreamCapture = Join-Path $tempRoot 'upstream.json'
  $upstreamPortFile = Join-Path $tempRoot 'upstream.port'
  $proxyInfo = Join-Path $tempRoot 'proxy-info.json'
  $stdinFile = Join-Path $tempRoot 'proxy-stdin.txt'
  $proxyOut = Join-Path $tempRoot 'proxy.stdout.txt'
  $proxyErr = Join-Path $tempRoot 'proxy.stderr.txt'
  New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
  Set-Content -Path $stdinFile -Value "sk_smoketest" -NoNewline
  Set-Content -Path $upstreamScript -Value @"
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import sys

capture_path = sys.argv[1]
port_path = sys.argv[2]

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length).decode("utf-8")
        with open(capture_path, "w", encoding="utf-8") as f:
            json.dump({
                "path": self.path,
                "authorization": self.headers.get("Authorization"),
                "body": body,
            }, f)
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"ok":true}')

    def log_message(self, format, *args):
        return

server = HTTPServer(("127.0.0.1", 0), Handler)
with open(port_path, "w", encoding="utf-8") as f:
    f.write(str(server.server_port))
server.serve_forever()
"@

  $upstream = $null
  $proxy = $null
  try {
    $upstream = Start-Process -FilePath python -ArgumentList @($upstreamScript, $upstreamCapture, $upstreamPortFile) -PassThru -NoNewWindow
    for ($i = 0; $i -lt 50; $i++) {
      if ((Test-Path $upstreamPortFile) -and ((Get-Content -Path $upstreamPortFile -Raw).Trim().Length -gt 0)) {
        break
      }
      Start-Sleep -Milliseconds 100
    }
    $upstreamPort = (Get-Content -Path $upstreamPortFile -Raw).Trim()

    Write-Host "==> codex-responses-api-proxy.exe forwards POST /v1/responses"
    $proxyBinary = Join-Path (Join-Path $PWD 'target\debug') 'codex-responses-api-proxy.exe'
    $proxy = Start-Process `
      -FilePath $proxyBinary `
      -ArgumentList @('--server-info', $proxyInfo, '--http-shutdown', '--upstream-url', "http://127.0.0.1:$upstreamPort/v1/responses") `
      -RedirectStandardInput $stdinFile `
      -RedirectStandardOutput $proxyOut `
      -RedirectStandardError $proxyErr `
      -PassThru `
      -NoNewWindow

    for ($i = 0; $i -lt 50; $i++) {
      if (Test-Path $proxyInfo) { break }
      Start-Sleep -Milliseconds 100
    }
    $proxyPort = ((Get-Content -Path $proxyInfo -Raw) | ConvertFrom-Json).port
    $response = Invoke-WebRequest -Uri "http://127.0.0.1:$proxyPort/v1/responses" -Method Post -ContentType 'application/json' -Body '{"input":"hello"}' -UseBasicParsing
    if ($response.Content -notlike '*"ok":true*') {
      throw "unexpected proxy response: $($response.Content)"
    }

    $captured = Get-Content -Path $upstreamCapture -Raw | ConvertFrom-Json
    if ($captured.path -ne '/v1/responses') {
      throw "unexpected upstream path: $($captured.path)"
    }
    if ($captured.authorization -ne 'Bearer sk_smoketest') {
      throw "authorization header was not forwarded correctly"
    }
    if ($captured.body -ne '{"input":"hello"}') {
      throw "unexpected upstream body: $($captured.body)"
    }

    Invoke-WebRequest -Uri "http://127.0.0.1:$proxyPort/shutdown" -UseBasicParsing | Out-Null
    Wait-Process -Id $proxy.Id -Timeout 10
  } finally {
    try { if ($proxy -and -not $proxy.HasExited) { $proxy.Kill() } } catch {}
    try { if ($upstream -and -not $upstream.HasExited) { $upstream.Kill() } } catch {}
    if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
  }
}

Invoke-BinaryHelp 'codex.exe'
Invoke-BinaryHelp 'codex-app-server.exe'
Invoke-BinaryHelp 'codex-mcp-server.exe'
Invoke-BinaryHelp 'codex-file-search.exe'
Invoke-FileSearchSmoke
Invoke-BinaryHelp 'codex-exec.exe'
Invoke-BinaryHelp 'codex-execpolicy.exe'
Invoke-ExecPolicySmoke
Invoke-BinaryHelp 'codex-execpolicy-legacy.exe'
Invoke-ExecPolicyLegacySmoke
Invoke-StdioToUdsSmoke
Invoke-BinaryHelp 'codex-responses-api-proxy.exe'
Invoke-ResponsesApiProxySmoke
Invoke-BinaryHelp 'codex-tui.exe'
Invoke-ApplyPatchSmoke
Invoke-ConfigSchemaSmoke

foreach ($binary in @('codex-windows-sandbox-setup.exe', 'codex-command-runner.exe')) {
  $binaryPath = Join-Path (Join-Path $PWD 'target\debug') $binary
  if (!(Test-Path $binaryPath)) {
    throw "$binary was not built"
  }
}

& (Join-Path (Join-Path $PWD 'target\debug') 'codex.exe') --version *> $null
