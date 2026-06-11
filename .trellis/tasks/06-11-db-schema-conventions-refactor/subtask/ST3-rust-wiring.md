# ST3: Rust 联动（路由/代理/命令）

- **目标**: router/proxy/lib 适配新模型 + 删 mapping 机制
- **产出**:
  - router.rs: model mapping 从 `group.model_mappings`(JSON) 读，弃 `db::list_model_mappings`；platform_id u64
  - proxy.rs: proxy_log 写入 id 用 `Uuid::new_v4().simple()`（无连字符）；platform_id u64；时间 ms
  - lib.rs: 删 `mapping_create/list/update/delete` commands + invoke_handler 注册；platform/group command 签名 id u64
- **验证**: `cargo build` 退出码 0
- **资源**: design.md、router.rs、proxy.rs、lib.rs
- **依赖**: ST2
- **失败处理**: 编译错误逐修
