# Stats 请求趋势改曲线 + 分钟级粒度自动降级

## 需求
1. 使用统计（Stats 页）的「请求趋势」从**柱状图改平滑曲线**（同首页 Home 的平滑曲线观感）。
2. 当所选范围**小时数据太少**时，**自动降级**展示更细粒度（5 分钟 / 1 分钟），让稀疏数据也有有意义走势。

## 现状（已核 file:line）
- 前端 `src/pages/Stats.tsx`：趋势是柱状（:343-395，buckets.map→bars，maxReq 归一）；granularity state `"daily"|"hourly"`（:90），today→hourly、7d/30d→daily 联动（:91）。
- `granularity` 类型 `src/services/api.ts:1103` = `"hourly"|"daily"`。
- 后端 `db.rs:2607` 时间分桶：`time_fmt = match granularity { Some("hourly")=>"%Y-%m-%d %H:00", _ =>"%Y-%m-%d"(daily) }`，SQL `strftime(time_fmt, created_at/1000,'unixepoch')` GROUP BY（:2645）。`StatsQuery.granularity: Option<String>`（models.rs:1100，已是 String 灵活）。
- 首页平滑曲线 `smoothPath`（Catmull-Rom→bezier）在 `src/pages/Home.tsx`（:200 附近）——**可抽到共享 util 给 Stats 复用**。

## 设计

### 后端 db.rs（加粒度，小改）
- `:2607` time_fmt match 增 arms：
  - `Some("minute")` → `"%Y-%m-%d %H:%M:00"`
  - `Some("5min")` → 5 分钟向下取整桶。strftime 无直接 5min floor，用 epoch 运算：bucket key = `(created_at/1000/300*300)`，SELECT `strftime('%Y-%m-%d %H:%M', created_at/1000/300*300, 'unixepoch')`（floor 到 300s）。
  - 其余保持 hourly/daily。
- 加 Rust 单测：minute/5min 分桶产出正确桶数（合成几条不同分钟的日志断言）。

### api.ts
- `granularity?: "hourly"|"daily"|"minute"|"5min"`（:1103 union 扩）。

### Stats.tsx
- **趋势柱状 → 平滑曲线**：复用共享 `smoothPath`（见下）渲染折线+面积（单 accent + 语义色，沿用首页观感），保留 hover tooltip / 峰值 / x 轴标注。
- **自动降级粒度**（仅短范围，防长范围爆桶）：
  - 拉到 hourly buckets 后，数**非空桶**（total_requests>0）。
  - 若范围 ≤ 24h（today preset 等）且非空小时桶 < 阈值（如 <4）→ 自动重查更细粒度：先 5min；若仍很稀疏 / 范围极短（如 ≤2h）→ 1min。
  - **护栏**：7d/30d 等长范围**绝不**降到 minute（桶数爆炸）——自动降级只对 ≤24h 范围生效；minute 仅当范围 ≤ 数小时。granularity 自动选择后在 UI 标注当前粒度（如「粒度：5 分钟（自动）」），用户知情。
  - 不引入新手动选项（按用户选「自动降级」）；现有 hourly/daily 联动保留，自动降级在其上叠加。
- 边界：曲线点数 <3 退直线；全 0 空态不变。

### 共享 smoothPath（DRY）
- 把 Home.tsx 的 `smoothPath`（+ clampY）抽到 `src/utils/`（如 `chart.ts`）或 `src/components/shared/`，Home.tsx + Stats.tsx 都 import。**Home.tsx 改为 import（行为不变，纯重构）**。

## 约束/范围
- 本任务在 **worktree 隔离** 执行（避开并发会话对 Stats.tsx/db.rs 的改动）。改：db.rs / api.ts / Stats.tsx / Home.tsx（仅抽 smoothPath 重构）/ 新建 chart util / i18n（粒度标注文案 8 locale）。
- 复用单 --accent/语义色/formatters，禁图表库/硬编码 hex/紫渐变/假数据。
- 加 i18n 后 Counter 查重。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（含 minute/5min 分桶新测）。
- `yarn build` + `yarn check:i18n` 过；locale 无新增重复。
- 行为：Stats 趋势为平滑曲线（非柱状）；today 数据稀疏时自动降到 5min/1min 并 UI 标注粒度；长范围不降 minute；hover/峰值/空态正常；Home 曲线行为不变（仅重构 import）。

## 失败处理
- 5min epoch floor SQL 桶 key 与 hourly 格式差异致前端 x 轴解析问题 → x 轴标注按粒度适配（minute/5min 标 HH:MM）。
- 长范围误降 minute → 严格护栏（范围时长上限判断）。
- worktree 合并冲突（并发会话改了同文件）→ 主会话合并时处理，禁强解。
- 门禁红修到绿；卡住标 `需要:`。
