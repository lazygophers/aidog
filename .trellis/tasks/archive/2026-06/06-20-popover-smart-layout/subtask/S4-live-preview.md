---
id: S4
slug: live-preview
deliverable: D1
parent-task: 06-20-popover-smart-layout
status: planned
execution-layer: sub-agent
isolation: worktree
depends-on: [S3]
blocks: []
estimated-tokens: 35000
---

# S4 · 配置页内嵌实时预览

## 目标

抽共享卡片渲染组件，配置页内嵌预览区复用真实卡片渲染浮窗外观，改任一配置(draft state，未保存)即时反映。

## 产出

- `src/components/PopoverCards.tsx`(或 popover.tsx export renderItem)：共享卡片渲染，浮窗 + 配置页共用
- `src/pages/PopoverConfigTab.tsx`：内嵌预览区，draft state 即时重渲
- `src/popover.tsx`：改为消费抽出的共享组件(单一事实源)

## 验证

```bash
cd /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout && yarn build
```

期望：yarn build 退出码 0。手动：配置页改 cols/size/color/拖拽 → 预览区即时反映(无需保存)；真实浮窗与预览外观一致。

## 资源

- 独占文件：`src/pages/PopoverConfigTab.tsx` `src/popover.tsx` + 新建 `src/components/PopoverCards.tsx`
- 审批槽位：否

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S3 | 配置 UI(cols/拖拽/尺寸/颜色)完成，draft state 结构定 | 编译通过 |

## 执行细节

按 design.md「预览」节：
- 抽 popover.tsx 卡片渲染为 `components/PopoverCards.tsx`，浮窗与配置页共用 → 避免预览/实际漂移
- 预览区用 ConfigTab 本地 draft state(未保存 items/rows) 渲染，套 `.popover-root` 同款容器
- 预览数据复用 ConfigTab 已轮询 stats(usePolling 30s)，不需真实浮窗数据通道
- 注意：popover.tsx 抽取后回归——真实浮窗仍正常

### Dispatch Prompt

```
Active task: .trellis/tasks/06-20-popover-smart-layout
# isolation: worktree (复用同一 worktree, 基于 S3 commit)

## 目标
抽共享卡片组件 components/PopoverCards.tsx，配置页内嵌预览区复用真实卡片渲染，draft state 即时反映。

## 已知
- S1-S3 完成：模型/渲染/配置 UI 就绪
- popover.tsx renderItem 现内联在浮窗，需抽为共享组件供 ConfigTab 复用
- ConfigTab 已有 usePolling stats(30s)，预览数据复用之
- 套 .popover-root 容器；预览用未保存的 draft state
- 读 design.md「预览」为准

## 工作目录与范围
- cwd: /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout
- 可改: src/components/PopoverCards.tsx(新建), src/popover.tsx, src/pages/PopoverConfigTab.tsx
- 禁改: src/services/api.ts, src-tauri/**, .trellis/**, **/dist/**

## 输出格式
diff。

## 验收标准
yarn build 退出码 0。配置改动预览即时反映；真实浮窗抽取后无回归(外观与预览一致)。

## 失败处理
- 瞬时错误→重试1次
- 抽取破坏真实浮窗→优先保浮窗正常，预览次之
- 业务阻塞→报 Blocked

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn。
```

## 回滚

- 触发：yarn build 红 / 真实浮窗回归
- 步骤：`git -C .worktrees/06-20-popover-smart-layout reset --hard HEAD`

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 抽取卡片组件破坏真实浮窗 | 浮窗坏 | 抽取后双验证浮窗 + 预览 |
| 预览数据通道与真实不一致 | 预览误导 | 共用渲染组件 + 复用同款 stats |

## 历史

- 2026-06-20: created
