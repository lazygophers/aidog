# implement.md: 平台高峰期禁用开关 disable_during_peak

> child of peak-hours-multiplier。依赖父 task 已落 `PeakWindow` / `resolve_multiplier` / bundled preset 解析 / `platform.extra.peak_hours` 读取。
> 本 task 加开关 + 路由排除 + 整组失败落库 + 前端 3 处标记。

## 执行层

- 载体: main 派 trellis-implement subagent（跨 Rust↔TS）
- worktree: 无
- 并行: 禁（共享 candidate_state + extra 类型）
- 依赖: peak-hours-multiplier（父 task ship 后启动）
- 门禁: `cargo build && cargo clippy && cargo test` + `yarn build` + `check-i18n.mjs`

## 改动清单

### 步骤 1 — Rust is_in_peak_window helper（D1）

`src-tauri/src/gateway/estimate/peak_hours.rs`（父 task 已建）：从 `resolve_multiplier` 拆出命中判定：

```rust
/// first-match 命中任一窗口（跨天 + days_of_week）→ true。空/无命中 → false。
pub fn is_in_peak_window(windows: &[PeakWindow], epoch_ms: i64) -> bool {
    let (hour, weekday) = utc_hour_weekday(epoch_ms);
    windows.iter().any(|w| hit(w, hour, weekday))
}
```

`resolve_multiplier` 内部调 `is_in_peak_window` 取首个命中窗口 multiplier（保父 task 行为不回归）。

### 步骤 2 — Rust extra serde（D2）

`src-tauri/src/gateway/models.rs` Platform extra 解析（参照 peak_hours 字段位置）加：

```rust
#[serde(default)]
pub disable_during_peak: Option<bool>,
```

### 步骤 3 — candidate_state 高峰禁用维度（D3）

`src-tauri/src/gateway/router/mod.rs:50`，expires_at 检查之后加：

```rust
// 高峰禁用（与 status 正交，临时闸门，不改 status 三态）
if platform.extra_disable_during_peak().unwrap_or(false) {
    let windows = peak_hours::platform_peak_hours(&platform.extra, &platform.platform_type);
    if peak_hours::is_in_peak_window(&windows, now_ms) {
        return None;
    }
}
```

`platform_peak_hours` = 父 task 的混合源（extra → preset default → 空）。

### 步骤 4 — 单平台组 bypass 覆盖（D4）

`src-tauri/src/gateway/router/candidates.rs:67-83` 单平台组分支：在「无视 status 必请求」之前，先跑高峰禁用检查（同 D3 逻辑）；命中则不纳入 → 返 NoCandidate。

注意：**仅高峰禁用维度覆盖 bypass**，status 维度照旧 bypass（单平台组 auto_disabled / 熔断仍必请求）。两维度独立，单元测试明确。

### 步骤 5 — route fail 落 proxy_log（D5）

`src-tauri/src/gateway/proxy/handler.rs:370` route fail 路径：

1. candidates 返结构化原因：加 `ExcludedReason` 枚举（`Status` / `Expired` / `CircuitBreaker` / `PeakDisabled` / `NoMatch`）
2. route fail 时若所有候选被 `PeakDisabled` 排除 → 落 proxy_log：
   ```rust
   log.blocked_by = "router";
   log.blocked_reason = "peak_hours";
   log.status_code = 503;
   // est_cost 保持 0
   ```
3. 其他原因（无候选 / 模型不匹配）照现状 warn log 不落库

### 步骤 6 — TS 类型 + helper（D6）

`src/services/api/types/part1.ts` Platform extra 解析加 `disable_during_peak?: boolean`。

新建 `src/utils/peakHours.ts`：

```ts
import type { PeakWindow } from "../domains/platforms/defaults";

/** first-match 命中任一窗口（跨天 + days_of_week）→ true。与 Rust is_in_peak_window 对称。 */
export function isCurrentlyPeak(windows: PeakWindow[] | undefined, nowMs: number): boolean {
  if (!windows?.length) return false;
  const d = new Date(nowMs);
  const hour = d.getUTCHours();
  const weekday = d.getUTCDay(); // 0=Sun…6=Sat
  return windows.some((w) => hit(w, hour, weekday));
}

function hit(w: PeakWindow, hour: number, weekday: number): boolean {
  if (w.days_of_week && !w.days_of_week.includes(weekday)) return false;
  return w.end_hour > w.start_hour
    ? hour >= w.start_hour && hour < w.end_hour
    : hour >= w.start_hour || hour < w.end_hour;
}
```

### 步骤 7 — 列表徽标（D7）

`src/pages/platforms/PlatformListView.tsx`：每张平台卡片，若 `platform.extra.disable_during_peak && isCurrentlyPeak(platform.extra.peak_hours, Date.now())` → 显「高峰禁用中」徽标（实时，每分钟刷新或聚焦时重算）。

### 步骤 8 — 编辑表单预览（D8）

`src/pages/platforms/formSections.tsx`：PeakHoursSection 加 `disable_during_peak` 开关 toggle + 旁显「当前: 高峰期 / 非高峰期」（基于 `isCurrentlyPeak` + 该平台 peak_hours，实时）。

### 步骤 9 — Groups 指示（D9）

`src/pages/Groups.tsx` 平台状态列：同 D7 逻辑加「高峰禁用中」标记。

### 步骤 10 — i18n（D10）

`src/locales/*.json`（8 个）补 key：`disable_during_peak` / `disable_during_peak_desc` / `currently_peak` / `currently_off_peak` / `peak_disabled_badge`。

### 步骤 11 — 文档（D11）

- `CLAUDE.md` 路由段补：disable_during_peak 正交维度（不改 status，临时闸门；单平台组不 bypass）
- `.wiki/modules/pricing.md` 补：disable_during_peak 语义 + 与 peak_hours 关系 + 整组失败落库说明

## 自检

`✅ lint=clippy无warn type=yarn build过 test=cargo test全过 TODO=0 验收物=disable_during_peak 全链路（extra serde + candidate_state 接入 + 单平台组 bypass 覆盖 + route fail 落库 + 前端 3 处标记 + i18n + 文档）`

## 失败处理

- cargo test 红：candidates 单平台组测试 — 确认高峰禁用优先级高于 status bypass，两维度独立测试
- route fail 落库失败：检查 ExcludedReason 枚举 + handler.rs 分支条件
- TS 判定与 Rust 不对称：cross-layer review helper 逻辑（跨天 + days_of_week）
- 前端徽标不刷新：加 visibility change / focus 监听重算 now
