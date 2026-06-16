# PRD — 定时备份 (scheduled-backup)

> Task: `06-16-scheduled-backup` | 作者: nico | 状态: planning

## 1. 背景 / 动机

aidog 已有「手动导入导出」模块 (`gateway/import_export/`): 用户在设置页手动点导出 → AES-256-GCM 加密 → `.aidogx` 容器。但需用户主动操作, 易遗忘, 数据库损坏 / 误删平台后无法恢复。

定时备份 = 把「全量导出」自动化: 用户设定间隔, 后端定时复用现有 `collect` 导出全 scope, 加密落盘到本地, 超期自动清理, 实现「零干预的安全网」。

## 2. 用户故事

- **US-1**: 作为用户, 我希望开启定时备份后, 应用按我设的间隔 (如每天) 自动导出全部数据 (平台 / 分组 / 设置 / Codex / Claude Code / skills 等), 无需我手动操作。
- **US-2**: 作为用户, 我希望旧备份自动清理, 默认保留 7 天, 避免磁盘膨胀, 但保留期我可调。
- **US-3**: 作为用户, 我希望能在设置页看到上次备份时间 + 下次预计时间, 并能立即手动触发一次备份。
- **US-4**: 作为用户, 我希望备份失败 (磁盘满 / 加密错) 时有系统通知提醒 (复用 notification 模块)。
- **US-5**: 作为用户, 我希望备份文件可被现有「导入」功能直接恢复 (同 `.aidogx` 格式, 密钥机制一致)。

## 3. 需求 (功能)

### 3.1 后端 backup job

| 项 | 规格 |
| --- | --- |
| 触发点 | (a) 应用 setup 启动时检查 (throttle: 距 `last_backup_at` < interval → 跳过); (b) 运行中常驻 `tokio` 定时任务循环 (按 interval 周期唤醒)。两者共用同一 `maybe_backup` 入口。 |
| 导出范围 | 全 scope (复用 `import_export::collect` 的 7 个 SCOPE_* 常量, 等价手动「导出全部」)。 |
| 落盘目录 | `~/.aidog/backups/` (`aidog_data_dir().join("backups")`)。 |
| 文件命名 | `aidog-backup-YYYYMMDD-HHMMSS.aidogx` (UTC, 避免时区歧义; 文件名自带时间便于人读 + retention 排序)。 |
| 加密 | 复用 `import_export::encrypt` (AES-256-GCM), 密钥来源与手动导出一致 (隐藏密钥机制, 见 design.md §3)。 |
| retention | 启动 + 每次备份成功后扫描 `backups/`, 删除 mtime 早于 `now - retention_days*86400` 的 `.aidogx`。默认 7 天, 可配 1–90。 |
| 失败处理 | 记 trace error + `last_backup_error` 字段 + 发 notification (若用户开启通知, 复用 `notification::dispatch`)。 |
| 并发防护 | `Mutex<bool>` (或 `AtomicBool`) 防同帧多次 backup 重入 (启动检查撞上定时器唤醒)。 |

### 3.2 设置 schema (db settings)

新增 settings key (scope = `app` 或 backup 专用 namespace):

| key | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `backup.enabled` | bool | false | 总开关 |
| `backup.interval_hours` | i64 | 24 | 间隔 (小时), 取值 ≥1; UI 给快捷枚举 1/6/12/24/168 |
| `backup.retention_days` | i64 | 7 | 保留天数, 取值 1–90 |
| `backup.last_backup_at` | i64 | 0 | 上次成功备份 epoch 秒 (throttle + 展示) |
| `backup.last_backup_error` | string | "" | 上次错误信息 (空=成功) |

> settings 复用 `db.rs` 现有 key-value (`list_all_settings_raw` / upsert)。namespace 走现有 settings 机制。

### 3.3 Tauri commands

| command | 入参 | 出参 | 说明 |
| --- | --- | --- | --- |
| `backup_settings_get` | — | `BackupSettings` | 读取 5 key |
| `backup_settings_set` | `BackupSettings` | `()` | 写入 (写后重启定时器循环) |
| `backup_run_now` | — | `BackupResult { ok, path?, error?, timestamp }` | 立即触发一次 (忽略 throttle) |

### 3.4 前端 UI

嵌入位置: 现有 `ImportExport.tsx` tab (设置页「导入导出」) 内新增「定时备份」section (与手动导入导出语义相邻, 不新增 tab, 降低导航复杂度)。

UI 元素:
- 开关: 启用定时备份
- 间隔下拉 (枚举: 每小时/6 小时/12 小时/每天/每周 → 映射 interval_hours 1/6/12/24/168)
- 保留天数 (数字输入或下拉 3/7/14/30)
- 状态展示: 「上次备份: <时间 or 从未」 / 「下次预计: <时间>」 / 上次错误 (红色, 若有)
- 按钮: 「立即备份一次」 → `backup_run_now`
- 备份目录路径展示 + 「在 Finder 打开」(macOS, 复用 opener)

### 3.5 i18n

7 语言 (zh-CN / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP)。新 key 前缀 `settings.backup.*`:
`title / enable / interval / intervalHour / interval6h / interval12h / intervalDaily / intervalWeekly / retention / retentionDays / lastBackup / never / nextBackup / runNow / running / backupDir / openDir / lastError / enableHint`

(详见 implement.md §5)

## 4. 验收标准

1. ✅ 复用 `import_export::collect` + `encrypt`, 不重复造导出 / 加密逻辑 (grep 确认无独立 collect 实现)。
2. ✅ 应用启动后, 若 `enabled && (now - last_backup_at) >= interval_hours*3600` → 自动备份一次。
3. ✅ 运行中, 定时器按 interval 周期触发备份; 关闭 enabled → 定时器停。
4. ✅ retention 扫描删除超期文件 (单测: 造旧文件 → 跑 cleanup → 验证删除)。
5. ✅ 备份文件可被现有「导入」功能恢复 (格式 / 密钥一致, 手测)。
6. ✅ 设置页 section 展示上次 / 下次 / 错误, 「立即备份」按钮可触发。
7. ✅ 7 语言 key 全覆盖 (`check-i18n.mjs` 通过)。
8. ✅ `cargo clippy` + `cargo test` + `yarn build` (tsc) 零 warning 零 error。
9. ✅ 重入防护: 短时间内多次触发只跑一次。

## 5. 非目标 (out of scope)

- ❌ 云备份 / 远程同步 (S3 / WebDAV) — 仅本地。
- ❌ 增量备份 — 每次全量。
- ❌ 备份压缩 (AES 容器已含; 全量 JSON 不大, 暂不压缩)。
- ❌ 备份加密密钥管理与手动导出分离 — 复用同一隐藏密钥。
- ❌ 跨设备恢复的无密钥方案 — 恢复仍需原设备 (同手动导出约束)。
- ❌ 备份历史 UI 浏览 / 单文件恢复 — 仅展示状态, 恢复走现有导入流程。

## 6. 开放问题 (需用户澄清 → design.md 落定)

- **Q1 间隔粒度**: 选 **枚举快捷 (1h/6h/12h/24h/168h)** 还是自由小时数输入? → **倾向枚举**, 简单且覆盖典型场景。
- **Q2 加密密钥**: 复用手动导出的隐藏密钥 (备份恢复需原设备), 还是用固定设备绑定密钥? → **倾向复用隐藏密钥** (与手动导出一致, 用户已有心智模型; 且容器格式不变, 导入侧零改动)。
- **Q3 备份失败通知**: 是否接 notification 模块? 若用户关通知则只留 `last_backup_error`。→ **倾向接**, 复用现有 dispatch。

> 上述倾向为推荐默认, 待 main 转 AskUserQuestion 确认后写入 design.md「决策记录」。
