# implement.md: platform peak_hours（UI + 估算接入）

> 配合 PRD 范围升级（schema-only → 全链路）。跨 Rust↔TS 三层，串行。

## 执行层

- 载体: main 派 trellis-implement subagent（跨边界，jsonl 上下文注入）
- worktree: 无（局部、单仓）
- 并行: 禁（共享 calc_est_cost 签名 + extra 类型，串行）
- 门禁: `cargo build && cargo clippy && cargo test` + `yarn build` + `node scripts/check-i18n.mjs` + `python3 -m json.tool`

## 改动清单

### 步骤 1 — 前端类型（D1）

`src/domains/platforms/defaults.ts:14-27` `DefaultsDoc` protocol 条目加：

```ts
/** 高峰/低峰时段倍率（多窗口，UTC+0 基准）。
 *  preset 给 per-protocol 默认；用户覆盖存 platform.extra.peak_hours。
 *  absent / 空数组 = 无调整（multiplier 1.0）。
 *  多窗口 first-match wins。跨天: end_hour < start_hour（半开 [start,end)）。 */
peak_hours?: PeakWindow[];
```

文件顶部导出 `PeakWindow` 类型（共享给 formSections / api types）：

```ts
export type PeakWindow = {
  start_hour: number;   // 0-23 UTC+0，含
  end_hour: number;     // 0-23 UTC+0，不含；<start 表跨天
  multiplier: number;   // >0；>1 加价 / <1 折扣
  days_of_week?: number[];  // 0=Sun…6=Sat；absent=每天
};
```

### 步骤 2 — preset JSON 占位（D2）

`src-tauri/defaults/platform-presets.json` **不加任何协议实际值**（schema-only 占位，留待用户按官方定价页手填）。校验合法即可。

### 步骤 3 — Rust PeakWindow + bundled preset 解析（D3）

新增 `src-tauri/src/gateway/estimate/peak_hours.rs`（或并入 `models.rs`，按现有分模块习惯）：

```rust
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct PeakWindow {
    pub start_hour: i32,
    pub end_hour: i32,
    pub multiplier: f64,
    pub days_of_week: Option<Vec<i32>>,
}

/// bundled preset 缓存解析（include_str! 同源 platform-presets.json，禁抄第二份真值）。
static PRESETS: OnceLock<serde_json::Value> = OnceLock::new();

/// t 的 UTC hour 命中窗口？跨天 (end<start): hour>=start || hour<end；同天: [start,end)。
fn hit(w: &PeakWindow, hour: i32, weekday: i32) -> bool {
    if let Some(days) = &w.days_of_week {
        if !days.contains(&weekday) { return false; }
    }
    if w.end_hour > w.start_hour {
        hour >= w.start_hour && hour < w.end_hour
    } else {
        hour >= w.start_hour || hour < w.end_hour
    }
}

/// first-match multiplier；空/无命中 = 1.0。
pub fn resolve_multiplier(windows: &[PeakWindow], epoch_ms: i64) -> f64 {
    // 用 chrono 或 unix_epoch → UTC (hour, weekday)
    let (hour, weekday) = utc_hour_weekday(epoch_ms);
    for w in windows {
        if hit(w, hour, weekday) { return w.multiplier; }
    }
    1.0
}

/// 按 protocol 查 bundled preset 默认（用户 extra 缺失时回退）。
pub fn default_peak_hours(protocol: &str) -> Vec<PeakWindow> { /* OnceLock 解析 */ }
```

`utc_hour_weekday`：项目已有 chrono 依赖则用 `DateTime::<Utc>::from_timestamp_millis`；无则手算（epoch_ms / 86400000 算 day，模 7 对齐 1970-01-01=Thursday）。先 grep 确认依赖。

### 步骤 4 — calc_est_cost 接入（D4）

`src-tauri/src/gateway/db/stats_today.rs:184` 签名加两参：

```rust
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    platform_id: i64,        // 新：查 platform.extra.peak_hours
    created_at_ms: i64,      // 新：请求 UTC 时间戳
) -> f64 {
    let base = /* 原 resolve_price 计算，不动 */;
    // 查 peak_hours：platform.extra → preset default → 1.0
    let windows = platform_peak_hours(db, platform_id, platform_type).await;
    base * resolve_multiplier(&windows, created_at_ms)
}
```

`platform_peak_hours`：读 `platform.extra` JSON 解析 peak_hours 字段；空/缺 → `default_peak_hours(platform_type)`。

### 步骤 5 — 3 调用点签名同步（D5）

- `src-tauri/src/gateway/proxy/log.rs:45`：传 `log.platform_id, log.created_at`
- `src-tauri/src/gateway/proxy/log.rs:106`：传 `cols.platform_id, cols.created_at`（确认 cols 含此二字段，缺则从 log 行取）
- `src-tauri/src/gateway/proxy/mock.rs:99`：mock 无真实平台，传 `(0, now_ms)` — mock multiplier 恒 1.0（无 platform.extra，preset 查 "mock" 返空）

### 步骤 6 — extra 类型对齐（D6）

- Rust `models.rs`：Platform extra 解析（参照 manual_budgets / Breaker 模式）加 `peak_hours` serde 字段
- TS `api/types/part1.ts`：Platform extra 解析加 `peak_hours?: PeakWindow[]`（import 自 defaults.ts）

### 步骤 7 — UI（D7）

`src/pages/platforms/formSections.tsx` 加 PeakHoursSection：
- 时区 toggle（前端态，默认本地，切换 UTC+0）：`Intl.DateTimeFormat().resolvedOptions().timeZone` 取本地
- 窗口列表：每行 start_hour (0-23) + end_hour (0-23) + multiplier (number) + days_of_week chips (7 个 SMTWTFS) + remove
- add window 按钮
- 展示换算：选中时区下，UTC 存值 ±offset 显示（保存回 UTC）
- 空数组 = 不写 extra.peak_hours（absent）

`src/pages/platforms/usePlatformForm.ts`：extra 读写加 peak_hours 序列化/反序列化 + UTC↔本地换算工具（用 `Intl.DateTimeFormat` 算 offset，禁手写偏移表）。

### 步骤 8 — i18n（D8）

`src/locales/*.json`（8 个）补 key：`peak_hours` / `peak_window` / `peak_hours_desc` / `start_hour` / `end_hour` / `multiplier` / `days_of_week` / `timezone_display` / `timezone_utc` / `timezone_local` / `add_window` / `remove_window`。

### 步骤 9 — 文档（D9）

- `CLAUDE.md` 「平台默认配置 (platform-presets.json)」段末加：peak_hours 可选字段说明 + 指针到 wiki
- `.wiki/modules/pricing.md` 加「peak_hours schema + 估算接入」段：schema + 判定伪码 + 混合源（extra→preset→1.0）+ first-match

## 自检

`✅ lint=clippy无warn type=yarn build过 test=cargo test全过 TODO=0 验收物=peak_hours 全链路（preset schema + Rust 估算接入 + 3 调用点 + UI 编辑 + 时区切换 + i18n 8 locale + 文档）`

## 失败处理

- cargo test 红（test_algo/test_db_ops）：先看是否 calc_est_cost 新签名漏改 mock fixture；mock.rs 必传 2 新参
- TS 编译错：PeakWindow import 路径 / extra 字段可选性
- check-i18n 红：对照新增 key 表逐 locale 补
- 时区换算偏差：确认用 `Intl.DateTimeFormat` 取 offset（DST 安全），禁硬编码
- Rust 读 bundled preset 解析炸：OnceLock 初始化失败回退空数组（=1.0），禁 panic
