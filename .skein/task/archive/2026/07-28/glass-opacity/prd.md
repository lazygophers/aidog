# 冰块玻璃质感 (ice glass) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 用户要玻璃从「磨砂」改「冰块」质感: 更透 (卡片底不透明度 ~10%) + 去磨砂 blur + 冰块清透光泽 + 收敛右下角阴影明显度。
- [x] 成功: 卡片呈清透冰块 —— 底 10% 半透, 无 frost blur, 对角高光 sheen + 亮边 + 薄内影(冰厚感); 全局 drop shadow 更轻。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/styles/globals.css` .glass/.glass-surface —— 底 color-mix 降 10%(hover ~16%), 去 backdrop-filter blur(磨砂), 加冰块对角高光 + 亮白边 + inset 薄影; `src/themes/mono.ts` --shadow-sm/md/lg light+dark alpha/offset 约减半(右下阴影更淡)。
- [x] 范围外: 不改金蓝色板/primary/accent; 不改 radius; 不动语义色; 不改 .glass-elevated(浮层保持实底可读)。
- [x] 约束: 去 blur 后卡片直接透出背景, 靠高光+亮边+透明度读作冰块非纯透明; 阴影仅降明显度不删。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] .glass/.glass-surface 底不透明度 ~10%, 无 backdrop-filter blur。
- [ ] 卡片有对角高光 sheen + 亮白顶边 (冰块光泽)。
- [ ] mono.ts shadow-sm/md/lg 明显减淡 (右下 drop shadow 更轻)。
- [ ] `yarn build` 通过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list glass-opacity`)
