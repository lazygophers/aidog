# 背景更明显 + 微光流沙 (shimmer bg) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 用户要背景色更明显, 且贴近原型「微光流沙玻璃」质感。
- [x] 成功: 金蓝光晕更浓 (alpha 上调 + light 底略加蓝) + 背景缓慢流沙 shimmer 动效 (光晕微漂移/明灭), 呈微光流动感。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/themes/mono.ts` app-bg-overlay 金蓝光晕/星点 alpha 上调 + light --background 略加蓝; `src/styles/globals.css` 把 overlay 移到 body::before 加缓慢 shimmer 动画 (translate+scale+opacity 明灭, 慢周期), body 只留 --bg-base。
- [x] 范围外: 不改玻璃卡片(冰块)本身; 不改 primary/accent 色板; 不动 radius/shadow; 动效克制不喧宾夺主。
- [x] 约束: body overflow hidden, ::before 用 inset 负值防漂移露边; 动效仅装饰 pointer-events:none z-index:-1; 尊重 prefers-reduced-motion 可后续加(YAGNI)。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] app-bg 金蓝光晕明显加强 (alpha 上调, light 底更蓝)。
- [ ] 背景有缓慢流沙 shimmer 动效 (光晕漂移+明灭)。
- [ ] 玻璃卡片/色板未受影响; `yarn build` 通过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list bg-stronger`)
