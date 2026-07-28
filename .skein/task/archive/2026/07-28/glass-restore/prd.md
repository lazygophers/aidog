# 恢复卡片玻璃质感 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] shadcn 迁移后卡片「完全没有玻璃质感」(用户反馈): 主卡片类 `.glass-surface` 用不透明 `--bg-surface`(=card) 且**无 backdrop-filter**, 只是纯色面板。
- [ ] 对照 `.glass-elevated`(已有 backdrop-filter blur+saturate) 补齐 `.glass-surface`。
- [ ] 成功: 卡片半透明 + 背景模糊, 叠 aurora 流光背景呈 Liquid Glass 质感。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: `src/styles/globals.css` `.glass-surface` 一处 — 半透明 bg(color-mix) + backdrop-filter blur+saturate + `-webkit-` 前缀(Tauri macOS WKWebView 必需)。改一处覆盖全站卡片(CompactCard/Home/Stats 等所有 glass-surface 用户)。
- [ ] 范围外: 不改 shadcn Dialog/Popover/Select/Input(交互浮层保持不透明保可读); 不改 `--card` 调色板 token(避免波及所有 bg-card); 不动其它 style 轴。
- [ ] 约束: `--glass-blur`/`--glass-saturate` 由 style 轴提供(aurora/liquidGlass 均有), 沿用 `.glass-elevated` 既有无 fallback 用法(一致); flat/terminal 等无 blur 的 style 退化为半透明无模糊(可接受, 用户当前用 aurora)。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] `.glass-surface` bg 用 `color-mix(in srgb, var(--bg-surface) N%, transparent)` 半透明。
- [ ] `.glass-surface` 含 `backdrop-filter` + `-webkit-backdrop-filter` blur+saturate。
- [ ] `yarn build` 通过。
- [ ] 浏览器实测(dark): 卡片可见半透明+背景模糊玻璃质感, 文字仍可读。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list glass-restore`)
