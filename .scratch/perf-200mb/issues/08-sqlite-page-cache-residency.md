# 08 SQLite page cache 常驻 —— 主进程 44→150MB 的根因

Type: task
Status: resolved
Blocked by: —
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

[03] 的 release 复验暴露矛盾：同为 `open -a` 起的 release 实例，[01] 全部采样点主进程 44–51MB，而复验时稳定 116–155MB，差 3 倍。这直接决定预算表「Rust 主进程 ≤30MB」那行是否现实，必须查清。

## Answer

**根因 = SQLite page cache，27 条连接各 2MB 默认 cache，全填满约 54MB。** 已定位到 file:line，证据链闭合。

### 一、不是泄漏

`leaks 33993` → `100554 leaks for 5981856 total leaked bytes` = **5.7MB**，全是 32-byte 的 `CryptKit::FEEKeyInfoProvider`（macOS Security 框架 TLS 握手泄漏，非本仓代码）。量级不解释 100MB，排除。

顺带：10 万个节点说明 TLS 连接数极大（压测累积），此泄漏随连接数线性涨，长跑会累积但速率低（5.7MB / 数万连接）。**不构成 200MB 目标的威胁，记录备查。**

### 二、增长是查询驱动的，不是纯时间

干净重启后手采：

| 时刻 | phys_footprint | MALLOC_SMALL | regions |
|---|---|---|---|
| t=25s | **50 MB** | 34 MB | 21 |
| t=5.6min（空闲，未操作 UI） | **107 MB** | 79 MB | 92 |
| t=22min（有 UI 操作，另一进程） | **149–155 MB** | 121–128 MB | 189–194 |

t=25s 的 50MB **正好对上 [01] 的 44–51MB —— [01] 测的是冷启动附近**，不是稳态。矛盾解除：两组数字都对，测的是曲线两端。

### 三、增长几乎全部是 5KB 块

`heap` 两进程对比：

| 进程 | 运行时长 | **5KB 块数** | 活跃堆总量 |
|---|---|---|---|
| 91760（空闲无操作） | 5.6 min | **1051** | 50.1 MB |
| 33993（有 UI 操作） | 22 min | **12436** | 108.0 MB |

增长 58MB 中，5KB 块贡献 11385 块 × 5KB = **57MB**。其余尺寸档（16B/32B/48B/64B 等）块数基本持平。**单一尺寸档解释了全部增长。**

`heap` 同时确认这 100MB 属 `non-object`（383120 nodes / 100264251 bytes），即无 ObjC 类型信息 = Rust 侧分配。

### 四、定位到 file:line

- `crates/aidog_core/src/gateway/db/mod.rs:12` — `const READ_POOL_SIZE: usize = 8;`
- 三个只读池：`mod.rs:235`（主库）/ `mod.rs:257`（proxy_log）/ `mod.rs:281`（platform）→ **24 条只读连接**
- 三条写连接：`Db::new`（`mod.rs:207`）/ log.db（`mod.rs:313`）/ platform.db（`mod.rs:343`）→ **共 27 条**
- **全仓无 `cache_size` / `mmap_size` 设置**（`grep -rn "cache_size\|mmap_size" --include="*.rs" crates/` 零命中，只有 `journal_mode` / `foreign_keys` / `busy_timeout` / `synchronous`）→ 走 SQLite 默认 `cache_size = -2000` = **2MB / 连接**

算术对账：
- 27 × 2MB = **54MB** 上限
- 2MB ÷ 4096B page = 500 pages/连接；24 只读连接全填满 = **12000 pages**
- 实测 5KB 块 **12436** —— 5KB = 4KB page + pcache header 开销

**误差 3.6%，命中。**

### 五、为什么必然填满

`~/.aidog/log.db` 实测 **7084.9MB**（[01] 记录）。库远大于 cache，任何 UI 列表/统计查询都会持续换页，**page cache 必定被填满并长期驻留**。这不是偶发，是稳态。

### 六、对预算表的影响

预算表要求 `aidog 主进程 (Rust) ≤ 30MB`。当前 page cache 单项就 54MB，**光这一项就超预算 24MB**。真实稳态 149MB 距 30MB 差 119MB。

**但这是好消息**：54MB 是可配置项，不是物理成本 —— 与合成面（窗口面积的物理函数）性质完全不同。

### 七、修复方向（本票不实施，交 spec / task）

主手段：给只读连接与写连接设 `PRAGMA cache_size`。

- 24 只读连接降到 `-256`（256KB）→ 24 × 256KB = 6MB，**省 ~42MB**
- 写连接可保留较大 cache（写路径更吃 cache 命中）

**代价与红线**：cache 命中率下降 → 查询变慢 → **直接压红线 3（UI 切页与列表流畅度）**。必须实测 Logs / Stats 页的查询耗时前后对比再定数值，不能拍脑袋。

次要方向（更省但改动大）：`READ_POOL_SIZE = 8` × 3 池是否必要？24 条只读连接对单用户应用偏多，池收缩同样省 cache，但影响并发读吞吐。需与 [07] 的 UI IPC 数据合看。

**关联**：与 log.db 7GB 治理（map fog）同源 —— 库缩小后 cache 压力自然降，两者应合并考虑。
