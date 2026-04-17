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
)

if [[ "$(uname -s)" == "Linux" ]]; then
  build_packages+=(codex-linux-sandbox)
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

run_help codex --help
run_help codex-app-server --help
run_help codex-mcp-server --help
run_help codex-file-search --help
run_help codex-exec --help
run_help codex-execpolicy --help
run_help codex-execpolicy-legacy --help
run_stdio_to_uds_smoke
run_help codex-responses-api-proxy --help
run_help codex-tui --help
run_help apply_patch --help

if [[ -x target/debug/codex-linux-sandbox ]]; then
  run_help codex-linux-sandbox --help
fi

run_help codex --version
