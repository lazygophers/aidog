# aidog_db 三库拆分：连接池 handle 路由与测试盲区

2026-09-28 f551a5a：`assert_group_composition_solo` 查 platform 表误走主库读池
（`group_platform.rs` 用 `call_read_traced`），生产报 `no such table: platform`；
全部测试绿——内存库 fallback 下三个 handle 复用同一主内存连接，**路由错误在
内存库测试里不可见**。用户真机操作才踩出。

## 路由表（真值源 `aidog_db/src/lib.rs` 的 `Db` 结构体 doc，self.0~self.7）

| 表 | 写 handle | 读 handle |
|---|---|---|
| platform / group（platform.db） | `call_platform_traced` | `call_read_platform_traced` |
| proxy_log（log.db） | `call_proxy_log_traced` | `call_read_proxy_log_traced` |
| 其余（aidog.db 主库） | `call_traced` | `call_read_traced` |

## 规则

1. 改任何 SQL 的 call 站点，先按表对照上表选 handle。「函数在哪个文件」不构成
   依据——`middleware.rs` 一个文件混着三种 handle，`settings.rs` 混着两种。
2. **内存库测试证明不了路由正确。** 新增/改动 split 表（platform / group /
   proxy_log）的 call 站点，验证二选一：用三个独立临时文件路径构造 `Db` 跑一次
   （错路由当场 `no such table`）；或真机跑通该路径。
3. 审计命令（列全部主库 handle 调用点，逐个人工核对闭包内查的表）：

   ```
   grep -rn 'call_read_traced\|call_traced(' src-tauri/crates/aidog_db/src/ | grep -v _platform_traced | grep -v _proxy_log_traced
   ```

长期修法（未做）：给 aidog_db 测试加一个三路径文件 fixture helper，让「路由错误
测试可见」成为默认而不是每次手挑。
