# PRD — CLI 集成 tab 改名 + 语言设置合并

> pending: `.trellis/workspace/nico/pending-next-task.md`；并行 task: `06-20-test-coverage-80`（文件集不相交：本 task 改前端 locale/组件，coverage 补测试）

## 目标
编程工具 tab 改名「CLI 集成」+ 新增「语言」设置项（从 claudeTab 抽 language 字段到此 tab 快捷编辑）。

## 决策（grill 已定 / codebase 已查）
- 新名：zh-CN「CLI 集成」/ en-US「CLI Integration」（其余 6 语言按语义译）
- 「语言」= Claude Code 会话语言（`settings.json` 的 `language` key，源 `claude-settings-schema.ts:37`）
- claudeTab = Claude settings 全集（SECTIONS 几十字段：model/effortLevel/outputStyle/language/agent/...），抽走 language 后 claudeTab 剩其余字段（语义分层：CLI 集成=常用快捷开关+语言，Claude=高级全集）
- 两 tab 语义不重叠：CLI 集成管「联动开关 + 常用语言」，Claude 管「完整 settings 高级配置」

## 交付项

### D1 — tab 改名（locale key + value）
- `src/locales/*.json`（8 语言）：`appSettings.codingToolsTab` → `appSettings.cliIntegrationTab`，value 改「CLI 集成」族
- `src/locales/*.json`：`codingTools.introTitle` → `codingTools.cliIntegrationTitle`（或保留 key 改 value），value 同步
- `src/App.tsx:38`：`labelKey` 同步新 key
- `src/components/settings/CodingToolsSettings.tsx:81`：`t()` 调用 key 同步
- **禁漏 locale**：8 语言全改，漏一即裸 key fallback（[[frontend-i18n-coverage]]）
- 验证：`yarn check:i18n`（若有）/ `grep -rn 'codingToolsTab\|编程工具\|Coding Tools' src/` 必须 0 残留（除迁移注释）

### D2 — 新增「语言」设置项到 CLI 集成 tab
- `CodingToolsSettings.tsx` 加语言下拉（options 复用 `claude-settings-schema.ts:37` 的 language options：zh-CN/en-US/ja-JP/ko-KR/fr-FR/de-DE/es-ES/pt-BR/it-IT/ru-RU/ar-SA/hi-IN/th-TH/vi-VN）
- 持久化：写 `~/.claude/settings.json` 的 `language` key（复用现有 sync 机制，查 `claude-settings-schema.ts` 写入路径）
- 读取：mount 时读当前 language 值回显
- 与 D1 改名同文件（CodingToolsSettings.tsx）→ 串行不并行

### D3 — claudeTab 去重（可选，视 implement 审查）
- 若 D2 在 CLI 集成编辑 language，claudeTab 的 language 字段是否保留？
- **推荐保留**（claudeTab 仍可高级编辑，两处写同 key，后写胜）—— 避免改 SECTIONS 影响其他逻辑
- 若用户裁定去重 → 从 `SECTIONS` 删 language 项 + 验证无破坏

## 验收
1. `yarn build` 绿（tsc 类型 + vite build）
2. `grep -rn 'codingToolsTab\|编程工具\|Coding Tools' src/` 0 残留（locale key 全迁移）
3. 8 locale 均含新 key（`appSettings.cliIntegrationTab` + introTitle 新 key）
4. CLI 集成 tab 显示语言下拉，切换后写 `~/.claude/settings.json` language key，重启读回显正确
5. claudeTab 无破坏（其余字段仍可编辑）

## 非目标
- 不改后端 Rust（仅前端 locale + 组件 + 复用现有 sync）
- 不重命名 `CodingToolsSettings.tsx` 文件（YAGNI，key 改够；文件名改增 diff 无功能收益，标 ponytail 跳过）
- 不动 claudeTab SECTIONS 其他字段（D3 推荐保留 language）

## 风险
- locale key 漏改 → 裸 key fallback（D1 验收 2 兜底 grep）
- language 写入路径错 → 不生效或写错位（D2 须复用现有 sync，禁自实现）
- 并行 coverage task 文件集不相交（前端 vs 测试），无冲突

## 触点
- `src/locales/*.json`（8 语言）
- `src/App.tsx:38`（labelKey）
- `src/components/settings/CodingToolsSettings.tsx`（t() + 语言 UI）
- `src/services/claude-settings-schema.ts`（language options 复用 + sync 写入路径参考）
- `src/pages/AppSettings.tsx`（tab 注册，组件名不变则无需改）
