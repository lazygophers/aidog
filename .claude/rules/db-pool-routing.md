# aidog_db 三库拆分：连接池 handle 路由与测试盲区

内存库 fallback 下三个 handle 复用同一主内存连接，**路由错误在内存库测试里不可见**——查错库的 call 站点在测试里照常绿，真机操作才报错。

## 路由表（真值源 `aidog_db/src/lib.rs` 的 `Db` 结构体 doc，self.0~self.7）

| 表 | 写 handle | 读 handle |
|---|---|---|
| platform / group（platform.db） | `call_platform_traced` | `call_read_platform_traced` |
| proxy_log（log.db） | `call_proxy_log_traced` | `call_read_proxy_log_traced` |
| 其余（aidog.db 主库） | `call_traced` | `call_read_traced` |

## 规则

1. 改任何 SQL 的 call 站点，先按表对照上表选 handle。「函数在哪个文件」不构成依据——`middleware.rs` 一个文件混着三种 handle，`settings.rs` 混着两种。
2. **内存库测试证明不了路由正确。** 新增/改动 split 表（platform / group / proxy_log）的 call 站点，验证二选一：用三个独立临时文件路径构造 `Db` 跑一次（错路由当场 `no such table`）；或真机跑通该路径。fixture helper：`aidog_db::test_support::three_file_db()`（test_support.rs，三独立文件路径），platform / group / proxy_log 的 call 站点测试一律用它。
3. 审计命令（列全部主库 handle 调用点，逐个人工核对闭包内查的表）：

   ```
   grep -rn 'call_read_traced\|call_traced(' src-tauri/crates/aidog_db/src/ | grep -v _platform_traced | grep -v _proxy_log_traced
   ```

## 错路由可以零报错

split 表 call 站点的错路由还有一种症状：**完全不炸**——真机错误被前端吞成「UI 态永不持久化」这类静默失效，比 `no such table` 更难发现，只能靠整仓审计抓到。因此新增/改动这类站点时 three_file_db 测试不是可选项，且断言要落到「写后重开 DB 读回」级别，不能只断言不 panic。

## 测试不得弹系统 GUI 窗

- **测试弹 GUI 窗 = 测试缺陷**，不是环境噪声：阻塞 CI/无头环境、并发测试在授权窗上排队、还可能把测试凭据写进用户真 keychain。涉及 keyring / master_key / 系统凭据的新测试，必须走 env 注入或 TMPDIR key 文件，不触真系统服务。
- 修法现状即规则（debug 构建下 master_key env 未设时先认 TMPDIR 测试 key 文件再 keychain，release 不短路，已内置），新测试无需重复实现；但给共享初始化路径（如 `init_tables_raw` 这类被大量老测试摸到的函数）加系统资源依赖时，先数一遍未注入凭据的既有测试路径——`ensure_test_master_key` 式 env 注入有并行竞态窗口，注入不到的路径就是弹窗路径。
