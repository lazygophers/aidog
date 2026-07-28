# 浮窗补 token 别名根治透明 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 浮窗背景始终透明(截图透出终端),前几轮改 60/90/100% 都无效。根因: popover.tsx 只 import popover.css,没 import globals.css → globals `:root` 里的别名 token (--bg-floating=var(--popover) / --text-primary=var(--foreground) / --glass-edge=var(--border) 等) 在 popover 文档全 undefined。`background: var(--bg-floating)` 无 fallback → 失效 → 窗口 transparent 透出桌面。applyTheme 只 inline 写基础 var(--popover/--foreground/--muted-foreground/--border),不写别名。目标: 浮窗背景实心深色 100% 不透明。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: src/styles/popover.css 顶部补一段 `:root` 别名块,映射 popover.css 用到的别名 → applyTheme 已写的基础 var + dark fallback(--bg-floating→var(--popover,#1c1c20) / --text-primary→var(--foreground,#ededf0) / --text-secondary·--text-tertiary→var(--muted-foreground,#9a9aa3) / --glass-edge→var(--border,rgba(255,255,255,0.08)))。
- [x] 范围外: 不 import globals.css(会带 Tailwind + body::before 32s 动画 + app 背景毁圆角透明); 不动 popover.tsx(applyTheme("dark") 已在); 不动置顶/失焦; 不动卡片布局。
- [x] 约束: 圆角外仍透明(窗口 transparent + border-radius); dark fallback 防 applyTheme 执行前首帧闪透。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] popover.css :root 别名块补齐; `.popover-root` background 解析为实心深色(非 transparent); 圆角外透明保留; `yarn build` 过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list popover-token-fix`)
