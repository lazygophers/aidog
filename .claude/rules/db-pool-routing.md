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

fixture helper 已存在（2026-10-18 起）：`aidog_db::test_support::three_file_db()`
（test_support.rs，主库 + log.db + platform.db 三独立文件路径）。platform / group /
proxy_log 的 call 站点测试一律用它构造 `Db`，错路由当场 `no such table`。

## 第二次复发（2026-10-18 ui_extra）

`update_extra_key`（ui_extra.rs）对 platform/group 表走 `call_traced`，f551a5a 同族
第二案（afe00198d 修）。症状新形态：**完全不炸**——内存库测试三 handle 同连接
测不出，真机错误被前端吞成「UI 态永不持久化」这种静默失效，比 `no such table`
更难发现（本例靠整仓审计抓到，不是靠报错）。教训：split 表 call 站点的错路由
可以零报错；新增/改动这类站点时 three_file_db 测试不是可选项，且断言要落到
「写后重开 DB 读回」级别，不能只断言不 panic。

## 测试不得弹系统 GUI 窗（2026-10-20 obs 轮）

obs 迁移挂进 `init_tables_raw` 后，`test_rw_pool` 等老测试也摸 master_key；未签名
的测试二进制读 keychain 每次弹 macOS Keychain 授权窗，用户被反复打断（插话拍板修：
debug 构建下 master_key env 未设时先认 TMPDIR 测试 key 文件再 keychain，release 不
短路，6d18cf3d9）。

- **测试弹 GUI 窗 = 测试缺陷**，不是环境噪声：它阻塞 CI/无头环境、并发测试在授权
  窗上排队、还可能把测试凭据写进用户真 keychain。涉及 keyring / master_key /
  系统凭据的新测试，必须走 env 注入或 TMPDIR key 文件，不触真系统服务。
- 修法现状即规则（debug 短路已内置），新测试无需重复实现；但给共享初始化路径
  （如 `init_tables_raw` 这类被大量老测试摸到的函数）加系统资源依赖时，先数一遍
  未注入凭据的既有测试路径——`ensure_test_master_key` 式 env 注入有并行竞态窗口，
  注入不到的路径就是弹窗路径。
