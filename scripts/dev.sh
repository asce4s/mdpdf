#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export LEPTOS_OUTPUT_NAME=mdpdf-app
export LEPTOS_SITE_ROOT=target/site
export LEPTOS_SITE_PKG_DIR=pkg
if [[ -x /tmp/mdpdf-tools/bin/cargo-leptos ]]; then
  MDPDF_LEPTOS=/tmp/mdpdf-tools/bin/cargo-leptos
else
  MDPDF_LEPTOS=$(command -v cargo-leptos || true)
fi
if [[ -z "$MDPDF_LEPTOS" ]]; then
  echo 'Install the pinned build tool: cargo install cargo-leptos --version 0.3.6 --locked' >&2
  exit 1
fi
npm run vendor
"$MDPDF_LEPTOS" build
exec target/debug/mdpdf-app
