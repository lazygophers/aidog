# 金色主题色 (gold accent) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 用户要求主题色改金色, 且要「金闪闪」金属光泽感。
- [x] 成功: mono 主题的强调色 (primary/ring/accent) 全换金色族, 主按钮带金属金渐变 sheen + 柔金 glow 呈现「闪」感; 黑白 glass 磨砂底与 a11y 语义色不变。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/themes/mono.ts` light/dark 的 --primary/--primary-foreground/--ring/--accent/--accent-foreground/--border 换金色; `globals.css` 给主按钮 (.bg-primary / shadcn primary Button) 加金属金 linear-gradient sheen + 柔金 box-shadow glow。
- [x] 范围外: 不改 glass 磨砂底黑白 (background/card/popover/secondary/muted 保持中性); 不改语义色 success/warning/danger (a11y); 不改结构 token (radius/blur/shadow)。
- [x] 约束: 金字底黑 fg 金属经典读法 (primary-foreground 近黑 #1a1400); dark 用高亮金 #e6c34d 提亮, light 用深金 #c9a227 保白底对比; 金字/金按钮上文字对比 ≥ 4.5:1。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] mono.ts light/dark primary/ring/accent 均为金色族 hex/rgba。
- [ ] 主按钮呈金属金渐变 + glow (非纯平色块)。
- [ ] glass 底、语义红仍为黑白/红 (未被金污染)。
- [ ] `yarn build` 通过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list gold-accent`)
