# s6 Liquid Glass 滚动容器审计报告

## 0. 任务前提核验（与代码事实出入）

任务描述断言「Logs ListView 4 glass-surface 嵌套 → 滚动祖先 backdrop-filter 触发 GPU 合成叠加」。

**核验结论：前提部分错误。**

- `.glass-surface` 类定义（`src/styles/globals.css:118-124`）**不带 `backdrop-filter`**：

  ```css
  .glass-surface {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-sm), inset 0 1px 0 rgba(255,255,255,0.03);
    transition: all 250ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  ```

  → 4 个 glass-surface div 嵌套**不触发** backdrop-filter GPU 合成叠加。
  → 「扁平化 glass 嵌套以减合成层」在本仓不适用（glass-surface 是 surface 样式，非真 glass layer）。

## 1. 真正带 backdrop-filter 的全局类

| 位置 | 选择器 | 用途 | 触发场景 |
|---|---|---|---|
| globals.css:108 | `.glass` | Liquid Glass 主层 | sidebar/card 等核心视觉 |
| globals.css:134 | `.glass-elevated` | 浮层玻璃 | 弹层/modal |
| globals.css:193 | `.btn` | **所有按钮** | filter bar/Pagination/header 按钮密集区 |
| globals.css:269 | `.input` | 输入框 | path 搜索 input |
| globals.css:420 | `.toast` | 临时通知 | 短时 |

## 2. Logs ListView 实际结构（含 Logs/ListView.tsx）

| 行 | className | 是否 backdrop-filter | 角色 |
|---|---|---|---|
| 59 | glass-surface | 否 | filter bar 外层（含 8+ `.btn`/1 `.input` backdrop-filter） |
| 163 | glass-surface | 否 | 空态占位 |
| 168 | glass-surface | 否 | **表格滚动容器**（overflow:auto） |
| 226 | glass-surface | 否 | 清空确认 modal（createPortal 到 body，独立合成树） |

**密集 backdrop-filter 真正来源**：filter bar 内的 `.btn`（line 193 blur(12px)）+ Pagination 的 `.btn`，非 glass-surface 本身。

## 3. 滚动祖先链

- `html/body/#root` overflow:hidden（globals.css:40）
- `App.tsx:168 <main style={{overflow:auto}}>` — 主滚动容器，**无 backdrop-filter**
- `Logs/ListView.tsx:168 <div className=glass-surface style={overflow:auto}>` — 嵌套滚动容器（表格内滚）

主滚动容器无 backdrop-filter，子层 backdrop-filter 不会因「滚动祖先」产生额外合成开销。

## 4. 本次最小改

**改动 1 处**：`src/pages/Logs/ListView.tsx:168` 表格滚动容器加 `contain: paint`。

```diff
- <div className="glass-surface" style={{ overflow: "auto" }}>
+ <div className="glass-surface" style={{ overflow: "auto", contain: "paint" }}>
```

**理由（一句话）**：CSS Containment 标准提示，隔离该容器的 paint/layout 影响范围，表格行多时减少兄弟节点（filter bar / pagination）的重绘面；不影响视觉。

**未做及原因**：

| 候选改 | 未做原因 |
|---|---|
| 扁平化 glass-surface 嵌套（去 line 168 glass-surface 类） | glass-surface 无 backdrop-filter，去类无性能收益，反而丢 border/shadow 视觉 |
| 改 `.btn` 全局 backdrop-filter | 影响全应用按钮，违反「最小改 + 不破坏主题」契约 |
| 改 `.glass`/`.glass-elevated` | Liquid Glass 主题核心，明令保留 |
| 去 line 59 filter bar 的 glass-surface | filter bar 是 Logs 视觉边界，非滚动容器，无性能问题 |
| 去 line 163 空态 glass-surface | 空态非滚动路径，无优化意义 |

## 5. 后续建议（交主 task，不在本 subtask 范围）

1. **真性能路径（如证实瓶颈）**：考虑给 `.btn`/`.input` 加 `@media (prefers-reduced-transparency: reduce)` 或在低端主题分支去掉 backdrop-filter。需先 profile 确认 `.btn` 是合成瓶颈再动，禁盲改。
2. **主题变量化 backdrop-filter**：当前 backdrop-filter 散落 5 处硬编码 blur 值（12/20/calc），可收敛到 `--glass-blur-sm/-md/-lg` 变量便于全局调档。属主题层重构，单独 task。
3. **ListView 虚拟化**：表格行 >100 时真正性能瓶颈在 DOM 节点数（每行 10+ td），非 backdrop-filter。需引入虚拟滚动（react-window 或自研），独立 task。

## 6. SPEC（供 finish sediment）

`.glass-surface` 不带 backdrop-filter（globals.css:118-124），是真 surface 样式（bg/border/shadow）；真正 backdrop-filter 在 `.glass`/`.glass-elevated`/`.btn`/`.input`/`.toast` 5 类。后续涉及「glass 嵌套性能」讨论前必须区分这两个 class，禁混为一谈。
