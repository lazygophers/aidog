# 平台分享 (platform-share)

## 目标

允许用户分享单个平台配置：点卡片「分享」按钮 → 弹窗展示分享内容 + 自动复制到剪贴板（自动复制成功 toast）；弹窗内提供手动「复制」按钮（手动复制成功 toast）；弹窗支持切换分享格式（YAML 默认 / JSON / Base64）。

## 用户决策 (已拍板)

1. **凭证**: 分享内容**包含 api_key**（平台分享本质 = 把可用配置给别人）。
2. **格式**: 默认**明文 YAML**，弹窗提供格式切换器，支持 YAML / JSON / Base64。
3. **入口**: 平台卡片按钮区，**编辑与复制之间**插「分享」icon。

## 需求拆解

### 后端 (Rust)
- 新增 command `platform_share(platform_id) -> ShareResult`，返回单平台可分享数据结构（含 api_key、base_url、platform_type、models、endpoints、extra、manual_budgets 等，**剥离 DB 内部字段**如 id/status/统计）。
- 序列化由前端按所选格式做（YAML/JSON/Base64），后端只返回干净的数据对象 → **后端可仅返回结构化 JSON Value，前端负责格式转换**。
  - 决策：后端返回 normalized 平台对象（serde JSON），前端用 `yaml`/`JSON.stringify`/`btoa` 三路转换。避免后端引入 YAML 依赖（除非已有）。先 grep 确认 Rust 是否已有 yaml crate；无则格式转换全放前端。

### 前端 (React/TS)
- `services/api.ts`: 新增 `platformApi.share(id)` 封装（invoke `platform_share`，camelCase 参数 `platformId`）。
- 新组件 `src/components/platforms/ShareModal.tsx`（参照 `UpdatePromptModal.tsx` 的 glass-elevated + overlay 结构）：
  - 顶部格式切换器（YAML / JSON / Base64 三选一，YAML 默认）。
  - 内容预览区（`<pre>` 展示当前格式文本，可滚动）。
  - 手动「复制」按钮 → `navigator.clipboard.writeText` + 成功 toast + icon 切换 1500ms。
  - 打开即自动 `navigator.clipboard.writeText`（当前格式）+ 自动复制成功 toast。
  - 切换格式时重算文本（不重复自动复制，仅手动按钮触发复制）。
- `PlatformCard.tsx`: 按钮区编辑与复制之间插「分享」icon，`onShare` action。
- `Platforms.tsx`: 接 `onShare` → 调 `platformApi.share` → 打开 ShareModal；复用现有全局 toast。

### 安全
- api_key 明文进入分享内容 = 用户**显式主动**分享自己配置，符合预期。但 UI 须有提示：分享内容含密钥，注意分享对象（弹窗加一行 warning 文案，i18n）。

### i18n
- 新增文案 key（分享按钮 title / 弹窗标题 / 格式切换 label / 复制成功 / 自动复制成功 / 含密钥 warning）全 8 locale 覆盖；跑 `check-i18n.mjs`。

## 验收标准
- 卡片有分享按钮，点击弹窗展示内容且自动复制成功有 toast。
- 弹窗手动复制按钮工作，成功有 toast + icon 反馈。
- 格式切换器 YAML/JSON/Base64 三路正确，默认 YAML。
- 分享内容含 api_key + 完整可导入字段。
- `cargo build` + `cargo clippy`（warning 清零）+ `yarn build` + `check-i18n.mjs` 全绿。
- Rust↔TS 边界字段名对齐（camelCase invoke 参数）。

## 复用 (Explore 探查结论)
- 剪贴板: `navigator.clipboard.writeText`（Groups.tsx/Home.tsx 已用）。
- Toast: Platforms.tsx 全局 toast `setToast({text, ok})` 3000ms。
- Modal: `UpdatePromptModal.tsx` 结构（glass-elevated + overlay zIndex 1100）。
- Platform 类型: `api.ts:166-204`；平台 API: `platformApi` `api.ts:400-475`。

## 单一交付
单一可验收交付，不拆 parent/child。后端 1 command + 前端 ShareModal + api 封装 + 卡片入口 + i18n，单 worktree。
