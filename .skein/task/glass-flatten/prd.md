# 扁平化玻璃卡面 — PRD

## 目标
- [x] 用户反复反馈"很丑, 各种阴影处理有问题"。放大截图确认根因: .glass/.glass-surface 冰块对角白渐变 sheen (rgba(255,255,255,.22)→.05 135deg) 在深色卡上读作左上亮右下暗的脏灰团, 重白 inset 加剧。这是"丑"的真源, 非 token。
- [x] 成功: 卡面纯色填充 (var(--bg-surface)) + 细边 + 单柔阴影, 现代扁平干净; 保留 flow-border hover 发光边框 + 极淡顶发丝。

## 边界
- [x] 范围内: globals.css .glass / .glass:hover / .glass-surface / .glass-surface:hover / .glass-elevated 重写。
- [x] 范围外: 不动 mono.ts token; 不动 flow-border conic; 不动业务组件。

## 验收标准
- [x] .glass/.glass-surface 去对角白渐变 background, 改 var(--bg-surface) 纯色填充。
- [x] 去重白 inset glow, 仅留极淡顶发丝 (inset 0 1px 0 低 alpha)。
- [x] hover 用细边高亮 + shadow-md, 不再叠白 sheen。
- [x] flow-border 发光边框保留生效。
- [x] yarn build 通过 + 双模截图卡面干净无脏灰团。

## 索引
- task.json (脚本真值)
