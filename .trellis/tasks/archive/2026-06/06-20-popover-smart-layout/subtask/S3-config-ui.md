---
id: S3
slug: config-ui
deliverable: D1
parent-task: 06-20-popover-smart-layout
status: planned
execution-layer: sub-agent
isolation: worktree
depends-on: [S2]
blocks: [S4]
estimated-tokens: 45000
---

# S3 · 配置 UI(列数/二维拖拽/尺寸/颜色)

## 目标

`PopoverConfigTab` 加：每行列数选择(1/2/3)、卡片跨行/行内拖拽吸附、加/删行、每卡尺寸选择(s/m/l)、每卡颜色编辑器(follow/preset/custom-hex)。

## 产出

- `src/pages/PopoverConfigTab.tsx`：行列数控件 + 二维拖拽 + 加删行 + 尺寸选择 + 颜色编辑器
- `src/components/SortableList.tsx`：扩展支持二维(rectSortingStrategy/多容器)
- i18n：新文案补 8 locale(若加)

## 验证

```bash
cd /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout && yarn build && yarn check:i18n
```

期望：yarn build 退出码 0；check:i18n 绿。手动：拖卡跨行/行内吸附生效；改 cols/size/color 保存后浮窗反映。

## 资源

- 独占文件：`src/pages/PopoverConfigTab.tsx` `src/components/SortableList.tsx`
- 审批槽位：否

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S2 | 浮窗按 row/cols/size/color 渲染(改配置能看到效果) | 编译通过 + 渲染就绪 |

## 执行细节

按 design.md「拖拽方案」「颜色编辑器」节：
- 列数：每行加 1/2/3 选择 → 写 `config.rows[row].cols`
- 拖拽：复用 @dnd-kit PointerSensor(WKWebView HTML5 DnD 失效，记忆 wkwebview-html5-dnd-drop-fails，PointerSensor 不依赖原生 DnD)。优先 flat rectSortingStrategy 落点反推 row/order；不稳退多容器(每行一 SortableContext + onDragOver 检测目标行)。实测选定，结论必须落 cortex。
- 加行/删空行操作 → 维护 rows[] 与 items.row 一致
- 尺寸：每卡 s/m/l 选择 → item.size
- 颜色：复用 TrayConfigTab 颜色编辑模式(follow/preset/custom)，custom 给 hex input(6 位校验)

### Dispatch Prompt

```
Active task: .trellis/tasks/06-20-popover-smart-layout
# isolation: worktree (复用同一 worktree, 基于 S2 commit)

## 目标
PopoverConfigTab 加每行列数选择 + 二维拖拽吸附 + 加删行 + 每卡尺寸 + 每卡颜色编辑器。

## 已知
- S1 模型 + S2 渲染已就绪(改 config 浮窗能反映)
- 现配置 UI: PopoverConfigTab.tsx:86-454(SortableList @dnd-kit verticalListSortingStrategy 单列)
- WKWebView HTML5 DnD drop 失效→必用 PointerSensor，二维实测 rectSortingStrategy 优先，不稳退多容器
- 颜色编辑复用 TrayConfigTab 的 follow/preset/custom 模式
- 读 design.md「拖拽方案」「颜色编辑器」为准

## 工作目录与范围
- cwd: /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout
- 可改: src/pages/PopoverConfigTab.tsx, src/components/SortableList.tsx, src/locales/*.json(新文案)
- 禁改: src/popover.tsx, src/services/api.ts, .trellis/**, **/dist/**

## 输出格式
diff。

## 验收标准
yarn build + yarn check:i18n 绿。拖拽跨行/行内吸附生效；cols/size/color 改后浮窗反映。

## 失败处理
- 瞬时错误→重试1次
- 二维拖拽 flat 方案落点判定不稳→退多容器方案(design 已授权)
- 业务阻塞→报 Blocked
- 二维拖拽 WKWebView 实测结论→落 cortex

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn。
```

## 回滚

- 触发：yarn build/check:i18n 红 / 拖拽不可用
- 步骤：`git -C .worktrees/06-20-popover-smart-layout reset --hard HEAD`

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 二维跨行拖拽 WKWebView 不稳 | 核心交互失效 | PointerSensor + elementFromPoint hit-test，flat 不稳退多容器 |
| 新文案漏 locale | check:i18n 红 | 跑 check:i18n 补全 8 语言 |

## 历史

- 2026-06-20: created
