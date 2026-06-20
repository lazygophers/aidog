---
id: S2
slug: render-grid
deliverable: D1
parent-task: 06-20-popover-smart-layout
status: planned
execution-layer: sub-agent
isolation: worktree
depends-on: [S1]
blocks: [S3, S4]
estimated-tokens: 35000
---

# S2 · 浮窗网格渲染 + 尺寸密度变体

## 目标

浮窗 `popover.tsx` 改为按 row 分组的二维网格渲染(每行 `grid-template-columns:repeat(cols,1fr)`)，各卡片支持 s/m/l 密度变体，item.color 上色数值。

## 产出

- `src/popover.tsx`：行分组 grid 渲染 + renderItem 读 size/color
- `src/styles/popover.css`：`.popover-grid-row` grid + `.pc-s/.pc-m/.pc-l` 尺寸变体
- 各卡片组件 s/m/l 密度分支(见 design 密度表)

## 验证

```bash
cd /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout && yarn build
```

期望：tsc + vite build 退出码 0。手动开浮窗：设 rows[0].cols=2 该行并排；size 改 s/l 视觉密度变化(需配合 S3 或手改 DB 配置验证，S2 阶段可临时硬编码 config 验证渲染)。

## 资源

- 独占文件：`src/popover.tsx` `src/styles/popover.css`
- 审批槽位：否

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S1 | PopoverItem.row/size/color + PopoverConfig.rows 字段 | 编译通过 |

## 执行细节

按 design.md「渲染算法」+「密度变体约定」节：
- `effectiveRow = item.row ?? item.order`；按 row 分 Map，行号升序
- 每行 cols = `config.rows?.[row]?.cols ?? 1`
- renderItem 加 size 参数 → 各卡密度分支；color → 复用 `resolveColor()`(popover.tsx:96-110)上色
- WKWebView 注意：纯 CSS grid 无 DnD，无 wkwebview-html5-dnd-drop-fails 风险

### Dispatch Prompt

```
Active task: .trellis/tasks/06-20-popover-smart-layout
# isolation: worktree (复用 S1 同一 worktree, 基于 S1 commit)

## 目标
popover.tsx 按 row 分组二维网格渲染 + 各卡 s/m/l 密度变体 + item.color 上色。

## 已知
- S1 已加 PopoverItem.row/size/color、PopoverConfig.rows、resolveColor() 已能解析 TrayColor
- 现渲染 popover.tsx:625-635 单列；卡片组件位置见 design/会话报告(MetricRow/PlatformToday/CostTrendCard/Group* 等)
- effectiveRow=row??order；cols=rows[row]?.cols??1
- 读 design.md「渲染算法」「密度变体约定」为准

## 工作目录与范围
- cwd: /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout
- 可改: src/popover.tsx, src/styles/popover.css
- 禁改: src/services/api.ts, src/pages/**, .trellis/**, **/dist/**

## 输出格式
diff。

## 验收标准
yarn build 退出码 0。各卡 s/m/l 有可见密度差异；多列 grid 正确。

## 失败处理
- 瞬时错误→重试1次
- 某卡密度变体设计不清→按 design 密度表，仍不清输出 `需要:` 停
- 业务阻塞→报 Blocked

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn。
```

## 回滚

- 触发：yarn build 红 / 渲染崩
- 步骤：`git -C .worktrees/06-20-popover-smart-layout reset --hard HEAD`

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 3 列 + l 卡空间不足挤压 | 视觉差 | design 已提示紧凑回退/文档说明，不强锁 |
| 卡片密度变体改动面大 | 工时超 | 优先 s/m/l 仅尺寸差，富信息按卡逐个加 |

## 历史

- 2026-06-20: created
