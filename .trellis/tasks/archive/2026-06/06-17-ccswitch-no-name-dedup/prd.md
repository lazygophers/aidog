# cc-switch 导入禁按 platform name 去重

## 背景
platform 表 `name` **无 UNIQUE 约束** (migrations/001_init.sql:8)。group 唯一性在 path。
但 cc-switch 导入 + .aidogx 导入的 apply 路径**按 platform name 去重/覆盖**:
- `apply.rs:492 upsert_platform_row`: `SELECT id FROM platform WHERE name=? AND deleted_at=0` →
  多同名时取**任一行 UPDATE** = 覆盖错平台 (数据完整性 bug)。
- `apply.rs:60 detect_conflicts`: name 命中标冲突 (误报)。
- `ccswitch.rs:239-246`: 收集 existing_platform_names 传前端。
- `CcSwitchImport.tsx:153,325`: existingNames.has(name) → 冲突 UI (overwrite/skip/rename)。

## 目标 (用户确认)
1. **总建新**: platform 导入保留原名, 不检测冲突/不 overwrite/skip/rename。重复导入同
   provider = 列表两个同名 platform (用户明确接受)。
2. **修共享 apply 路径**: cc-switch + .aidogx 两路径都改 (name 非唯一是通用真相, 无稳定
   跨机 platform identity → always-insert 是唯一正确行为)。

## 设计
### 后端 (apply.rs)
- `upsert_platform_row`: 删 `SELECT id WHERE name → UPDATE` 分支, **总 INSERT** 新行
  (insert_platform_row)。effective_name 仍尊重 rename decision (若 .aidogx 传 rename) —
  但 cc-switch 路径不再产生 platform 决策 (见下)。保留 effective_name 参数供 rename 兼容。
- `detect_conflicts`: 删 platform scope 冲突扫描 (lines 64-81)。group/setting/file 冲突保留
  (那些 key 唯一)。
- `resolve_name` 对 platform: skip 决策仍生效 (用户主动跳过), rename 仍生效; 仅 overwrite
  不再语义化为"覆盖现有"而是"建新" (overwrite 对 platform = default insert)。

### cc-switch 后端 (ccswitch.rs)
- 删 `existing_platform_names` 字段 (CcswitchReadResult) + 239-246 收集逻辑。
- read() 不再 list_platforms (省一次 DB 查询)。

### 前端 (CcSwitchImport.tsx)
- 删 existingNames state (65) + setExistingNames (107) + handlePreview name 冲突 (153) +
  isConflict (325)。
- handlePreview: platform scope 不再有冲突项 → conflicts 始终空 → decisions 始终空。
- 导入按钮: 不依赖冲突决策, 直接全部选中建新。
- UI: 不再显示"已存在同名平台"冲突行 + overwrite/skip/rename 单选 (platform 项)。

### 不改
- `relink_group_platform` (apply.rs:360 按 name 找 pid): cc-switch payload.group_platform 空;
  .aidogx export 的 group_platform 是导出文件内已确定的 name 对, 导入后平台用原名, relink
  能找到刚建的行。同名歧义是边缘案例, 不在本任务 scope (留 TODO)。
- Decision 枚举: 保留 (group 仍用 overwrite/skip/rename)。

## 验证
1. cc-switch 导入同 provider 两次 → 列表两同名 platform (id 不同), 无覆盖。
2. .aidogx 导入含同名 platform → 建新, 不覆盖现有。
3. cargo test (import_export 相关) + clippy 0 warning。
4. yarn build (tsc 删字段后无悬空引用)。
5. i18n: 删冲突文案 key 若变孤儿 (ccswitch.conflictExisting/Incoming) — 清或留 (查引用)。

## 不做
- relink_group_platform 按 name 歧义 (TODO)。
- 平台稳定 identity 机制 (id 不跨机, 无方案)。
