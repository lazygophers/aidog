# 执行计划：06-16-logs-status-code

base_branch: `next`（见 task.json）。worktree_path: null，建议主仓直接改或 `.trellis/worktrees/06-16-logs-status-code`。

## 现状锚点（file:line）

- `src/pages/Logs.tsx:315` — Meta grid「状态」格：`<MetaItem label={t("logs.status", "状态")} value={`${detail.status_code}`} highlight={detail.status_code === 200 ? "ok" : "err"} />`
- `src/pages/Logs.tsx:325` — attempts 门槛：`{detail.attempts && detail.attempts.length > 1 && (`
- `src/pages/Logs.tsx:334-360` — attempts 行渲染：复用 `a.status_code === 0 ? connFailed : a.status_code`、`ok = 2xx` 配色逻辑
- `src/pages/Logs.tsx:354-356` — 上游单行 status 渲染范式（直接拷贝配色 + 0→connFailed 兜底）
- `src/services/api.ts:649` — `upstream_status_code: number`（必返回）
- `src/services/api.ts:660` — `attempts: ProxyAttempt[]`（单次成功长度 1）
- `src-tauri/src/gateway/db.rs:1926,1929,1936` — 详情 API 已映射三字段（无需后端改动）
- `src/locales/zh-CN.json:318-368` — `logs.*` 现有 key；缺 `upstreamStatus` / `notCaptured`

## 改动文件清单

- `src/pages/Logs.tsx`（仅详情视图，列表区零 diff）
- `src/locales/{zh-CN,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json` × 8

## subtask 拆分

### ST1 — i18n 8 语言补 key（可独立先跑，无依赖）

- **目标**：在 8 个 locale 文件的 `logs.*` 区段插入 `upstreamStatus` + 可选 `notCaptured`。
- **产出**：
  - `logs.upstreamStatus`：上游状态 / Upstream status / حالة المنبع / Statut amont / Upstream-Status / Статус восходящего потока / 上游ステータス / Estado ascendente
  - `logs.notCaptured`（兜底占位，`upstream_status_code` 为 0 / null / undefined 时显示）：未捕获 / Not captured / 未捕捉 / غير ملتقط / Nicht erfasst / Не зафиксирован / 未取得 / No capturado
    - 若实现侧选择复用 `logs.connFailed`（连接失败）则此 key 可省；推荐**新增** `notCaptured`，语义更准（0 也可能是上游未返回而非连接失败）。
- **验证**：
  - 8 文件 JSON parse 通过、无尾逗号
  - `node scripts/check-i18n.mjs`（若存在）通过；无 missing key
  - 翻译语义一致（由翻译对照表统一核对）
- **资源**：现有 locale 文件、记忆条 `frontend-i18n-coverage`
- **依赖**：无
- **并行**：与 ST2 无文件交集，可并行

### ST2 — Logs.tsx 详情页改动（依赖 ST1 的 key 命名，但可先按预定 key 名编码后联调）

- **目标**：Meta grid 加 upstream chip；attempts 条件 `> 1` → `>= 1`。
- **产出**：
  1. **Meta chip**（在 Logs.tsx:315 现有「状态」`MetaItem` 之后插入一行，复用 `MetaItem` 组件签名 `label / value / highlight`）：
     ```tsx
     <MetaItem
       label={t("logs.upstreamStatus", "上游状态")}
       value={
         detail.upstream_status_code === 0 || detail.upstream_status_code == null
           ? t("logs.notCaptured", "未捕获")
           : `${detail.upstream_status_code}`
       }
       highlight={
         detail.upstream_status_code === 0 || detail.upstream_status_code == null
           ? undefined
           : detail.upstream_status_code >= 200 && detail.upstream_status_code < 300 ? "ok" : "err"
       }
     />
     ```
     - 严格沿用 `MetaItem` 现有 `highlight` 协议（Logs.tsx:315 用法："ok" / "err" / undefined）。
     - 不引入新组件；不动 grid 列数（auto-fill minmax 160px 自适应）。
  2. **attempts 门槛**：Logs.tsx:325 改为
     ```tsx
     {detail.attempts && detail.attempts.length >= 1 && (
     ```
     - 渲染逻辑（Logs.tsx:334-360）不动，单次请求直接渲染一行 #1。
- **验证**：
  - `yarn build` 通过（tsc + vite）
  - 手动：构造 3 类样本验证
    - (a) 单平台一次成功（attempts=1，upstream 200）→ Meta 显示 200 成功色；Attempts 1 行
    - (b) 多平台重试（attempts≥2）→ Meta 显示最终上游状态；Attempts 多行
    - (c) 旧数据 / 上游连接失败（upstream_status_code=0 或 attempts=[]）→ Meta 显示「未捕获」；Attempts 块隐藏（空数组兜底）
  - 列表表格 diff 为 0（`git diff src/pages/Logs.tsx` 只命中详情 view 函数体）
- **资源**：Logs.tsx、MetaItem 组件、api.ts 类型、记忆条 `streaming-sse-log-aggregation` / `perf-hotpath-optimization`（仅参考，不触发性能改动）
- **依赖**：ST1 的 key 名（可硬编码后联调）
- **并行**：与 ST1 文件不交集，可并行启动；联调点 = t() key 名

## 顺序与并行

```
ST1 (i18n 8 语言)  ─┐
                    ├─ 并行 → 联调 (key 名匹配) → ST3 验收
ST2 (Logs.tsx)     ─┘
```

无需串行化（i18n 与 Logs.tsx 文件不交集，符合 CLAUDE.md「共享文件先串行化」例外）。

## 风险与回退

- **i18n key 命名分歧**：若 `MetaItem` 不支持 `undefined` highlight，先用空字符串或新增 `muted` 语义（查 `MetaItem` 定义决定）。
- **upstream_status_code 语义**：0 既可能=连接失败也可能=上游未返回；用 `notCaptured` 中性占位避免误判（不强制复用 connFailed）。
- **回退**：单文件 git checkout 即可；无 DB 迁移、无破坏性变更。

## 五要素自检

- [x] ST1 五要素齐全（目标 / 产出 / 验证 / 资源 / 依赖）
- [x] ST2 五要素齐全
- [x] 文件清单与 PRD 影响范围一致
- [x] 所有 file:line 引用已核实（Logs.tsx 行号 + db.rs 行号 + api.ts 行号）

## 验收对齐 PRD

- AC1 ← ST2 Meta chip
- AC2 ← ST2 attempts 门槛
- AC3 ← ST2 文件范围约束
- AC4 ← ST1 8 语言补 key
- AC5 ← ST2 yarn build
- AC6 ← ST2 兜底占位逻辑
