# Skills 页布局修正 (列表=已装 / catalog 仅搜索出 / agent 移出筛选 / 加统计)

## Goal

修正 Skills 页信息架构：主列表展示已安装 skills（默认全局）而非 catalog 浏览结果；agent 选择移出 scope 筛选区并兼做统计；新增 skills 统计（总数 + 每 agent 启用数）。

## 背景 / 现状问题

- `Skills.tsx:60-64` 进页即 `browseCatalog()`，`:292-341` 把全网 catalog 当主列表铺出 → 用户看到的"列表"是可装 catalog，不是已装。**错**。
- agent SVG 图标当前塞在 scope 筛选块（`:219-264`）内，与 scope 混在"筛选"区。**用户明确要求移出筛选区**。
- 无任何统计：看不到一共几个 skills、每个 agent 各启用几个。

## Requirements

### R1 — 页面 = 纯「已安装」skills 管理（彻底删 catalog/搜索/安装）
- **完全移除**：搜索框、catalog 浏览/搜索结果区、安装按钮、`browseCatalog`/`search`/`install` 相关 state(catalog/catalogLoading/searched/keyword)+handler(handleSearch/handleInstall/loadCatalog)+import。
- 页面只展示「已安装」列表（当前选中 agent + scope，scope 默认 global）+ 卸载 + 更新全部。
- **「装新 skill / 未安装」需求用户尚未提出 → 本任务不做**，先去掉这套 UI。
- 注：后端 skills_browse_catalog/skills_search/skills_install command 保留（不删后端），仅前端不再调用/展示。

### R2 — agent 图标移出 scope 筛选区，独立成行 + 兼做统计
- scope 筛选块（`筛选`）只留 scope（global/project + 路径选择）。**agent 图标从该块移除**。
- agent SVG 图标（claude/codex）独立成一行（统计/切换区），**不在筛选块内**。
- 保持 active/inactive 视觉态；点击切换当前查看的 agent，下方已装列表随之刷新。

### R3 — 统计
- 统计区展示：**总计** N 个已装 skills + **每 agent 启用数**（claude: N、codex: M），各带 agent SVG 图标。
- 数据来源：对当前 scope 分别 `listInstalled(scope, "claude")` + `listInstalled(scope, "codex")` 取 count（前端 2 调，无需新后端 command）。总计 = 两 agent 之和（每 agent 分别计，符合"每个 agent 分别启用了几个"）。
- scope 切换（global↔project）时统计随之更新。

## Acceptance Criteria

- [ ] 进页默认展示已安装列表，不铺 catalog
- [ ] 仅搜索后才出现 catalog 结果（带安装按钮）
- [ ] scope 筛选块内无 agent 图标
- [ ] agent 图标独立成行（统计/切换区），active/inactive 可切换，切换刷新已装列表
- [ ] 统计区显示总计 + claude/codex 各自启用数，带图标
- [ ] scope 切换时统计 + 列表更新
- [ ] i18n 新增 key 8 locale 全补（如 skills.total / skills.searchHint 等）
- [ ] yarn build 绿；无残留裸 key（check-i18n.mjs 零缺失）

## Definition of Done

- yarn build 绿；check-i18n.mjs 零缺失；8 locale parity
- 改动落 worktree，闭环 check→commit(merge)→archive
- 仅前端改动（Skills.tsx + locales），无后端/契约变更

## Technical Approach

- 单文件为主：`src/pages/Skills.tsx` 重排 + `src/locales/*.json` 加 key。
- 删进页 browseCatalog 自动调用；catalog state 只由 handleSearch 填充；新增 `searched` 标志区分"未搜索/已搜索空"。
- 统计：useEffect 依赖 [scope] 时并行 listInstalled(claude)+listInstalled(codex)，存 counts；当前 agent 的 list 复用其一避免重复调（或统一存 perAgent map）。
- 布局顺序：Header → env warning → scope 筛选(仅 scope) → agent 图标+统计行 → 搜索框(+catalog 结果) → 已装列表。

## Out of Scope

- 后端新增 stats command（前端聚合即可）
- catalog 分页 / 缓存
- agent 之外的 scope 形态改动

## Technical Notes

- 现状行号：browseCatalog 自动(60-64) / loadInstalled(78-97) / handleSearch(100-107) / scope+agent 筛选块(219-264) / catalog 渲染(292-341) / 已装渲染(343-381)
- 参考 [[skills-management-module]]；merge 注意 [[worktree-stale-base-merge-conflict]]（locale 冲突取 master 版+最小重应用）
