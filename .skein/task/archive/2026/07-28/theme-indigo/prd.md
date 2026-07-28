# 靛蓝冷石板主题配色 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 默认色板 gruvbox 暖褐(泥棕/橙)审美陈旧、割裂, 用户反馈「审美太丑」。
- [ ] 新增 `indigo`(靛蓝·冷石板)色板: 冷靛蓝主色 + 冷灰(slate)中性阶, 专业信任调, 高对比。
- [ ] 设为默认色板(DEFAULT_COLOR="indigo"), light + dark 双模式完整。
- [ ] 成功: 应用默认呈现冷靛蓝专业风; 对比度达标; 语言选择器等 UI 组件视觉协调。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: 新建 `src/themes/palettes/indigo.ts`; `types.ts` ThemeColor 加 "indigo"; `index.ts` 注册 + DEFAULT_COLOR; 8 locale 加 `theme.color.indigo`。
- [ ] 范围外: 不删既有 4 色板(gruvbox/nord/dracula/catppuccin, 用户仍可切); 不改 style 轴; 不改语义色(globals.css 固定 --color-success/warning/danger)。
- [ ] 约束: palette 只需 21 个 shadcn token(同 gruvbox.ts schema); 派生 token(text-*/bg-glass/accent-subtle)由 globals.css 自动派生, 禁重复定义。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] `indigo.ts` 含 light+dark 各 21 个 shadcn token, 结构对齐 gruvbox.ts。
- [ ] ThemeColor union 含 "indigo"; paletteMap 注册 indigo; DEFAULT_COLOR="indigo"。
- [ ] 8 locale 均有 `theme.color.indigo`(译名)。
- [ ] `yarn build` 通过; `yarn test` 通过; `scripts/check-i18n.mjs` 零缺失。
- [ ] 浏览器实测(dark+light): 默认呈冷靛蓝, 主色按钮靛蓝, 中性面板冷灰, 无白底/割裂。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list theme-indigo`)
