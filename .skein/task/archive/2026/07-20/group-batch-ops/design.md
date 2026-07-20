# 分组内平台多选 + 批量操作 — 详细设计

## 架构

### 前端(per-group 多选 + 4 modal)
- **多选态**:`GroupListItem` 加 `mode: "view" | "select"` per-group(本地 state,不持久化);select 模式下每张 `PlatformCard` 包一层 checkbox 容器。选中态:`Set<number>` per-group。
- **工具栏**:`GroupListItem` 顶部条件渲染 select 模式工具栏([取消][全选] 已选 N 个 | [删除][覆盖模型][改状态][移组])。文字按钮,非图标。
- **4 个 modal**(各自独立,createPortal document.body):
  - `BatchDeleteModal`(全列可滚 + 跨组警告)
  - `BatchOverrideModelsModal`(三来源 + 全 diff)
  - `BatchSetStatusModal`(启用/禁用 + 无候选警告)
  - `BatchMoveGroupModal`(移动/加入 + 目标组下拉)
- 复用 `components/shared/Modal.tsx` 基元(b4 已抽)。

### 后端(4 batch command,各原子事务)
新增 `commands_platform/src/batch.rs`:
```rust
#[tauri::command]
async fn batch_delete_platforms(db: State<'_, Db>, ids: Vec<u64>) -> Result<BatchReport, String>
// txn: for id in ids { platform_delete(db, id) } 任一失败 → rollback

#[tauri::command]
async fn batch_override_models(db: State<'_, Db>, ids: Vec<u64>, models: Vec<String>) -> Result<BatchReport, String>
// txn: for id in ids { update platform set models = ? }

#[tauri::command]
async fn batch_set_status(db: State<'_, Db>, ids: Vec<u64>, status: String) -> Result<BatchReport, String>
// status: "enabled" | "disabled" only (reject auto_disabled)

#[tauri::command]
async fn batch_move_group(db: State<'_, Db>, ids: Vec<u64>, target_group_id: u64, mode: String) -> Result<BatchReport, String>
// mode: "move" | "add"
//   move: 从当前组移除关联 + 加目标组(保留其他组关联)
//   add:  仅加目标组(保留所有现组)
```
- 事务:用现有 db txn pattern(`db.transaction(|tx| { ... })` 或 begin/commit/rollback,查现有 db.rs txn idiom)。
- `BatchReport { applied: u64, skipped: Vec<SkipReason> }`(原子事务下 applied=0 或全 N,skipped 空;但保留结构供未来部分成功扩展)。
- 注册进 `generate_handler!`(startup.rs)— **新 command 需 yarn tauri dev 重启**(memory `tauri-rust-command-needs-restart`)。

### 数据流
```
用户勾选平台(per-group Set<number>)
→ 点工具栏批量按钮 → 开对应 modal
→ modal 预览(删除:全列+跨组警告 / 模型:diff / 状态:无候选警告 / 移组:目标组)
→ 用户确认 → invoke batch_* command(ids, params)
→ Rust txn 执行 → BatchReport
→ 成功:toast + 刷新 platforms/groups(现有 fetch 复用)+ 退出多选模式
→ 失败:toast 错误原因 + "未改动"(原子回滚)
```

## 关键取舍

### 原子事务 vs 逐个部分成功
选原子。理由:数据一致(禁半完成状态);UX 简单(全成或全败 + 原因)。代价:N 大时一个失败拖整批,但批量操作频率低,可接受。

### 物理删 vs 移除关联
选物理删。理由:用户措辞"删除选择的所有平台"= 删实体。跨组副作用靠 modal 警告(显式告知"将从 N 组移除")。移除关联(移出当前组)归"移组"操作的 move 模式。

### 4 独立 command vs 1 通用 batch_apply
选独立。理由:用户硬约束"每个批量单独"。类型安全(各 command 参数明确,非通用 enum op);command 边界清晰。代价:4 个 command 注册,可接受。

### per-group vs 全局多选
选 per-group。理由:用户措辞"在分组内,多选"。每组独立上下文清晰。全局多选跨组选择语义复杂(平台跨组共享时勾选归谁)。后续若需全局可扩。

### 覆盖模型三来源合一个 modal vs 分 tab
合一个 modal,来源用 radio 切换(手输/preset 下拉/从别平台复制)。理由:一个操作 = 一个 modal(用户硬约束),来源是值输入方式,非独立操作。

### 移组 move 模式与"当前组"识别
per-group 多选下,"当前组"= 触发多选的那个分组 id。move = 从该组关联移除 + 加目标组(保留平台在其他组的关联)。若目标组=当前组 → 禁用确认(无意义)。

## 技术选型
- 前端 modal:复用 `components/shared/Modal.tsx`(b4 基元,createPortal 已处理 liquid glass 居中)。
- 事务:复用现有 db.rs txn idiom(grep `transaction\|begin\|with_transaction` 确认 pattern)。
- 跨组副作用数据:`get_platform_groups(platform_id)` 查关联,modal 渲染警告。
- 整组无候选警告:改状态 modal 计算选中平台占当前组候选比例(若选中=全部且改 disabled → 警告)。

## 风险
1. 事务 idiom 不统一(db.rs 多处 begin/commit 手写 vs transaction closure)→ s1 先查现有 pattern 统一用。
2. 平台跨组共享 → 删除/移组的跨组副作用 modal 展示数据需 `get_platform_groups` 批量查(N+1 风险,但 N 通常 ≤ 几十,可接受)。
3. 新 Rust command 需重启 dev(memory 提醒)→ 验收时先重启。
