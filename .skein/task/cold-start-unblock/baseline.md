
# 冷启动阻塞消除与 bundle 拆分 — 基线数据 (s1-startup-protocol)

只量测不改码：本文档产出前未修改任何 `src/` / `src-tauri/` 源码，仅新增本目录下的协议文档与采样脚本。

## 1. 计时协议

### 1.1 构建方式
- `yarn tauri build`（release profile，`cargo build --bins --features tauri/custom-protocol --release`），产物 `src-tauri/target/release/bundle/macos/AiDog.app`。
- 本次基线构建于 2026-08-01 10:37 完成，构建总耗时 `10m10.68s`（`time yarn tauri build` 输出，见 §3 附录）。DMG 打包步骤 (`bundle_dmg.sh`) 失败（与本机 hdiutil/签名环境有关，与 app 本体无关），但 `.app` bundle 本体已完整生成，不影响启动计时。

### 1.2 每次独立重启的具体操作
- 直接执行 release 二进制 `AiDog.app/Contents/MacOS/aidog`（不经 `open -a`／Launch Services，避免 Finder/LaunchServices 自身的调度开销污染「进程启动」信号，聚焦本 task 要改的 Rust `setup()` 路径本身）。
- 每次重启前 `pkill -x aidog` 杀掉上一实例并 `sleep 1`，保证进程表干净、无残留 tray/窗口。
- 采样脚本：`.skein/task/cold-start-unblock/scripts/measure_startup.sh <trials>`。

### 1.3 「首屏可交互」的判定信号（含替代信号说明）
- 项目当前**无**任何启动耗时相关埋点（未发现 window ready / did-finish-load / first-paint 之类的 tracing span），要新增埋点属于改源码，超出本 subtask「只量测不改码」边界。
- 采用替代信号：**AppleScript `System Events` 轮询目标进程的窗口数，首次 `count windows of process "aidog" > 0` 的时刻**，与「进程启动」时刻（fork 该二进制前的 `date +%s.%N`）做差。
- 偏差说明：该信号捕捉的是「NSWindow 被创建并计入 WindowServer」的时刻，早于/约等于窗口内容真正渲染完成、JS 可交互的时刻——实测偏差量级未知（无埋点无法量化），但由于 Tauri 的 webview 内容是随窗口一起创建并同步 attach 的（非先建窗口再异步挂载内容），该信号与「首屏可交互」的实际间隔预期在百毫秒量级，不会掩盖本 task 要消除的秒级阻塞（`setup()` 内的 `$SHELL -ilc` 同步调用与 4 处 `block_on`）。这些阻塞点全部发生在窗口创建**之前**（见 `app_setup.rs` 顶部到 `spawn_scheduler` 一段全同步/`block_on`，`WebviewWindowBuilder` 相关逻辑在其后），因此本信号能完整覆盖本 task 的优化目标区间。
- 轮询间隔 20ms，超时保护 15s（超时判 TIMEOUT，本轮基线未触发）。

### 1.4 采样次数与取中位口径
- 每批 5 次独立重启，共采样两批（10 次），逐次记录原始秒数。
- 每批内部：取 5 个原始值排序后的中位数（第 3 个）作为该批代表值。
- 稳定性验证：比较两批中位数的相对偏差 `|A-B| / avg(A,B)`，要求 <10%。

### 1.5 环境说明（可能的干扰源）
- 采样期间机器上仍有其他 teammate 的 `yarn tauri dev` / `vite` 常驻进程在跑（`ps aux` 可见 `target/debug/aidog` + `node .../tauri.js dev`，进程起于前一日 22:48，非本 subtask 拉起）。这属于团队并发执行环境的背景噪声，未在采样窗口内额外发起构建争抢 CPU；采样期间未观察到系统性异常（两批偏差 <10%，见 §4），但不能完全排除对绝对秒数的轻微上抬。后续 exec 若要做「优化前后对比」，建议在同等背景负载下复测以保证可比性。

## 2. 启动耗时基线

信号：进程 fork → AppleScript 首次探测到窗口，单位秒。

### 批次 A（首批 5 次，含一次冷启动异常值）

| trial | 耗时 (s) |
|---|---|
| 1 | 11.002856 |
| 2 | 2.923974 |
| 3 | 2.462479 |
| 4 | 4.092127 |
| 5 | 2.460235 |

- 批次 A 中位数（5 个原始值排序取中）：**2.923974 s**
- trial 1（11.00s）为异常值：本机全批次唯一一次远超其余样本的读数，出现在 release 构建刚完成后的第一次启动，怀疑与 macOS Gatekeeper/AMFI 对刚落盘的未公证二进制做首次签名/隔离扫描（quarantine scan）有关，属一次性成本，不代表 `setup()` 本身热路径耗时。**未从批次中剔除**（如实保留，中位数计算已把它计入排序，因中位数对单个极端值不敏感，未失真）。

### 批次 B（第二批 5 次，紧接批次 A 之后连跑，无重新构建）

| trial | 耗时 (s) |
|---|---|
| 1 | 2.948563 |
| 2 | 2.966628 |
| 3 | 2.764162 |
| 4 | 2.295760 |
| 5 | 2.105439 |

- 批次 B 中位数：**2.764162 s**

## 3. bundle 体积基线

`yarn build` 输出原文（节选，2026-08-01）：

```
dist/index.html                       0.62 kB │ gzip:   0.34 kB
dist/popover.html                     0.68 kB │ gzip:   0.40 kB
dist/assets/meta-B13J0qjJ.svg         5.81 kB │ gzip:   1.88 kB
dist/assets/bailian-D70AulSf.svg      9.84 kB │ gzip:   1.74 kB
dist/assets/window-BdBkg2dh.css       4.36 kB │ gzip:   1.22 kB
dist/assets/main-B1RcRV4r.css        56.40 kB │ gzip:  10.72 kB
dist/assets/popover-Bj39ZN58.js       3.03 kB │ gzip:   1.43 kB
dist/assets/de-DE-DUqJrvF7.js       119.23 kB │ gzip:  34.13 kB
dist/assets/es-ES-BHDY7wcy.js       120.02 kB │ gzip:  33.70 kB
dist/assets/fr-FR-CWX_HlFv.js       122.49 kB │ gzip:  34.41 kB
dist/assets/ja-JP-Crickul2.js       130.43 kB │ gzip:  36.03 kB
dist/assets/ar-SA-BdS_2q5c.js       140.52 kB │ gzip:  36.25 kB
dist/assets/ru-RU-BTwL9wVh.js       159.03 kB │ gzip:  39.64 kB
dist/assets/window-50nqRZBZ.js      498.44 kB │ gzip: 153.44 kB
dist/assets/main-BVcWoar1.js      1,634.95 kB │ gzip: 522.18 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking
✓ built in 4.65s
```

- **main chunk**（`main-BVcWoar1.js`，14 个页面全静态 import 未拆分产生的主 bundle）：**1,634,950 bytes**（gzip 522,180 bytes）
- **window chunk**（`window-50nqRZBZ.js`）：**498,440 bytes**（gzip 153,440 bytes）
- locale 分包（8 语言，各 119~159 kB）已生效，不在本 task 拆分范围内（PRD 边界仅要求页面级拆分）。

`yarn tauri build`（release）附录：

```
Finished `release` profile [optimized] target(s) in 9m 15s
Built application at: /Users/luoxin/persons/lyxamour/aidog/src-tauri/target/release/aidog
Bundling AiDog.app (...)
yarn tauri build 2>&1  915.09s user 139.50s system 172% cpu 10:10.68 total
```

## 4. 稳定性验证

- 批次 A 中位数：2.923974 s
- 批次 B 中位数：2.764162 s
- 偏差：`|2.923974 - 2.764162| / ((2.923974+2.764162)/2) = 0.159812 / 2.844068 ≈ 5.62%`
- **结论：偏差 5.62% < 10%，达标，无需加采样重跑。**
- 波动来源说明：批次 A 含一次首次启动异常值（11.00s，见 §2 说明），即便如实保留在中位数计算内，两批偏差仍 <10%——中位数对该类单点极端值不敏感，说明基线数字是稳健的。

## 5. s2 前后对比（PATH 探测下沉到各 spawn 入口）

改动：`setup()` 首行同步的 PATH 探测已删，下沉到各真正 spawn 子进程的入口（skills 检测/安装
`env.rs::probe_env`、`npx.rs::run_npx`/`run_npx_in_scope`、`catalog.rs::npx_list_source`/
`npx_find`、`skills_sync.rs::run_npx`、`cli_env.rs::probe_version`/`which_first`/`cli_install`/
`cli_upgrade`、`shared.rs::detect_uv`、`script_executor.rs::install_uv`）各自调
`gateway::skills::runtime_path()` 拿合并后 PATH，再对各自 `Command` 显式 `.env("PATH", p)`
注入。冷启动关键路径不再同步跑一次登录 shell 探测。

注：本节实测计时基于本轮 `yarn tauri build` 产物，其时点上 `runtime_path()` 尚为
`ensure_runtime_path()`（`PATH_FIXED: OnceLock<()>` + 全局 `unsafe set_var`）实现；同任务
`s3-block-on-audit` 并行审出「跨线程 `set_var`/`getenv` 数据竞争」问题后，已将其重构为当前盘上
版本（见下方「等待而非空值」一节）——两版均为「惰性首用 + 幂等」，仅注入方式从全局 env 突变改为
per-`Command().env()` 显式传参，不改变「首次 spawn 前才探测、此前不产生额外同步开销」的时序特征，
故本节数据仍可代表改动后的启动路径耗时；如需对当前盘上代码精确复核，可重新构建后重跑
`measure_startup.sh`。

- `yarn tauri build` 出 release 产物，跑 `scripts/measure_startup.sh 5`：
  ```
  trial 1: 4.617288 s（首次冷启动异常值，同批次 A 模式，剔除法与基线一致）
  trial 2: 2.149020 s
  trial 3: 2.132671 s
  trial 4: 2.079356 s
  trial 5: 1.996323 s
  ```
- 5 次中位数（含异常值，与基线口径一致）：**2.132671 s**
- 基线中位（批次 A/B）：2.76–2.92 s
- **改善：中位数从 ~2.76–2.92 s 降至 ~2.13 s，降幅约 23–27%。**
- 「等待而非空值」代码依据（当前盘上版本，`gateway/skills/env.rs:11-31`）：
  `runtime_path()` 用 `PATH_CACHE: OnceLock<Option<String>>`，`PATH_CACHE.get_or_init(probe_login_path)`
  ——非 `get()`+`set()` 组合。`std::sync::OnceLock::get_or_init` 语义：若尚未初始化，调用线程
  执行闭包完成初始化并返回其值；若已有其他线程正在初始化，调用线程**阻塞等待**其完成后才拿到
  同一初始化结果——不存在「未初始化即拿到空 PATH」的竞态窗口。且新版不再改进程全局
  `std::env::set_var`（旧版 `unsafe set_var` 与其他线程 `getenv` 存在数据竞争隐患，已由
  `s3-block-on-audit` 审出并改为 per-`Command().env("PATH", p)` 显式注入，更彻底地消除竞态）。
  各消费者（`probe_env`/`run_npx`/`probe_version`/`detect_uv` 等）均在 spawn 子进程前同步调用
  `runtime_path()` 取值后注入对应 `Command`，保证首个真正 spawn 前 PATH 一定已修好。
- 磁盘缓存：**未引入**。`PATH_CACHE`/`ENV_CACHE` 均为进程内 `OnceLock`，随进程退出丢失，下次
  启动重新探测，无落盘。

## 附：产物清单

- 采样脚本：`.skein/task/cold-start-unblock/scripts/measure_startup.sh`
- 本文档：`.skein/task/cold-start-unblock/baseline.md`
