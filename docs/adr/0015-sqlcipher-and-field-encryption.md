# ADR 0015: 三个数据库整库 SQLCipher 加密 + 敏感列 `enc1:` AES-GCM

## 背景

2026-10-19 调研曾裁决「整库加密条件触发、现阶段不做」。2026-10-20 用户以「合规要求」满足触发条件（要求 at-rest encryption 全覆盖），spec `db-sqlcipher-full` 转入实施，并拆为 `db-log-redact-hardening`（日志脱敏收紧）、`db-field-encryption`（敏感列加密）、`db-sqlcipher-full`（整库加密）三个 spec（账本 `.scratch/memory/2026-10-20.md:4,59`）。

## 决策

1. **整库加密**：三个文件库（`aidog.db` / `log.db` / `platform.db`）打开时统一先执行 `PRAGMA key`，key 形式为 `x'<hex>'`（`aidog_db/src/sqlcipher.rs:27,48`）。内存库不设 key（行为同标准 SQLite，既有测试透明）。
2. **存量明文库迁移**：启动时经官方 `sqlcipher_export` 一次性迁移（`sqlcipher.rs:59,85`）；rename 替换成功前明文库原样保留，任一步失败保留明文并报错，下次启动重试。
3. **敏感列加密**：`platform.api_key`、MCP env / headers、`cc_proxy_auth`、OAuth raw 列存 `enc1:<base64(nonce12||ct+tag)>`（AES-256-GCM，`aidog_db/src/crypt.rs:13,25`）；读取侧非该前缀按明文兼容（存量未迁移行）。
4. **主密钥来源（当前行为，以 `crypt.rs:4-12` 为准）**，优先级：
   1. 环境变量 `AIDOG_DB_MASTER_KEY_FILE`（测试 / 无头显式注入，指向 key 文件，缺省创建）；
   2. key 文件 `~/.aidog/master.key`（权限 0600），存在即用，**不碰 keychain**；
   3. keychain 存量条目：一次性读出并落盘 key 文件，此后永走第 2 步；不删 keychain 条目，留作文件丢失时的恢复手段；
   4. 全新安装：生成新 key 写 key 文件，**不写 keychain**（未签名或每次重签的二进制每读一次 keychain 就弹系统授权窗）。

   2026-10-20 账本与 `sqlcipher.rs` 头部注释仍写「keychain 主密钥」，那是 2026-10-21「永远静默」拍板之前的口径，已过时。
5. 威胁面：防备份 / 同步带走 DB 而没带走 key 文件；不防同用户进程（能读家目录的进程本就在威胁模型边界外）。
6. **排查链路**：macOS 系统 `sqlite3` 不带 `PRAGMA key`，请求排查走内核 `/api/rpc` 管理面（见 skill `aidog-request-inspect`）。

## 后果

- 密文损坏一行会使整个 `list_platforms` / `list_mcp_servers` 失败（表级而非行级）。
- 首启迁移临时占约 2 倍库容，`.cipher-mig` 残留需清理；两项真机验收账本记为未执行（`.scratch/memory/2026-10-20.md:106`）。
- 测试纪律：fixture 路径进程级唯一；测试禁触真 keychain（已见 `.claude/rules/db-pool-routing.md`、`shared-worktree.md`）。
- **CI 三平台（Windows / Linux）crypto 链接：未验证。** 当时仅 darwin 本机实证，账本记「待 CI 实证」（`2026-10-20.md:82,106`），本次整理未找到后续 CI 结果证据。

## 备选方案

- **只做列加密，不做整库加密**（2026-10-19 原裁决）：用户合规要求 at-rest 全覆盖，已被触发条件推翻。
- **log.db 豁免整库加密**（把 bodies 移出加密库）：落选，log.db 在合规语义核心范围内，豁免削弱 spec 目标。
- **keychain 作为主密钥来源**：初版方案，因未签名二进制反复弹授权窗改为 key 文件优先（见决策 4）。

## 已裁决不修

| 事项 | 不修理由 | 复活条件 | 裁决日期 | 来源 |
|---|---|---|---|---|
| `group_key` 不加密 | 真实泄漏面在磁盘 JSON，只加密 DB 列近乎假防护；加密 DB 列并把磁盘改为引用注入成本收益不成比例 | `bind_lan` 长期启用，或威胁模型升级 | 2026-10-20 | `.scratch/memory/2026-10-20.md:34` |
| bodies 全开场景 CPU +4.2ms/请求（+43%） | 端到端延迟与吞吐零回退（差异在噪声带），+43% 仅在 bodies 全开的调试场景兑现；否掉「把 bodies 移出加密库」与「另开 bodies 减载票」 | ① bodies 全开且大请求体成主负载并有 CPU 瓶颈信号；② 平均请求体 ≥1MB 或 retention 使库增长一个数量级；③ p99 周期性劣化 >20% 且已排除环境 | 2026-10-20 | `2026-10-20.md:91` |
| 三份同形加密列迁移 helper（`migrate_*` 含 oauth raw）不抽公共函数 | 三处连接 / 表 / 列形态各异，参数化泛型比三份约 40 行更难读；「模式照抄先例」是票面要求 | 出现第四个加密列或第四个同形迁移时抽公共 migrate | 2026-10-20 | `2026-10-20.md:25,72` |
| `sqlcipher.rs` 的 `is_memory_path` 三处重复、migrate / key 前奏四处重复 | smell 判断题，沿袭原 pragma 重复模式 | 内存路径语义需要改时统一 | 2026-10-20 | `2026-10-20.md:82` |
