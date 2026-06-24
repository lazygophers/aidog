# PRD — 前端体感卡慢优化 (高频读写热路径)

## 背景

用户主诉「平台页渲染、分组页渲染、创建/修改的保存都很慢，整体很卡」。
经 `aidog-perf-audit` 全链路只读审计（2026-06-24）：

- **后端三条热路径已到位**（无显著待办）：代理写路径 diff-UPDATE 只写变化列；DB WAL+synchronous=NORMAL+8 只读连接池；stats_agg_hourly 预聚合；retention 硬删+incremental_vacuum；转换层 same-proto 旁路。
- **体感卡慢真因 = 前端 React 巨石组件全量重渲 + memo 击穿**，与 SQL prepare 未缓存。

## 范围

前端 React 渲染优化 + DB 固定 SQL prepare 缓存 + 巨石组件拆分。**全做**（用户拍板含巨石拆分）。执行分层：低风险快赢先落（阶段 A），巨石拆分压轴（阶段 B，须保既有微妙状态机不破）。

### MUST — 阶段 A（低风险快赢，先落先验收）

1. **修 `PlatformCard` 的 `quota` prop memo 击穿**（审计 Top 1，最高性价比）
   - 定位：`src/pages/Platforms.tsx:3220` 传 `quota={computeQuotaDisplay(...)}` 每渲染现算新对象 → 击穿 PlatformCard 浅比较（`src/components/platforms/PlatformCard.tsx:66`）。
   - Platforms 组件 55 个 useState，任一变化（toast/输入/拖拽/quota 局部刷新）→ 全部卡重渲。
   - 改：用 `useMemo` 按 `[p, quotaMap[p.id], quotaRealIds[p.id]]` 缓存 `computeQuotaDisplay` 结果（或下沉 PlatformCard 内部 useMemo，父只传原始 `quotaMap[p.id]`）。
   - 连带审 `usage` / `lastTest` / `platformMembership` 等 props 是否每渲染新建引用，一并稳引用化（参照 `cardActions` 已用的 latest-ref 范式 `Platforms.tsx:2308`）。

2. **Platforms / Groups 渲染内派生数组 `.filter/.sort` 用 `useMemo`**（审计 Top 2b）
   - Platforms（43 处 .map/.filter/.sort）、Groups（53 处）派生数组每渲染全量重算。
   - 改：把 standalonePlatforms 派生、分组归属计算等渲染内数组运算用 `useMemo` 按真实依赖缓存。**仅做 useMemo 化，不拆组件**。

3. **DB 固定 SQL `prepare` → `prepare_cached`**（审计 Top 3，纯 API 替换零风险）
   - 定位：`src-tauri/src/gateway/db/` 0 处 prepare_cached，54 处 `conn.prepare`。
   - 改：**固定 SQL**（list_proxy_logs `db/proxy_log.rs:261`、get_proxy_log `:409`、upsert 两条、usage/stats 固定查询）改 `prepare_cached` 命中 rusqlite statement cache。
   - **动态拼接 SQL 不改**（`build_filter_where` / diff-UPDATE `changed_since` 列集可变 `proxy_log.rs:244/345`，缓存命中率低）。

4. **调查并最小修复「创建/修改保存慢」**
   - 主诉保存慢，疑点：平台/分组 create/save 后是否触发全量 reload 或乐观改写回弹（关联记忆 [[mount-fetch-late-resolve-overwrites-optimistic]] / [[platforms-partial-refresh-epoch-guard]]）。
   - 先定位（profiler / 代码路径）保存慢真因，再最小修复；若根因属高风险大改，则记录待办、不在本 task 强改。

### MUST — 阶段 B（巨石组件拆分，高风险，A 验收后做）

5. **拆 `Platforms.tsx`(3302行) / `Groups.tsx`(1831行) 巨石组件**（审计 Top 2c）
   - 目标：缩小重渲范围 — 把列表项 / 编辑区 / 弹窗 / 派生计算抽成独立 memo 子组件，高频局部态（toast/输入/拖拽中间态/quota 局部刷新）下沉到各自子组件，父容器只持有真正全局态。
   - 拆分策略（保守、分步、每步可编译可验收）：
     - 先抽**纯展示子组件**（无状态，props 稳引用化后 memo），不动状态机。
     - 再抽**局部态子组件**（编辑表单 / 行内编辑 / 拖拽容器），把对应 useState 从父下沉到子。
     - 父保留：跨子组件共享态、列表数据源、epoch generation 守卫、navContext 入口、广播订阅。
   - **不可破坏的微妙状态机**（拆分时逐一保真，对应记忆）：
     - 拖拽 pointer hit-test（[[wkwebview-html5-dnd-drop-fails]]）
     - navContext 导航入口透传（[[navcontext-render-passthrough]] / [[navcontext-edit-retrigger-stale]]）
     - epoch generation 局部刷新守卫（[[platforms-partial-refresh-epoch-guard]]）
     - 乐观操作 dirtyRef + cancelled 守卫（[[mount-fetch-late-resolve-overwrites-optimistic]]）
     - groupDetails 同步刷新（[[platforms-groupdetails-refresh-gap]]）
   - 每抽一个子组件即 `yarn build` 验证 + 功能自查，不一次性大爆改。

### MUST NOT（本 task 不做）

- 不改后端三条已到位热路径（代理写/连接池/预聚合/retention/转换层）。
- 不改动态拼接 SQL 的 prepare（命中率低，无收益反增风险）。
- 不破坏既有微妙状态机：拖拽 pointer hit-test、navContext 导航守卫、epoch generation 守卫、乐观操作 dirtyRef 守卫。

## 验收标准

- **Rust 门禁**：`cd src-tauri && cargo build`（0 error）、`cargo clippy`（本任务代码 0 warning，第三方 block warning 豁免）、`cargo test` 全绿（prepare_cached 改动后 db 测试不破）。
- **前端门禁**：`yarn build`（tsc && vite build 通过）、`node scripts/check-i18n.mjs` 绿（若涉文案）。
- **功能无回归**：拖拽排序 / quota 实时刷新 / 平台增删改局部刷新 / 导航 navContext 均正常。
- **体感改善（定性）**：平台页 ≥20 平台时，单个局部交互（toast/输入/拖拽/单卡 quota 刷新）不再触发全列表卡重渲（React DevTools Profiler 验证重渲范围收窄）。
- 保存慢：定位真因并记录；能低风险修则修，验证保存响应改善。

## 风险 / 备注

- Top 1/2 改动须保 memo 依赖项齐全，否则 quota/拖拽实时性回归（漏依赖致 stale UI）。
- prepare_cached 在 `:memory:` fallback 共连接下 statement cache 仍正确（审计已核）。
- 保存慢根因若深（如 mount-fetch 覆盖乐观操作），按记忆既有修法（dirtyRef+cancelled 守卫）评估，超范围则拆子 task。
