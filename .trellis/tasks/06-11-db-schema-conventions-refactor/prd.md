# PRD: DB Schema 规范化重构

## 背景

用户连续提出 10 条数据库设计规范，要求全局统一修复 + 完善测试 + 后续遵守。本任务为破坏式 schema 重构，涉及 Rust 后端 + SQLite + 前端 TS 全链路。

## 规范清单（用户原始需求，逐条）

| # | 规范 | 类型 |
| --- | --- | --- |
| R1 | 所有表的时间字段必须用**毫秒级时间戳**（INTEGER ms，非 ISO 字符串） | 类型变更 |
| R2 | 表名必须**单数**，禁复数 | 重命名 |
| R3 | `platforms.protocol` 列重命名为 `platform_type` | 重命名 |
| R4 | **移除 `model_mappings` 表** | 删除 |
| R5 | 手动处理 `~/.aidog/aidog.db` 中不合规数据 + 改代码确保全部合规 | 数据迁移 |
| R6 | 每处修复联动全局统一 + 完善测试 + 后续遵守规范 | 总纲 |
| R7 | 所有表主键必须为 **uint64 数字类型**（非字符串）；**proxy_logs 除外** | 类型变更 |
| R8 | uuid **禁止 `-` 连接符**（proxy_logs 主键用无连字符 hex32 uuid） | 格式 |
| R9 | 每个表必须有 `created_at` / `updated_at` / `deleted_at`（软删除）字段 | 字段补全 |
| R10 | 所有数字/字符串字段默认值必须为 `0` / 空字符串，**禁 NULL** | 约束 |

## 决策结论（已确认）

| 决策 | 结论 |
| --- | --- |
| D1 uint64 主键生成 | **自增 AUTOINCREMENT**（业务量远低 2^53，前端直接用 number，不加 string 包装） |
| D2 现有数据迁移 | **保留数据**，写**独立一次性迁移脚本**（不进 app 代码），迁移完成即删除脚本 |
| D3 复合主键表 | group_platform / setting **加代理 uint64 自增主键** + 保留原复合唯一约束（UNIQUE） |
| D4 model_mappings 去向 | 表删除，**映射迁至其他机制**（design 阶段定具体载体，候选：group 表内联 JSON 字段） |

## 现状（已调研）

### 实际库表（`~/.aidog/aidog.db`）
- `platforms` / `groups` / `group_platforms` / `model_mappings` / `proxy_logs` / `settings`（全复数）

### 主键现状
- 业务表 `id`：TEXT（uuid 含 `-`）
- `proxy_logs.id`：TEXT（uuid）
- 复合主键：`group_platforms(group_id, platform_id)`、`settings(scope, key)`（无单独 id）

### 时间字段现状
- `created_at` / `updated_at`：全 TEXT（`chrono::Utc::now().to_rfc3339()`，见 db.rs:147 `now()`）
- proxy_logs retention 清理按 `created_at` 字符串比较（db.rs:787-818 `to_rfc3339()` cutoff）

### model_mappings 引用面（R4 删除需联动）
- migrations/001_init.sql（CREATE TABLE）
- models.rs：`ModelMapping` / `CreateModelMapping` / `UpdateModelMapping` 结构体
- db.rs：`create/list/update/delete_model_mapping` CRUD
- router.rs:9-62：路由用 mapping 做 `source_model → target_platform/target_model` 映射
- lib.rs:243-266：`mapping_create/list/update/delete` Tauri commands
- 前端：api.ts mappingApi + Groups.tsx 等

### 受影响文件清单
- `src-tauri/migrations/001_init.sql`（schema 重写）
- `src-tauri/src/gateway/db.rs`（1069 行：now()、全 CRUD、retention 时间比较、model_mapping CRUD 删除、migration 策略）
- `src-tauri/src/gateway/models.rs`（Platform/Group/GroupPlatform/Setting/ProxyLog 结构体：id 类型、protocol→platform_type、时间 String→i64、deleted_at 补全、删 ModelMapping）
- `src-tauri/src/gateway/router.rs`（model_mapping 路由逻辑）
- `src-tauri/src/gateway/proxy.rs`（proxy_logs 写入）
- `src-tauri/src/lib.rs`（Tauri commands：删 mapping_*）
- `src/services/api.ts`（类型 + invoke：id 类型、Protocol→platformType、删 mappingApi）
- `src/pages/*.tsx`（Groups/Platforms/Logs/Stats 等引用）

## 待决策点（无法自决，需用户确认）

1. **uint64 主键生成策略**：自增 AUTOINCREMENT（前端 number 安全） vs 雪花/分布式 ID（大数前端需 string 传输避免 JS 2^53 精度丢失）
2. **现有数据迁移**：保数据就地迁移（重生成主键 + 重建外键关联） vs 接受清空重置
3. **复合主键表**（group_platform / setting）：加代理 uint64 主键 + 保留唯一约束 vs 复合主键豁免 R7
4. **model_mappings 删除后路由行为**：source_model 直接透传给 target 平台（无映射）确认

## 验收标准

- 全部 10 条规范在 schema + Rust 模型 + 前端类型一致落地
- `~/.aidog/aidog.db` 现有数据按决策迁移完成且查询正常
- 应用编译通过（cargo build + tsc）
- 测试覆盖：schema 约束 / 时间戳读写 / 软删除过滤 / 主键类型 / 默认值非 null
- 后续规范固化进 `.trellis/spec/`（新建 backend/db 层）

## 调度图

```mermaid
graph TD
    D[决策确认] --> S1[schema 重写 migrations + 数据迁移脚本]
    S1 --> S2[Rust models.rs 模型对齐]
    S2 --> S3[db.rs CRUD + now() + retention 重构]
    S3 --> S4[router.rs/proxy.rs/lib.rs 联动 + 删 model_mapping]
    S4 --> S5[前端 api.ts + pages 类型对齐]
    S3 --> T1[Rust 单测]
    S5 --> T2[前端类型校验]
    S5 --> SPEC[沉淀 backend/db spec]
```
