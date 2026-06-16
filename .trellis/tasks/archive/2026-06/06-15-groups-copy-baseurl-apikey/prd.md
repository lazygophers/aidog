# Groups(分组与路由) 列表页加复制 base_url + 每 item 复制 api_key

## 需求
1. 分组与路由列表页（`src/pages/Groups.tsx`）支持**复制代理的 base_url**（页面级）。
2. **每个 group item 支持复制 api_key**。

## 事实（已核 file:line）
- **代理 base_url** = `http://127.0.0.1:<port>/proxy`（权威值，lib.rs:1189 写进每组 settings 的 `ANTHROPIC_BASE_URL`）。
- **api_key（每 group）** = `group.name`（lib.rs:1193 `ANTHROPIC_AUTH_TOKEN = group_name`；buildCodexCommand 也用 `AIDOG_KEY=<group>`）。即分组名就是调代理的鉴权 token。
- **port 来源**：`proxyApi.getSettings()` → `proxy_get_settings` 返回 `ProxySettings{port}`（api.ts:573-577）。
- **现成组件**：`Groups.tsx::CopyButton`（:198，带复制反馈 ✓ 动画，支持 label/icon）可直接复用。已有 buildClaudeCommand/buildCodexCommand 的 Claude/Codex 复制按钮（:448/:813）。
- group item 渲染两处：卡片态（:813 附近）+ 编辑态（:448 附近）。

## 实现
1. **取 port**：Groups 组件加载时 `proxyApi.getSettings()` 取 port（state，默认 7890 兜底），构造 `proxyBaseUrl = \`http://127.0.0.1:${port}/proxy\``。
2. **页面级复制 base_url**：在 Groups 页头部/工具栏区放一个 `<CopyButton text={proxyBaseUrl} label="..." title=... />`，文案如「复制代理地址」+ 可显示该 url（只读小字/或仅按钮）。位置与现有页头协调（不破坏布局）。
3. **每 item 复制 api_key**：在每个 group 卡片态(:813 区，与 Claude/Codex 复制按钮同排) + 编辑态(:448 区) 加 `<CopyButton text={group.name /* 或 editName */} label="Key"/"API Key" title=... />`，复制该分组 name（=api_key）。
4. **i18n**：新文案 key（复制代理地址 / 复制 API Key 等）8 locale 全补；加 key 后 Counter 查重。

## 验收
- `yarn build`（tsc+vite）+ `yarn check:i18n` 过；locale 无重复 key。
- 后端无改动（纯前端，复用现有 proxy_get_settings）；若未动后端则免 cargo。
- 行为：页头有复制代理 base_url 按钮，复制得 `http://127.0.0.1:<port>/proxy`；每 group（卡片+编辑态）有复制 api_key 按钮，复制得该 group name；均有复制反馈。
- 不破坏现有 Claude/Codex 复制按钮 + 列表布局/拖拽。

## 失败处理
- proxyApi.getSettings 失败 → port 兜底 7890，base_url 仍可复制；记录。
- 布局拥挤（按钮过多）→ 用 icon-only CopyButton（CopyButton 无 label 即 icon 态）+ title 提示，保持紧凑。
- 门禁红修到绿；范围外标 `需要:`。
