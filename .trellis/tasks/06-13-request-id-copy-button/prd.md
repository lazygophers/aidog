# PRD — 请求详情 Request ID 独立复制按钮

## 目标
请求详情页（`src/pages/Logs.tsx` 详情视图）的 Request ID 行已有「复制完整信息」总按钮，但缺少针对 Request ID 自身的独立复制。新增一个独立复制按钮，复制结果为 `request_id=<detail.id>` 格式（便于直接喂给 aidog-request-inspect skill 按 id 查代理请求）。

## 现状
- `Logs.tsx:267-271`：Request ID 行只展示 `detail.id`，无复制。
- `Logs.tsx:255-261`：Header 处有 `copyDetail` 总复制按钮 + `copied` 勾选反馈样式。

## 范围
- 仅改 `src/pages/Logs.tsx` Request ID 行：加一个 inline 复制按钮（btn-ghost btn-icon 风格，复用现有 copy svg）。
- 复制内容：`request_id=${detail.id}`。
- 复制成功给短暂反馈（独立 state，不复用 header 的 `copied`，避免两按钮联动）。
- i18n：新增 key `logs.copyRequestId`（zh-CN「复制请求 ID」+ 兜底英文），按现有 7 语言约定补。

## 验收标准
- 点击 Request ID 行的复制按钮 → 剪贴板得到 `request_id=<id>`。
- 反馈勾选只作用于该按钮，不触发 header 总复制按钮的勾选。
- tsc / lint 无新增 warning。

## 失败处理
- 若 `navigator.clipboard` 不可用，沿用现有 `copyDetail` 的处理方式（已 await writeText，无降级），保持一致即可。
