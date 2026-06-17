# PRD: 添加平台时可选不建分组或加入指定分组

## 背景 / 现状

当前 `platform_create` (lib.rs:143-182) 建平台后**无条件**自动建 1 个分组：
- name `{slug(platform-name)}-auto`，path `/{protocol}-{id}`
- `routing_mode=Failover`，`auto_from_platform=<platform_id>`，`max_retries=2`
- 平台关联进该分组（priority 0 / weight 1）
- 该 auto 分组**不可手动删**（db.rs:1390 `delete_group` 拦截非空 `auto_from_platform`，仅 `force_delete_group` 能删，目前只平台删除时调）

相关联动：
- `platform_update` (lib.rs:218+)：更新后 `ensure` 该平台有 auto 分组，无则补建。
- `ensure_platform_groups` (lib.rs:60-106)：启动/周期为**所有**缺 auto 分组的平台补建。
- `set_group_platforms` (db.rs:1410)：按 group 维度**整组替换**平台列表（platform→groups 多对多靠它逐组写）。

用户痛点：无法在添加平台时控制分组归属 —— 要么被迫要一个 auto 分组，要么事后手动管理。

## 目标

添加/编辑平台时，用户用**两个复选框**控制分组：
1. `[✓] 创建默认分组`（默认勾 = 当前行为；可取消）
2. `[ ] 加入已有分组`（默认不勾；勾则展开**多选**现有分组 chips）

两框组合：
- 只勾① → 当前行为（建 auto 分组）。
- 勾①+② → 建 auto 分组 **并** 加入选定的已有分组。
- 只勾② → 不建 auto 分组，仅加入选定分组。
- 都不勾 → **完全无分组**（平台游离，不在任何分组；ensure 永不补建）。

## 用户决策（已确认）

| 决策点 | 选择 |
|---|---|
| 选择器形态 | 双复选（建默认 + 加入已有），非三选一单选 |
| 「不建分组」持久性 | **持久跳过** —— ensure_platform_groups 永不给该平台补建 |
| 入口范围 | 手动添加(Platforms 表单) + cc-switch 导入 + platform_update 编辑。**智能粘贴批量不加**（保持现状 auto-group） |

## 方案

### 1. 数据模型（持久化「不要 auto 分组」选择）

- `Platform.auto_group: bool`（默认 `true`，`#[serde(default)]`）。
- DB 迁移：`ALTER TABLE platform ADD COLUMN auto_group INTEGER NOT NULL DEFAULT 1`，走 db.rs 现有 **guarded ALTER 幂等模式**（`let _ = conn.execute(...)`，列已存在则忽略）。老库老平台默认 1 = 行为不变。
- 「加入已有分组」**不新增持久化** —— 复用 `group_platform` 表（platform↔group 多对多，已是 plain membership）。
- 入参透传：`CreatePlatform` / `UpdatePlatform` 加 `auto_group: Option<bool>` + `join_group_ids: Option<Vec<u64>>`。

### 2. backend command

**`platform_create`**（lib.rs:143）改：
1. `create_platform` 落 `auto_group` 值（None→true）。
2. `auto_group==true` → 建 auto 分组 + 关联（现行逻辑，包进 `if`）。
3. `join_group_ids` 非空 → 对每个 gid 调 platform-centric 关联（见下「#4 平台入组 helper」），plain membership，**不**写 `auto_from_platform`。
4. `auto_group==false && join_group_ids 空` → 啥都不做（游离平台）。

**`platform_update`**（lib.rs:218）改：
1. 落 `auto_group`（若入参给）。
2. auto 分组对账（仅当 `auto_group` 入参显式给定）：
   - `true` 且该平台当前无 auto 分组 → 建（现行 ensure 逻辑）。
   - `false` 且存在 auto 分组 → `force_delete_group` 删之（auto 分组按设计只含本平台，删除安全）+ 解关联。**【评审点 A】**
3. `join_group_ids` 非空 → 全量同步该平台的**手动**组成员关系（加入列表内、移出列表外但**不动 auto 分组**）。**【评审点 B：全量同步 vs 仅增量加入】**

**`ensure_platform_groups`**（lib.rs:60）改：循环内 `if !platform.auto_group { continue; }`（持久跳过）。

### 3. 前端

- `services/api.ts`：`CreatePlatform` / `UpdatePlatform` TS 类型加 `auto_group?: boolean` + `join_group_ids?: number[]`。
- `Platforms.tsx` 添加/编辑表单：分组区两复选框 + 多选 chips（读 `list_groups`）。默认 `auto_group=true`、`join_group_ids=[]`。i18n 7 语言补 key。
- **cc-switch 导入**：导入预览/确认步加同款双复选（一个选择作用于整批导入平台）。

### 4. 平台入组 helper（新增）

`set_group_platforms` 是 group-centric 整组替换，不适合「把平台 P 加入/移出多个 group」。新增 db.rs：
- `add_platform_to_group(db, platform_id, group_id, priority, weight)` —— 读该 group 现有 platforms，追加 P（去重），`set_group_platforms` 回写。
- 或更直接：`sync_platform_manual_groups(db, platform_id, target_group_ids)` —— 全量对账 P 的手动组关系（auto 组排除）。update 用。

### 5. 不改

- 智能粘贴批量添加（`platform-smart-paste-parser`）—— 保持 auto-group。
- auto 分组删除保护（`delete_group` 仍拦非空 `auto_from_platform`）。
- 路由 / 熔断 / 调度逻辑。

## 评审点（prd 通过前需确认）

- **A. update 取消勾选「创建默认分组」**：✅ 已定 → `force_delete_group` 删掉 auto 分组（auto 组只含本平台，删除安全）。
- **B. update「加入已有分组」语义**：✅ 已定 → 全量同步（列表外手动组移出、列表内加入；auto 组不动）。
- **C. cc-switch 导入**：✅ 已定 → 平台单独选 + 批量开关（默认逐个，开批量则整批应用可覆盖）。

## 验收

1. `yarn tauri dev`：
   - 手动加平台，两框都不勾 → 平台创建成功、不在任何分组、Groups 页无新增；重启后 ensure 不补建。
   - 只勾「加入已有分组」选 2 个 → 平台进那 2 组、无 auto 组。
   - 勾「创建默认分组」+ 加入 1 组 → 既有 auto 组又在选定组。
   - 编辑已有平台：取消「创建默认分组」→ 其 auto 组消失；改「加入已有分组」→ 组成员同步。
   - cc-switch 导入：双复选作用于整批。
2. `cargo test`（db 新 helper + ensure 跳过 + create/update 分组对账单测）。
3. `cargo clippy` 0 warning（除已接受 block）。
4. 前端 i18n 7 语言 key 全覆盖（`check-i18n.mjs` 过）。
5. 老库迁移：`auto_group` 列默认 1，老平台行为不变。

## 文件 / 范围

- `src-tauri/src/gateway/models.rs`（Platform / CreatePlatform / UpdatePlatform 字段）
- `src-tauri/src/gateway/db.rs`（迁移 ALTER + create_platform 落列 + row read + 平台入组 helper）
- `src-tauri/src/lib.rs`（platform_create / platform_update / ensure_platform_groups 逻辑）
- `src/services/api.ts`（TS 类型）
- `src/pages/Platforms.tsx`（表单双复选 + chips）
- cc-switch 导入相关前端组件（双复选）
- i18n 7 语言 key
- 单测（db.rs / lib.rs）

## subtask

单一交付（平台分组归属控制），不拆 child。exec 按 backend（model+db+commands+tests）→ frontend（表单+cc-switch+i18n）两段，可串行（frontend 依赖 backend 字段）。
