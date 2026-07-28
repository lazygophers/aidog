# 复制按钮深色适配 — PRD (主入口)

## 目标
- [ ] 「复制代理地址」按钮复制成功后的对勾反馈在深色模式下不可见。根因: `src/components/shared/CopyButton.tsx:112` 对勾 SVG `stroke="var(--accent)"` —— `--accent` 是背景类语义 token(配套 --accent-foreground, 明暗成对翻转), dark「静谧」下 `--accent: #20242c`(近黑炭灰) 画在深色卡面 = 不可见。目标: dark 下对勾清晰可见。

## 边界
- [ ] 范围内: 仅改 `src/components/shared/CopyButton.tsx:112`, `var(--accent)` → `var(--color-success)`(globals.css:29-32 标注 theme-independent contrast-safe on light+dark; 姊妹按钮 pages/Logs/primitives.tsx:62 已用此模式)。共享组件一处修, 三处「复制代理地址」调用点(Home.tsx:146/468, Groups/GroupListView.tsx:166)全部同时修好。
- [ ] 范围外: 不动按钮容器/code 元素样式(已用 token 无写死色); 不动 ghost variant(已带 bg-transparent); 不动其它复制按钮(GroupListItem.tsx:208 复制 group_key 是菜单版, 非本次目标, 但若同样问题可顺带评估)。

## 验收标准
- [ ] CopyButton.tsx:112 stroke 改为 var(--color-success); dark 下对勾可见; light 下仍正常; `yarn build` 过。

## 索引
- [ ] 任务/子任务/调度: task.json (`skein subtask list copybtn-dark`)
