#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export GHIDRA_INSTALL_DIR="$project_root/tools/ghidra_12.1.4_PUBLIC"
exec npx --yes rea-agents@6.7.0 "$@"
