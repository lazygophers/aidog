#!/usr/bin/env bash
# 构建 release aidog-kernel → src-tauri/target/release/aidog-kernel
set -euo pipefail; source "$(dirname "$0")/env.sh"
cargo build --release -p aidog_kernel --manifest-path "$REPO/src-tauri/Cargo.toml"
ls -la "$KERNEL_BIN"
