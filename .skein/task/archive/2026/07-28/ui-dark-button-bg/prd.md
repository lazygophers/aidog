# dark 按钮白底修复 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] dark 模式下侧栏底部主题/模式/语言选择器(及全站所有 `variant=ghost`/`link` 按钮)显示浏览器 UA 默认 `ButtonFace` 浅灰(rgb(239,239,239))白底, 与暗背景刺眼割裂。
- [ ] 根因: `globals.css:3` Tailwind v4 preflight 禁用 + 无 button reset, ghost/link variant 不设 base 背景 → UA ButtonFace 透出。
- [ ] 附带: 侧栏语言选择器渲染裸 key `lang.zh-CN`(locale id 与 `lang.*` key 不匹配)。
- [ ] 成功: dark 模式 ghost/link 按钮透明背景, 语言项显示译名。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: `src/components/ui/button.tsx` ghost/link variant 补 `bg-transparent`; 侧栏 lang picker key 与 locale id 对齐。
- [ ] 范围外: 不启用 preflight(migration 未完, 风险大); 不改 default/outline/secondary/destructive variant(已有显式 bg)。
- [ ] 约束: `bg-transparent` 是 utility 层, hover:bg-accent 伪类特异性更高故 hover 仍生效。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] ghost variant 含 `bg-transparent`; link variant 含 `bg-transparent`。
- [ ] 侧栏语言项显示译名(如「简体中文」)非裸 key。
- [ ] `yarn build` 通过; `yarn test` 通过; `scripts/check-i18n.mjs` 零缺失。
- [ ] 浏览器 dark 模式实测: 底部 3 选择器背景透明/暗, 非白底。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list ui-dark-button-bg`)
