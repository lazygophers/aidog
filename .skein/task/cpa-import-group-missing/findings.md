# 调研收敛 — CPA 导入平台未落到指定分组

## 现象
用户从分组上下文(lockedGroupId)或表单设了分组后, 点 CPA 导入创建平台, 结果平台未关联到指定分组(走默认 auto_group)。

## 根因
apply 链两端都没传 group:

**前端** `CpaImportModal`(CpaImportModal.tsx:66):
- props 仅 `{ open, onClose, onApplied }`, **无 group 输入**
- `handleApply`(CpaImportModal.tsx:353-372) payload = `MappedPlatform[]`(结构无 group 字段)
- 调用点 `PlatformEditForm.tsx:140-156` 渲染 CpaImportModal 时**没传 form 的 group state**

**后端** `cpa_import_apply`(cpa_import.rs:86-129):
- 签名 `(platforms: Vec<MappedPlatform>, db)`, 无 group 参数
- CreatePlatform 写死 `auto_group: Some(true)` + `join_group_ids: None`(cpa_import.rs:107-108)

## form group state(usePlatformForm.ts:214-224)
- `autoGroup: boolean`(默认 true)
- `joinGroupIds: number[]`
- `lockedGroupId: number | null`(从分组页打开新建时锁定)

用户「指定分组」= form 已设的 group(lockedGroupId 或手选 joinGroupIds)。CPA 导入应复用。

## 修法方向
apply 链透传 group:
- 前端: CpaImportModal 加 group props(从 PlatformEditForm form state 传), apply payload 带 group
- 后端: cpa_import_apply 加 group 参数, CreatePlatform 用传入值替代写死

待 brainstorm: group 粒度(全批共享参数 vs MappedPlatform per-platform 字段) + UI(modal 显示/改 group vs 静默用 form group)

## 引用
- CpaImportModal.tsx:66 props 定义(无 group)
- CpaImportModal.tsx:353-372 handleApply payload 构造
- PlatformEditForm.tsx:140-156 CpaImportModal 渲染(未传 group)
- cpa_import.rs:86-129 cpa_import_apply(CreatePlatform 写死 auto_group/join_group_ids)
- usePlatformForm.ts:214-224 form group state
- formSections.tsx:782-817 GroupAssignSection(表单分组 UI)
