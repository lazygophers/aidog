# Research: rusqlite → tokio-rusqlite 迁移模式

- **Query**: 研究 rusqlite → tokio-rusqlite 迁移模式，为 aidog DB 异步化提供可落地指引与避坑清单
- **Scope**: mixed（内部代码现状 + 外部库 API）
- **Date**: 2026-06-12

## 摘要（先读这段）

- 目标库版本应选 **`tokio-rusqlite = "0.6.0"`**，它内部 pin `rusqlite ^0.32`，**与项目当前 rusqlite 0.32 完全对齐，零强制升级**。最新版 0.7.0 强制 `rusqlite ^0.37`（破坏性，需同时升 rusqlite 5 个小版本），除非另有理由否则**不要用 0.7.0**。出处见下。
- `tokio_rusqlite::Connection` 是 `#[derive(Clone)]` + 内部 `Sender<Message>`（crossbeam channel）→ 天然 `Send + Sync + Clone`，**可直接 `app.manage()`，不再需要 `Mutex`，也不再需要 `Arc<Db>`**。
- 闭包模式：`conn.call(|c: &mut rusqlite::Connection| { ...; Ok(x) }).await`。闭包签名是 `FnOnce(&mut rusqlite::Connection) -> Result<R, E> + Send + 'static`，`&mut` 使事务可用。
- **关键现状修正**：本项目 64 个 `#[tauri::command]` 中**仅 8 个是 `async fn`**，其余 56 个是同步 `fn`。db.rs 的函数本身全是同步 `pub fn`。改成 async 后，**所有 db 函数签名变 `async fn` + 所有 command 必须变 `async fn` + 所有调用点加 `.await`**——这是比 56 处 `.lock()` 更大的扩散面。需评估是否值得（见避坑 §7）。

---

## Findings

### 1. 版本与 Cargo 片段

| 项 | 值 | 出处 |
|---|---|---|
| tokio-rusqlite 最新稳定版 | `0.7.0`（2025-11-16） | crates.io API `/api/v1/crates/tokio-rusqlite` → `max_stable_version: 0.7.0` |
| 0.7.0 依赖 rusqlite | `^0.37`（normal & dev 均 bundled） | crates.io `/0.7.0/dependencies`：`rusqlite ^0.37.0 default features=[]` |
| **0.6.0 依赖 rusqlite** | **`^0.32`** | crates.io `/0.6.0/dependencies`：`rusqlite ^0.32` |
| 0.5.1 依赖 rusqlite | `^0.31` | crates.io `/0.5.1/dependencies` |
| feature 透传机制 | `bundled = ["rusqlite/bundled"]`，所有 feature 都是 `rusqlite/<x>` 直通 | crates.io `/0.7.0` features map（0.6.0 同构） |

**推荐 Cargo.toml 改法（src-tauri/Cargo.toml:26，保持 rusqlite 0.32 不动）：**

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
tokio-rusqlite = { version = "0.6", features = ["bundled"] }
```

注意点：
- `bundled` 在两个 crate 上都启用是安全的——cargo feature 合并，最终只编译一份 bundled SQLite（同一 rusqlite 0.32 实例）。**两边 features 必须一致**，否则可能出现 feature 不统一。
- tokio 已是项目依赖（Tauri 2 自带），无需额外加；tokio-rusqlite 仅需 tokio `sync` feature（它自己声明）。
- docs.rs: https://docs.rs/tokio-rusqlite/ ；GitHub: https://github.com/programatik29/tokio-rusqlite

> 推测: 若团队希望长期跟最新并愿意升 rusqlite 到 0.37，可用 0.7.0，但需验证 0.32→0.37 间 rusqlite API 变更（如 `params`、`OptionalExtension`、`Row::get` 签名）对现有 56 处调用的影响——本任务未覆盖该差异审计。

---

### 2. 核心 API + Db 重定义

API 事实（出处：GitHub `src/lib.rs` master 分支，行号见括注）：

```rust
#[derive(Clone)]                          // lib.rs:166
pub struct Connection { sender: Sender<Message> }   // 内部仅一个 channel sender → Send+Sync+Clone

pub async fn open<P: AsRef<Path>>(path: P)          // lib.rs:182
    -> std::result::Result<Self, rusqlite::Error>
pub async fn open_in_memory()                       // lib.rs:192
    -> std::result::Result<Self, rusqlite::Error>

pub async fn call<F, R, E>(&self, function: F)      // lib.rs:272
    -> std::result::Result<R, Error<E>>
where
    F: FnOnce(&mut rusqlite::Connection)
         -> std::result::Result<R, E> + 'static + Send,

pub async fn close(self) -> Result<()>              // lib.rs:347
```

错误类型（lib.rs:116）：
```rust
pub enum Error<E = rusqlite::Error> {
    ConnectionClosed,
    Close((Connection, rusqlite::Error)),
    Error(E),                 // 闭包内返回的 rusqlite::Error 落这里
}
impl From<rusqlite::Error> for Error { ... }   // lib.rs:151
```

**Db 重定义（当前 `db.rs:7` `pub struct Db(pub Mutex<Connection>)` → 改为）：**

```rust
use tokio_rusqlite::Connection;   // 替换 rusqlite::Connection 的持有方式

// 选项 A：保留 newtype，但去掉 Mutex（tokio-rusqlite 内部单后台线程已串行化）
#[derive(Clone)]
pub struct Db(pub Connection);

impl Db {
    pub async fn new(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).await.map_err(|e| e.to_string())?;
        // pragma 在 open 后用一次 call 设（见 §6）
        conn.call(|c| {
            c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; \
                             PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL;")?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(Self(conn))
    }
}
```

要点：
- **不需要 `Mutex`**：tokio-rusqlite 把所有 `call` 闭包丢给单个后台线程顺序执行，本身就是串行的。再包 `Mutex` 是反模式（会阻塞 async runtime）。
- **不需要 `Arc<Db>`**（proxy.rs:34 当前用 `Arc<Db>`）：`Connection` 自身 `Clone` 且克隆是廉价的 channel sender 克隆，共享后台线程。proxy 的 `ProxyState.db` 可直接持 `Db`（Clone）或 `Connection`。
- `Db` 加 `#[derive(Clone)]` 后，`tauri::State<Db>` 与 `app.manage(db)`（lib.rs:1945）照常工作。

---

### 3. 迁移模式（机械转换规则）+ before/after

**错误映射规则**：闭包内沿用现有 `rusqlite::Error`（`map_err` 仍可在闭包内用，或让 `?` 冒泡到 `E`）。外层 `.call(...).await` 返回 `tokio_rusqlite::Error`，统一 `.map_err(|e| e.to_string())` 落到现有 `Result<_, String>`——**现有错误处理风格几乎不变**，只是 `.lock()` 那一句的 `map_err` 挪走，`.call().await` 末尾接 `.map_err(|e| e.to_string())`。

闭包内若用 `?`，`E` 推导为 `rusqlite::Error`，外层得 `Error<rusqlite::Error>`，`to_string()` 直接可用。

#### Before/After 1 — 简单 execute + last_insert_rowid（db.rs:147-153）

```rust
// BEFORE (sync)
let conn = db.0.lock().map_err(|e| e.to_string())?;
conn.execute(
    "INSERT INTO platform (...) VALUES (?1, ...)",
    params![input.name, /* ... */],
).map_err(|e| format!("create platform: {e}"))?;
let id = conn.last_insert_rowid() as u64;
```
```rust
// AFTER (async) — 闭包必须返回需要的全部出参（id）
// 注意：闭包捕获的变量需 move 进去且为 'static（见避坑 §7-a）
let id = db.0.call(move |conn| {
    conn.execute(
        "INSERT INTO platform (...) VALUES (?1, ...)",
        params![input.name, /* ... */],
    )?;                                   // ? → rusqlite::Error
    Ok(conn.last_insert_rowid() as u64)
}).await.map_err(|e| format!("create platform: {e}"))?;
```

#### Before/After 2 — prepare + query_map 收集（db.rs:181-188 list_platforms）

```rust
// BEFORE
let conn = db.0.lock().map_err(|e| e.to_string())?;
let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY ...");
let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
let rows = stmt.query_map([], row_to_platform).map_err(|e| e.to_string())?;
rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
```
```rust
// AFTER — 整段挪进闭包；stmt/rows 都是闭包内局部，await 时不跨线程
db.0.call(|conn| {
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY ...");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_platform)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()   // 闭包返回 Result<Vec<Platform>, rusqlite::Error>
}).await.map_err(|e| e.to_string())
```
> `row_to_platform`（db.rs:99）是 `fn(&Row) -> SqlResult<Platform>`，已是 `'static` 自由函数，可直接被闭包引用，无捕获问题。

#### Before/After 3 — query_row + OptionalExtension（db.rs:191-201 get_platform）

```rust
// BEFORE
let conn = db.0.lock().map_err(|e| e.to_string())?;
let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
let result = stmt.query_row(params![id as i64], row_to_platform)
    .optional().map_err(|e| e.to_string())?;
Ok(result)
```
```rust
// AFTER — OptionalExtension 的 .optional() 在闭包内照常用（trait 来自 rusqlite，需 use）
db.0.call(move |conn| {
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_row(params![id as i64], row_to_platform).optional()  // -> Result<Option<Platform>, rusqlite::Error>
}).await.map_err(|e| e.to_string())
```

#### Before/After 4 — 事务（db.rs:307-321 set_tray_platform）

```rust
// BEFORE — 需要 &mut conn 才能开 tx
let mut conn = db.0.lock().map_err(|e| e.to_string())?;
let tx = conn.transaction().map_err(|e| e.to_string())?;
tx.execute("UPDATE platform SET show_in_tray = 0, ... WHERE show_in_tray = 1", params![ts])?;
tx.execute("UPDATE platform SET show_in_tray = 1, ... WHERE id = ?3 ...", params![display, ts, platform_id as i64])?;
tx.commit().map_err(|e| e.to_string())?;
```
```rust
// AFTER — call 闭包入参就是 &mut rusqlite::Connection，可直接 conn.transaction()
db.0.call(move |conn| {
    let tx = conn.transaction()?;                 // &mut 可用（lib.rs:273 闭包签名是 &mut）
    tx.execute("UPDATE platform SET show_in_tray = 0, ... WHERE show_in_tray = 1", params![ts])?;
    tx.execute("UPDATE platform SET show_in_tray = 1, ... WHERE id = ?3 ...",
               params![display, ts, platform_id as i64])?;
    tx.commit()?;
    Ok(())
}).await.map_err(|e| e.to_string())
```
> **事务原子性优势**：旧代码若在两条 execute 之间 await 别的任务，Mutex 持锁期间无法 await（同步锁），但整库串行。新模型下**单个 `call` 闭包是原子执行的**（后台线程跑完整个闭包才处理下一个），事务语义更干净。

#### 错误映射小结表

| 层 | 类型 | 处理 |
|---|---|---|
| 闭包内 `?` | `rusqlite::Error` | 不变，沿用 `params!` / `.optional()` / `row_to_*` |
| 闭包内显式 `map_err` | 仍可用，如 `format!("create platform: {e}")` 留在闭包外更简单 |
| `.call().await` 返回 | `tokio_rusqlite::Error` | `.map_err(\|e\| e.to_string())` → `String` |
| `Connection::open().await` | `rusqlite::Error` | `.map_err(\|e\| e.to_string())` |

---

### 4. Tauri State 注入可行性

**可行，且更简单。** 事实链：
- `Connection` = `#[derive(Clone)] { sender: Sender<Message> }`（GitHub lib.rs:166-168）。`crossbeam_channel::Sender` 是 `Send + Sync`，因此 `Connection: Send + Sync + Clone`。
- `tauri::manage` 要求 `T: Send + Sync + 'static`——满足。
- 当前 lib.rs:1945 `app.manage(db)` + 各 command `db: State<'_, Db>` 模式**保持不变**，只要 `Db` 重定义为持有 `Connection`（去掉 Mutex）。
- proxy.rs:32-34 注释明确写"用 Arc<Db> 而非 Mutex<Db>：Db 内部已自带 Mutex<Connection>"——迁移后该理由消失，`Arc` 可去掉（`Connection` 自带廉价 Clone）。若想最小改动，保留 `Arc<Db>` 也能编译（Arc<Clone> 仍合法），但冗余。

---

### 5. 事务 / prepared statement / OptionalExtension 在闭包内写法

- **transaction**：闭包入参是 `&mut rusqlite::Connection`，`conn.transaction()` / `conn.unchecked_transaction()` 照常可用（见 §3 Before/After 4）。整个闭包在后台线程同步跑完，天然原子。
- **prepared statement**：`let mut stmt = conn.prepare(sql)?;` 在闭包内创建、使用、丢弃。**stmt 不可跨 `call` 边界返回**（借用 conn 且非 Send-friendly across await）——必须在同一闭包内 collect 完结果再返回拥有所有权的 `Vec`/`Option`（见 Before/After 2）。
- **OptionalExtension**：`use rusqlite::OptionalExtension;`（db.rs:1 已 import）。`.optional()` 在闭包内对 `query_row` 结果调用，返回 `Result<Option<T>, rusqlite::Error>`，整体作闭包返回值（见 Before/After 3）。
- **query_map 收集**：必须在闭包内 `.collect::<rusqlite::Result<Vec<_>>>()`，因为迭代器借用 stmt，不能逃出闭包。

---

### 6. busy_timeout + synchronous=NORMAL pragma 设置位置

在 `Db::new` 里 `Connection::open().await` **之后**，用**一次 `call` 闭包** `execute_batch` 设置所有 pragma（见 §2 Db 重定义代码）：

```rust
conn.call(|c| {
    c.execute_batch(
        "PRAGMA journal_mode=WAL; \
         PRAGMA foreign_keys=ON; \
         PRAGMA busy_timeout=5000; \
         PRAGMA synchronous=NORMAL;")?;
    Ok(())
}).await.map_err(|e| e.to_string())?;
```

要点：
- pragma 是 connection 级状态，绑定在后台线程那条物理连接上。tokio-rusqlite 单连接单后台线程 → 设一次永久生效。
- 现状 db.rs:42 只设了 `journal_mode=WAL; foreign_keys=ON`，**当前没有 busy_timeout 也没有 synchronous=NORMAL**。本次迁移可顺带补上（WAL 下 `synchronous=NORMAL` 是常见优化；单连接模型下 busy_timeout 实际很少触发，因为不再有多连接竞争，但设置无害）。
- 也可用 `conn.call(|c| c.busy_timeout(Duration::from_secs(5)))` 的方法形式，等价。
- `init_tables`（db.rs:47-72）的 `execute_batch` + 9 个 `ALTER TABLE` 同理整段包进**一个 `call` 闭包**（保持原顺序，`let _ = c.execute(ALTER...)` 忽略 duplicate column 的写法不变）。

---

### 7. 避坑清单

a. **闭包必须 `'static`，不能借用外部非 'static 引用**。`F: ... + 'static + Send`（lib.rs:274）。所有捕获变量要么 `Copy`（如 `id: u64`），要么 `move` 进闭包并拥有所有权（如 `String`、`input`）。
   - 实操：闭包前加 `move`；若外层后面还要用某变量，先 `.clone()` 再 move。
   - `&Db` / `&str` 参数：不能把外部 `&str` 直接捕获进 `'static` 闭包 → 在闭包外 `let sql = format!(...)`（拥有 String）再 `move`，或在闭包内构造 sql（推荐，见 Before/After 2/3 把 `format!` 挪进闭包）。

b. **闭包返回值必须拥有所有权且 Send**。不能返回 `Statement`、`Rows`、`Transaction`（借用 conn）。一律在闭包内 collect 成 `Vec<T>` / `Option<T>` / 标量。

c. **多 await 顺序 = 多次后台往返**。把"读-改-写"逻辑尽量塞进**单个 `call`**，避免 `let x = db.call(read).await?; db.call(write(x)).await?;` 这种两次往返（既慢又破坏原子性，中间可能被其他 call 插入）。需要原子性时务必单闭包 + transaction。
   - 现状 db.rs:205 `update_platform` 先 `get_platform(db, id)?` 再写——迁移后这是两次 await。若需严格原子，合并进一个 call 闭包；若可接受当前语义（本就两条独立锁），分两次 await 也行（行为等价于旧的两次 lock）。

d. **闭包内 `panic` 会怎样**：panic 发生在后台线程。tokio-rusqlite 后台线程 panic 会导致连接通道关闭，后续 `call` 返回 `Error::ConnectionClosed`。避免在闭包内 `unwrap()`/`expect()`/`unreachable!()`。
   - 现状 db.rs:107 `serde_json::from_str(&platform_type_str).unwrap()` 在 `row_to_platform` 内——迁移后这个 unwrap 跑在后台线程，panic 会毒化整个连接。**建议借迁移把这类 unwrap 改成 `?` + map 到 rusqlite 错误**（属顺带加固，非强制）。

e. **不要在闭包内再包 `Mutex` / 不要在 Db 外再包 `Mutex<Db>`**：双重串行化 + 同步锁阻塞 async runtime，是反模式。

f. **不要 `block_on` / 同步锁混用**：迁移后 db.rs 全 async，调用链（lib.rs command、proxy.rs handler）必须全程 async。tray/定时任务等若在非 async 上下文调用 db，需 `tokio::runtime::Handle::block_on` 或改造为 async task。

g. **扩散面警告（最大坑）**：本项目 64 个 command 仅 8 个 async（lib.rs:322/367 等）。迁移后：
   - db.rs ~56 个 `pub fn` → `pub async fn`；
   - 调用它们的 56 个同步 command → 必须改 `async fn` 并 `.await`；
   - proxy.rs / estimate.rs / quota.rs / price_sync.rs 中所有 db 调用加 `.await`（这些多数已在 async 上下文，较好改）；
   - estimate.rs:5 处、manual_budget.rs:1 处、lib.rs:5 处直接 `.lock()` 调用点同步改。
   - **评估建议**：确认异步化收益（不阻塞 Tauri 的 async runtime / 避免长事务卡 UI）是否值得这个改动面。Tauri command 即使是同步 `fn` 也跑在独立线程池，当前 Mutex<Connection> 阻塞的是该 command 线程而非主 UI 线程——异步化的实际收益取决于是否有长查询阻塞问题。（本判断属推测: 需结合实际性能数据决定。）

h. **`close()` 消耗 self**：`Db` 若需显式关闭，`close(self)` 拿走所有权；但应用生命周期内一般不主动 close（managed state 随进程退出）。`call_unwrap` 在已 close 连接上会 panic（lib.rs:314）——别用 `call_unwrap`，用 `call`。

---

### 8. 测试策略（:memory: async 测试）

现状测试 db.rs:1602 `mod tests`，`test_db()` 用 `Db::new(":memory:")`（同步）。迁移后改 async：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Db {
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }

    #[tokio::test]                          // 需 tokio macros feature（Tauri 已带 tokio full）
    async fn test_create_and_list_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("p1")).await.unwrap();
        assert_eq!(p.name, "p1");
        let list = list_platforms(&db).await.unwrap();
        assert_eq!(list.len(), 1);
    }
}
```

要点：
- `Connection::open_in_memory().await`（lib.rs:192）或 `Connection::open(":memory:").await` 均可。
- **`:memory:` 与单后台线程天然契合**：内存库只存在于那条后台连接，所有 `call` 共享同一库，测试无需额外同步。
- 每个 `#[tokio::test]` 各自 `test_db().await` 得独立内存库（独立后台线程），测试隔离。
- 若 `#[tokio::test]` 未启用：确认 `tokio` 的 `macros` + `rt` feature 可用（Tauri 2 通常已 `tokio = { features = ["full"] }`，需在 src-tauri/Cargo.toml 确认；测试也可用 `#[tokio::test(flavor = "current_thread")]`）。

---

## Related Files (内部现状索引)

| File:Line | 说明 |
|---|---|
| `src-tauri/Cargo.toml:26` | `rusqlite = { version = "0.32", features = ["bundled"] }` 待加 tokio-rusqlite |
| `src-tauri/src/gateway/db.rs:7` | `pub struct Db(pub Mutex<Connection>)` 待重定义 |
| `src-tauri/src/gateway/db.rs:40-45` | `Db::new` + WAL pragma 设置点 |
| `src-tauri/src/gateway/db.rs:47-72` | `init_tables`（execute_batch + 9 ALTER）整段进一个 call |
| `src-tauri/src/gateway/db.rs:307-321` | `set_tray_platform` 事务范例 |
| `src-tauri/src/gateway/db.rs:1602-1611` | 测试 `test_db()` :memory: 范例 |
| `src-tauri/src/lib.rs:1945` | `app.manage(db)` State 注入点 |
| `src-tauri/src/lib.rs` | 64 `#[tauri::command]`，仅 8 `async fn`（322/367 等） |
| `src-tauri/src/gateway/proxy.rs:32-34` | `pub db: Arc<Db>` + 注释（迁移后 Arc 可去） |
| `.lock()` 分布 | db.rs:55, lib.rs:5, estimate.rs:5, manual_budget.rs:1（共 ~66，与任务给的 ~56 量级一致） |

## External References

- crates.io API（版本+依赖事实源）：
  - `https://crates.io/api/v1/crates/tokio-rusqlite` → max_stable `0.7.0`
  - `https://crates.io/api/v1/crates/tokio-rusqlite/0.7.0/dependencies` → rusqlite `^0.37`
  - `https://crates.io/api/v1/crates/tokio-rusqlite/0.6.0/dependencies` → rusqlite `^0.32`（推荐版）
- docs.rs：https://docs.rs/tokio-rusqlite/
- GitHub README（call 闭包范例 + `#![forbid(unsafe_code)]`）：https://github.com/programatik29/tokio-rusqlite/blob/master/README.md
- GitHub `src/lib.rs`（API 签名权威源，行号引用基于 master）：https://github.com/programatik29/tokio-rusqlite/blob/master/src/lib.rs
  - `pub enum Error<E = rusqlite::Error>` lib.rs:116
  - `#[derive(Clone)] pub struct Connection` lib.rs:166
  - `pub async fn open` lib.rs:182 / `open_in_memory` lib.rs:192
  - `pub async fn call<F,R,E>` lib.rs:272，闭包 `FnOnce(&mut rusqlite::Connection) -> Result<R,E> + 'static + Send`

## Caveats / Not Found

- **版本抉择是关键决策点**：0.6.0（rusqlite 0.32 对齐，零升级）vs 0.7.0（rusqlite 0.37，需审计 0.32→0.37 API 差异）。本研究**未做 rusqlite 0.32→0.37 API diff 审计**——若选 0.7.0 需补这一步。
- 推测: 异步化的实际性能/体验收益未量化。Tauri 同步 command 已在线程池跑，当前 `Mutex<Connection>` 阻塞的是 command worker 线程而非 UI；是否值得 ~120 处 async 扩散需结合实测（是否存在长事务/慢查询卡顿）判断。
- tokio-rusqlite 0.6.0 的 `src/lib.rs` 行号引用取自 **master 分支**（最新），与 0.6.0 tag 的 API 形态一致（call/Error/Connection 结构自 0.5+ 稳定），但精确行号可能与 0.6.0 tag 有微小偏移——API 签名本身可靠。
