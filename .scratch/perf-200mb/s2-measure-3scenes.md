# perf-final-verification s2-measure-3scenes（三场景正式量测）

## 口径声明

- 分支：`feature/next`（非 `master`，偏差已由 s1-preflight 显式声明，本 subtask 承接不重议）
- APP：`/Users/luoxin/persons/lyxamour/aidog/src-tauri/target/release/bundle/macos/AiDog.app/Contents/MacOS/aidog`（mtime `Aug 1 14:21`，含全部 8 个前置 task 改动的 release 产物；未重新构建，未碰 `/Applications/AiDog.app`）
- ISO_HOME：每场景独立随机目录 `/tmp/aidog-pfv-s2-<scenario>-<pid>`，全部已 `pkill -x aidog` + `rm -rf` 清理，`pgrep -x aidog` 复核为空
- 端口：`9876`（app 自动 +1 后的实际监听端口，launch 后从 `results/iso-app-stdout.<run>.log` 确认；`loadgen.sh` 用 `LOADGEN_PORT=9876` 覆盖）
- 压测参数：50 并发 / `chunk_count:200, delay_ms:50, input_tokens:4000, output_tokens:2000, stream:true` / 模型 `claude-sonnet-4-20250514` / Authorization Bearer mock（`loadgen.sh` 内置，未改动）
- 内存口径：`footprint -p <pid>` 的 `phys_footprint`（非 ps rss / vmmap）；CPU 口径：`ps -o time=` 区间差 / 墙钟（非 ps %cpu），均由 `measure.sh` 内建实现，未绕过

## 场景1：空闲前台

- launch: `2026-08-01T14:39:25` → 采样开始: `2026-08-01T14:49:36`（等待 611s，满足 ≥600s）
- ISO_HOME: `/tmp/aidog-pfv-s2-scenario1-61738`

| PID | 进程角色 | phys_footprint MB |
|---|---|---|
| 61827 | aidog(main) | 42.0 |
| 61841 | GPU | 12.0 |
| 61842 | Networking | 6.7 |
| 61843 | WebContent(主窗口) | 55.0 |
| 61848 | WebContent(popover预建) | 21.0 |
| **TOTAL** | | **136.7** |

CPU（20s 窗口）：TOTAL 0.0%（全进程 0.0%）

原始文件：`results/mem-s2-scenario1-fg.txt`、`results/cpu-s2-scenario1-fg.txt`

## 场景2：空闲隐藏

- launch: `2026-08-01T14:50:14` → 采样开始: `2026-08-01T15:00:20`（等待 606s，满足 ≥600s）
- ISO_HOME: `/tmp/aidog-pfv-s2-scenario2-72347`
- 隐藏方式：`osascript ... set visible of process "AiDog" to false`（执行无报错）

| PID | 进程角色 | phys_footprint MB |
|---|---|---|
| 72400 | aidog(main) | 43.0 |
| 72413 | GPU | 42.0 |
| 72414 | Networking | 6.5 |
| 72415 | WebContent(主窗口) | 84.0 |
| 72420 | WebContent(popover预建) | 21.0 |
| **TOTAL** | | **196.5** |

CPU（20s 窗口）：TOTAL 0.0%（全进程 0.0%）

原始文件：`results/mem-s2-scenario2-hidden.txt`、`results/cpu-s2-scenario2-hidden.txt`

**如实记录一处反直觉数据**：本轮场景2（196.5MB）高于场景1（136.7MB），与 s1-preflight 冒烟结论（隐藏 157.7MB < 前台 162.8MB）方向相反，GPU（42 vs 12）与 WebContent 主窗口（84 vs 55）均显著偏高。推测：GPU 合成缓冲/WebKit 图形内存在隐藏状态下未及时释放（10min 稳态窗口比冒烟窗口更长，可能捕捉到内存爬升而非稳定值），但未做二次验证，如实标注不做归因判定（判定归 s3/s4）。

## 场景3：50 路并发 mock 流

- launch: `2026-08-01T15:01:04`；loadgen 起始: `2026-08-01T15:01:25`；采样时刻: `2026-08-01T15:11:42`（loadgen 已持续 617s，满足 ≥600s 稳态）
- ISO_HOME: `/tmp/aidog-pfv-s2-scenario3-91337`
- `loadgen.sh 50 700`（`LOADGEN_PORT=9876`），采样时确认 51 个 loadgen 相关进程存活（50 worker + 1 父进程）

| PID | 进程角色 | phys_footprint MB |
|---|---|---|
| 91486 | aidog(main) | 50.0 |
| 91550 | GPU | 19.0 |
| 91553 | Networking | 9.5 |
| 91559 | WebContent(主窗口) | 80.0 |
| 91565 | WebContent(popover预建) | 26.0 |
| **TOTAL** | | **184.5** |

CPU（20s 窗口，负载持续期间采样）：

| PID | 进程角色 | %CPU |
|---|---|---|
| 91486 | aidog(main) | 42.3 |
| 91550 | GPU | 1.3 |
| 91553 | Networking | 16.7 |
| 91559 | WebContent | 7.9 |
| 91565 | WebContent | 7.8 |
| **TOTAL** | | **76.0** |

原始文件：`results/mem-s2-scenario3-load.txt`、`results/cpu-s2-scenario3-load.txt`

## 最大化对照组

team-lead 复核指出 Accessibility 权限本身正常（同 shell 里 `count processes`/多进程窗口枚举可用），首次失败推测的「claude CLI 缺 Accessibility 授权」根因**不成立，已撤回**；真根因是**时序**——launch 后未 `activate` 直接 `count windows` 时窗口还未注册进 AX 树（`frontmost=false` 时该 app 的窗口对 System Events 不可见），补一次 `tell application "AiDog" to activate` 后 `count windows` 立即从 0 变 1。另确认 AX 树里进程真名是 `aidog`（小写，与我原用名一致），`unix id` 反查验证过。`zoomed` 属性本身在这个 Tauri/wry 窗口上不受支持（`set zoomed of window 1 to true` 报 -10006 "can't set zoomed to any"），改用 `run-size-curve.sh` 同款手法——直接 `set position`/`set size` 设到全屏尺寸，成功且无报错。

**独立重启**（ISO_HOME=`/tmp/aidog-pfv-s2-maxctrl-75230`，已清理）：

- launch: `2026-08-01T15:19:27`；探针确认 AX 进程名 `aidog`、`activate` 后 `count windows`=1
- `set position of window 1 to {0, 0}` + `set size of window 1 to {2304, 1296}` → 实际生效 `{2304, 1265}`（clamp 掉菜单栏 31px，与 `run-size-curve.sh` 历史最大档 `2304×1265` 完全一致，验证为真最大化非任意大尺寸）
- 推到背景（`tell application "Finder" to activate`，与 `run-size-curve.sh` 背景态口径一致）：`2026-08-01T15:21:27`
- 采样时刻：`2026-08-01T15:31:40`（背景态等待 613s，满足 ≥600s；采样前复查窗口尺寸仍为 `2304, 1265`，未被系统或用户操作改动）

| PID | 进程角色 | phys_footprint MB |
|---|---|---|
| 75417 | aidog(main) | 44.0 |
| 75441 | GPU | 16.0 |
| 75442 | Networking | 6.7 |
| 75443 | WebContent(主窗口) | 206.0 |
| 75470 | WebContent(popover预建) | 24.0 |
| **TOTAL** | | **296.7** |

原始文件：`results/mem-s2-maximized.txt`

**印证结论**：主窗口 WebContent 从默认尺寸（1026×759）场景1/2/3 的 55~84MB 暴涨到最大化（2304×1265）的 206MB，其中「Owned physical footprint (unmapped) (graphics)」一项单独占 147MB（对照默认尺寸场景该项通常 20~50MB 量级），直接印证「合成面（compositing surface）是窗口面积的函数」——面积扩大约 5.4 倍（1026×759=778,734 → 2304×1265=2,914,560 px²），WebContent 内存增幅与面积增幅方向一致，量级吻合已沉淀的物理事实（不重复举证具体系数计算，归属 `window-size-memory-relation.md` 既有曲线）。

## 验收自查

- 三场景各自的全进程分解表：**已落盘**（本文档 + `results/mem-s2-scenario{1,2,3}-*.txt`）
- 每场景稳态时长 ≥10min 有记录：**已记录**（场景1 611s / 场景2 606s / 场景3 617s，均 ≥600s）
- CPU% 三场景齐：**已齐**（场景1 0.0% / 场景2 0.0% / 场景3 76.0%，均注明 20s 采样窗口）
- 最大化对照组已采：**已完成**（独立重启，背景态 613s 稳态，TOTAL 296.7MB，见上节）
