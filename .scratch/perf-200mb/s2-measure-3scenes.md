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

## 最大化对照组 —— **阻塞，未采得数据**

尝试独立重启 + `osascript` 最大化窗口（`set zoomed of window 1 to true`），持续报错：

```
tell application "System Events" to tell process "aidog" to count windows  → 0
tell application "System Events" to get frontmost of process "aidog"      → false（set frontmost to true 后仍为 false）
tell application "System Events" to get UI elements enabled               → true（全局 Accessibility API 已开）
```

已尝试 ≥4 种手法（`zoomed` 属性 / 进程名大小写 `AiDog`/`aidog` 两种 / `unix id` 直接引用 process / `frontmost` 属性 / Cmd+Ctrl+F 全屏快捷键 keystroke），均无报错但也无效果，`count windows` 恒为 0。

**根因推测**：本会话控制 osascript 的调用方是 CLI 进程 `claude`（非 Terminal.app/普通签名 GUI app），macOS 的「个体 App 级」Accessibility/Automation 授权（System Settings → Privacy & Security → Accessibility/Automation）大概率未对该进程授予，故 System Events 能列出进程名（进程级枚举无需该授权）但拿不到窗口 UI 元素（需要该授权）。这与 `pkill`/直接二进制 launch（进程级操作，不经 Accessibility）一直正常形成对照，指向权限缺失而非脚本逻辑问题。

已按「连试 ≥3 次跑不通 → 停手回传，禁改参数凑通」执行：**停手，未产出最大化对照组数据**，已清理该轮 ISO_HOME（`/tmp/aidog-pfv-s2-maximized-61895`，已 `pkill -x aidog` + `rm -rf`，`pgrep -x aidog` 复核为空）。

`需要:` 若要补做最大化对照组，需先在 System Settings 里给运行 `claude` CLI 的进程授予 Accessibility/Automation 权限（人工 GUI 操作，agent 无法自行完成），授权后可复用本文档的重启+采样流程直接补测。

## 验收自查

- 三场景各自的全进程分解表：**已落盘**（本文档 + `results/mem-s2-scenario{1,2,3}-*.txt`）
- 每场景稳态时长 ≥10min 有记录：**已记录**（场景1 611s / 场景2 606s / 场景3 617s，均 ≥600s）
- CPU% 三场景齐：**已齐**（场景1 0.0% / 场景2 0.0% / 场景3 76.0%，均注明 20s 采样窗口）
- 最大化对照组已采：**未完成，阻塞**（见上节，环境 Accessibility 权限缺失，非脚本/参数问题）
