
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


### 实测 (基于当前盘上 per-Command 版本重新构建)

改动: `setup()` 首行的同步 `$SHELL -ilc echo $PATH` 已删; 登录 shell PATH 探测改为惰性 `OnceLock<Option<String>>` 缓存
(`gateway/skills/env.rs::runtime_path`), 由 23 个真 spawn 子进程的 `Command` 各自 `.env("PATH", p)` 注入,
不再改进程全局 env (原 `unsafe std::env::set_var` 已删)。

同协议同脚本 (`scripts/measure_startup.sh 5`, release 构建, 每次独立重启, 取中位), 两批各 5 次:

### 批次 C (改后第一批)

| trial | 耗时 (s) |
|---|---|
| 1 | 2.587351 |
| 2 | 2.069489 |
| 3 | 1.868393 |
| 4 | 1.939100 |
| 5 | 2.243715 |

中位数: **2.069489 s**

### 批次 D (改后第二批, 连跑)

| trial | 耗时 (s) |
|---|---|
| 1 | 2.493121 |
| 2 | 1.994563 |
| 3 | 1.866220 |
| 4 | 1.860715 |
| 5 | 2.030218 |

中位数: **1.994563 s**

- 两批偏差: `|2.069489 - 1.994563| / 2.032026 ≈ 3.69%` (<10%, 稳定)

### 对比结论

| | 基线 (批 A / B) | 改后 (批 C / D) |
|---|---|---|
| 各批中位 | 2.923974 / 2.764162 | 2.069489 / 1.994563 |
| 两批中位均值 | **2.844068 s** | **2.032026 s** |

**下降 0.812 s (-28.6%)**。与调研阶段实测的 `$SHELL -ilc` 单点耗时 (0.71 / 0.74 / 1.54 s) 量级吻合。

注: 两批改后样本各自的 trial 1 都偏高 (2.59 / 2.49), 与基线批次同样呈「每批首次略高」的形态,
属重启后文件缓存未热, 非本次改动引入; 中位数口径已规避。


## §6 s3 block_on 去留 (`src-tauri/src/app_setup.rs`)

前置结论：Tauri v2 `App::setup()`（内部函数，非本文件的 `app_setup::setup`）先按 `tauri.conf.json` 建好 config 声明的窗口（`WebviewWindowBuilder::from_config(...).build()`，`tauri-2.11.5/src/app.rs:2524-2526`）再调用用户 `setup` 闭包（同文件 :2530-2532）——即主窗口对象在我们的 `setup()` 跑之前就已创建；但 winit/tao 需要事件循环转一圈才能真正把窗口贴到 WindowServer 上屏（AppleScript `count windows of process` 探测到的时刻）。我们的 `setup()` 是在事件循环恢复前同步执行的回调，其中任何 `block_on` 都会顶住事件循环、直接推迟窗口上屏——这正是 baseline §5（PATH 探测异步化）实测降幅的机制，也是本节判定的依据。

| file:line | 去留 | 理由 |
|---|---|---|
| `app_setup.rs:103`（原 log_settings 迁移+加载，现已后台化 `log_init_startup` spawn，:102-129） | 挪后台 | 结果只喂 `logging::init_logging`（tracing 落盘 guard）与 `cleanup_old_logs`；`setup()` 内无任何后续逻辑读取 `log_settings` 本身或依赖 logging 提前就绪。未初始化前 tracing 宏落空 subscriber（no-op，不 panic），代价仅是这段窗口内的早期日志不落盘，纯观测性副作用，非功能正确性问题 |
| `app_setup.rs:120`（原 try_sync_settings，现已后台化 `sync_settings_startup` spawn，:131-149） | 挪后台 | 写的是外部 `settings.{group}.json` / `~/.claude` 配置文件；同一逻辑本就以 `sync_group_settings` tauri command 形式暴露给前端随时手动触发（`sync_settings.rs:397-404`），设计上已容忍"稍后才同步"。`setup()` 内无后续逻辑依赖其完成 |
| `app_setup.rs:128`（原 ensure_default_coding_tools_settings，现已后台化 `coding_tools_defaults_startup` spawn，:151-172） | 挪后台 | 写 `~/.claude/config.json` / `~/.claude.json` 联动开关默认值，`setup()` 内无后续逻辑消费其结果；用户不可能在冷启动的毫秒级窗口内就手动打开 CC/Codex 触碰这些文件 |
| `app_setup.rs:137`（`engine.reload`，:174-193，**维持 `block_on`，未挪**） | 保留 | `MiddlewareEngine::new()` 起手是空规则桶；若把 reload 挪成后台 spawn，会出现一段窗口——`engine` 已经 `app.manage` 挂进状态表，但桶还是空的。`setup()` 下方 `settings.autostart` 分支会 `spawn` 拉起 `proxy_start`（:474 一带，超出本次范围未动），一旦代理开始接请求，这段窗口内的请求会**静默绕过**中间件规则（屏蔽/改写/限流等业务规则失效而非报错）——这是行为变更，违反本任务「只动时序不改业务逻辑」的边界，且 DB 查询+建桶量级是同步可感知的（非纳秒级理论竞态）。三处已挪后台的项都不存在这种「初始空值 = 改变业务语义」的性质，唯独此处是「必须启动期同步完成」，故维持原 `block_on`，不后台化 |

保留项说明（为何必须启动期同步完成）：
- `engine.reload` 结果被同一进程内后续「代理是否已在接受请求」这件事直接依赖，而不仅仅是「日后某个用户操作才会读到」的旁路文件；一旦破坏该顺序保证，中间件规则会在真实流量窗口内失效，且没有其它兜底（`resolve_rules` 对空桶 fail-open，等同于全放行），影响面是安全/合规类业务规则，不是可接受的观测性降级。

竞态排除依据（3 处已挪项）：
1. 三处的输出均不被 `setup()` 内任何后续代码路径读取或等待（grep 确认 `log_settings` / `try_sync_settings` 返回值 / `ensure_default_coding_tools_settings` 返回值均无后续消费者），只有副作用（写文件/DB 行/tracing guard），不存在"读到脏值导致错误分支"的竞态形态。
2. 三者互相之间也无共享可变状态竞争：各自读写不同的 DB scope/key 与不同的磁盘文件（`log_settings.json`→DB / `settings.{group}.json` / `~/.claude/config.json`），并发执行安全。
3. spawn 写法复用本文件既有 idiom（`tauri::async_runtime::spawn(async move { use tracing::Instrument; let span = tracing::info_span!(...); async { ... }.instrument(span).await })`，与 :49-99 三处 DB 一次性迁移 spawn 同构），`handle.try_state::<Db>()` 兜底 `Db` 尚未 `manage` 的极端情况（本任务时序下不会发生，纯防御）。

### 门禁

- `cargo clippy --workspace --all-targets`：`touch app_setup.rs` 后重跑，仅剩 ts-rs 宏解析 serde 属性的既有 warning（与本次改动无关，`grep -A5 app_setup.rs` 命中 0 行），`app_setup.rs` 自身零 lint。
- `cargo test --workspace`：1639 passed / 1 failed（`quota::http::test_http::quota_get_json_network_error`，命中已知 flaky 清单，非本次改动引入），`proxy::test_integration::mock_platform_ttft_and_inter_chunk_split` 本轮未见失败。无新增红。

### 改前/改后启动中位数（`measure_startup.sh 5`，release 构建，2026-08-01）

改前（s2 完成态，即上方 §5 批次 C/D）两批中位均值：**2.032026 s**。

本次（s3：3 处 block_on 挪后台 + `cleanup_old_logs` 随 log 初始化一并后台化）两批各 5 次：

#### 批次 G（main 侧复测，机器静默：无并发构建）

| trial | 耗时 (s) |
|---|---|
| 1 | 3.115632（首次冷启动异常值，同批 A/C/E 模式） |
| 2 | 0.991867 |
| 3 | 1.133431 |
| 4 | 0.929764 |
| 5 | 1.092225 |

中位数: **1.092225 s**

#### 批次 H（紧接 G 连跑）

| trial | 耗时 (s) |
|---|---|
| 1 | 1.104435 |
| 2 | 1.379016 |
| 3 | 0.995383 |
| 4 | 1.087905 |
| 5 | 1.093860 |

中位数: **1.093860 s**

- 两批偏差: `|1.092225 - 1.093860| / 1.093043 ≈ 0.15%`（<10%，极稳）
- 两批中位均值：**1.093043 s**

> 作废的批次 E/F：executor 侧曾采过一组（中位均值 1.361 s），但采样窗口与 main 侧的
> `yarn tauri build` 重叠，CPU 被编译占用，且所用二进制归属存疑（两个构建并发写同一
> target 目录）。已改用上方 G/H —— 构建完全结束、机器静默后复测，故以 G/H 为准。

### 对比结论（累计三阶段）

| | 基线（s0，批 A/B） | s2 后（批 C/D） | s3 后（批 G/H） |
|---|---|---|---|
| 两批中位均值 | 2.844068 s | 2.032026 s | **1.093043 s** |
| 相对 s2 降幅 | — | — | -46.2% |
| 相对基线累计降幅 | — | -28.6% | **-61.6%** |

结论：3 处 block_on 挪后台 + log 初始化/`cleanup_old_logs` 后台化，冷启动中位数在 s2 基础上再降 46.2%，累计较原始基线下降 61.6%。`engine.reload` 保留同步执行（理由见上方去留表），其耗时（单次 DB 查询 + 内存建桶）在当前中位数中占比小，不是本轮瓶颈来源。


## 附：产物清单

- 采样脚本：`.skein/task/cold-start-unblock/scripts/measure_startup.sh`
- 本文档：`.skein/task/cold-start-unblock/baseline.md`


## §7 s5 bundle 拆分

### 改动清单

- `src/App.tsx`：11 个侧栏页（Home/Platforms/AppSettings/Logs/Stats/Notifications/Skills/Mcp/CliProxy/RequestLog/About）由静态 import 改 `React.lazy()`（具名导出适配为 default）；`handleNavigate` 内 `setActiveNav`/`setNavContext` 包进 `useTransition` 的 `startTransition`；渲染处用 `<Suspense fallback={null}>` 包住原来的 `<div key={effectiveNav}>` 页面切换块。
- `src/pages/AppSettings.tsx`：11 个 settings 子 tab（Settings/CodexSettings/PricingTab/TrayConfigTab/PopoverConfigTab/MiddlewareSettingsTab/SchedulingSettingsTab/NotificationSettingsTab/ImportExportTab/CodingToolsSettingsTab/MitmConfigTab）同样改 `React.lazy()`，`AppSettings` 组件拆出 `AppSettingsTabContent` 内层组件，外包一层 `<Suspense fallback={null}>`（settings tab 切换同样经 App.tsx 的 `handleNavigate` → `startTransition`，故复用同一无闪烁机制，未单独加 transition）。
- 未改 `vite.config.ts`（rollup 对每个动态 `import()` 自动切 chunk，未手工 `manualChunks`，无此必要）。
- 未引入 react-router（`grep -rn react-router src/ package.json` 只命中 navGuard.ts 里说明"无 react-router"的注释）。

### `yarn build` 输出（改后）

```
dist/assets/main-CE1mNgF0.js                  191.80 kB │ gzip:  59.94 kB
dist/assets/Skills-DsJV3adY.js                195.51 kB │ gzip:  59.23 kB
dist/assets/Platforms-BNAiFHO3.js             270.40 kB │ gzip:  77.70 kB
dist/assets/AppSettings-DdG2xfaq.js            31.54 kB │ gzip:   7.39 kB
dist/assets/Settings-DHfZeZAh.js              129.92 kB │ gzip:  36.43 kB
dist/assets/PricingTab-DQsmbSnp.js             11.77 kB │ gzip:   3.91 kB
dist/assets/TrayConfigTab-DnPNCnbB.js          22.76 kB │ gzip:   6.20 kB
dist/assets/PopoverConfigTab-CAIt7X7L.js       19.60 kB │ gzip:   6.85 kB
dist/assets/MiddlewareRules-SHv1Ae69.js        14.05 kB │ gzip:   4.36 kB
dist/assets/SchedulingSettings-Becka9sy.js      3.88 kB │ gzip:   1.52 kB
dist/assets/NotificationSettings-BD1t_idC.js   17.66 kB │ gzip:   5.36 kB
dist/assets/ImportExportTab-wN3sh5qK.js        56.61 kB │ gzip:  15.10 kB
dist/assets/CodingToolsSettings-CHz_kBsB.js    11.34 kB │ gzip:   3.88 kB
dist/assets/MitmConfig-BcemGgjE.js             16.88 kB │ gzip:   5.17 kB
dist/assets/CodexSettings-D-bCfutc.js           9.53 kB │ gzip:   3.79 kB
dist/assets/Home-Da6OFGXM.js                   17.21 kB │ gzip:   4.65 kB
dist/assets/Mcp-tvHVSLaZ.js                    21.21 kB │ gzip:   6.58 kB
dist/assets/RequestLog-CkHDxJct.js              7.01 kB │ gzip:   2.78 kB
dist/assets/Logs-BpF1_KWy.js                   11.58 kB │ gzip:   4.10 kB
dist/assets/Stats-CulOpaHS.js                  16.85 kB │ gzip:   5.80 kB
dist/assets/Notifications-CKKIazZt.js           2.70 kB │ gzip:   1.20 kB
dist/assets/About-NnmHbHnh.js                  11.86 kB │ gzip:   3.40 kB
dist/assets/cliProxy-DGyJDJ1P.js                 0.68 kB │ gzip:   0.32 kB
```
（其余 vendor/共享 chunk 如 proxy-*.js 468 kB、pinyin-*.js 302.72 kB、8 语言 locale chunk 119~159 kB 不变——本次不动范围。）

main chunk：改前 **1,634,950 B**（基线 §3）→ 改后 **191,800 B**（`ls -la` 实测 191,560 B gzip 前，见下一次 rebuild 191,800，两次构建 hash 不同但数量级一致）—— 降幅约 **88.3%**。

### 验证

- `yarn build`：通过（两轮，含 App.tsx 拆分 + AppSettings 子 tab 拆分各一轮）
- `yarn test`：26 files / 332 tests 全绿（两轮均验证）
- `node scripts/check-i18n.mjs`：✅ 零缺失
- `yarn tsc`（`yarn build` 内含 `tsc && vite build`，`tsc` 无报错即通过）：通过
- 未引入 react-router：`grep -rn react-router src/ package.json` 仅命中 `navGuard.ts` 注释原文
- navGuard 离页拦截：`registerNavGuard`/`requestNavigation` 是模块级单例（`src/utils/navGuard.ts`），与组件是静态导入还是 `React.lazy()` 动态导入无关——`Settings.tsx:390` 在 `useEffect` 里 `registerNavGuard`，卸载时反注册，生命周期钩子在懒加载场景下行为不变，逻辑代码依据见该行。

### 首屏加载判定

冷启动首帧 `effectiveNav === "home"`，App.tsx 里只有 `Home` 分支条件为真 → 触发的动态 `import()` 只有 `./pages/Home` 一个 chunk；其余 10 个页面 chunk（Platforms/AppSettings及其11子tab/Logs/Stats/...）在对应分支未命中前不会被浏览器请求。判定依据：Vite/Rollup 对 `lazy(() => import(...))` 各自产出独立 chunk（见上方 build 输出各文件名各异），且原生 ESM 动态 `import()` 只在实际执行到该表达式时才发请求，未渲染到的 JSX 分支不会执行到对应的 `import()` 调用。

### 切页不闪烁（红线 3）机制

- `handleNavigate` 里 `setActiveNav`/`setNavContext` 包进 `startTransition`（React 18+ 语义）：当这次状态更新导致子树挂起（等待新页面 chunk `import()` resolve）时，React **不会**把已挂载的 Suspense 边界打回 fallback，而是保留当前已提交的旧树在屏幕上，直到新 chunk 加载完成再一次性替换——这是 React 官方给"路由/tab 切换避免加载态闪烁"场景设计的机制（对应 `useTransition`/`isPending` API），比"先展示 loading 骨架再替换"更不闪。
- `<Suspense fallback={null}>` 包裹整个 `key={effectiveNav}` 的页面块——这层 fallback 理论上只在**没有旧树可留**时才会被渲染到屏幕（即应用冷启动第一帧），此时页面本就是空白，`null` 与任何骨架视觉等价，不构成"闪烁"（闪烁定义是"内容切内容之间出现可见的空白/loading 插入"，冷启动首帧不适用这个定义）。
- 本次改动未新增 `isPending` 型骨架 UI（YAGNI）——现有 `animate-fade-in` 类沿用原有淡入动效，视觉过渡与拆分前一致。

## §8 s6 nav key 判定

### `key={effectiveNav}` 意图判定（代码依据，非推测）

- 溯源：`git log -p -S'key={' -- src/App.tsx` 定位到该 key 首次引入于 commit `967c0622`（`feat(ui): Liquid Glass full redesign`，2026-06-09），diff 原文：外层 `<main>` 从无包裹容器改为 `<div className="animate-fade-in" key={activeNav}>`，提交信息只字未提"防止状态串页"或"强制刷新数据"，明确写的是 UI 重设计（fadeIn keyframe 动画）。
- 机制验证：`.animate-fade-in` 定义在 `src/styles/globals.css:210-211`，`animation: fadeIn 350ms ... both`。CSS animation 只在元素**新插入 DOM**（或 class 重新赋值）时重放；若外层 `div` 不因 key 变化而重新 mount，浏览器不会重放这段 keyframe（同一 DOM 节点、同一 className，动画只播一次）。
- 关键反例：本 subtask 逐行读 `App.tsx:198-210` 确认，页面级组件（Home/Platforms/.../About）本来就是**互斥条件渲染**（`{effectiveNav === "x" && <X/>}`，同一时刻只有一个分支为真），从 "home" 切到 "platforms" 时，React reconciliation 因为子元素 **type 不同**（`<Home>` vs `<Platforms>`），本来就会完整卸载旧组件、创建新组件——这一步**与外层 div 的 key 完全无关**，去掉 key 不会改变子页面的 mount/unmount 行为。
- 因此 `key={effectiveNav}` 唯一的**实际效果**是让外层 `<div className="animate-fade-in">` 本身在切页时重新插入 DOM，从而重放 fadeIn 动画；它并不控制、也从未控制过页面组件的重新取数或状态清空（那部分本就由互斥条件渲染保证）。s6 任务描述里"强制整树重挂载，每次切页全量重新取数"这个前提对**跨页切换**场景不成立——重新取数的成本本就存在（组件确实会重新 mount 并触发内部 `useEffect` 取数），无论 key 在不在。

### 去留结论：**保留**

理由：
1. 去掉 key 对"全量重挂载/重新取数"这个 PRD 想省的成本**没有实质省钱**——子页面切换的 mount/unmount 开销由互斥条件渲染决定，跟外层 div 的 key 无关；唯一变化是外层 div 从"重建节点"变成"复用节点+替换子节点"，差值是一个空 div 节点的创建/销毁，量级可忽略（远小于子页面组件树本身的 mount 成本）。
2. 去掉 key 会实质丢失 `animate-fade-in` 的动画重放（机制见上），是可感知的视觉倒退，踩红线 3（"切页体验不得退化"）。
3. 无跨页状态串页风险可言（保留结论下这条不适用，因为没有去掉）：即便去掉，也不会引入"新的"串页风险——因为子组件的挂载/卸载边界不由这个 key 决定，key 只影响外层壳。换言之，这个 key 不是"数据隔离阀门"，动它不影响数据正确性，只影响动画。

### 逐页比对

不适用（决策为保留原状，未改动任何页面渲染逻辑，无需逐页状态残留比对）。

### 切页流畅度判定依据

未改动 `src/App.tsx`，`key={effectiveNav}` 与 `startTransition`/`Suspense fallback={null}` 机制（§7 已验证）均原样保留，流畅度与 s5 验证结果一致，无下降。

### 门禁结果

`yarn build` 通过（`main-CE1mNgF0.js` 191.80 kB，与 §7 记录量级一致）；`yarn test` 26 files / 332 tests 全绿；`node scripts/check-i18n.mjs` ✅ 零缺失。

## 9. s7 最终验收 (全部改动落地后)

计时二进制: `AiDog.app` 构建于 2026-08-01 12:26, 晚于全部源码改动 (最后一处 `formSections.tsx` 12:18),
含 s2–s6 全部成果 + 时段窗口输入框加宽。同协议同脚本, 两批各 5 次。

### 批次 I

| trial | 耗时 (s) |
|---|---|
| 1 | 7.653654（首次冷启动异常值，同批 A/C/G 模式；本次量级更大，疑与刚落盘未公证二进制的 Gatekeeper 首扫叠加） |
| 2 | 1.348065 |
| 3 | 1.026035 |
| 4 | 1.310517 |
| 5 | 1.066613 |

中位数: **1.310517 s**

### 批次 J

| trial | 耗时 (s) |
|---|---|
| 1 | 1.603438 |
| 2 | 1.120316 |
| 3 | 1.433797 |
| 4 | 1.060223 |
| 5 | 1.390839 |

中位数: **1.390839 s**

- 两批偏差: `|1.310517 - 1.390839| / 1.350678 ≈ 5.95%` (<10%, 稳定)
- 两批中位均值: **1.350678 s**

### 全程对比

| | 基线 (A/B) | s2 后 (C/D) | s3 后 (G/H) | 最终 (I/J) |
|---|---|---|---|---|
| 两批中位均值 | 2.844068 s | 2.032026 s | 1.093043 s | **1.350678 s** |
| 相对基线 | — | -28.6% | -61.6% | **-52.5%** |

**红线 4 (冷启动不慢于基线): 通过** —— 1.351 s vs 基线 2.844 s。

诚实标注: 最终值 (1.351 s) 高于 s3 阶段单测的 1.093 s。两者差 0.26 s, 超出该阶段两批偏差 (0.15%),
不能归因于噪声。可能来源: ① s5 页面 lazy 化后首屏多一次 Home chunk 的动态 import 往返;
② 两次采样间机器背景负载不同。**未进一步定位** —— 因红线只要求「不慢于基线」, 且 1.351 s 仍较基线降 52.5%,
未做二分归因。若后续要压到 1.1 s 以内, 首先该验的是首屏 chunk 预取 (`<link rel="modulepreload">` 当前页 chunk)。

### bundle 体积最终值

| | 基线 | 最终 |
|---|---|---|
| main chunk | 1,634,950 B | **191,801 B (-88.3%)** |

### 门禁

- `yarn build`: 通过 (tsc + vite 均无报错)
- `yarn test`: 332 passed (332)
- `node scripts/check-i18n.mjs`: ✅ 零缺失
- `cargo clippy --workspace --all-targets`: 零 lint warning (仅 ts-rs 宏解析 serde 属性的既有噪声)
- `cargo test --workspace`: 1639 passed / 已知 flaky 例外见 §6
