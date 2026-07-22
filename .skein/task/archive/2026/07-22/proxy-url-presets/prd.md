# 代理 URL 下拉预置端口 — PRD

## 目标
- [ ] 代理 URL 输入支持 datalist（下拉选 + 自由输入），预置 4 个常见本地代理端口：http://127.0.0.1:{7890, 8080, 1080, 7899}（Clash/V2Ray/SS/常用）。

## 边界
- [ ] 范围内：CodingToolsSettings.tsx 代理 URL input 加 `list` 属性 + `<datalist>` 预置项；新增 PROXY_URL_PRESETS 常量。
- [ ] 范围外：NO_PROXY 不加预置；不改 i18n（datalist 无文案）；不动 handler/load 逻辑（仅 UI 层）。
- [ ] 约束：datalist 原生支持自由输入（非 select 强选），保留用户自定义 URL 能力。

## 验收标准
- [ ] yarn build 通过
- [ ] yarn test 全过
- [ ] 代理 URL input 带 datalist，含 4 个预置 http://127.0.0.1:port 项，仍可自由输入任意 URL

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json
