# accent 金色加深 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] light 主题 `--accent` 金色 (#fbefd3) 太浅,用户要稍深。改为 #fac76c。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/themes/mono.ts` light `--accent` 单值 #fbefd3→#fac76c。
- [x] 范围外: 不动 dark accent (#20242c 灰非金)、不动 accent-foreground、不动其他 token。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] mono.ts light `--accent` = "#fac76c"；`yarn build` 过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list accent-gold-deepen`)
