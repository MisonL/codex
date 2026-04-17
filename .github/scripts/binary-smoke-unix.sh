#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}/codex-rs"

build_packages=(
  codex-cli
  codex-app-server
  codex-mcp-server
  codex-file-search
  codex-exec
  codex-execpolicy
  codex-execpolicy-legacy
  codex-stdio-to-uds
  codex-responses-api-proxy
  codex-tui
  codex-apply-patch
  codex-core
)

if [[ "$(uname -s)" == "Linux" ]]; then
  build_packages+=(codex-linux-sandbox)
fi
if [[ "$(uname -s)" != "Linux" || -n "${CI:-}" ]]; then
  build_packages+=(codex-shell-escalation)
fi

build_args=()
for pkg in "${build_packages[@]}"; do
  build_args+=(-p "${pkg}")
done

cargo build --all-features --bins "${build_args[@]}"

run_help() {
  local binary="$1"
  shift
  echo "==> ${binary} $*"
  "target/debug/${binary}" "$@" >/dev/null
}

run_stdio_to_uds_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  local socket_path="${tmp_dir}/socket"

  python3 - "$socket_path" <<'PY' &
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
PY
  local server_pid=$!

  # Wait briefly for the server to bind before connecting.
  for _ in {1..50}; do
    [[ -S "$socket_path" ]] && break
    sleep 0.1
  done

  echo "==> codex-stdio-to-uds <socket-path>"
  printf 'request' | target/debug/codex-stdio-to-uds "$socket_path" >"${tmp_dir}/stdout.txt"
  wait "$server_pid"
  grep -Fx "response" "${tmp_dir}/stdout.txt" >/dev/null
  rm -rf "$tmp_dir"
}

run_apply_patch_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  local patch
  patch="$(cat <<'PATCH'
*** Begin Patch
*** Add File: smoke.txt
+hello
*** End Patch
PATCH
)"

  echo "==> apply_patch <patch>"
  (
    cd "$tmp_dir"
    "${repo_root}/codex-rs/target/debug/apply_patch" "$patch" >/dev/null
  )
  grep -Fx "hello" "${tmp_dir}/smoke.txt" >/dev/null
  rm -rf "$tmp_dir"
}

run_file_search_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  mkdir -p "${tmp_dir}/src" "${tmp_dir}/target"
  printf 'alpha\n' >"${tmp_dir}/src/alpha_result.txt"
  printf 'alpha\n' >"${tmp_dir}/target/alpha_ignored.txt"

  echo "==> codex-file-search --json -C <dir> alpha --exclude target/**"
  target/debug/codex-file-search --json -C "$tmp_dir" alpha --exclude 'target/**' >"${tmp_dir}/matches.jsonl"
  grep -F '"path":"src/alpha_result.txt"' "${tmp_dir}/matches.jsonl" >/dev/null
  if grep -F '"path":"target/alpha_ignored.txt"' "${tmp_dir}/matches.jsonl" >/dev/null; then
    echo "excluded file appeared in search results" >&2
    exit 1
  fi
  rm -rf "$tmp_dir"
}

run_execpolicy_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  local rules="${tmp_dir}/policy.rules"
  cat >"$rules" <<'RULES'
prefix_rule(
    pattern = ["git", "push"],
    decision = "forbidden",
)
RULES

  echo "==> codex-execpolicy check"
  target/debug/codex-execpolicy check --rules "$rules" git push origin main >"${tmp_dir}/execpolicy.json"
  grep -F '"decision":"forbidden"' "${tmp_dir}/execpolicy.json" >/dev/null
  grep -F '"matchedPrefix":["git","push"]' "${tmp_dir}/execpolicy.json" >/dev/null
  rm -rf "$tmp_dir"
}

run_execpolicy_legacy_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"

  echo "==> codex-execpolicy-legacy check-json"
  target/debug/codex-execpolicy-legacy check-json '{"program":"pwd","args":[]}' >"${tmp_dir}/legacy.json"
  grep -F '"result":"safe"' "${tmp_dir}/legacy.json" >/dev/null
  rm -rf "$tmp_dir"
}

run_config_schema_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"

  echo "==> codex-write-config-schema --out <file>"
  target/debug/codex-write-config-schema --out "${tmp_dir}/config.schema.json"
  python3 - "${tmp_dir}/config.schema.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    schema = json.load(f)
if schema.get("type") != "object":
    raise SystemExit("config schema should be a JSON object schema")
PY
  rm -rf "$tmp_dir"
}

run_responses_api_proxy_smoke() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  local upstream_capture="${tmp_dir}/upstream.json"
  local upstream_port_file="${tmp_dir}/upstream.port"
  local proxy_info="${tmp_dir}/proxy-info.json"
  local upstream_pid=""
  local proxy_pid=""

  cleanup_responses_api_proxy_smoke() {
    [[ -n "${proxy_pid:-}" ]] && kill "$proxy_pid" >/dev/null 2>&1 || true
    [[ -n "${upstream_pid:-}" ]] && kill "$upstream_pid" >/dev/null 2>&1 || true
    rm -rf "$tmp_dir"
  }
  trap cleanup_responses_api_proxy_smoke RETURN

  cat >"${tmp_dir}/upstream.py" <<'PY'
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
PY

  python3 "${tmp_dir}/upstream.py" "$upstream_capture" "$upstream_port_file" &
  upstream_pid=$!

  for _ in {1..50}; do
    [[ -s "$upstream_port_file" ]] && break
    sleep 0.1
  done
  local upstream_port
  upstream_port="$(cat "$upstream_port_file")"

  echo "==> codex-responses-api-proxy forwards POST /v1/responses"
  printf 'sk_smoketest\n' | target/debug/codex-responses-api-proxy \
    --server-info "$proxy_info" \
    --http-shutdown \
    --upstream-url "http://127.0.0.1:${upstream_port}/v1/responses" \
    >"${tmp_dir}/proxy.out" 2>"${tmp_dir}/proxy.err" &
  proxy_pid=$!

  for _ in {1..50}; do
    [[ -s "$proxy_info" ]] && break
    sleep 0.1
  done
  local proxy_port
  proxy_port="$(python3 - "$proxy_info" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    print(json.load(f)["port"])
PY
)"

  curl -fsS \
    -X POST \
    -H 'Content-Type: application/json' \
    --data '{"input":"hello"}' \
    "http://127.0.0.1:${proxy_port}/v1/responses" \
    >"${tmp_dir}/proxy-response.json"
  grep -F '"ok":true' "${tmp_dir}/proxy-response.json" >/dev/null
  python3 - "$upstream_capture" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    captured = json.load(f)
if captured["path"] != "/v1/responses":
    raise SystemExit(f"unexpected path: {captured['path']}")
if captured["authorization"] != "Bearer sk_smoketest":
    raise SystemExit("authorization header was not forwarded correctly")
if json.loads(captured["body"]) != {"input": "hello"}:
    raise SystemExit(f"unexpected body: {captured['body']}")
PY

  curl -fsS "http://127.0.0.1:${proxy_port}/shutdown" >/dev/null || true
  wait "$proxy_pid" 2>/dev/null || true
  kill "$upstream_pid" >/dev/null 2>&1 || true
  trap - RETURN
  rm -rf "$tmp_dir"
}

run_help codex --help
run_help codex-app-server --help
run_help codex-mcp-server --help
run_help codex-file-search --help
run_file_search_smoke
run_help codex-exec --help
run_help codex-execpolicy --help
run_execpolicy_smoke
run_help codex-execpolicy-legacy --help
run_execpolicy_legacy_smoke
run_stdio_to_uds_smoke
run_help codex-responses-api-proxy --help
run_responses_api_proxy_smoke
run_help codex-tui --help
run_apply_patch_smoke
run_config_schema_smoke

if [[ -x target/debug/codex-execve-wrapper ]]; then
  run_help codex-execve-wrapper --help
fi

if [[ -x target/debug/codex-linux-sandbox ]]; then
  run_help codex-linux-sandbox --help
fi

run_help codex --version
