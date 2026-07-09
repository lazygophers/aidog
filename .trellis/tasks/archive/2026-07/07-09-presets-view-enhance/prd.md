# presets-view 展示信息完善

## Goal

把 `make presets-view` 生成的 `.aidoc/presets.html`（`scripts/presets_view/generate.py` 单文件 vanilla JS 渲染）展示信息从「够用」升到「完善」：补齐 4 类静态信息（peak_hours / 模型槽位映射 / coding_plan 独立模型 / 聚合统计 header）+ 3 类交互（模型详情弹窗 / 价格可视化 bar / 过滤排序扩展）。

**为什么**：用户要「presets-view 需要展示更加完善的信息」。当前展示 = logo + name/desc + 链接 + endpoints + 模型表（$/M + max/ctx + source），但 platform-presets.json 内还有 peak_hours（高峰倍率）、models.default 槽位映射（opus/sonnet/haiku/fable/default → 哪个模型）、coding_plan 独立分支模型 等关键信息未展示；交互只有搜索 + only-override + 3 排序键。完善后这个内部工具能真正用来核对预设完整度 / 比价 / 排查槽位配置。

## 现状（已读 generate.py 439 行）

**build_view_data（line 84-138）每协议产出**：key / name / desc / client_type / homepage / logo_url / source_urls / endpoints_default / endpoints_coding_plan / models（model_list.default 行）/ model_count / has_override / min_input_M。

**cardHtml（line 320-360）展示**：logo + name+key+client_type+coding_plan/override 徽标 + model_count / desc / 链接（官网/定价/文档）/ endpoints / 模型表（id + default_platform + input/output/cache $/M + max_in/out/ctx + source）。

**applyFilter（line 362-389）**：搜索（name/key/model_id）+ only-override + 排序（name/models/min_input）。

**JSON 有但当前未注入 view_data / 未展示**：
- `peak_hours`（per-protocol，窗口数组：start_hour/end_hour/start_minute/end_minute/multiplier/days_of_week/days_of_month/models/start_at/end_at）—— 完全未读未展。
- `models.default` 槽位映射（`{default, opus, fable, sonnet, haiku, ...} → model_id`）—— 只用 model_list.default 平铺，未展槽位对应。
- `model_list.coding_plan` / `models.coding_plan` —— coding_plan 端点的独立模型列表 / 槽位映射，完全未读。
- `is_coding_plan` flag —— 未展（虽然 endpoints_coding_plan 非空隐含，但协议级 flag 独立）。
- models.json 的 `context_tiers`（分级 context 定价）—— resolve_price 未取，未展。

## 真值源（已验证 JSON shape）

- `src-tauri/defaults/platform-presets.json`：
  - `protocols[k].peak_hours: PeakWindow[]`（absent = 无调整）
  - `protocols[k].models.default: {default?, opus?, fable?, sonnet?, haiku?, ...}` 槽位 → model_id
  - `protocols[k].models.coding_plan: {...}` cp 分支槽位
  - `protocols[k].model_list.coding_plan: [...]` cp 分支模型列表
  - `protocols[k].is_coding_plan: bool`
- `src-tauri/defaults/models.json`：
  - `models[k].context_tiers`（分级定价数组，可选）
  - 现有：input/output/cache cost per token、max_input/output_tokens、context_window、default_platform、pricing（per-protocol override）
- 跨层对称：PeakWindow shape 同 `src/domains/platforms/defaults.ts:10` TS `PeakWindow`（前后端一致，本任务在 generate.py 内 Python 解析，不跨层但 shape 对齐）。

## Requirements

### R1 build_view_data 扩展（注入新字段）

- R1.1 `peak_hours`：每协议读 `p.get("peak_hours")` → 原样注入（窗口数组；absent → `[]`）。窗口字段全保（start/end hour+minute / multiplier / days_of_week/month / models / start_at/end_at），前端渲染时格式化。
- R1.2 `slots_default`：读 `(p.get("models") or {}).get("default", {})` → `{slot: model_id}` 字典注入（slot ∈ default/opus/fable/sonnet/haiku/…）。
- R1.3 `slots_coding_plan`：读 `(p.get("models") or {}).get("coding_plan", {})` → 同上（cp 分支槽位，absent → `{}`）。
- R1.4 `model_ids_coding_plan`：读 `(p.get("model_list") or {}).get("coding_plan", [])` → cp 分支模型列表，走 resolve_price 产 model_rows（同现有 default 分支逻辑），注入 `models_coding_plan`。
- R1.5 `is_coding_plan`：注入 bool（`bool(p.get("is_coding_plan"))`）。
- R1.6 `context_tiers`：resolve_price meta 加 `context_tiers`（models.json 取），随 model_row 注入供详情弹窗展。
- R1.7 **聚合统计 header**（view_data 顶层加 `stats`）：
  - `total_protocols` / `total_models`（去重 model_id 全局）
  - `pricing_coverage_pct`（有价模型 / 总模型）
  - `protocols_with_peak` / `protocols_with_cp` / `protocols_with_override`
  - `overall_min_input_M` / `overall_median_input_M` / `overall_max_input_M`（全局 input $/M 分布）

### R2 cardHtml 展示扩展

- R2.1 **peak_hours 徽标 + 展开区**：有 peak_hours 时卡片头加「⚡peak」徽标；卡片展开区加 peak_hours 表（每窗口一行：时段 `HH:MM-HH:MM` UTC+0 / multiplier（>1 红 / <1 绿）/ days / model scope）。跨天窗口（end<start）显 `22:00→06:00+1`。
- R2.2 **槽位映射表**（default 分支）：展 `slots_default` 成「slot → model_id」小表（2 列：slot 名 / model_id 链接 = 点击触发模型详情弹窗）。空 → 不展。
- R2.3 **coding_plan 独立区**：slots_coding_plan 或 models_coding_plan 非空时，卡片展开区加独立「Coding Plan 分支」子区（槽位表 + 模型表，与 default 分支视觉分隔）。
- R2.4 `is_coding_plan` 徽标（协议层 cp 标记，区别于 endpoints_coding_plan 非空）。

### R3 聚合统计 header（顶部）

- R3.1 顶部 sticky header 加统计条：`N 协议 · M 模型 · 覆盖率 X% · peak Y 协议 · cp Z 协议`。
- R3.2 全局 input $/M 分布条（min/median/max 三点标尺），视觉直觉贵贱区间。

### R4 模型详情弹窗（新交互）

- R4.1 点击模型表 / 槽位表的 model_id → 弹 modal 展 full breakdown：
  - 全定价（input/output/cache $/M + per-token 原始值）
  - max_input / max_output / context_window（已展于表，弹窗重复 + 加 human-readable 如 `1M tokens`）
  - `context_tiers`（分级定价数组，若非空展表）
  - default_platform
  - 所有 protocol override（`pricing` 字典各协议的 $/M）
- R4.3 弹窗 vanilla JS（无依赖，同现有风格），createPortal 等效（fixed inset + backdrop）。
- R4.4 Esc / 点 backdrop 关闭。

### R5 价格可视化 bar（新交互）

- R5.1 卡片展开区模型表每行加 input $/M 水平 bar（div 宽度按 `input_M / overall_max_input_M` 比例），颜色按价格档（低绿/中黄/高红）。
- R5.2 可选：顶部加全局分布 bar（同 R3.2）。

### R6 过滤排序扩展

- R6.1 过滤加：`has_peak_hours` / `has_coding_plan` / `is_coding_plan` / `client_type`（下拉）复选/选择。
- R6.2 排序加：`output_M` / `ctx` / `has_peak`（有 peak 优先）。
- R6.3 搜索扩到含 peak_hours model scope、slots_default 的 model_id。

### R7 门禁

- R7.1 `python3 scripts/presets_view/generate.py` 跑通无异常，`.aidoc/presets.html` 生成。
- R7.2 生成 HTML 浏览器打开无 JS 错误（vanilla JS 运行时渲染，payload 嵌 `<script type="application/json">` 遵 HTML JSON Embedding guide 禁 html.escape，仅 replace `<`）。
- R7.3 payload 用 `<` → `\\u003c` 替换（防 `</script>` 注入），禁 `html.escape(payload)`（会致 `JSON.parse` 炸，见 spec sediment 契约）。
- R7.4 主仓零改动（worktree 内）。

## Acceptance Criteria

- [ ] build_view_data 注入 peak_hours / slots_default / slots_coding_plan / models_coding_plan / is_coding_plan / context_tiers / stats
- [ ] cardHtml 展 peak_hours 徽标+表 / 槽位映射表 / cp 独立区 / is_coding_plan 徽标
- [ ] 顶部聚合统计 header（协议数/模型数/覆盖率/peak/cp + 全局 input 分布条）
- [ ] 模型详情弹窗（full breakdown + context_tiers + overrides）
- [ ] 模型表价格 bar（颜色档 + 比例宽度）
- [ ] 过滤（has_peak/cp/is_cp/client_type）+ 排序（output_M/ctx/has_peak）扩展
- [ ] generate.py 跑通，HTML 无 JS 错误，JSON 嵌入用 `<` 替换非 html.escape
- [ ] 主仓零改动

## Definition of Done

- 4 类静态信息 + 3 类交互全落地
- 生成 HTML 浏览器打开信息完整、交互无报错
- payload 嵌入守 HTML JSON Embedding guide
- journal 记录新增字段映射 + 跨层 PeakWindow shape 对齐

## Technical Approach

```
build_view_data (generate.py 84-138) 扩展:
  每协议加: peak_hours / slots_default / slots_coding_plan /
            models_coding_plan (resolve_price cp 分支) / is_coding_plan
  model_row 加: context_tiers
  顶层加: stats { total_protocols, total_models, pricing_coverage_pct,
                 protocols_with_peak/cp/override, input_M min/median/max }

cardHtml (320-360) 扩展:
  head badges += ⚡peak / is_coding_plan
  body += peak_hours 表 + slots 映射表 + cp 独立子区
  model 表行 += 价格 bar cell

新增 renderModal(): 全局单例 modal DOM + showModelDetail(key) 填充
新增 renderStatsBar(header): 顶部统计条 + 分布条

applyFilter (362-389) 扩展:
  filter += has_peak / has_cp / is_cp / client_type
  sort += output_M / ctx / has_peak
  search 命中 += peak models scope + slots model_id

JSON 嵌入: payload.replace("<", "\\u003c") （禁 html.escape）
```

## Decision (ADR-lite)

**Context**：完善 presets-view 信息 + 交互，单文件 generate.py。
**Decision**：
1. 单文件增强（不拆多文件）—— generate.py 已是 stdlib-only 单文件，维持；HTML/CSS/JS 内联于 Python template string（既有风格）。
2. 新数据全在 build_view_data 注入 + cardHtml 渲染（前端运行时 vanilla JS），非 Python 端预渲染 HTML —— 保现有架构（payload JSON + JS 渲染）。
3. modal 单例（全局 DOM，show 时填 content），非每卡一个 modal。
4. 价格 bar 用 div width %（非 canvas），颜色按整体分布分位档（绿 < median / 黄 < 75 分位 / 红 ≥ 75 分位）。
5. peak_hours 时段格式化 UTC+0（与 Rust `gateway::peak_hours` 一致，不转本地时区 —— presets-view 是内部核对工具，UTC+0 直观对齐配置真值）。
**Consequences**：
- generate.py 行数会增 ~200-300（modal + stats + bar + 扩展渲染），仍单文件可控。
- payload 体积增（peak_hours + slots + stats），但协议数 60 内、单次加载无性能问题。
- modal 单例需事件委托（data-attr 传 model_id + protocol_key），与现有 grid click 委托同模式。

## Out of Scope

- 改 platform-presets.json / models.json 数据本身（仅展示）
- 跨层改 Rust / 前端 React（generate.py 独立工具，零跨层）
- 加第三方 JS 库（维持 stdlib only + vanilla JS）
- presets-view 改为在线服务（仍本地 HTML 文件）
- 国际化（内部工具，中文为主，沿用现有）

## Technical Notes

- PeakWindow shape（platform-presets.json）：`{start_hour, end_hour, multiplier, start_minute?, end_minute?, days_of_week?, days_of_month?, models?, start_at?, end_at?}` —— 同 `src/domains/platforms/defaults.ts:10`。
- models.default 槽位枚举：default / opus / fable / sonnet / haiku（ModelSlot），但 JSON 可能含其他自定义槽 —— 渲染时遍历 dict 即可。
- context_tiers：models.json 内分级 context 定价数组，shape 见 models.json（实施时读样本确认）。
- 既有 guide：`.trellis/spec/guides/html-json-embedding.md`（禁 html.escape，仅 replace `<`）+ `.trellis/spec/guides/code-reuse-rules.md`。
