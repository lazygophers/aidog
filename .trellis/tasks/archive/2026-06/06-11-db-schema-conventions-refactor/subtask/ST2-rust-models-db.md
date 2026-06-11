# ST2: Rust 模型 + DB 层（契约核心）

- **目标**: models.rs / db.rs 对齐新 schema（所有下游依赖此契约）
- **产出**:
  - models.rs: `id` String→u64（proxy_log 留 String）；`created_at/updated_at` String→i64 + 新增 `deleted_at: i64`；`Platform.protocol`→`platform_type`；`Option<String>`→`String`（extra/auto_from_platform）；删 `ModelMapping.id/group_id`（作 JSON 元素）；删 Create/UpdateModelMapping
  - db.rs: `now()`→`timestamp_millis() i64`；全 CRUD row.get 类型同步；表名单数（`"group"` 转义）；retention 比较改 ms 整数；软删除（DELETE→UPDATE deleted_at=now；list 加 `WHERE deleted_at=0`）；删 model_mapping CRUD；group CRUD 读写 model_mappings JSON
- **验证**: `cargo build` 退出码 0
- **资源**: design.md、models.rs、db.rs(1069行)
- **依赖**: ST1
- **失败处理**: 编译错误逐个修；类型不一致回 design 对照
