#!/usr/bin/env bash
# One-time developer setup for macOS/Linux.
set -euo pipefail
corepack enable
pnpm install
cargo fetch
