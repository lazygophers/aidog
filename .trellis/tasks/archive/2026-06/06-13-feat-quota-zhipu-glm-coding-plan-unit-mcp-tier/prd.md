# feat(quota): Zhipu GLM coding plan 增强

## 目标
智谱 Coding Plan 查询增强：修复分类精度 + 新增 MCP tier + 前端展示重置时间。

## 变更清单

### 1. 后端 `quota.rs` — Zhipu 分类改用 `unit` 字段
- 优先 `unit=3` → five_hour, `unit=6` → weekly_limit
- 缺失 `unit` 时回退 nextResetTime 排序（兼容老响应）
- 新增 `TIME_LIMIT` 解析 → `name: "mcp_monthly"`, 含 `limit/remaining` 绝对量

### 2. 后端 `quota.rs` — Kimi resets_at
- 已有 `resets_at` 字段 ✅，无需改动

### 3. 前端 `Platforms.tsx` — Tier 展示增强
- `tierLabel` 支持 `"mcp_monthly"` → `"MCP/月"`
- 显示 `resets_at` 倒计时（X 天 X 小时后 / X 小时 X 分钟后）
- MCP tier 显示绝对量（used / total 次）

## 不改
- `QuotaTier` 结构体（已有 `limit/remaining/resets_at`）
- Kimi / 其他平台查询逻辑
