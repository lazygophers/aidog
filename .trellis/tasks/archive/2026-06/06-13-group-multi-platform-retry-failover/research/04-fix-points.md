# Research: 改造点清单 + 风险

- **Query**: 重试循环放哪? 401/403 检测+状态联动写哪? max_retries 存哪? bool→enum 迁移影响面? 与现有 failover 关系?
- **Scope**: internal
- **Date**: 2026-06-13

## 改造点清单（按层）

### 1. 重试循环（核心，proxy.rs）
- **位置**：`proxy.rs:644` 单次 `select_platform` → 改为循环。需要 router 暴露"候选平台有序列表"（现 `select_platform` 只返回 1 个，见 01）。
  - 方案 A：router 新增 `select_platform_candidates()` 返回 `Vec<RouteResult>`（按 priority/权重排序），proxy 逐个尝试。
  - 方案 B：proxy 内每次循环重新调 `select_platform` 并传"已尝试 platform_id 排除集"。
- **循环体边界**：必须包住上游 `send()`（`proxy.rs:826-854`）+ 非成功判定（`proxy.rs:869-885`）。
- **避坑**：循环前已有多个提前 return 分支（manual_budget 402 `proxy.rs:709`、Mock `:749`、ClaudeCode 透传 `:767`）——这些是"选定平台后的旁路"，重试循环需在选平台之后、但要决定这些分支是否参与重试（Mock/透传通常不重试）。
- **流式分支**（`proxy.rs:920+`）：重试更难——一旦开始 SSE 流就不能换平台。**首字节前失败可重试，已开始流式不可重试**，这是设计难点。

### 2. 401/403 检测 + 自动禁用联动（proxy.rs:869）
- 在 `if !status.is_success()` 块（`proxy.rs:869-885`）内判 `status == 401 || 403` → 调新 DB 方法把该平台标记 auto_disabled。
- 需新 db 函数（仿 `db.rs:299-314` update_platform 写 enabled），或专门 `set_platform_status(id, AutoDisabled)`。

### 3. 平台状态 bool→enum（影响面最大）
迁移波及（详见 02-platform-status.md）：
- **DB**：`migrations/001_init.sql:16` + `init_tables`（`db.rs:82-109`）加 migration；`PLATFORM_COLUMNS`（`db.rs:78`）、`row_to_platform`（`db.rs:152`）、create/update（`db.rs:214/282/299-314`）、group_platform JOIN（`db.rs:847/870`）。
- **models.rs**：`Platform.enabled`（`:340`）、`UpdatePlatform.enabled`（`:405`）。
- **router.rs**：`:117` `.find(|gp| gp.platform.enabled)`、`:124` `.filter(...enabled)` → 改为状态过滤（auto_disabled 也排除）。
- **前端**：`api.ts:164/325`、`Platforms.tsx:1146/1283-1287/1807/2521`（toggle + 灰显 + 计数）。
- **兼容**：旧库 0/1 数据迁移；"用户手动禁用"与"自动禁用"必须可区分（否则自动恢复会误开用户主动关的平台）。

### 4. 最大重试次数配置（存储位置 = 决策点）
- 候选位置：
  - **group 级**：`"group"` 表加列（仿 `request_timeout_secs` `001_init.sql:36`）+ `Group`/`CreateGroup`/`UpdateGroup`（`models.rs:411-475`）。语义清晰（不同分组不同策略）。
  - **全局 setting**：`setting` 表（scope/key/value，`001_init.sql:56`）+ 仿 ProxyLogSettings 模式。简单但一刀切。
- 现有超时配置是 group 级（`request_timeout_secs`），与重试同属"分组路由策略"，group 级更一致。**需 main 决策**。

### 5. proxy_log 多尝试记录（存储结构 = 决策点）
- 见 03：JSON 数组列 `attempts`（低侵入，复用单行 upsert）vs `proxy_log_attempt` 子表（规范化，可单独查询/统计）。
- 列表页"重试次数"需 `ProxyLogSummary` 加 `attempt_count`；详情需 `attempts` 明细。

## 与现有 failover routing_mode 的关系

- 现 `RoutingMode::Failover`（`models.rs:141-144`、`router.rs:111-120`）是**静态优先级选 1 个**，不是运行时失败切换。
- **建议复用其 priority 排序逻辑**作为重试候选顺序（failover 模式 → 按 priority 依次重试；load_balance → 加权随机选下一个未试平台），**但需新增"候选列表 + 逐个尝试"机制**，而非现有"选 1 个就返回"。
- 取舍：新需求的"重试"是正交于 routing_mode 的运行时能力。可设计为：routing_mode 决定候选**排序**，max_retries 决定候选**尝试上限**。是否这样耦合需 main 拍板。

## 主要风险

1. **流式请求重试**：SSE 一旦开始无法回滚换平台（首字节前 vs 后的边界处理）。
2. **bool→enum 迁移**：旧库兼容 + "手动禁用 vs 自动禁用"区分（决定自动恢复是否误开）。
3. **自动禁用恢复机制缺失**：需求只说"自动禁用"，**没说怎么恢复**（手动？定时探活？冷却 N 分钟后自动恢复？）——若不恢复，平台一旦 401/403 永久不可用，整组可能逐个被禁直到全挂。**强决策点**。
4. **多尝试写库与现有单行 upsert 架构冲突**：需选 JSON 列 or 子表。
5. 重试放大上游压力 / 成本（每次重试都是真实上游请求，可能多计费）。

## 需 main 问用户的决策点（汇总）

- **`需要:` 最大重试次数配置级别** — group 级（与现 timeout 一致）还是全局 setting？
- **`需要:` 自动禁用的恢复机制** — 永久禁用待手动恢复 / 冷却定时自动恢复 / 探活恢复？（不定则平台会逐个永久失效）
- **`需要:` 每次尝试记录的存储结构** — proxy_log 加 `attempts` JSON 列 还是建 `proxy_log_attempt` 子表？
- **`需要:` 平台状态建模** — bool `enabled` 改 enum，还是保留 enabled + 新增独立 `auto_disabled`/`status` 列（区分用户禁用 vs 自动禁用）？
- **`需要:` 流式请求是否参与重试**（仅首字节前？还是流式不重试）？
