# 实施计划 — platform-expiry

决策见 `prd.md`「已定决策」。核心: 独立 `expires_at` 字段 + `candidate_state` 路由排除, 不动 PlatformStatus 三态枚举。

## S1 — 后端 (Rust)

### S1.1 Migration 034
`src-tauri/src/gateway/db/schema_late.rs` `run_migrations_late` 末尾 (Migration 033 之后) 加:
```rust
// Migration 034: platform 过期时间（0 = 永不过期；>0 到期后路由排除 + purge 清理）。
// 幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column。
let _ = conn.execute(
    "ALTER TABLE platform ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0",
    [],
);
```
同步更新文件头注释 `Migrations 021–034`。

### S1.2 Platform struct
`src-tauri/src/gateway/models/platform.rs` `Platform` 在 `auto_disable_strikes` 后加:
```rust
/// 过期时间（毫秒 unix 时间戳，0 = 永不过期）；>0 且 now>=expires_at 时路由排除（等效自动禁用，
/// 但不改 status 枚举；改值清空/延后即恢复）。独立维度，与 status 正交。
#[serde(default)]
pub expires_at: i64,
```

### S1.3 CreatePlatform / UpdatePlatform
同文件 (第二页，读 fullContent page 2 确认精确位置) 加:
- `CreatePlatform`: `#[serde(default)] pub expires_at: Option<i64>` (None=默认 0；Some(t)=设)
- `UpdatePlatform`: `#[serde(default)] pub expires_at: Option<i64>` (None=不动；Some(0)=清空；Some(t)=设)

### S1.4 DB row 映射
`src-tauri/src/gateway/db/` 下 platform load/select 函数 (grep `auto_disable_strikes` 定位 row→struct 映射处，可能 platform.rs / platform_crud.rs)：SELECT 列加 `expires_at`，`row.get` 映射到 struct。

### S1.5 platform_create / platform_update SQL
- `INSERT INTO platform (... expires_at ...) VALUES (... :expires_at ...)` — create 用 `input.expires_at.unwrap_or(0)`
- `UPDATE platform SET ... expires_at = ? ...` — update 仅当 `Some(v)` 时写入 (动态拼 SET 子句，与现有 Option 字段同模式)

### S1.6 candidate_state 排除
`src-tauri/src/gateway/router/mod.rs:48` 入口加:
```rust
pub(crate) fn candidate_state(platform: &Platform, now_ms: i64) -> Option<bool> {
    // 过期平台直接排除（等效自动禁用，独立于 status 枚举）
    if platform.expires_at > 0 && now_ms >= platform.expires_at {
        return None;
    }
    match platform.status { ... }  // 原逻辑不变
}
```
更新函数 doc 注释加「过期」维度说明。

### S1.7 purge 扩展
`src-tauri/src/gateway/db/platform_lifecycle.rs:90` `purge_auto_disabled_platforms`:
- 全局 (None 分支, ~L110): SQL 加 `OR (expires_at > 0 AND expires_at < ?now)`，now 用现有 `now()` helper:
  ```sql
  SELECT id FROM platform
  WHERE deleted_at = 0 AND (status = 'auto_disabled' OR (expires_at > 0 AND expires_at < ?1))
  ```
  绑定 `now()`。
- 分组级 (Some(gid) 分支, ~L155): 同理在 `SELECT id FROM platform WHERE id IN (...) AND deleted_at = 0` 加 `OR (expires_at > 0 AND expires_at < ?N)`。注意占位符编号随 pids 动态 +1。

### S1.8 测试
- `src-tauri/src/gateway/router/test_mod.rs`: 加 `candidate_state` 过期用例 (expires_at 未来 → Some; 过去 → None; 0 → 不影响)。
- `src-tauri/src/gateway/db/test_platform_lifecycle.rs`: 加过期平台被 purge 删除用例 (全局 + 分组级)。
- 测试固件 (test_platform.rs / test_support.rs / test_db_ops.rs / test_selection.rs / test_candidates.rs): `CreatePlatform` 字面量加 `expires_at: None` (与 default_level_priority 同批改法，注意 perl 误伤 UpdatePlatform — 用精确 Edit)。
- `cargo clippy --all-targets -- -D warnings` + `cargo test` 全绿。

## S2 — 前端 (TS/React)

### S2.1 api.ts 类型
`src/services/api.ts`:
- `Platform` 接口加 `expires_at: number` (0 = 无)
- `CreatePlatformInput` / `UpdatePlatformInput` 加 `expires_at?: number` (undefined=不动；0=清空；>0=设)

### S2.2 Platforms.tsx 表单
`src/pages/Platforms.tsx`:
- state: `const [expiresAt, setExpiresAt] = useState<number>(0)` (0=空)
- handleEdit 回填: `setExpiresAt(editing.expires_at ?? 0)`
- handleSave: create/update 传 `expires_at: expiresAt || undefined` (0 → undefined 不传，避免覆盖；延后/清空语义由后端 None 处理。注意: 清空 = 传 0; 用户从有值改 0 需显式传 0 → 用 dirty 检测或始终传 expiresAt number)
  - **决策**: 始终传 `expires_at: expiresAt` (number, 0 或时间戳)。create 用 `expires_at: expiresAt`；update 用 `expires_at: expiresAt` (Some(0) 清空)。UpdatePlatform Option 语义对齐。
- FormSection UI: datetime-local input + 清空按钮。非必填。i18n key `expiresAt` / `expiresAtPlaceholder` / `expiresAtClear`。

### S2.3 PlatformCard 展示
`src/components/platforms/PlatformCard.tsx`:
- `expires_at > 0 && now*1000 >= expires_at` → 红色「已过期」badge (类似 auto_disabled 展示位)
- `expires_at > 0 && 未过期` → 小字「MM-DD HH:MM 到期」(用 formatters.ts 既有格式化；临近 24h 内高亮)

### S2.4 SmartPasteModal 回填
`src/components/platforms/SmartPasteModal.tsx:49` parsed 结果: 读回填表单逻辑 (grep `parsed.apiKeys` / `setBaseUrl` 定位 onConfirm/灌表单处)，加 `parsed.expiresAt` → 调父级 setExpiresAt 或 form 回填通道 (与现有 baseUrl/apiKey 回填同路径)。

### S2.5 platformPaste.ts 解析
`src/utils/platformPaste.ts`:
- `ParsedPaste` 类型加 `expiresAt: number | null` (null=未识别)
- 空 text 返回里加 `expiresAt: null`
- 加 `extractExpiryAt(text): number | null`:
  - 模式 (宽松，用户裁定): `MM-DD HH:MM` / `MM-DD` / `YYYY-MM-DD` / `YYYY-MM-DD HH:MM`
  - 补全年份为当年；解析出的日期已过 (当年该日 < 今天) → 推次年
  - 回退保护: 时间早于 `now - 7d` (明显历史日期) → null
  - 多候选: 优先取靠近「过期/到期/exp/expire/有效期」语义词的；无语义词取第一个有效未来日期
  - 返回毫秒时间戳
- parsePlatformPaste return 加 `expiresAt: extractExpiryAt(text)`
- platformPaste.test.ts 加用例: 「即将过期 06-28 23:59」识别；历史日期跳过；多日期取近语义词；无语义词取首个未来日期

### S2.6 i18n + 验证
- 8 locale 文件 (src/locales/*.json) 加 `expiresAt` / `expiresAtPlaceholder` / `expiresAtClear` / `expired` 4 key
- `yarn build` (tsc + vite) 全绿
- `node scripts/check-i18n.mjs` 零缺失

## 执行顺序

单 agent 顺序 S1 → S2 (字段名已在 prd 定契约，无跨层歧义；避免多 worktree 合并开销)。完成后 main 跑 check。

## 验收 (全过才算完)

1. cargo test + clippy 全绿
2. yarn build 全绿
3. check-i18n 零缺失
4. router 单测覆盖过期排除
5. purge 单测覆盖过期清理
6. platformPaste 单测覆盖宽松时间识别
