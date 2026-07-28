# backdrop-filter 用量收敛 —— 审计报告（阶段 A）

subtask: `frontend-compositing-purge/s6-backdrop-audit`

## 1. 全量清点表

| file:line | 选择器 / 属性 | 实例数量级 | blur 半径 | 背景是否不透明 |
|---|---|---|---|---|
| `globals.css:227-234` `.glass-elevated` | 每 glass-elevated 卡面 1 个 | 37 个 tsx 文件引用（`grep -l` 计），design.md 记 51 处实例 | 30px（`--glass-blur`+10px=30px @ mono 主题）| **是** —— `background: var(--bg-floating)` → `var(--popover)`，两主题两模式全部 `oklch(... )` 纯色，无 alpha |
| `globals.css:292`（**已删**）`.btn` | 每按钮 1 个 | design.md 记 12 处；`\bbtn\b` 全仓匹配 12 | 12px | **是** —— `background: var(--bg-glass)` → `var(--card)`，无 alpha |
| `globals.css:368`（**已删**）`.input` | 每输入框 1 个 | design.md 记 47 处；实测 `\binput\b` 全仓匹配 131（含原生 `<input>` 标签，非全部走 `.input` 类），量级与 design.md 一致 | 12px | **是** —— 同上 `var(--bg-glass)` |
| `globals.css:519` `.toast` | 每条 toast 1 个（同屏通常 ≤2） | 11 个 tsx 文件引用 | 20px | **是** —— `background: var(--bg-floating)`，无 alpha |
| `SectionAnchorNav.tsx:35-36`（**已删**）内联 `backdropFilter` | 1（sticky 导航条） | 1 | 20px | **是** —— `background: "var(--bg-glass)"` |
| `SettingsHeader.tsx:78-79`（**已删**）内联 `backdropFilter` | 1（sticky header） | 1 | 20px | **是** —— 同上 |
| `CodexSettings.tsx:163-164`（**已删**）内联 `backdropFilter` | 1（sticky header） | 1 | 20px | **是** —— 同上 |
| `SearchableProtocolSelect.tsx:167-168` 内联 `backdropFilter` | 1（下拉触发器） | 1 | 12px | **否** —— `background: "color-mix(in srgb, var(--card) 80%, transparent)"`，真 20% 透明 |

**核心判据**：CSS 规范下 `backdrop-filter` 生效顺序是「先对元素下方内容取样并模糊 → 再在其上绘制该元素自身的 `background`」。若 `background` 是 100% 不透明色，模糊结果会被完全遮盖，用户**物理上看不见任何 blur 效果** —— 这不是主观视觉判断，是渲染管线的必然结果。

用这把尺子逐一核查全仓 8 处 `backdrop-filter` 声明的 `background`：`--bg-glass` 恒等于 `--card`，`--bg-floating` 恒等于 `--popover`；两者在 `globals.css:44-48`（light 默认）、`globals.css:717-723`（dark 覆盖）、`themes/mono.ts`（当前生效主题，light/dark 两组）里全部是纯 `oklch()` / 十六进制色，**零处**带 alpha 通道或 `color-mix(...transparent)`。唯一例外是 `SearchableProtocolSelect.tsx` 手写的 `color-mix(in srgb, var(--card) 80%, transparent)` —— 这是全仓唯一一处背景真透明、blur 真正在做事的地方。

## 2. 分类与去留判断

### 小控件（`.btn` / `.input` / 3 处 sticky-bar 内联）—— **去，已执行**
核心工作项 + 顺手项（同一诊断、同一 tsx-inline 授权范围内）。理由：背景全不透明，blur **零视觉输出**，纯粹是白付的合成层开销（59+3=62 层）。删除是零风险操作 —— 不是「牺牲视觉换性能」，是删除从未生效过的代码。

### 浮层（`.glass-elevated` / `.toast`）—— **本次不动，回传 main 裁定**
同样满足「背景全不透明→blur 零效果」的判据，且量级更大（`.glass-elevated` 单项 30px blur × 37+ 文件引用，很可能是全仓最大的单项合成层开销来源，超过我已删的 62 处小控件之和）。**未纳入本次执行**，原因：
1. 超出本 subtask 明确授权范围（team-lead brief 核心工作项仅列 `.input`/`.btn`，`.glass-elevated` 被框定为「大面积玻璃基底、风格承载体」保留项）
2. 若这是设计上「等未来给 `--bg-floating` 接透明度」的未完成特性（而非死代码），贸然删除可能与后续设计意图冲突，需要用户/main 确认这是「死代码」还是「待激活特性」

**建议 main 单开一个 subtask 复核**：若确认是死代码，`.glass-elevated`（51 处）+ `.toast`（11 处）的清除预期收益 > 本次已做的全部工作。

### 唯一真透明项 —— `SearchableProtocolSelect.tsx:167` —— **留**
`color-mix(in srgb, var(--card) 80%, transparent)`，backdrop-filter 在此处货真价实地模糊了下拉触发器背后的内容，是全仓仅有的一处名副其实的「液态玻璃」。不动。

## 3. 保留项理由（逐条）

- **`.glass-elevated`（未删，待复核）**：暂留是流程保守选择，非视觉必要性判断（技术上此刻它和已删的 `.btn`/`.input` 同属死代码），见上「浮层」分类说明。
- **`SearchableProtocolSelect.tsx` 下拉触发器**：背景真 20% 透明，blur 是必要的可读性保障（否则文字会叠在被透出的杂乱背景上难以辨认）；1 处、独立合成层，代价极低，收益是唯一真实的。

## 4. 视觉比对依据

| 改动 | 改前 | 改后 | 视觉影响 |
|---|---|---|---|
| `.btn` / `.input` 去 blur | `background: var(--card)`（不透明）+ `backdrop-filter: blur(12px)` | `background: var(--card)`（不透明），无 backdrop-filter | **零差异** —— 不透明背景完全遮蔽 backdrop，改动前后像素级渲染结果理论一致（blur 作用域内容从未穿透 background 显形） |
| 3 处 sticky-bar 去 blur | `background: var(--bg-glass)`（不透明）+ `backdropFilter: "blur(20px)"` | 同背景，无 backdropFilter | **零差异**，理由同上 |

无需补偿手段（不透明度提升 / 静态渐变等）—— 因为改动前后视觉输出本就相同，不存在「补」的必要。

## 5. 量测数据（阶段 B，待办）

**当前仍在阶段 A**，尚未申请到量测窗口，本节留空。已完成清单：
- [x] 全量清点（8 处声明，逐条判背景透明度）
- [x] 分类与去留判断
- [x] 执行 `.btn`/`.input`/3 处内联 sticky-bar 共 5 处 CSS/TSX 改动（详见 git diff）
- [ ] WebKit malloc 量测数据 + 三项自证（等 main 放行量测窗口后补）
