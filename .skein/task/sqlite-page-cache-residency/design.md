# SQLite page cache 常驻治理 — 详细设计

## 现状（已实测，非推测）

### 连接拓扑

`src-tauri/crates/aidog_core/src/gateway/db/mod.rs`：

| 连接 | 位置 | 条数 |
|---|---|---|
| 主库写连接 | `mod.rs:207` | 1 |
| log.db 写连接 | `mod.rs:313` | 1 |
| platform.db 写连接 | `mod.rs:343` | 1 |
| 主库只读池 | `mod.rs:235` → `build_read_pool` | 8 |
| proxy_log 只读池 | `mod.rs:257` | 8 |
| platform 只读池 | `mod.rs:281` | 8 |
| **合计** | | **27** |

`const READ_POOL_SIZE: usize = 8`（`mod.rs:12`）。

### 缺口

`grep -rn "cache_size\|mmap_size" --include="*.rs" crates/` **零命中**。现有 pragma 只有 `journal_mode=WAL` / `foreign_keys` / `busy_timeout` / `synchronous`（写连接）。

→ 27 条连接全部走 SQLite 默认 `cache_size = -2000` = **2MB/连接** ≈ **54MB 上限**。

### 证据链

| 证据 | 数据 |
|---|---|
| 冷启动 t=25s | phys 50MB / MALLOC_SMALL 34MB / 21 regions |
| 空闲 t=5.6min | phys 107MB / MALLOC_SMALL 79MB / 92 regions |
| 有 UI 操作 t=22min | phys 149MB / MALLOC_SMALL 121MB / 189 regions |
| `heap` 5KB 块数 | 1051（5.6min）→ **12436**（22min） |
| 算术对账 | 24 只读 × (2MB ÷ 4096B) = 12000 pages，实测 12436，**误差 3.6%** |
| 排除泄漏 | `leaks` 仅 5.7MB，全是 macOS `CryptKit` TLS 框架泄漏，非本仓 |

增长 58MB 中 5KB 块贡献 57MB，**单一尺寸档解释全部增长**。

log.db 实测 7084MB，远大于 cache → page cache **必然被填满并长期驻留**，是稳态非偶发。

## 方案（当前方案 = 精简守现状）

给连接补 `PRAGMA cache_size`，数值由实测二分定，不拍脑袋。

改动面只有两处 idiom：

1. `build_read_pool` 内只读连接的 `execute_batch`（`mod.rs:397` 附近）追加 `PRAGMA cache_size=<读档>;`
2. 三处写连接的 `execute_batch`（`mod.rs:207` / `mod.rs:313` / `mod.rs:343`）追加 `PRAGMA cache_size=<写档>;`

### 参数化（只读档一个 env）

数值不写死，二分实验需反复切档；每档 `cargo build --release` 太慢。走**环境变量读取**（`AIDOG_SQLITE_READ_CACHE_KB`），缺省回落 SQLite 默认。

选环境变量不选 settings 字段的理由：settings 要过 schema + 前端展示 + i18n，为一个调试旋钮不值（YAGNI）；环境变量零 UI 面、零迁移、进程级生效，正好匹配「起进程 → 采样 → 换档重起」的实验循环。二分收敛后默认值固化进源码，env 入口保留为 debug 旋钮，加 `ponytail:` 注释标明。

**启动方式硬约束**：GUI 应用由 launchd 起，**不继承 shell 里 export 的变量**。必须用 `open --env AIDOG_SQLITE_READ_CACHE_KB=<档> -a AiDog`（`man open` 已验支持 `--env`）。

**不加写档 env**（grill YAGNI 裁定）：写档不参与二分、维持默认，加了就是「以后可能要」。

### 只压读档，不碰写档

- **只读连接可激进**：读 cache miss 的代价是一次磁盘读；WAL 下只读连接不阻塞写。24 条 × 省下的量 = 全部收益来源。
- **写连接一律不动**：`mod.rs:124/131/136` 注释写死三个只读池「供 UI 热读路径（stats / 列表 / 日志查询）走」；grill 期已 grep 核实 `proxy/` 与 `router/` 下的 `.read()` 全是 `settings_cache` 的 RwLock，**不是 db 池** → 转发热路径走写连接。动写档 = 压红线 1（转发延迟），收益又只有 3×2MB。故写档不设旋钮、不入曲线表。

**推论：红线 1 在本设计下结构性无风险**，验收只需覆盖红线 3。

### 归因判决实验（fail-fast 硬门）

根因链末端是算术推断：实测 12436 个 5KB 块 ≈ 24 条只读连接 × 500 pages，误差 3.6%。**未被直接证实** —— 若这批 5KB 块其实是别的东西（reqwest 缓冲 / tracing / tokenizer），整个 task 白做。

故在 baseline 之前插一步最便宜的判决实验：**单跑 `AIDOG_SQLITE_READ_CACHE_KB=64` 一档，只看 5KB 块数是否跟着掉**。掉了归因坐实；不掉立即停手回 08 票重新归因，不浪费下游三个 subtask。成本约 10 分钟。

### 库体积是结论的前提，不是背景

7GB 的 log.db 是「cache 必被填满」的**充分条件**。库小则 cache 本来就填不满，本优化对新用户零收益。故：

- 每次采样记录 log.db 与 WAL 实际体积，大库组要求 ≥5GB（保证复现性，日后 VACUUM 过也知道数字不可比）
- 另跑一组 <100MB 小库对照，回答「定值对小库用户是否安全」

小库环境用 mock 灌少量数据现造，**禁动用户真实库**，用完即删。

### p95 口径（定死，非 wishful）

三条固定查询，各跑 ≥30 次取 p95，**分列记录不合并**：

1. Logs 列表分页查询
2. Stats 聚合查询
3. 平台余额聚合查询

另单列「基线最慢的那条 SQL」作重点跟踪项 —— 合并算总体 p95 会被大量快查询稀释，掩盖回归。

**耗时埋点无需新增**：`sql_profile_callback` 已注册在全部连接上（含只读池，`mod.rs:403`）。

### 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 收缩 `READ_POOL_SIZE` | 用户已裁定不在本 task 动（影响并发读吞吐，需与 [07] UI IPC 数据合看） |
| `mmap_size` 换 page cache | mmap 页是 file-backed，不计 phys_footprint —— 看着「省」实为换口径记账，没真省；且 7GB 库 mmap 有地址空间与一致性风险 |
| 先治 log.db 体积 | 用户已裁定不并入；且库缩小后 cache 上限仍是 27×2MB，不解决根因 |
| 共用单条只读连接 | 破坏并发读，直接压红线 3 |

## 数据流（量测链路）

```
open --env AIDOG_SQLITE_READ_CACHE_KB=<档位> -a AiDog   # 干净重启，禁复用旧进程
  → loadgen.sh 灌 mock 分组
  → t=25s 采冷启动 phys_footprint
  → 驱动三条固定查询各 ≥30 次，收 sql_profile 耗时
  → 采稳态 phys_footprint + heap 5KB 块数 + 三条 p95（分列）+ log.db/WAL 体积
  → 汇总落 .scratch/perf-200mb/results/，逐次原始采样即清
```

**判据硬约束**：内存只认 `footprint` 的 `phys_footprint` 与 `heap` 的 5KB 块数。禁 `ps rss` / `vmmap` —— 二者均漏算 `Owned physical footprint (unmapped) (graphics)`，[01] 已证。

**为什么每档必须独立重启**：内存曲线有冷（50MB@25s）/ 稳（149MB@22min）两端，同进程内改档混采得到的是两个不同稳态的插值，不可比 —— [03] 的 release 复验就是栽在这里。

**二分程序**：默认档为基线，另采 `-1024` / `-256` / `-64` 至少 4 档，出曲线表，取「三条查询 p95 相对基线上升均 ≤10%」前提下内存增量最小、且对小库安全的档。

**清场**：临时脚本、逐次原始采样、小库环境副本，各 subtask 结束即删；`results/` 最终只留基线指标集 + 大库曲线表 + 小库对照表三份。

## 可能性分支（不进当前方案，仅留痕）

- **SQLite shared-cache 模式** — 触发条件：若二分发现即使小 cache 也伤 p95，说明工作集确实大，届时可评估多条只读连接共用一份 page cache。代价是引入表级锁竞争。当前无证据支持，YAGNI。
- **按库分档** — 触发条件：若实测发现 log.db 与 platform.db 的 cache 敏感度差异显著（一个降了没事、一个降了就慢），再拆 per-db 档位。当前读/写两档已足。
- **空闲时 `PRAGMA shrink_memory`** — 触发条件：若窗口隐藏后仍需进一步压常驻，可对只读池发 `shrink_memory`。多一条状态路径，与 [06] 空闲 CPU 票可能合并考虑，当前不做。
