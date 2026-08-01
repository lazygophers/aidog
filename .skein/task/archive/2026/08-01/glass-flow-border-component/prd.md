# 流光边框重构为独立组件 — PRD (主入口)

## 目标
- [ ] 把蓝金流光描边从「全局挂在 .glass / .glass-surface 上」改成「显式 opt-in 的独立类」，让流光成为少数几处的签名效果而非 116 处元素的默认附赠
- [ ] 顺带消除无谓的 ::after 伪元素（当前每个 .glass / .glass-surface 都无条件生成一个）
## 边界
- 🔴 本 task 不以性能为由 —— 原登记理由「@property 逐帧动画是全局 ~50% CPU 底噪」已被 frontend-compositing-purge/s2-flow-border 证伪（animation 只挂 :hover::after，空闲态零 tick），用户拍板改目标为纯视觉重构
- 禁做「两层 DOM（静态裁切壳 + 旋转渐变层）替代 @property」—— 该方案省不出东西反而增 DOM，已作废
- 不改流光本身的视觉表现（conic 渐变色值 / 3s 周期 / hover 触发 / mask 收在 hover 的既有优化一律保留）
- 不动 .glass / .glass-surface 的其余样式（背景 / 边框 / 圆角 / 阴影）
- opt-in 点位需逐个人工指定，不做全量自动迁移
## 验收标准
- [x] 流光描边不再由 .glass / .glass-surface 全局挂载：globals.css 中 .glass::after / .glass-surface::after 的 conic-gradient + opacity 规则已移除，改由独立 opt-in 类（如 .flow-border）承载
- [x] opt-in 点位限于「页面顶层主卡片」：PlatformCard / GroupCard / 首页统计卡等页面主体卡片，逐个显式加类；设置页表单块、弹窗、导入导出面板、内嵌小容器一律不加
- [x] 流光视觉表现零变化：保留 opt-in 元素的 conic 色值、3s 周期、hover 触发时机、mask/mask-composite 收在 :hover 内的既有优化，与改前逐帧一致
- [x] .glass / .glass-surface 的其余样式（背景、边框、圆角、阴影、position: relative）不受影响，非 opt-in 元素外观除「无流光」外无其他变化
- [x] yarn build / yarn test / node scripts/check-i18n.mjs 全绿
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list glass-flow-border-component`)
