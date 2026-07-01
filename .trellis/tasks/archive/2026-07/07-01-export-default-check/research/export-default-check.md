# Research: 导出默认勾选 skills/mcp 行为定位

- **Query**: 导出时 skills/mcp 默认不勾，需手动勾选；期望默认勾选 skills + mcp。定位初始化逻辑出最小改点方案（只读调研）
- **Scope**: internal
- **Date**: 2026-07-01

## Findings

### 核心结论（先看）

skills 默认不勾是**前端刻意行为**，非 bug；mcp 默认勾选已生效。两处初始化点（导出 / 导入各一）对 `scope === "skills"` 做了显式排除，注释明确引用 "F2 导入误删修复"（防 skills 导入触发 npx 自动安装）。

- **skills 当前默认 = 不勾**（ImportExport.tsx:199-205 导出，:301-307 导入）
- **mcp 当前默认 = 已勾**（无排除逻辑，落入"默认全选"路径）
- 用户说的「skills/mcp 默认不勾」实测只有 skills 命中排除，mcp 实际是勾上的 → 推测: 用户表述把两者并称，真痛点是 skills

### 文件清单

| File Path | Description |
|---|---|
| `src/components/settings/ImportExport.tsx` | 导出/导入 UI，默认勾选初始化点 |
| `src/services/api.ts:1890-1907` | `ImportItem` / `ImportPreview` 类型（无 defaultChecked 字段） |
| `src/services/api.ts:1922-1924` | `exportPreview` invoke 封装 |
| `src-tauri/src/commands/backup.rs:44-64` | `export_preview` command 实现 |
| `src-tauri/src/gateway/import_export/mod.rs:153-173` | Rust `ImportItem` / `ImportPreview` 结构 |

### 1. 导出 preview 默认勾选初始化（前端唯一控制点）

`ImportExport.tsx:187-212` `loadExportPreview`：调 `exportPreview` 拉全量 items 后，初始化 `exportSelected` Set。

**关键行 199-205**：
```ts
setExportSelected(
  new Set(
    prev.items
      .filter((it) => it.scope !== "skills")   // ← skills 显式排除
      .map((it) => itemKey(it.scope, it.key)),
  ),
);
```

- 注释（line 185, 527）：「默认全选（**skills 例外，需手动勾选**）」
- mcp scope **无排除**，走默认全选路径 → 导出 preview 展开后 mcp 条目已勾

`exportSelected` 声明点：`ImportExport.tsx:166` `useState<Set<string>>(new Set())`（初始空，由 loadExportPreview 填充）。

scope 级初始勾选（影响是否触发 preview 拉取）：`ImportExport.tsx:159-161`
```ts
const [scopes, setScopes] = useState<Set<ImportExportScope>>(
  new Set<ImportExportScope>(["platform", "group", "group_platform", "setting"]),
);
```
默认仅勾 4 个 scope，**mcp / skills 不在默认 scope 集**。用户需先勾 mcp / skills scope 卡片才会触发对应条目加载。

### 2. 导入侧对称逻辑（同类排除，确认是刻意设计）

`ImportExport.tsx:285-311` `loadPreview`，初始化 `selectedItems`：

**关键行 301-307**：
```ts
setSelectedItems(
  new Set(
    prev.items
      .filter((it) => it.scope !== "skills")
      .map((it) => itemKey(it.scope, it.key)),
  ),
);
```

注释（line 299-301）：「逐项默认全选（**排除 skills scope**：skills 导入可能触发 npx 操作，强制用户显式勾选 skills 才导入，防导入 .aidogx 默认全选误触 — 见 F2 导入误删修复）」。

→ 导出 / 导入两侧 skills 排除同因同源，是对 npx 自动执行的防御。

### 3. 后端 export_preview 返回结构（无 selected 字段）

`src-tauri/src/commands/backup.rs:46-64`：collect → export_items → 拼装 `ImportPreview { manifest, scopes, conflicts:[], counts, items }`。

Rust `ImportItem`（`mod.rs:152-160`）字段：`scope / key / label / conflict`，**无 `default_checked` / `selected` 字段**。

`ImportPreview`（`mod.rs:163-173`）：`manifest / scopes / conflicts / counts / items`，**无 per-item 选中态**。

→ 默认勾选状态 100% 由前端决定，后端是纯数据源。前端 `ImportItem` 类型（`api.ts:1891-1898`）同样无 `defaultChecked`，与 Rust 一致。

### 4. 最小改点方案

**推荐：前端单点改（YAGNI，后端零改动）**

| 选项 | 改点 | 改动量 | 评价 |
|---|---|---|---|
| **A. 仅改导出 skills 排除** | `ImportExport.tsx:202` 删 `.filter((it) => it.scope !== "skills")` | 1 行 | 改导出默认勾选，不动导入防御 |
| B. 导出 + 导入都改 | :202 + :304 删两处 filter | 2 行 | 导入侧会失去 npx 误触防御（**不建议**，违 F2 修复意图） |
| C. 后端加 defaultChecked 字段 | `mod.rs` + `backup.rs` + `api.ts` + 前端读字段初始化 | 4 处 | 过度工程，YAGNI 反例 |

**推荐 A**：仅 `ImportExport.tsx:199-205` 的 filter 去掉（或改为 scope 白名单），使 skills 条目进默认全选集。理由：

1. 导出 skills **不会触发 npx**（npx 仅导入 apply 路径触发），排除无安全依据
2. 导入侧排除（:304）保留不动 — 维持 F2「导入需用户显式勾 skills」防御
3. 后端零改动，单点 1 行
4. mcp 无需改（当前已默认全选）

**关于 mcp 默认 scope 不勾（:159-161）**：若用户期望「打开导出页 mcp 就在 preview 里」而非「勾 mcp 后其条目默认勾」，需另改 :159-161 的 `scopes` 初始 Set 加 `"mcp"` + `"skills"`。但 :159-161 改动会让首次进页就拉 skills/mcp 全量 items（更多 invoke），**与 06-30-export-ux-i18n 的 debounce 自动展开设计耦合**，推测: 不在本次范围。需要: main 确认是否要改 scope 级初始勾选（:159-161），还是只改条目级默认勾选（:202）。

### 5. 相关引用

- `ImportExport.tsx:185` 注释 — 设计意图原文
- `ImportExport.tsx:299-301` 注释 — F2 导入误删修复引用
- `ImportExport.tsx:527` UI 提示文案 — 「默认全选，skills 需手动」（改 :202 后此文案需同步）

## Caveats / Not Found

- **skills vs mcp 表述歧义**：用户说「skills/mcp 默认不勾」，代码实测 mcp 已默认全选，仅 skills 被排除。推测: 用户把两者并称，真痛点是 skills。需要: main 向用户确认是否确实只针对 skills（mcp 当前行为是否也算 bug）。
- **scope 级 vs 条目级默认**：用户描述模糊，未明确是 scope 卡片初始勾选（:159-161）还是 preview 条目初始勾选（:202）。本报告假设指条目级（与「需手动勾选」表述更匹配）。需要: main 确认范围。
- 未读 `import_export/apply.rs` 的 `export_items`（不影响结论：items 由前端初始化选中态，与后端字段无关）。
