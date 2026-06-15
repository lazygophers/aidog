# 使用统计：平台筛选语义错配（有数据但筛空）

## 根因
`query_stats_inner`（`db.rs:2585`）的"平台"维度全程误用 `target_protocol` 字段（入站协议名 "openai"/"anthropic"），而前端筛选器 value = `platform.platform_type`（展示名 "DeepSeek"）：

1. **filter**: `target_protocol = ?N` ← value 是展示名，字段是协议名 → 永不匹配 → 筛空。
2. **dimension** `group_by=platform` → `GROUP BY target_protocol`：维度表"按平台"实际按入站协议聚合，名字也对不上前端平台列表。

正确语义：平台 = `proxy_log.platform_id`（auto 分组 `platform_id=0` 经 `group.auto_from_platform` 回溯 = `eff_pid`，现成片段见 `db.rs:1074`）。

## 方案（单一交付，跨 4 文件）
**重命名 `filter_protocol` → `filter_platform`，语义=platform_id；filter + dimension 改用 eff_pid 子查询。**

### 后端
1. `models.rs:1104`: `filter_protocol` → `filter_platform: Option<String>`（值=platform_id 十进制字符串）
2. `db.rs` `QueryParams` + `query_stats_inner`:
   - 字段重命名
   - WHERE filter 改为：子查询 eff_pid = CAST(?N AS INTEGER)
   - dimension `group_by=platform` → GROUP BY eff_pid，子查询 JOIN `platform` 取 name 作 dimension name
   - 抽 eff_pid 子查询为内联片段（两处复用同模式）
3. `lib.rs:746`: StatsQuery 透传，字段名跟随（serde 自动）

### 前端
4. `api.ts:1107`: `filter_protocol` → `filter_platform`
5. `Stats.tsx`:
   - state `filterProtocol` → `filterPlatform`
   - `allProtocols` → `allPlatforms = platforms.map(p => ({ value: String(p.id), label: p.name }))`
   - invoke `filter_protocol` → `filter_platform`
   - 维度 groupBy=platform 时 dimension name 现已是 platform 真名

## 验证
- `cargo clippy` + `cargo test`（db 测试）+ `npx tsc --noEmit` 全过无 warning。
- 视觉：选某平台 → 该平台数据正常显示（不再空）；按平台维度 → 列出真实平台名。

## 非目标
- 不动 group/model 筛选（语义已对）。
- 不改 dimension model/group 维度。
