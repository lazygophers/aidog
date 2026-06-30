# Pending Next Task（排队中，合并「编程工具 tab 重构」）

> 等 group-env-vars task finish/archive 后启动。不提前 create（task.py create 偷 current pointer，见 [[trellis-create-steals-current-task]]）。

## 合并需求（一个 task，三件事）
1. **改名**：tab「编程工具」→「CLI 集成」
2. **加语言设置**：tab 内新增「语言」设置项
3. **移配置**：将 Claude Code 现有语言配置移动到此 tab

## 改名决策（grill 已定）
- **新名**：zh-CN「CLI 集成」/ en-US「CLI Integration」（其它 5 语言 implement 时按语义译）
- **范围**：7 语言 locale 全改
- **key+value 都改**：
  - tab label key（App.tsx:38 `labelKey: "appSettings.???"`，locale zh-CN.json:41）→ 重命名为语义匹配（如 `appSettings.cliIntegrationTab`）
  - 页面内标题 key（`codingTools.???` zh-CN.json:68，CodingToolsSettings.tsx:81 调用）→ 同步重命名
  - ⚠️ locale key 名含疑似特殊字符（终端 rg 渲染为 "n"）—— implement agent **必读真实文件**确认 key 全名，禁凭 grep 输出猜

## grill 弱点（implement 前须解）
- ⚠️ **调用点追踪**：key 重命名须同步 7 locale + `App.tsx` labelKey + `CodingToolsSettings.tsx` t() 调用 + 可能的 CodexSettings。漏一即裸 key fallback
- ⚠️ **claudeTab 边界**：兄弟 tab `appSettings.claudeTab`=「Claude」是什么？移 Claude Code 语言配置前须查 claudeTab 当前内容，厘清「移走后 claudeTab 剩什么」+「CLI 集成 vs Claude 是否重叠」
- ⚠️ **「语言」语义**：pending 原预录点 —— 加的「语言」是 i18n UI 语言？Claude Code 会话语言？还是别的？grill 时未细问，task 启动 brainstorm 须先澄清
- ⚠️ **scope 拆分**：三件事（改名 / 加语言 / 移配置）须 implement.md 拆 subtask，文件集交集（同 tab + locale）→ 多为串行非并行

## 启动条件
- group-env-vars 已 archive（worktree 销、current 释放）
- 启动走标准 /trellisx-flow：create → brainstorm（先澄清「语言」语义 + claudeTab 边界）→ prd → start → exec → check → finish

## 触点预录
- `src/locales/*.json`（7 语言）：tab label key + introTitle key 重命名 + value 改
- `src/App.tsx:38`：labelKey 同步
- `src/components/settings/CodingToolsSettings.tsx`：t() 调用 key 同步 + 文件名可考虑重命名（CodingToolsSettings → CliIntegrationSettings?）
- `src/pages/AppSettings.tsx`：tab 注册 + 组件引用
- 语言设置新增 UI + 后端持久化（若「语言」= 会话/i18n 配置，须查 Claude Code 语言配置现存点）
