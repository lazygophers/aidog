# 单色玻璃主题精简 (collapse 三轴→单主题) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 原三轴主题系统 (9 style × 5 palette × mode) 过重, 用户要求「只保留一种主题风格, 配色只需黑白, 别的都删」。
- [x] 用户从样例中选定「磨砂玻璃 (单色 glass)」: 黑底 + 白色半透磨砂卡片 + blur 景深 + 白顶边 sheen + 柔阴影。
- [x] 成功: 主题系统收敛为单一 mono glass 主题 × mode (dark=黑 / light=白), style 轴与 palette 轴整个删除。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: 新增 `src/themes/mono.ts` 唯一主题定义 (结构 token + shadcn 色 token, light 白/dark 黑); 精简 `types.ts`/`index.ts` (ThemeMode + ThemeDefinition + applyTheme(mode)); 删 `styles/*`(9) + `palettes/*`(5); 改 AppContext/Sidebar/popover 只留 mode 轴; 8 locale 删 `theme.style.*`/`theme.color.*` key; `globals.css` glass 类中性白磨砂 (去 --primary 染色)。
- [x] 范围外: 不改 shadcn 语义色 token 数量; 语义色 (success/warning/danger 红) 保留不单色化 (a11y); 不改 blur/radius/shadow 结构值。
- [x] 约束: DB 旧 theme 行存 {style,color,mode}, 新码只读 `.mode`/只写 `{mode}` — 无 migration 不崩; 单色主题无品牌 hue, glass veil 用中性白 (dark 磨砂霜 / light 极淡)。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] `src/themes/` 仅剩 index.ts / mono.ts / types.ts / useThemeMode.ts (styles/ palettes/ 目录删除)。
- [x] `yarn build` 通过 (tsc 无残留 ThemeStyle/ThemeColor/getAvailable* 引用)。
- [x] Sidebar 主题 UI 收敛为单个黑/白 toggle (删 style + color 双 Dropdown)。
- [x] 8 locale 无 `theme.style.*`/`theme.color.*` key, 保留 theme.dark/label/light。
- [ ] 主进程 dev app 实测: dark=黑磨砂 / light=白磨砂, 切换正常。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list glass-sidebar`)
