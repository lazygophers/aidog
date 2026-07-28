# shadcn 基础设施+主题 token 体系 — PRD (主入口)

## 目标
- [ ] 引入 shadcn/ui + Tailwind v4 到现有 Vite + React 19 + Tauri 项目(nova preset 起点对齐,项目状态见 src/themes + package.json)
- [ ] 建立 shadcn 语义色 token 体系(globals.css @theme inline 块,语义 token: --background/--foreground/--primary/--muted/--border/--radius 等)
- [ ] 现有主题系统迁移到 shadcn token: 9 Style 全保留映射 radius/blur/shadow/glass token; 调色板从 12 精简到 4 (gruvbox/nord/dracula/cattpuccin) 各生成 light+dark shadcn 语义色 token
- [ ] 保留 applyTheme 运行时切换机制 (style × color × mode), 变量名改投到 shadcn 语义 token, 主题选择器 UI 适配
- [ ] tracer-bullet 端到端穿通: 至少 1 页面用 shadcn Button + 切主题视觉变化正确, 证明整条路走得通
## 边界
- 仅基建 + 主题 token 体系 + 主题切换机制, 不改业务页面 (归 shadcn-pages) / 不批量 add 组件 (归 shadcn-primitives, 本 task 只 add button 做 tracer-bullet 验证)
- 调色板砍到 4: gruvbox + nord + dracula + cattpuccin (用户拍板); 其余 8 个 (appleBlue/solarized/rosePine/tokyoNight/oneDark/material/github/nightOwl) 删除, 用户已选设置回退默认
- 9 Style 全保留 (liquidGlass/flat/soft/sharp/aurora/paper/terminal/bento/sketchy), 映射 radius/blur/shadow/glass 到 shadcn token
- 保留运行时主题切换产品功能 (applyTheme + 主题选择器), 不改 Tauri 后端 / 不改 i18n key 体系 (locale 文案沿用)
- 不引入 SSR (Tauri 桌面端), 所有组件视为 client (isRSC=false)
## 验收标准
- [x] shadcn init 成功: components.json + vite + tailwind v4 配置就绪, npx shadcn@latest info 显示正确 project context
- [x] cn() util (clsx + tailwind-merge) 可用, import 路径符合 components.json aliases
- [x] globals.css 含完整 shadcn token (@theme inline 块, nova preset 语义色 + radius)
- [x] 4 调色板各生成 light + dark shadcn 语义色 token, applyTheme(color) 切换 --primary/--background 等生效
- [x] 9 Style 的 radius/blur/shadow/glass token 切换生效, applyTheme(style) 视觉变化正确
- [x] applyTheme(style,color,mode) 三轴运行时切换工作, 无残留旧变量 (clearThemeKeys 清并集)
- [x] 主题选择器 UI (Settings 内) 正常显示 4 色 × 9 样式 × light/dark
- [x] tracer-bullet: Home 或 About 页 1 处用 shadcn Button, 切主题视觉跟着变
- [x] yarn build 过 + yarn test (vitest) 过无回归 + cargo 无关 (本 task 不碰 Rust)
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list shadcn-infra`)
