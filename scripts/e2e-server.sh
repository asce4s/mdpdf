#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export LEPTOS_OUTPUT_NAME=mdpdf-app
export LEPTOS_SITE_ROOT=target/site
export MDPDF_POOL_SIZE=2
if [[ ! -f target/site/pkg/mdpdf-app.wasm ]]; then
  echo 'Build the UI first with cargo leptos build (or npm run dev).' >&2
  exit 1
fi
exec "${MDPDF_SERVER_BIN:-target/debug/mdpdf-app}"
