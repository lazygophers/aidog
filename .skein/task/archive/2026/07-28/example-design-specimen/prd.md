# example/ 设计样例: 黑/亮双模主题色+组件+动效+图表 — PRD (主入口)

## 目标
- [ ] example/ 目录下单 HTML 自包含样例文件, 浏览器直接打开
- [ ] data-mode toggle light/dark 双模即时切换
- [ ] 内联 mono 主题色变量 + globals.css 玻璃签名 + 全动效
- [ ] 覆盖色板/排版/按钮/表单/反馈/导航/数据/动效/图表全分区
- [ ] 作为设计稿对齐基准 (后续 UI 改动参照此样例风格)
## 边界
- 新建: example/design-specimen.html (单文件, 自包含)
- 引用源: src/themes/mono.ts (色值) + src/styles/globals.css (玻璃/动效 CSS) + src/utils/chart.ts (smoothPath) + src/components/shared/colorScale.ts/usageColor.ts (色阶)
- 不动: 任何 src/ 代码, vite 配置, package.json
## 验收标准
- [ ] 浏览器打开 example/design-specimen.html 无 console 错误, 无外部请求 (纯本地)
- [ ] light/dark toggle 切换, 全部色板/组件/图表随之换色
- [ ] 玻璃 .glass hover 触发 flow-border 发光边框
- [ ] 动效可见: spinner 旋转 / skeleton shimmer / pulseGlow 脉冲 / statusPulse
- [ ] 图表: SVG 折线 (smoothPath 平滑曲线) + 环形 (progress) + 柱状 + colorScale/usageColor 色阶条
- [ ] shadcn 组件风格 (圆角/阴影/间距) 与 src/components/ui/ 一致
- [ ] 双模对比: primary light 蓝(#0087EB) / dark 金(#FFD98A) 互换签名色正确呈现
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list example-design-specimen`)
