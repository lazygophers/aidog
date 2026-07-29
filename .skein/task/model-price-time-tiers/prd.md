# 模型单价时间维度化 — PRD (主入口)

## 目标

用户原话：「@src-tauri/defaults 的配置修复，glm 的两倍不应该作为高峰期配置，而是模型价格设置的一部分」+「注意关联的业务逻辑也要做修复」。

`src-tauri/defaults/platform-presets.json:103` 的 glm_coding `peak_hours` 数组第 2 个窗口
`{start_hour:0, end_hour:24, multiplier:2.0, models:["glm-5.2","glm-5-turbo"], start_at:1790784000}`（下称 **W2**）
是**永久性涨价**被表达成了「全天高峰窗口」。语义污染的代价不是理论的 ——
`start_at: 1790784000` = **2026-09-30T16:00:00Z**，今天惰性（`period_active` 跳过，
`gateway/peak_hours.rs:156-166`），**2026-10-01 00:00 CST 起自动翻转**：

- [x] 计费全天 ×2.0（`gateway/billing.rs:106`）—— 这是本意
- [x] 开了 `disable_during_peak` 的 glm_coding 平台 **24/7 被路由排除**
   （`router/mod.rs:51-76` → `candidates.rs:236-241` `Err("peak_disabled")`
   → `handler.rs:385-387` 落 `blocked_reason='peak_hours'` 503）—— 可用性硬伤
- [x] 客户端直发 `glm-5.2`/`glm-5-turbo` 时被 `models.peak` 分支全天降级到 glm-4.7
   （`candidates.rs:539-558`）
- [x] 前端高峰徽标 24/7 常亮（`PlatformCard.tsx:263,280-307`；`isCurrentlyPeak` 默认
   `requestModel=""` 跳过 model 过滤，`peakHours.ts:138`）
- [x] 平台表单默认模型列表显示 peak 分支（`useProtocolMeta.ts:77-81`）

成功长相：涨价表达在**模型价格配置**里（`price_data.time_tiers`，带生效时刻），
`peak_hours` 只留真高峰（W1 工作日 UTC 06-10 ×3.0）；三条成本计算路径口径一致；
全新安装无需手点同步也能拿到真实价格。

## 边界

### 用户 2026-07-29 拍板（契约锁定）

- [x] **方案 B**：`price_data` 加 `time_tiers`，不改表结构（`price_data` 是 JSON blob，
      `schema_early.rs:35-44`，零 migration）
- [x] **二维叠价 = time 选表 → context 分档**：`time_tiers` 条目是一张**完整价表**
      （base 价 + 可选自带 `context_tiers`）。apply 顺序 `apply_time_tier` → `apply_context_tier`。
      理由：glm-5-turbo 既有 32k 长文档又要时段涨价，扁平 time 条目表达不了二维，
      长文请求会被抹回 base 涨价价（比现 32k 档还低）
- [x] **取消 glm_coding 全天 `models.peak` 降级**：删 W2 后仅 W1（工作日 UTC 06-10）仍降级
- [x] **devin ACU 路径保持现状**：`proxy/devin.rs:394/436/907` 直写 `est_cost = acus_consumed`，
      不叠 peak 倍率（ACU 是厂商实际计量），仅加 `// ponytail:` 注释锁设计意图
- [x] **models.json bundled 兜底做读侧**：不做启动 seed、不比 `generated_at` 版本仲裁
- [x] **一并修 3 个关联问题**：`maybe_auto_sync` 接回生产 / 存量 `extra.peak_hours` W2 副本清理 /
      前端 `isCurrentlyPeak` + `start_at` 护栏测试

### 范围内

1. **删 W2**：`platform-presets.json:103` glm_coding `peak_hours` 数组第 2 元素，保留 W1
2. **`time_tiers` 机制**：`gateway/db/model_price.rs` 新增 `apply_time_tier` +
   `resolve_price` 加 `now_ms: i64` 入参（`<= 0` = 无时间上下文，跳过；复用
   `est_cost_from:98` 的 `created_at_ms <= 0` 早退约定）
3. **`models.json` 写入 glm-5.2 / glm-5-turbo 的 `time_tiers`**（现价 ×2，`start_at` 沿用 1790784000）
4. **`models.json` bundled 兜底**：`include_str!` + `OnceLock`，在 `resolve_price` 的
   `pd` 取值处做「DB 无该模型 → bundled entry」两级回退（后 3 档回退链一行不改，
   天然继承 tier 逻辑）
5. **estimate 链补 peak 倍率**：`gateway/estimate/db_ops.rs` 的余额扣减（`:214`）与
   手动预算（`:233`）共用一个 `resolved_price`，一处乘算同时修好（既存 bug）
6. **`maybe_auto_sync` 接回生产**：`gateway/price_sync.rs:158` 现标 `#[allow(dead_code)]`，
   生产调用点 0 —— UI 上「自动同步价格」开关能点但从不触发
7. **存量 `extra.peak_hours` W2 副本清理**：用户点过「导入默认配置」（`formSections.tsx:596-600`）
   会把 preset 窗口复制进 `platform.extra`，删 bundled 清不掉
8. **devin ACU 注释** + **前端 `isCurrentlyPeak` + `start_at` 护栏测试**

### 范围外

- [x] 不加 `time_tiers` 前端可视化（照 `context_tiers` 先例 —— `grep -rn "context_tiers" src/`
      零命中，`PricingTab.tsx:468-470` 展示的是 `price_data_to_summary`
      （`model_price.rs:23-44`）读的 top-level 基准价，新字段落 blob 里天然被忽略）
- [x] 不做 `generated_at` 版本仲裁（bundled 纯读侧兜底，DB 恒优先）
- [x] 不做启动 seed（读侧兜底覆盖同样场景，代码少一个数量级）
- [x] 不改 `model_price` 表结构（仅第 7 项存量 extra 清理是数据 migration）
- [x] 不给 `model_price_resolve` 命令加 `at_ms` 时段预览参数（YAGNI）

### 已知约束

- [x] **Rust 只读 bundled，前端读 app-data merged** —— Rust `default_peak_hours` →
      `presets_cache::presets()` → `include_str!`（`presets_cache.rs:10`）；前端
      `getDefaultPeakHours` → `get_defaults_json`（`defaults.rs:19-58`）= app data
      `~/.aidog/platform-presets.json` 优先 + bundled deep merge，且 deep merge 只补缺失的
      protocol key（`:36-41`），glm_coding 已存在 → **app data 的旧数组整体保留**。
      Rust 侧重编即生效，前端等 `defaults_sync` 拉新版。存在窗口期，最终自愈。
- [x] `platform-presets.json` 是**手维护真值源**，禁机器生成覆盖
- [x] `resolve_price` 签名变更波及 9 处调用点（3 生产 + 6 测试）；改 `aidog_core` 公开签名
      必须跑 `cargo test --workspace`（memory `cargo-workspace-gate-not-single-crate`）
- [x] `estimate/db_ops.rs:196` 刻意传 `fallback_input=0.0, fallback_output=0.0`
      （「未知模型不扣余额」），**禁改调 `calc_est_cost`** —— 后者用 `settings.fallback_*`
      （默认 3.0 $/M）会开始静默偷扣预估余额，且每请求多两次 DB 查询

## 验收标准

- [x] `platform-presets.json` glm_coding `peak_hours` 只剩 W1（工作日 UTC 06-10 ×3.0），
      `last_updated` 已刷新
- [x] `apply_time_tier` 已实现：按 `start_at <= now_ms` 选最大的一档，命中后整体换价表，
      再跑 `apply_context_tier`（顺序 time→context），`source` 带 `+time` 后缀
- [x] `resolve_price` 加 `now_ms: i64`，`<= 0` 跳过 time_tiers；9 处调用点全部补参
      （`billing.rs:40` 传 `created_at_ms`；`estimate/db_ops.rs:196` 与
      `platform_cmd/price.rs:60` 传 `now()`；6 处测试传 `0`）
- [x] `models.json` 的 glm-5.2 / glm-5-turbo 已新建 `pricing.glm_coding` 节点
      （base 三价逐位等于各自 top-level 现价）并在其下加 `time_tiers`
      （`start_at: 1790784000`，价格 = 现价 ×2；glm-5-turbo 的 time 条目内嵌
      `context_tiers` 32k 档 ×2）。**普通 `glm` 协议价格不变。**
- [x] bundled 兜底生效：DB 无该模型 → 读 `include_str!` 的 models.json entry 重跑回退链；
      DB 有 → DB 赢
- [x] `estimate/db_ops.rs` 余额扣减与手动预算**均已乘 peak 倍率**，且**未**改调 `calc_est_cost`
- [x] `maybe_auto_sync` 有真实生产调用点（`#[allow(dead_code)]` 已删），
      UI「自动同步价格」开关实际生效
- [x] 存量 `platform.extra.peak_hours` 里 W2 形状的窗口已被 migration 清除
- [x] `proxy/devin.rs` 三处 ACU 赋值已加注释说明不叠 peak 倍率
- [x] 前端新增 `isCurrentlyPeak` + `start_at` 护栏测试（未到生效时刻不命中 / 到点后命中）
- [x] 新增 Rust 测试：`apply_time_tier` 三件套（选档 / 无档透传 / 部分字段覆盖）
      + bundled 兜底 2 例 + estimate 倍率生效 1 例
- [x] 门禁全绿：`cargo test --workspace` / `cargo clippy` 零 warning /
      `npx tsc --noEmit` / `yarn test`

## 索引

- [x] 详细设计: [design.md](design.md)
- [x] 任务/子任务/调度: task.json (`skein subtask list model-price-time-tiers`)
