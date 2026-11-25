# 沙箱门禁（sandbox-exec）：探针先行、写入面、PATH 与能力边界

门禁（cargo test / clippy）放进 `sandbox-exec` 隔离跑时适用；资源上限与看门狗照 perf-testing.md §0。

## 探针先行

- 进门禁前先用探针脚本实测四项，全过才跑：回环 connect allowed、外网 blocked（EPERM）、真实 HOME 写 blocked（EPERM）、隔离 HOME / tmpdir 写 allowed；profile 改动后探针复跑。
- 策略未通过双向探针（回环成功 + 外网失败）前，只用全禁网 profile，不声称「只放回环」。

## 写入面清单

- profile 写入面必含：`src-tauri/target`、`.tmp/<任务名>/`、`src/services/api/types/generated`（ts-rs export_bindings 测试写它）、`/dev/null`（build script 内跑 git）、`/private/var/folders`（tempfile）、`~/.cargo`（package cache 锁）。
- 跑前 `mkdir -p src-tauri/target`：target 目录不存在时 cargo 原子建目录用随机同级临时名（如 `src-tauri/targetWG9jG1`），不在按子路径放行的清单内。
- 真实 `~/.aidog` 整目录显式 deny（命中即 EPERM 显性失败），其余真实 HOME 由 deny default 兜底。

## PATH 与工具链

- 沙箱 / 离线跑 cargo 前，PATH 收紧到工具链 bin + 系统目录（`~/.rustup/toolchains/<版本>/bin:/usr/bin:/bin:/usr/sbin:/sbin`），排掉 mise shims；设 `MISE_EXEC_AUTO_INSTALL=0`。
- 沙箱内 PATH 首位是 mise shims 时，cargo 解析 `rustc` 命中 shim 联网失败；测试 helper / 脚本内 exec 外部命令用绝对路径（`/usr/bin/python3`），不裸 `python3` / `make`——PATH 命中 mise shim 会触发自动安装联网。
- 构建缓存里 CMake 生成器记的是 shim 绝对路径（如 `~/.local/share/mise/shims/gmake`）时，改 PATH 无效，须清该包的 `target/debug/build/<pkg>` 缓存目录让它重新探测；build script 需 cmake 时 PATH 加真实 cmake 目录（`/usr/local/RemoteDevelopmentToolkit/bin`），用 env 增量赋值，不用 `env -i`（剥 run token 且缺工具）。
- inline cargo 会被 hook 改写经 `mise exec`：沙箱内 mise 触发 rust 安装撞 `~/.rustup` EPERM。门禁链落盘成脚本文件执行。

## 能力边界与已知不可解

- `/bin/ps`、`osascript` 等 Apple restricted binary 在沙箱内 exec 被 EPERM 拒：watchdog 等需要它们的进程放沙箱外，包链结构 watchdog → taskpolicy → sandbox-exec → cargo。
- profile 必含 `(allow mach-lookup)`：缺它 FSEvents watch 测试与 TLS trust 评估（wss 握手）失败；外网 socket 仍由 `deny network*` 拦。
- 全量测试的沙箱不可解清单（预期红，不作代码回归）：`proc_starts_batch_real_ps`、`ca::tests::osacompile_*`、`transport_retryable_connect_yes_timeout_no`（断言依赖 connect 超时语义，沙箱 EPERM 即时拒改变错误类别）。
- 全量首跑 EXIT 非 0 时，先按失败分类归因（写入面缺失 / restricted binary / mach 服务 / 断言语义变化），补 profile 复跑收敛，不把沙箱引入的失败当代码回归修。
