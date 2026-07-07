# implement.md: codex 协议切 OpenAI Responses API

> 配合 PRD。1 文件 1 字段改 + 验证。

## 执行层

- 载体: trellis-implement 内联直做
- worktree: 默认（flow 强制）
- 并行: 禁
- 门禁: cargo build + cargo test + json.tool + grep

## 改动清单

### 步骤 1 — preset codex protocol 改（D1）

`src-tauri/defaults/platform-presets.json` 内 `protocols.codex.endpoints.default[0].protocol`:

```json
// 改前
"protocol": "openai"

// 改后
"protocol": "openai_responses"
```

仅此 1 字段。不改 base_url / client_type / model / name / desc。

### 步骤 2 — 门禁（D2）

```bash
# JSON 有效
python3 -m json.tool src-tauri/defaults/platform-presets.json > /dev/null

# codex 条目 protocol 已改
python3 -c "
import json
d = json.load(open('src-tauri/defaults/platform-presets.json'))
ep = d['protocols']['codex']['endpoints']['default'][0]
assert ep['protocol'] == 'openai_responses', f\"got {ep['protocol']}\"
print('codex protocol =', ep['protocol'])
"

# Rust 编译 + 测试不回归
cd src-tauri && cargo build 2>&1 | tail -3
cd src-tauri && cargo test --quiet 2>&1 | tail -5

# 无其他 codex 硬编码 openai（defaults_sync / 前端 defaultClientForProtocol）
grep -rn "codex" src-tauri/src/commands/defaults_sync.rs src/domains/platforms/defaults.ts 2>/dev/null | grep -i "openai\b" || echo "no hardcoded codex→openai"
```

全 exit 0 + codex protocol = openai_responses。

### 步骤 3 — 存量 migration（open decision，start 前定）

依 PRD Open Decision 用户裁定：
- 方案 A（推荐）: 跳过本步
- 方案 B: 加 DB migration（schema_early.rs 或 mod.rs，查当前最大 migration 号 +1），UPDATE platform endpoint codex 的 protocol openai→openai_responses
- 方案 C: 前端 Platforms 页 codex platform 显示提示（protocol=openai 时建议改）

## 自检

`✅ lint=clippy无new warn type=cargo build过 test=cargo test不回归 TODO=0 验收物=preset codex protocol=openai_responses + 全链路门禁过`

## 失败处理

- cargo test 回归: 查是否硬编码 codex→openai 的测试（test_schema.rs:211 migration_012 kimi codex_tui 是 client_type 非 protocol，应不影响）；定点修
- JSON 无效: 仅改 protocol 值，禁动结构
