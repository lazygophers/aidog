#!/usr/bin/env bash
# check-test-isolation.sh — 测试隔离 lint 守卫（防再犯）
#
# 扫所有 test_*.rs / *_test.rs 文件，禁止三类违规：
#   (1) 裸 dirs::home_dir() / dirs::config_dir()（不在 HomeGuard 上下文）
#       例外：test_support.rs 自身（HomeGuard 定义所在，需用 dirs 解析 tempdir）
#   (2) Command::new("(python3|uv|node|git|sh)").{spawn,output,status}()
#       真实 spawn 外部进程，破坏测试隔离
#   (3) reqwest::{get, Client::new().*send}() — 真实出站连接
#
# 注释行（// 或 /// 开头）不计违规。
# 修复后本脚本扫出 0 违规。新增违规时 exit 1 + 列出 file:line。
#
# 手动跑：bash src-tauri/.ci/check-test-isolation.sh
# 挂 CI：在 test job 前置 `bash src-tauri/.ci/check-test-isolation.sh`
#   （本仓库暂无 ci.yml test job，见 .ci/README.md 手动跑说明）
set -euo pipefail

# 解析 src-tauri 根：脚本可能在任意 cwd 被调用，按相对自身路径定位。
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$SCRIPT_DIR/../src"

if [ ! -d "$SRC_DIR" ]; then
    echo "ERROR: src dir not found at $SRC_DIR" >&2
    exit 2
fi

# 过滤注释行：去前导空白后以 // 开头（含 /// doc 注释）即跳过。
strip_comments() {
    awk -F: '{
        line=$0;
        # 去 path:lineno: 前缀后取内容判断
        n=split(line, a, ":");
        if (n >= 3) {
            content="";
            for (i=3; i<=n; i++) content = content (i>3?":":"") a[i];
            gsub(/^[[:space:]]+/, "", content);
            if (content ~ /^\/\//) next;
        }
        print line;
    }'
}

violations=""

# ── (1) 裸 dirs::home_dir() / dirs::config_dir() ──
dirs_hits="$(grep -rn --include="test_*.rs" --include="*_test.rs" \
    -E 'dirs::(home_dir|config_dir)' "$SRC_DIR" \
    | grep -v '/test_support.rs:' \
    | strip_comments || true)"
if [ -n "$dirs_hits" ]; then
    violations="${violations}== (1) 裸 dirs::home_dir/config_dir（须包 HomeGuard）==\n${dirs_hits}\n\n"
fi

# ── (2) Command::new("(python3|uv|node|git|sh)").{spawn,output,status} ──
# 先取含 Command::new("xxx") 的行，再判断同行是否调 .spawn/.output/.status。
spawn_hits="$(grep -rn --include="test_*.rs" --include="*_test.rs" \
    -E 'Command::new\("(python3|uv|node|git|sh)"\)' "$SRC_DIR" \
    | strip_comments \
    | grep -E '\.(spawn|output|status)\s*\(' || true)"
if [ -n "$spawn_hits" ]; then
    violations="${violations}== (2) spawn 真实外部进程（须 mock / #[ignore]）==\n${spawn_hits}\n\n"
fi

# ── (3) reqwest::{get, Client::new().*send} 真实出站 ──
# reqwest::get(...) 本身即真实出站；Client::new()(...).send() 同理（须 .send 收尾）。
reqwest_hits="$(grep -rn --include="test_*.rs" --include="*_test.rs" \
    -E 'reqwest::get\s*\(' "$SRC_DIR" \
    | strip_comments || true)"
client_hits="$(grep -rn --include="test_*.rs" --include="*_test.rs" \
    -E 'reqwest::Client::new\s*\(\s*\)' "$SRC_DIR" \
    | strip_comments \
    | grep -E '\.send\s*\(' || true)"
reqwest_hits="${reqwest_hits}${client_hits:+$'\n'"$client_hits"}"
if [ -n "$reqwest_hits" ]; then
    violations="${violations}== (3) reqwest 真实出站连接（须 stub server）==\n${reqwest_hits}\n\n"
fi

if [ -n "$violations" ]; then
    echo "FAIL: test-isolation 违规（详见 .trellis/tasks/07-01-test-isolation-fix/prd.md）：" >&2
    printf "%b" "$violations" >&2
    exit 1
fi

echo "OK: test-isolation 检查通过（0 违规）"
