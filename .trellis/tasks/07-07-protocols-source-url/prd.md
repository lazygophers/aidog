# platform-presets protocols 数据源站点 URL

## Goal
为 `src-tauri/defaults/platform-presets.json` 每个 protocol 项加**官方数据源站点 URL** (文档/定价/模型列表页), 让后续手动核对更新 endpoint/model/price 时**一站直达**, 不必每次搜索引擎找官方站。维护用 metadata, 非 UI 展示。

## What I already know
- 现状 (file:src-tauri/defaults/platform-presets.json): 60 protocols, 字段 = `client_type / endpoints / models / model_list / name / desc` (name/desc 由 task `07-07-protocols-i18n-name-desc-search` A1 agent 在加, in_progress)
- `endpoints.default[].base_url` 是 **API 端点** (如 `https://api.anthropic.com`), 非文档/定价站 — 不能复用
- 无任何 `source_url` / `doc_url` / `homepage` 类字段
- `client_type` 62 变体映射 (models.rs), 部分协议是 aggregator (如 openrouter / novita / siliconflow) 转售多家模型 — 这类站 URL 应指 aggregator 自身文档

## Assumptions (temporary)
- 字段形态: 单 `source_url` string (YAGNI 结构化多 URL), 指官方文档/定价首页 (含模型列表 + 定价表)
- 命名: snake_case + `_url` 后缀 (与 `base_url` 风格一致)
- 前端不展示 (维护用 metadata, 不进 Tauri command 透传协议, Rust struct 加但前端不渲染)
- 数据: 手查每 protocol 官方站 + 写入 JSON (60 个 URL)

## 决策
- ✅ Q1 字段形态: **结构化 `source_urls: {docs, pricing}`** (用户裁定) — 文档页 + 定价页分指, 维护时直奔目标
- ✅ Q2 前端展示: **不展示** (用户裁定, 维护 metadata, YAGNI UI)
- ✅ Rust struct: **不加** (A1 已证 `get_defaults_json` 透传 raw String 无 ProtocolPreset struct, 加=死代码) — 仅 JSON 字段 + 前端 TS 类型可选标注
- ✅ 无 mock 协议 (60 全 `default`/`claude_code`/`codex_tui`), 全部需填 URL
- ✅ URL 数据源: agent 自主 WebSearch + 厂商官网查 (非手填, agent 60 个查完一次写)

## 改动范围

### A. platform-presets.json 数据 (核心)
- 60 protocols × 2 URL (docs + pricing) = 120 URLs 查 + 写入
- agent 自主查每 protocol 官方文档/定价站 (WebSearch + 厂商官网)
- aggregator 协议 (openrouter/novita/siliconflow/oneapi/newapi 类) URL 指 aggregator 自身 (非上游厂商)
- 若 docs/pricing 同页 (部分小厂), 两子字段填同 URL
- 单 subtask 一次写完 (无 fan-out, 60 项同 JSON 文件, 不拆)

### B. 前端 TS 类型 (defaults.ts)
- `DefaultsDoc` 协议条目类型加可选 `source_urls?: { docs: string; pricing: string }` (与 A1 加 name/desc 同位置)
- 不渲染 (Q2 决策), 仅类型标注, 防前端消费处类型错位

### 不动 (重要边界)
- **Rust 源码**: `get_defaults_json` 透传 raw String, 无 struct, 加 Rust 字段=死代码 (A1 已证)
- **Tauri command**: 不透传 source_urls 到前端消费 (Q2 不展示, 仅 TS 类型预留)
- **price_sync 逻辑**: 仅 last_updated 自然更新, jsDelivr/raw URL 不变
- **前端组件**: 零改动 (不渲染 source_urls)

## Acceptance
- [ ] 60 protocols 全部含 `source_urls: {docs, pricing}`, 两子字段零缺失 (无 mock 特例)
- [ ] URL 抽检 (≥20 个, 含 aggregator + 厂商直连 + claude_code/codex_tui) HTTP 200 / 重定向到官方文档/定价
- [ ] aggregator 协议 URL 指 aggregator 自身 (非上游厂商), 抽检 ≥5 个 aggregator
- [ ] `yarn build` (tsc + vite) 全绿 (TS 类型可选, 不破坏现有消费)
- [ ] `cargo build/test --lib/clippy --lib` 全绿 (无 Rust 改动, 仅回归)
- [ ] 不破坏 sync 逻辑 (jsDelivr/raw URL 不变, last_updated 更新)

## Open Questions
- (无, brainstorm Q1/Q2 已闭合)

## Out of Scope (explicit)
- 前端展示 URL (UI 改动, YAGNI)
- URL 自动校验脚本 (手检为准, 一次性)
- 价格同步逻辑改动 (price_sync.rs 不动, 仅 URL 字符串路径)
- 多语言翻译 (URL 无本地化)
- endpoint/model_list 数据本身更新 (本 task 只加索引 URL, 不更新数据)

## Technical Notes
- **依赖**: 与 `07-07-protocols-i18n-name-desc-search` (in_progress) 文件集**完全重叠** (都改 platform-presets.json + Rust ProtocolPreset struct) → MUST 串行: i18n finish 后才 start 本 task, 禁双 worktree 同改 JSON
- aggregator 协议 (openrouter/novita/siliconflow/oneapi/newapi 类) URL 指 aggregator 文档 (模型 + 定价都在那)
- 厂商直连协议 (anthropic/openai/deepseek 等) URL 指厂商官方文档站
- mock 协议 (如有) URL 留空或指本项目 README

## Decision (pending brainstorm)
- 字段形态 (Q1) 待用户裁定
