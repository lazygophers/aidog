# Implement · 浮窗智能布局

执行：单 worktree `.worktrees/06-20-popover-smart-layout` 内串行 S1→S2→S3→S4，每 subtask 完成跑门禁后 main 提交一个 commit 再派下一个。

## Checklist

### S1 · 数据模型 (gate: cargo test/clippy)
- [ ] models.rs: `RowMeta` struct + `PopoverItem` 加 row/size/color + `PopoverConfig` 加 rows，全带 serde default
- [ ] `TrayColor` 确保 impl Default(mode=follow)
- [ ] api.ts: 同步 RowMeta/PopoverItem/PopoverConfig 字段(optional)
- [ ] models.rs test: 旧 JSON(无新字段)反序列化兜底 + 新字段往返
- [ ] `cd src-tauri && cargo test && cargo clippy`

### S2 · 网格渲染+密度 (gate: yarn build)
- [ ] popover.tsx: 按 effectiveRow(=row??order)分组，每行 grid-template-columns:repeat(cols,1fr)
- [ ] popover.css: `.popover-grid-row` grid + `.pc-s/.pc-m/.pc-l` 尺寸变体
- [ ] 各卡片组件按 size 渲染密度变体(s/m/l，见 design 密度表)
- [ ] item.color → resolveColor() 上色数值
- [ ] `yarn build`(tsc 通过) + 手动开浮窗验证多列/尺寸

### S3 · 配置 UI (gate: yarn build)
- [ ] 每行列数选择控件(1/2/3) → 写 rows[].cols
- [ ] 二维拖拽：rectSortingStrategy 优先，跨行落点反推 row/order(不稳则多容器)
- [ ] 加行/删空行操作
- [ ] 每卡尺寸选择(s/m/l)
- [ ] 每卡颜色编辑器(follow/preset/custom-hex，复用 Tray 模式)
- [ ] `yarn build` + 手动验证拖拽吸附/列数/尺寸/颜色

### S4 · 实时预览 (gate: yarn build)
- [ ] 抽共享卡片渲染 `components/PopoverCards.tsx`(或 export renderItem)，popover.tsx + ConfigTab 共用
- [ ] 配置页内嵌预览区，draft state 即时重渲，套 .popover-root
- [ ] `yarn build` + 手动验证改配置预览即时反映

## Review Gate

每 subtask gate 命令绿才 commit：
```bash
cd src-tauri && cargo test && cargo clippy 2>&1 | grep -i warning  # S1
cd /Users/luoxin/persons/lyxamour/aidog && yarn build              # S2/S3/S4
```
全完成跑 `yarn check:i18n`(若配置 UI 加了文案)补全 8 locale。

## Rollback

任一 subtask 失败：`git -C .worktrees/06-20-popover-smart-layout reset --hard HEAD`(回上一 subtask commit)。整体放弃：不合并 worktree。

## 完成收尾

- bump `.version`
- worktree commit 合并回 master + 移除(task.py finish 经 after_finish hook)
- 非平凡发现落 cortex(二维拖拽 WKWebView 方案 / 密度变体)
