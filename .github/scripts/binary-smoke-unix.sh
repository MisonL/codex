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

run_help codex --help
run_help codex-app-server --help
run_help codex-mcp-server --help
run_help codex-file-search --help
run_help codex-exec --help
run_help codex-execpolicy --help
run_help codex-execpolicy-legacy --help
run_help codex-stdio-to-uds --help
run_help codex-responses-api-proxy --help
run_help codex-tui --help
run_help apply_patch --help

if [[ -x target/debug/codex-linux-sandbox ]]; then
  run_help codex-linux-sandbox --help
fi

run_help codex --version
