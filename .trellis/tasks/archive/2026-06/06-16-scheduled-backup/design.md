# Design — 定时备份 (scheduled-backup)

> Task: `06-16-scheduled-backup` | 依赖 prd.md

## 1. 模块布局

```
src-tauri/src/gateway/
  backup.rs              ← 新增: BackupSettings / maybe_backup / run_backup / cleanup_expired / spawn_scheduler
  import_export/         ← 复用 (collect / encrypt / Payload) — 不改
  ...
src-tauri/src/lib.rs     ← +3 commands (backup_settings_get/set, backup_run_now) + setup spawn scheduler
src/components/settings/ImportExport.tsx  ← 嵌入「定时备份」section
src/services/api.ts      ← +BackupSettings / BackupResult 类型 + 3 invoke 封装
src/locales/*/           ← 7 语言新 key
```

## 2. 架构决策记录

| # | 决策 | 选项 | 选定 | 理由 |
| --- | --- | --- | --- | --- |
| D1 | 定时机制 | (a) 仅启动检查 throttle; (b) 仅常驻 loop; (c) 两者 | **(c)** | price_sync 用 (a) 但无运行中持续需求; backup 需常驻运行 (用户长开应用), 启动检查补「关机错过」场景。 |
| D2 | loop 实现 | (a) `tokio::time::interval`; (b) `sleep` 循环 + 每轮读 settings | **(b)** | 每轮读 settings 使「改间隔 / 关开关」即时生效, 无需重启 loop; `interval` 周期固定不灵活。tick 间隔 = min(interval, 60s) 轮询判 `now-last>=interval`。 |
| D3 | 调度生命周期 | spawn 一个长任务, 改设置时 kill+重启 vs 单任务读 settings | **单任务读 settings** | 改设置只写 db, loop 每轮自然 pickup; 免 handle 管理。loop 在 setup spawn, app 生命周期内常驻。 |
| D4 | 加密密钥 | 复用隐藏密钥 vs 新设备密钥 | **复用隐藏密钥** | 与手动导出一致, 用户心智统一; 容器格式不变, 导入侧零改动; 恢复需原设备 (同手动导出, 已为既有约束)。 |
| D5 | 文件命名时区 | local vs UTC | **UTC** | mtime 排序 + retention 不涉及时区; 文件名 UTC 避免歧义; UI 展示 last_backup_at 时前端转 local。 |
| D6 | UI 嵌入 | 新 tab vs ImportExport section | **ImportExport section** | 语义相邻 (备份=导出), 免增导航; ImportExport.tsx 已是设置页一 tab。 |
| D7 | 间隔粒度 | 枚举 vs 自由输入 | **自由小时数** (≥1, UI 数字输入) | 用户选定 — 灵活; 后端校验 `interval_hours ≥ 1`, UI 给快捷预设 (1/6/12/24/168) + 自定义输入。 |
| D8 | retention 触发 | 独立 loop vs 挂在 backup 成功后 | **backup 成功后 + 启动** | 清理与备份天然同源, 无需额外 loop; 启动补一次防长期不开 backup。 |

## 3. 加密密钥链路 (复用现状)

import_export 隐藏密钥机制 (推测: 见 `import_export/mod.rs` 的 encrypt/decrypt, 密钥派生自设备特征 + 落盘盐, 详见源码)。backup.rs 直接调 `gateway::import_export::encrypt(&bytes)`:

```rust
let payload = gateway::import_export::collect::collect(&db, &ALL_SCOPES).await?;
let bytes = payload.to_bytes_verified()?; // 含 checksum
let encrypted = gateway::import_export::encrypt(&bytes)?;
std::fs::write(&path, &encrypted)?;
```

> `需要:` 若 import_export 密钥非「设备绑定 + 可离线恢复」, 需 main 核实恢复链路 (影响 Q2)。当前按「与手动导出完全一致」实现, 行为可预期。

## 4. 数据模型

### 4.1 BackupSettings (Rust + TS)

```rust
// backup.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSettings {
    pub enabled: bool,
    pub interval_hours: i64,   // ≥1
    pub retention_days: i64,   // 1..=90
    pub last_backup_at: i64,   // epoch sec, 只读 (后端写)
    pub last_backup_error: String,
}
```

```ts
// api.ts
export interface BackupSettings {
  enabled: boolean;
  interval_hours: number;
  retention_days: number;
  last_backup_at: number;
  last_backup_error: string;
}
export interface BackupResult {
  ok: boolean;
  path?: string;
  error?: string;
  timestamp: number;
}
```

### 4.2 db settings 读写

复用 `db.rs` 现有 settings KV (`get_setting`/`set_setting` 或 raw row)。namespace 建议 `backup.<field>` (单行 JSON 亦可, 但与现有 `setting` 表 (scope,key,value) 三元组风格一致 → 用 5 个 key)。`BackupSettings::load(&db)` / `save(&db)` 封装。

## 5. 核心流程伪码

### 5.1 maybe_backup (throttle + 重入防护)

```rust
static BACKUP_RUNNING: AtomicBool = AtomicBool::new(false);

pub async fn maybe_backup(db: &Db) -> Result<Option<PathBuf>, String> {
    let s = BackupSettings::load(db).await?;
    if !s.enabled { return Ok(None); }
    let now = now_secs();
    if s.last_backup_at > 0 && now - s.last_backup_at < s.interval_hours * 3600 {
        return Ok(None); // 未到点
    }
    run_backup(db).await.map(Some)
}
```

### 5.2 run_backup (导出 + 落盘 + retention + 通知)

```rust
pub async fn run_backup(db: &Db) -> Result<PathBuf, String> {
    if BACKUP_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("backup already running".into());
    }
    let result = run_backup_inner(db).await;
    BACKUP_RUNNING.store(false, Ordering::SeqCst);
    match result {
        Ok(path) => { update_last(db, now, "".into()).await;
                      let _ = cleanup_expired(db).await; Ok(path) }
        Err(e) => { update_last_err(db, e.clone()).await;
                    notify_backup_failed(&e).await; Err(e) }
    }
}
```

### 5.3 scheduler loop (setup spawn)

```rust
pub fn spawn_scheduler(handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let db = /* from handle state */;
            let _ = maybe_backup(&db).await;
            let s = BackupSettings::load(&db).await.unwrap_or_default();
            let tick = (s.interval_hours * 3600).min(60).max(1) as u64;
            tokio::time::sleep(Duration::from_secs(tick)).await;
        }
    });
}
```

> 启动时 setup 内先 `maybe_backup`(补关机错过), 再 `spawn_scheduler`。

### 5.4 cleanup_expired

```rust
async fn cleanup_expired(db: &Db) -> Result<(), String> {
    let days = BackupSettings::load(db).await?.retention_days;
    let cutoff = now_secs() - days * 86400;
    let dir = aidog_data_dir()?.join("backups");
    for entry in read_dir(&dir)? {
        let p = entry?.path();
        if p.extension().and_then(|e| e.to_str()) != Some("aidogx") { continue; }
        if let Ok(meta) = p.metadata() {
            let mtime = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64;
            if mtime < cutoff { let _ = std::fs::remove_file(&p); }
        }
    }
    Ok(())
}
```

## 6. lib.rs 改动点

1. `mod backup;` (gateway mod 树)
2. setup 闭包内: `backup::maybe_backup(&db).await` (block_on) + `backup::spawn_scheduler(handle)`
3. 3 个 `#[tauri::command]`: `backup_settings_get` / `backup_settings_set` / `backup_run_now`
4. `invoke_handler` 注册 3 command
5. `aidog_data_dir` 已导出 (现有), backup.rs 直接用

## 7. 前端改动点

### 7.1 api.ts
+ `BackupSettings` / `BackupResult` 类型 + `backupSettingsApi.{get,set,runNow}` (仿现有 `importExportApi`)

### 7.2 ImportExport.tsx
- 顶部或底部加 `<ScheduledBackupSection />` (可内联或拆子组件)
- useEffect 加载 settings; onChange debounce → `backup_settings_set`
- 「立即备份」按钮 → `backup_run_now` → toast 结果
- 时间展示用现有 `utils/formatters.ts` (禁页内重定义)

### 7.3 i18n key (前缀 `settings.backup.*`)
`title / enable / enableHint / interval / intervalHour / interval6h / interval12h / intervalDaily / intervalWeekly / retention / retentionDays / lastBackup / never / nextBackup / runNow / running / backupDir / openDir / lastError / successToast / failedToast`

7 语言文件: `src/locales/{zh-CN,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP}/common.json` (或现有 settings namespace 文件 — 按 `frontend-i18n-coverage` 规约定位)。

## 8. 测试策略

| 层 | 测试 | 位置 |
| --- | --- | --- |
| 单元 | `cleanup_expired`: 造 3 文件 (旧/边界/新) → 删旧留新 | `backup.rs` `#[cfg(test)]` |
| 单元 | `BackupSettings::load/save` round-trip | 同上 |
| 单元 | `maybe_backup` throttle: last_backup_at 近 → 返回 None | 同上 |
| 集成 (手测) | 备份 → 导入恢复 (格式一致) | 手动 |
| Lint | clippy / tsc / check-i18n.mjs | CI |

## 9. 风险

- **R1 setup 阻塞**: `maybe_backup` 在 setup 内 block_on 若导出慢 → 启动卡。**缓解**: setup 内不直接跑, spawn 异步跑 maybe_backup (只 spawn_scheduler 一处, 内含首次 maybe_backup)。
- **R2 密钥恢复**: 若隐藏密钥设备绑定 → 换机恢复失败。**缓解**: 文档说明 + 与手动导出同约束 (接受)。
- **R3 磁盘满**: 写文件失败 → 记 error + 通知 + 不更新 last_backup_at (下轮重试)。**缓解**: 已在 run_backup 错误分支处理。
- **R4 时区**: 文件名 UTC, UI local。前端转 + ISO 展示, 后端只存 epoch。
