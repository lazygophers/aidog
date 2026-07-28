# 金蓝双色主题 (gold+blue glass) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 用户要金色+蓝色主题, 参考 `ccplugin/prototype/skein-glass-prototype.html`「微光流沙玻璃」。
- [x] 成功: mono 主题呈金+蓝双色玻璃 —— light 晨曦(蓝为主 accent + 暖金光晕 + 薰衣草蓝底), dark 夜空金沙(金为主 accent/辉 + 蓝 active/glow + 深 navy 底 + 金星点)。保留玻璃磨砂质感 + a11y 语义色。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/themes/mono.ts` 全色 token 重映射(金蓝双色, 双模式); `src/styles/globals.css` app-bg 分层金蓝 radial(light 晨曦/dark 金星点+蓝光晕) + glass 顶边 sheen(蓝白/金蓝) + hover 蓝金 conic 流光描边 + .bg-primary 金属渐变随 --primary 自适应。
- [x] 范围外: 不加数字递增/卡片入场 JS(aidog 卡片非 skein 看板, 无 data-count); 不改 status 语义色相(success/warning/danger a11y 固定); 不改结构 token(radius/blur)。
- [x] 约束: 原型色值 —— 蓝 #0087EB/#3BA0FF/#B1E3FF, 金 #FFD98A/#E8B860; light 蓝 primary 白 fg, dark 金 primary 黑 fg(#1a1206); 文字对比 ≥ 4.5:1。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] mono.ts light primary 蓝系 / dark primary 金系, accent 互为签名色。
- [ ] app-bg light 晨曦金蓝光晕 / dark 深 navy + 金星点 + 蓝光晕。
- [ ] hover 卡片有蓝金流光描边; 主按钮金属渐变(蓝 light/金 dark)。
- [ ] glass 磨砂 + 语义红仍在; `yarn build` 通过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list gold-blue`)
