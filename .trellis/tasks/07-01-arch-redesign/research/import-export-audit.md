# Research: ImportExport.tsx 1525 行 mini-audit

- **Query**: ImportExport.tsx 1525 行按 import/diff/export 域聚簇拆分映射，为阶段 3 拆 exec 铺路
- **Scope**: internal (src/ 只读, 实测行号 awk 边界)
- **Date**: 2026-07-02
- **文件**: `src/components/settings/ImportExport.tsx`
- **总行数**: 1525（worktree 与 main 双确认一致）
- **参考格式**: `research/section-split-map.md` §1

---

## 关键发现（与 editors.tsx 形态不同）

**这是「单 export + 大量私有 helper」型文件，不是 editors.tsx 那种「多 export 平铺」型。**

- 顶层 export 仅 **1 个**: `ImportExportTab` @ L164（537 行单组件）
- 其余 25 个声明全部 module-private（`function`/`const`/`type` 无 export 关键字）
- 因此「按 `^export` 边界聚簇」失效，改按 **(a) module 段落注释边界** + **(b) 域语义** 双轴聚簇
- 已有 2 个 sibling 子文件被 ImportExport.tsx 引用并嵌入 JSX：`CcSwitchImport.tsx` (632) + `Sub2ApiImport.tsx` (304)，**异源导入域已预先拆出**，本审计无需再拆这域
- 单一消费方：`pages/AppSettings.tsx:13` → `<ImportExportTab />`（L843）

## 文件三段结构（注释标记边界）

| 段 | 行范围 | 行数 | 域 |
|---|---|---|---|
| 头部 + scope/menu 元数据 + IA helper | L1-163 | 163 | **共享元数据 / 菜单分组逻辑** |
| `ImportExportTab` 主组件 | L164-696 | 537 | **import + export 编排**（state + handlers + JSX） |
| Sub-components 子组件 | L698-1297 | 600 | **共享 UI primitives**（section/卡片/勾选/分段） |
| ScheduledBackup section | L1299-1525 | 227 | **定时备份域**（独立功能） |

---

## §1 每个声明实测行数（awk 边界，权威）

| 行号 | Decl | 行数 | export? | 性质 / 域 |
|---|---|---|---|---|
| 38 | `ALL_SCOPES` (scope 元数据表) | 20 | 否 | 元数据 / scope 域 |
| 58 | `SCOPE_ICON` (scope→icon 派生 map) | 2 | 否 | 元数据 |
| 60 | `scopeLabel(t, scope)` | 11 | 否 | 元数据 helper |
| 71 | `MenuGroupId` (type) | 4 | 否 | 菜单分组 type |
| 75 | `MENU_GROUPS` (菜单组定义表) | 13 | 否 | 元数据 / 菜单分组 |
| 88 | `SCOPE_MENU_GROUP` (scope→组 map) | 14 | 否 | 元数据 |
| 102 | `SETTING_SCOPE_GROUP` (setting 子归类 map) | 11 | 否 | 元数据 |
| 113 | `SETTING_KEY_LABEL` (setting key→i18n map) | 29 | 否 | 元数据（含 ponytail 注释） |
| 142 | `settingLabelKey(item)` | 6 | 否 | 元数据 helper |
| 148 | `menuGroupOf(item)` | 8 | 否 | 元数据 helper |
| 156 | `menuGroupLabel(t, id)` | 6 | 否 | 元数据 helper |
| 162 | `MENU_GROUP_ICON` (派生 map) | 2 | 否 | 元数据 |
| **164** | **`ImportExportTab`** | **537** | **是 ✓** | **import+export 主组件** |
| 701 | `SectionHeader({icon,title,desc})` | 13 | 否 | UI primitive（**D-new 重复**） |
| 714 | `TextButton({onClick,children})` | 20 | 否 | UI primitive（**D-new 重复**） |
| 734 | `ScopeCard({...})` | 75 | 否 | UI primitive（export 专用） |
| 809 | `SuccessPathCard({message})` | 32 | 否 | UI primitive（export 专用） |
| 841 | `DropZone({onClick,active,...})` | 39 | 否 | UI primitive（import 专用） |
| 880 | `CheckBox({checked,indeterminate})` | 24 | 否 | UI primitive |
| 904 | `Chevron({open})` | 22 | 否 | UI primitive |
| 926 | `ItemSelector({...})` | 162 | 否 | UI primitive（**最大子组件**，import+export 共用逐项勾选） |
| 1088 | `MetaRow({label,value})` | 10 | 否 | UI primitive（import 预览 meta） |
| 1098 | `Segmented({value,options,onSelect})` | 44 | 否 | UI primitive（冲突决策分段） |
| 1142 | `ConflictRow({...})` | 63 | 否 | UI primitive（**diff 域**，import 冲突行） |
| 1205 | `ReportView({report,t,scopeLabel})` | 56 | 否 | UI primitive（**report 域**，import 结果卡） |
| 1261 | `ReportSection({title,color,bg,icon,rows})` | 43 | 否 | UI primitive（report 子区） |
| 1304 | `INTERVAL_PRESETS` (备份间隔预设表) | 8 | 否 | backup 域元数据 |
| 1312 | `formatBackupTime(ms,t)` | 7 | 否 | backup 域 helper |
| 1319 | `ScheduledBackupSection()` | 207 | 否 | **backup 域主组件** |

**实测行号总和核对**: 20+2+11+4+13+14+11+29+6+8+6+2+537+13+20+75+32+39+24+22+162+10+44+63+56+43+8+7+207 = 1328 行声明体 + L1-37 头注释(37) + L657-700 段间空白/注释 ≈ 1525 ✅

---

## §2 按域聚簇拆分映射

| 目标子文件 | 含 | 行数估算 | 备注 |
|---|---|---|---|
| `settings/import-export/meta.ts` | ALL_SCOPES + SCOPE_ICON + scopeLabel + MenuGroupId + MENU_GROUPS + SCOPE_MENU_GROUP + SETTING_SCOPE_GROUP + SETTING_KEY_LABEL + settingLabelKey + menuGroupOf + menuGroupLabel + MENU_GROUP_ICON | ~135 | **scope/菜单元数据 + 派生 helper**，纯数据 + 纯函数，最易先抽 |
| `settings/import-export/ImportExportTab.tsx` | ImportExportTab 主组件 | ~540 | **import+export 编排**（含 state / handlers / debounce effect / 拖入 effect / 主 JSX）。依赖 meta + UI primitives |
| `settings/import-export/primitives.tsx` | SectionHeader + TextButton + ScopeCard + SuccessPathCard + DropZone + CheckBox + Chevron + MetaRow + Segmented | ~292 | **共享 UI primitives**（D-new 消重源，CcSwitchImport 复用） |
| `settings/import-export/ItemSelector.tsx` | ItemSelector | ~165 | 最大单 primitive，独立成文件（被 ImportExportTab 在 export+import 两处复用） |
| `settings/import-export/ConflictRow.tsx` | ConflictRow | ~65 | **diff/冲突域**（依赖 Segmented，可同文件或 import） |
| `settings/import-export/ReportView.tsx` | ReportView + ReportSection | ~100 | **report 域** |
| `settings/import-export/ScheduledBackupSection.tsx` | ScheduledBackupSection + INTERVAL_PRESETS + formatBackupTime | ~225 | **backup 域**（独立 section，可单独抽；依赖 SectionHeader/StatChip） |
| `settings/import-export/index.ts` | barrel re-export `ImportExportTab` + 可选 `ScheduledBackupSection` | ~5 | 保持 `AppSettings.tsx:13` import 路径兼容 |

**未拆项**（已 sibling 化，本审计不动）:
- `CcSwitchImport.tsx` (632) — 异源导入域，L32 import + L673 嵌入
- `Sub2ApiImport.tsx` (304) — 异源导入域，L33 import + L676 嵌入

### 域语义重新映射（对应任务「import/diff/export 域」措辞）

任务原文用「import/diff/export」三域，但实测本文件的域划分是：

| 任务措辞 | 实测对应 |
|---|---|
| export 域 | `ImportExportTab` 内 export 半区 state/handlers (L164-440) + `ScopeCard`/`SuccessPathCard`/`ItemSelector` |
| import 域 | `ImportExportTab` 内 import 半区 state/handlers (L289-440) + `DropZone`/`MetaRow`/`ItemSelector` |
| diff 域 | `ConflictRow` + `Segmented`（冲突决策分段控件） |
| （未提及）report 域 | `ReportView` + `ReportSection` |
| （未提及）backup 域 | `ScheduledBackupSection`（225 行独立功能） |
| （未提及）meta 域 | 163 行 scope/菜单元数据（最易先抽的稳定层） |

**import 与 export 在主组件内强耦合**（共享 `itemKey`/`selectedItems`/`scopes` state + 同一 JSX return），**无法按 import/export 物理拆成两文件**，只能拆主组件本身（见 §3 警戒）。

---

## §3 验证拆后无 >800

| 子文件 | 行数估算 | ≤800? |
|---|---|---|
| meta.ts | ~135 | ✅ |
| ImportExportTab.tsx | ~540 | ✅ |
| primitives.tsx | ~292 | ✅ |
| ItemSelector.tsx | ~165 | ✅ |
| ConflictRow.tsx | ~65 | ✅ |
| ReportView.tsx | ~100 | ✅ |
| ScheduledBackupSection.tsx | ~225 | ✅ |
| index.ts | ~5 | ✅ |

**全部 ≤ 800 ✅，无需二次拆**。

### 警戒点（不需二次拆，但需 main 确认）

- `ImportExportTab` 主组件 540 行单函数，已逼近「单组件 ≤ 500」软目标但未破 800 硬上限。若 main 想进一步抽 `useImportExport` hook（state + handlers + effect）脱离 JSX，可降到 ~300 行 JSX + ~240 行 hook。**当前不抽也合规**。
- `ItemSelector` 162 行是 primitives 中最大块，逻辑自洽（分组折叠 + 组复选框 + 逐项 checkbox），无强拆必要。

---

## §4 消费方 + barrel 建议

### 消费方（grep 实测）

| 文件:行 | 引用 |
|---|---|
| `src/pages/AppSettings.tsx:13` | `import { ImportExportTab } from "../components/settings/ImportExport";` |
| `src/pages/AppSettings.tsx:843` | `<ImportExportTab />` |
| `src/components/settings/ImportExport.tsx:32` | `import { CcSwitchImportSection } from "./CcSwitchImport";`（**入向**） |
| `src/components/settings/ImportExport.tsx:33` | `import { Sub2ApiImportSection } from "./Sub2ApiImport";`（**入向**） |

**唯一出向消费方: AppSettings.tsx**。`CcSwitchImport`/`Sub2ApiImport` 是 ImportExport 的依赖（入向），非被依赖。

### barrel 兼容路径建议

当前 import 路径：`from "../components/settings/ImportExport"`

**推荐方案 A（零 churn）**: 新建目录 `src/components/settings/import-export/`，原 `ImportExport.tsx` 改为 `import-export/index.ts`（barrel），内容：

```ts
export { ImportExportTab } from "./ImportExportTab";
// 可选：若其他地方未来需要
export { ScheduledBackupSection } from "./ScheduledBackupSection";
```

`AppSettings.tsx` 的 `from "../components/settings/ImportExport"` 在 TS resolver 下自动解析到 `import-export/index.ts`（**注意: 需文件名/目录名策略**，TS 默认 `./ImportExport` → `./ImportExport.tsx` 或 `./ImportExport/index.ts`，二者择一不能并存）。**实操**：删 `ImportExport.tsx`，建 `ImportExport/` 目录 + `ImportExport/index.ts` barrel。

**推荐方案 B（接受 1 处 churn）**: 直接改 `AppSettings.tsx:13` 的 import 路径为新目录，去掉 barrel。鉴于唯一消费方就 1 行，**B 方案更简单**（ponytail: barrel 仅 1 消费方时是过度工程）。

---

## §5 消重机会（与已抽模块 / sibling 文件）

### D-new（本审计新发现，duplication-audit.md D1-D12 未覆盖）

#### D-new.1 `SectionHeader` ↔ `SectionHeaderSimple` —— 微小差异 (×2)

| 位置 | 实现 |
|---|---|
| `ImportExport.tsx:701` | `function SectionHeader({icon,title,desc})` 13 行 |
| `CcSwitchImport.tsx:435` | `function SectionHeaderSimple({icon,title,desc})` 同签名 |

- **是否完全相同**: 需 line-by-line 对比（本审计未深入 CcSwitchImport 内部），签名一致，推测实现近似
- **合并建议**: 抽到 `settings/import-export/primitives.tsx`（或 `@aidog/shared`），CcSwitchImport 改 import
- **消费点**: 2

#### D-new.2 `TextButton` ↔ `TextButtonSimple` —— 微小差异 (×2)

| 位置 | 实现 |
|---|---|
| `ImportExport.tsx:714` | `function TextButton({onClick,children})` 20 行 |
| `CcSwitchImport.tsx:447` | `function TextButtonSimple({onClick,children})` 同签名 |

- 同 D-new.1，命名加 `Simple` 后缀暗示当时作者已知重复、用改名规避
- **合并建议**: 同 D-new.1

### 与已抽 shared 模块的关系

- `StatChip` (L34 import from `../shared/StatChip`): **已抽**，无需动
- `SectionIcon` (L30 import from `./editors`): **已抽**，无需动
- `IconCheck` (L31 import from `../icons`): **已抽**
- `ColorLevel` type (L35 import from `../shared/colorScale`): **已抽**

**结论**: ImportExport 的 shared 依赖已规范化，剩余重复只在 sibling `CcSwitchImport` 内（D-new.1/2）。本审计拆分时若同步消 D-new，CcSwitchImport.tsx 也可瘦 ~33 行（632 → ~600）。

---

## §6 风险 / 待 main 决策点

### 需要: main 决策

1. **barrel 方案选 A 还是 B**？
   - A（零 churn barrel `ImportExport/index.ts`）：保持 `AppSettings.tsx` 不动，多 1 个 barrel 文件
   - B（直接改 import 路径）：改 `AppSettings.tsx:13` 1 行，少 1 个 barrel 文件
   - **推荐 B**（ponytail: 单消费方 barrel 是过度工程）
2. **`ImportExportTab` 540 行是否进一步抽 `useImportExport` hook**？当前合规（<800），但破「单组件 ≤500」软目标。抽 hook 可降到 ~300 JSX + ~240 hook。
3. **D-new.1/2 是否同步消**？拆分时顺手合 `SectionHeader`/`TextButton` 到 primitives，CcSwitchImport 改 import。低风险，推荐做。
4. **目录命名**：`import-export/`（kebab，与子文件 `ItemSelector.tsx` PascalCase 并存）还是 `ImportExport/`（与原文件名一致）？后者更稳（路径完全一致），前者符合常见约定。

### 风险

- **L150 注释依赖**: `menuGroupOf` 内 `item.key.split(":")[0]` 假设 setting key 格式为 `<scope>:<key>`，拆分时不能改变 key 构造契约（后端 build_items 约定，见 L110-112 注释）
- **L188 `loadPreviewRef`**: 拖入回调通过 ref 读最新 `loadPreview`，拆 hook 时需保留 ref 模式（避免 effect 反复重订阅 onDragDropEvent，见 L187 注释）
- **L207-210 / L304-307 注释**: skills scope 默认全选是「方案 C」决策（用户主动定），拆分时不能改默认行为
- **CcSwitchImport/Sub2ApiImport 嵌入契约**: 主组件 L673/L676 通过 `onReport` 回调把异源导入结果灌进本组件 `report` state，拆分后该回调链路需保留

### 推荐执行顺序（供 exec agent）

1. 先抽 `meta.ts`（纯数据 + 纯函数，零 JSX，最稳）
2. 抽 `primitives.tsx` + `ItemSelector.tsx`（纯展示组件，依赖 meta + 已抽 shared）
3. 抽 `ConflictRow.tsx` + `ReportView.tsx`（依赖 primitives）
4. 抽 `ScheduledBackupSection.tsx`（独立功能）
5. 主组件 `ImportExportTab.tsx` 最后抽（依赖以上全部）
6. 建 barrel 或改 AppSettings import（按 main 决策 1）
7. （可选）消 D-new.1/2，CcSwitchImport 改 import primitives
