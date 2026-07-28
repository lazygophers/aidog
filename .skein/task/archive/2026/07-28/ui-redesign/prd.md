# 全量 UI/UX + 配色重设计 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 用户抱怨卡片阴影过突出、快捷操作主按钮过 3D/金属/不协调、下拉菜单/工具栏图标按钮/toggle/tooltip 未做暗色适配且丑。目标: 把选定设计方向落地到真实 token 系统 (mono.ts + globals.css + 组件), 让全应用双模视觉现代、协调、扁平。
- [x] 成功: light=方向B 柔感微光 (淡紫蓝渐变底+双层柔阴影+大圆角+tinted近白卡面); dark=方向A 处理但底色用C (#0a0a0c 中性近黑+中性深灰卡面+金主色); 保留发光边框 (glass hover 蓝金流光 conic 描边+亮发丝边缘); 主按钮扁平化去金属渐变+外发光; 6 抱怨组件全部协调。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: `src/themes/mono.ts` light/dark token 重配; `src/styles/globals.css` 扁平 `.bg-primary` + 收敛 `.glass-elevated`/卡片阴影 + 保留 flow-border shimmer; 修 4 抱怨组件 (下拉菜单 popover token 双模 / 工具栏图标按钮 / toggle / tooltip)。
- [x] 范围外: 不改布局结构/信息架构; 不加新依赖; 不改业务逻辑; 不动主题切换机制 (applyTheme/data-mode 保留)。
- [x] 约束: 语义 token 驱动, bg 必配 foreground; 保留 flow-border conic shimmer 签名效; 对比度正文 ≥4.5:1; `yarn build` 必须过。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] `mono.ts` light: 淡紫蓝渐变底 (via --app-bg-overlay) + tinted 近白卡面 + 柔和偏蓝阴影 (--shadow-color 偏蓝) + 蓝主色; dark: --background #0a0a0c + 中性深灰卡面 #161619/#1c1c20 + border rgba(255,255,255,.07/.08) + 金主色 #ffd98a。
- [x] `globals.css` `.bg-primary` 去金属多段渐变+外发光, 改近纯色/单柔渐变; 卡片阴影整体更轻; flow-border conic shimmer + 亮发丝边缘保留。
- [x] 下拉菜单/工具栏图标按钮/toggle/tooltip 双模正确、协调。
- [x] `yarn build` 通过。

## 索引
- [ ] 详细设计: [direction-approved.md](direction-approved.md) (huashu gate, 落地契约真值源)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list ui-redesign`)
