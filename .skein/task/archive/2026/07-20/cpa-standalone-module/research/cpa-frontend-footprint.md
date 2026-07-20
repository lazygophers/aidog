# CPA 前端落点 (B)

## B1. CpaImportModal.tsx (`src/components/platforms/CpaImportModal.tsx`, 624 行)

入口组件, 三段式 UI (parse → preview 编辑 → apply)。
- Props (`CpaImportModalProps` line 59): open / onClose / onApplied。
- 状态: sourcePath / authDir / parsing / error / skipped / sourceFiles / rows (Record<name, RowState>) / order (name 序) / originals / existing (已有 Platform 列表, 查重) / applying / dragActive。
- 拖拽: Tauri `onDragDropEvent` (memory `tauri-dragdrop-event`), 两个 drop target (source/authdir), dragTargetRef 区分。
- 调 cpaImportApi.parse → 填 rows/order; 用户编辑后调 onApplied (单条 applyCpaToForm / 多条 runBatchCreateFromCpa)。

## B2. platformPasteApply.ts — CPA apply 链

`src/pages/platforms/platformPasteApply.ts`:
- `applyCpaToForm(p: MappedPlatform, ctx)` (line 357): 单条灌 PlatformEditForm 表单字段。
- `runBatchCreateFromCpa(providers, ctx)` (line 406): 多条批量 createPlatform + 进度 toast, disabled 条目 post-create 置 status (line 451 注释对齐 cpa_import.rs:115-128)。
- 调用 `platformsApi.create` (非 cpa_import_apply command, 已弃用 — 见 B3)。

## B3. api 封装 (`src/services/api/platforms.ts`)

`cpaImportApi` (line 387-399):
- `parse(path, authDir?)` → invoke `cpa_import_parse`
- `previewQuota(baseUrl, apiKey)` → invoke `cpa_import_preview_quota`
- `apply(platforms)` → invoke `cpa_import_apply`, **@deprecated line 395**: "改用前端 applyCpaToForm / runBatchCreateFromCpa"。apply command 后端仍存但前端不再调 (实际建平台走通用 platformsApi.create)。

## B4. 类型 (`src/services/api/types/part4.ts`)

镜像 Rust serde (注释 line 8)：
- `MappedPlatform` (line 11-29): protocol / name / base_url / api_key / models / extra / disabled / source_label。
- `CpaSkipReason` (line 32): skipped 文件。
- `CpaImportParseResult` (line 40-44): platforms / skipped / source_files。
- `CpaBatchFailure` (line 47) / `CpaBatchReport` (line 53-56)。

## B5. Protocol union + PROTOCOLS 数组处理

**Protocol union** (`src/services/api/types/part1.ts:32`): 含 `"cpa-grok" | "cpa-aistudio" | "cpa-antigravity" | "cpa-vertex"` (line 31 注释: CPA 导入专属)。

**PROTOCOLS 派生** (`src/domains/platforms/defaults.ts::buildProtocolsFromPresets` line 275+): **从 platform-presets.json 动态构建**, 无硬编码 cpa-*。删 JSON 的 4 cpa-* 条目后, PROTOCOLS 下拉自动消失, 无需改 constants.ts。
- `ENDPOINT_PROTOCOLS` (constants.ts:11-17): 仅 5 请求格式协议, 不含 cpa-*。
- `PROTOCOL_LABELS` (constants.ts:30-37): 仅 5 请求格式, 不含 cpa-*。cpa-* 的展示 label 走 JSON name 派生 (getProtocolLabelMap)。

**matchPlatform** (`src/utils/platformPaste.ts:321`): 智能粘贴协议识别, grep 0 命中 cpa。cpa-* 协议不在 matchPlatform 候选 (无 hosts/codingKeyPrefixes 配置), 仅能通过 CpaImportModal 显式导入。**mock 协议同样排除** (platformPaste.ts:15)。

**getDefaultEndpoints** (`defaults.ts`): 从 presets 读 cpa-* 的 endpoints.default (显式声明 wire), 给前端预览补全 base_url (如 cpa-vertex base_url="" 需用户填)。

## B6. PlatformEditForm 集成

`src/pages/platforms/PlatformEditForm.tsx`:
- line 8: import CpaImportModal。
- line 91-92: showCpaImport state, 仅新建态入口展示 (line 115 "导入 CPA 配置" 按钮)。
- line 139-151: CpaImportModal onApplied 回调, 单条 applyCpaToForm / 多条 runBatchCreateFromCpa。
- `usePlatformForm.ts:32,146-147,643-652,812`: applyCpaToForm / runBatchCreateFromCpa 暴露。
- `usePlatformsState.ts:181-182`: 同上接口声明。

## 前端清理清单 (删 cpa-* 后)

| 文件 | 操作 |
|---|---|
| `src/components/platforms/CpaImportModal.tsx` | 整文件删 (624 行) |
| `src/pages/platforms/platformPasteApply.ts` | 删 applyCpaToForm / runBatchCreateFromCpa (line 357-480) |
| `src/pages/platforms/usePlatformForm.ts` | 删 import + 2 方法暴露 (line 32,146-147,643-652,812) |
| `src/pages/platforms/usePlatformsState.ts` | 删接口声明 (line 181-182) |
| `src/pages/platforms/PlatformEditForm.tsx` | 删 import + showCpaImport + 按钮 + Modal (line 8,91-92,115-116,139-151) |
| `src/services/api/platforms.ts` | 删 cpaImportApi (line 385-399) + import MappedPlatform/CpaImportParseResult/CpaBatchReport (line 4) |
| `src/services/api/types/part4.ts` | 删 MappedPlatform/CpaSkipReason/CpaImportParseResult/CpaBatchFailure/CpaBatchReport (整文件或相关段) |
| `src/services/api/types/part1.ts` | Protocol union 删 4 cpa-* (line 32) |
| i18n `platform.cpaImport.*` | 8 语言文件删 cpaImport 键 |
| platform-presets.json | 删 4 cpa-* 条目 |

注: PROTOCOLS 数组 / matchPlatform / ENDPOINT_PROTOCOLS / PROTOCOL_LABELS **无需改** (动态派生 / 不含 cpa-*)。

**新模块设计关键约束**: 前端 cpa 入口强耦合在 PlatformEditForm (新建态按钮), 新模块若独立菜单则入口从侧栏进, 不再寄生 Platforms 页。apply 链 (platformPasteApply + usePlatformForm) 与通用 platform create 共用 platformsApi.create, 删 cpa 部分不影响通用 paste 流程。
