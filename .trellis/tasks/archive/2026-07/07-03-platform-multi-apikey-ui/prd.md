# PRD — 平台添加多 apikey 智能识别批量创建 UI

## 现状（已有基建，复用非重造）
- `usePlatformForm.ts:545-556 handleSave`：创建态 + 非 keyOptional + `splitApiKeys(apiKey).length > 1` → **直接** `runBatchCreateFromPaste(keys)` 批量创建
- `platformPasteApply.ts:191-213` 智能粘贴：多 key → 灌表单 + `void runBatchCreateFromPaste` **立刻**创建（L211）
- `runBatchCreateFromPaste`（platformPasteApply.ts:236）：批量循环成熟，每 key 一平台，name=`{base}-{尾4位}` 撞名追号，进度 toast `批量创建中… done/total`，失败不中断末尾汇总「成功 X / 失败 Y + 失败 key」
- `splitApiKeys`：多行/逗号/空白/分号分隔拆分
- ApiKeyField（formSections.tsx）：单 input，onChange setApiKey

**差距**：当前「输入/粘贴多 key 即直接创建」，缺预览确认层。用户要「识别 → 填表单 → 展示预览 → 确认后才批量创建」。

## 目标
添加平台表单支持多 apikey 智能识别 + **实时预览**（只读确认）+ 确认后批量创建。智能粘贴路径同步改预览（不再立刻创建）。

## 决策（用户 2026-07-03 AskUserQuestion）
| # | 决策点 | 锁定 |
|---|---|---|
| D1 | 预览触发 | **实时预览** —— apikey input onChange 拆分多 key → 表单下方实时显「将创建 N 个平台」列表 + 确认按钮 |
| D2 | 预览可编辑性 | **只读确认** —— name 自动生成复用现有规则 `{base}-{尾4位}` 撞名追号，列表纯展示，不支持改 name / 跳过 key |
| D3 | 智能粘贴路径 | **同步改预览** —— 粘贴多 key 也灌表单显预览，不再 `void runBatchCreateFromPaste` 立刻创建 |

## 方案

### 1. 预览组件 `<MultiKeyPreview>`（新建，pages/platforms/ 内）
- props：`keys: string[]`（splitApiKeys 结果）、`baseName: string`、`protocol`、`baseUrl`、`usedNames: Set<string>`（撞名预览）、`onConfirm: () => void`、`t`
- 渲染：标题「将创建 {{N}} 个平台」+ 列表（每行：#序号 + name 预览 `{base}-{尾4位}` 撞名追号预览 + 协议 + base_url + key 尾4位掩码）+ 「确认批量创建」按钮
- name 撞名追号预览逻辑抽 shared util（platformPasteApply.ts 现有 runBatchCreateFromPaste 内联逻辑 L266-275 抽出 `previewBatchNames(keys, baseName, usedNames): string[]`，runBatchCreate 内复用同一函数，保证预览 = 实际创建名一致）

### 2. usePlatformForm 改
- 新增 state `batchPreviewKeys: string[] | null`（null = 非批量态，数组 = 待确认预览）
- apikey onChange：创建态 + 非 keyOptional + splitApiKeys.length > 1 → setBatchPreviewKeys(keys)；否则 null
- handleSave：多 key 时不直接 runBatchCreateFromPaste，而是若 batchPreviewKeys 非 null 提示「请确认下方批量预览」（或禁用保存按钮，引导点预览确认）
- 新增 `confirmBatchCreate()`：调 runBatchCreateFromPaste(batchPreviewKeys) → 成功后 resetForm + 关表单

### 3. platformPasteApply 智能粘贴改（L191-213）
- 多 key 路径：不再 `void runBatchCreateFromPaste`（删 L211）
- 改：灌表单 setApiKey(多 key 拼接文本，让用户可见) + setBatchPreviewKeys 触发预览（与手动表单多 key 同路径，统一预览 UX）
- 单 key 路径不变（L217 setApiKey(keys[0])）

### 4. i18n（8 locale 全补）
新增 key：
- `platform.batch.previewTitle` 「将创建 {{count}} 个平台」
- `platform.batch.previewHint` 「确认后将批量创建，name 自动生成 `{{base}}-key尾4位`，撞名追号」
- `platform.batch.confirm` 「确认批量创建」
- `platform.batch.cancel` 「取消」

## 验收
- [ ] 创建态表单 apikey input 粘/输多 key（换行/逗号/分号分隔）→ 下方实时显「将创建 N 个平台」预览列表
- [ ] 预览列表 name = `{base}-{尾4位}` 撞名追号，与实际创建名一致（previewBatchNames shared util）
- [ ] 点「确认批量创建」→ 复用 runBatchCreateFromPaste → 进度 toast + 末尾汇总「成功 X / 失败 Y」
- [ ] 编辑态（editing != null）多 key 不触发预览（apiKey 是已存在平台单值，原样保存）
- [ ] keyOptional 平台（透传/opencode_zen）apiKey 留空不触发预览
- [ ] 智能粘贴多 key → 灌表单显预览（不再立刻创建）
- [ ] 智能粘贴单 key → 直接灌表单（原行为）
- [ ] 预览只读（无改 name / 跳过 key UI）
- [ ] yarn build 绿 + check-i18n exit 0（8 locale key 齐全）

## 非目标
- 不改 runBatchCreateFromPaste 核心批量逻辑（只改触发时机 + 抽 name 预览 util）
- 不改 Platform 数据模型（api_key 仍单字段，多 key = 多平台）
- 不改后端 command（platformApi.create 不变，批量 = 前端循环 N 次调用）
- 不加 name 可编辑 / key 勾选（D2 只读）

## 调度
- 单 subtask（前端单域改动，≤4 文件：MultiKeyPreview 新建 + usePlatformForm + platformPasteApply + i18n 8 locale）
- 依赖：arch-redesign 阶段 6 Platforms 子文件拆分**不冲突**（本 task 改 usePlatformForm/platformPasteApply 内部逻辑，arch 阶段 4 已拆完 Platforms；阶段 6 拆其他文件 Skills/Mcp/Logs 等，文件集不相交）。但需协调 worktree（本 task 独立 worktree，基于含 arch Platforms 拆分的 master）
- 并发：active 集当前 arch + mitm 满，本 task 等 active 位空出再 start

## 风险 + 缓解
| 风险 | 缓解 |
|---|---|
| name 预览与实际创建不一致（撞名态在创建中变化）| previewBatchNames shared util 单源，预览用当前 usedNames 快照，runBatchCreate 内循环动态追号（预览提示「撞名追号」即可，不强求预览 = 最终名 100%）|
| 粘贴多 key 灌表单 apiKey 文本太长 UI 撑爆 | ApiKeyField 已有 showKey toggle 掩码；多 key 时显「N 个 key」摘要 + 预览列表承载详情 |
| arch 阶段 6 同时改 pages/platforms/ | 本 task start 前确认 arch 阶段 6 Platforms 相关 subtask 已合（或基于 arch worktree 分支起 worktree）；文件锁 usePlatformForm.ts/platformPasteApply.ts |
