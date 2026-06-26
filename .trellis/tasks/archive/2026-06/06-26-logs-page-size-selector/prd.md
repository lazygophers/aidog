# PRD — 请求日志默认每页 20 条且可调档

## 背景

请求日志页（`src/pages/Logs.tsx`）当前每页固定 `PAGE_SIZE = 50`（line 19 常量硬编码）。用户要求：**默认改为 20 条/页**，并**允许用户手动修改**每页条数。

用户决策（AskUserQuestion）：交互形态 = **下拉选择器固定档位**（工具栏/分页区加 page size 下拉，固定档位 20/50/100，默认 20），**不持久化偏好**（切页面重置回默认 20）。

## 现状核查（planning 已查证）

| 链路 | 文件:行 | 现状 |
|---|---|---|
| 每页条数常量 | Logs.tsx:19 `const PAGE_SIZE = 50` | 硬编码常量 |
| 列表拉取用 PAGE_SIZE | Logs.tsx:171 `listFiltered(activeFilter, PAGE_SIZE, offset)` / 178 `list(PAGE_SIZE, offset)` | 传 limit |
| load 依赖 | Logs.tsx:186 `[offset, hasFilter, activeFilter]` | 无 pageSize |
| filter 变化重置 offset | Logs.tsx:191 `useEffect(() => setOffset(0), [hasFilter, activeFilter])` | — |
| totalPages / currentPage | Logs.tsx:427-428 用 PAGE_SIZE 计算 | — |
| Pagination 组件 props | Logs.tsx:578-586 传 `pageSize={PAGE_SIZE}` + `onPageChange` | 已有 pageSize 入参 |
| Pagination 渲染 | Logs.tsx:662-722 footer（左 range 文本 `rangeStart–rangeEnd / total` + 右页码按钮组） | 下拉档位插此处左侧最自然 |

后端 `proxyLogApi.list/listFiltered` 已接受 limit 参数（line 171/178），无需改后端。

## 目标

1. 默认每页 **20** 条（替换原 50）。
2. 分页区加**固定档位下拉**（20/50/100），用户可手动切换，立即按新档位重新分页。
3. 切档时重置到第 1 页（offset=0），避免越界空页。
4. 不持久化（页面重新挂载回默认 20）。

## 范围

### 必做
1. **常量改 state**：`const PAGE_SIZE = 50` → `const [pageSize, setPageSize] = useState(20)`（默认 20）。
2. **替换全部 PAGE_SIZE 引用**为 `pageSize`：load（171/178）、totalPages（427）、currentPage（428）、Pagination `pageSize` prop（583）、`onPageChange` 内 `(page-1)*PAGE_SIZE`（584）。
3. **load useCallback 依赖加 `pageSize`**（186 行 deps 数组），确保切档触发重新拉取。
4. **切档重置 offset**：pageSize 变化时 `setOffset(0)`（扩展 191 行 reset effect 的依赖数组，或新增 effect）。避免高档位→低档位时 offset 越界。
5. **加档位下拉选择器**：在 `Pagination` 组件 footer 左侧（range 文本旁）插入档位 `<select>`，固定选项 20/50/100，绑定 `pageSize` + `onPageSizeChange` 回调。Pagination 增 `pageSize` 已有、需新增 `onPageSizeChange?: (size: number) => void` prop，从 Logs 主组件传 `setPageSize`。下拉须复用项目既有控件风格（查 Logs.tsx 内既有 FilterSelect / `<select>` 用法，沿用 Liquid Glass 样式，禁裸原生无样式 select）。
6. **i18n**：下拉标签/aria（如 `logs.pageSize` = "每页"）须补全 **8 locale**（src/locales/*.json），跑 `scripts/check-i18n.mjs` 零缺失。档位数字 20/50/100 是纯数字无需翻译。

### 不做（明确排除）
- 不改后端 Rust（list/listFiltered 已接 limit 参数）。
- 不持久化偏好（无 localStorage / 无后端 settings）。
- 不改任意值输入框（用户选了固定档位方案）。
- 不动 Pagination 既有页码按钮逻辑（7 按钮省略号）。

## 验证

- `yarn build`（tsc + vite）零错误零 warning。
- `node scripts/check-i18n.mjs`（或 package.json 对应脚本）零缺失（新 key 8 locale 全覆盖）。
- 手工逻辑核对：默认进页 = 20 条/页；切到 50/100 → 立即重拉且回第 1 页；range 文本与 totalPages 随档位正确重算。
- 无范围外改动（仅 `src/pages/Logs.tsx` + `src/locales/*.json`）。

## 资源

- 改：`src/pages/Logs.tsx`（PAGE_SIZE→state + 6 处引用 + load deps + offset 重置 + Pagination 加下拉）
- 改：`src/locales/*.json`（8 locale 新增 `logs.pageSize` 类 key）
- 参考（不改）：`proxyLogApi.list/listFiltered/count`（后端已接 limit）
- 现有 memory：无直接相关（Logs 分页此前无 memory）

## 依赖

无外部依赖。单一前端交付，单 worktree，串行 implement → check → finish。

## 风险/取舍

- 切档不重置 offset 会导致高档位深翻页后切低档位 offset 越界 → 空页。必须重置 offset=0（必做 #4）。
- 下拉控件须沿用既有样式，禁引入裸原生 select 破坏 Liquid Glass 视觉一致性。
