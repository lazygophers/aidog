# 移除 mitm_ca / mitm_whitelist 表，数据迁 setting

## Goal

把 MITM 子系统当前独占的两张表（`mitm_ca` 单行 CA、`mitm_whitelist` 多行规则）**数据迁移到通用 `setting` 表**，并 **DROP 掉两张旧表**。MITM 不再持有专属 schema，复用 setting 的 scope/key/value JSON 机制（与 `middleware:settings` / `global:coding_tools_settings` 同模式）。

**为什么**：用户要求「移除表 mitm_ca、mitm_whitelist，这里的数据应该是存在 setting」。两张表语义都是「MITM 配置」而非「业务实体」——CA 是单例配置对象、白名单是规则数组，天然适合 setting 的 scope/key JSON 存储，无需专属表。删表后 schema 更干净，setting 缓存机制（`db.1.settings` HashMap + 写时 invalidate）顺带给 MITM 白名单热路径带来内存命中。

## 现状（已实现）

- `src-tauri/src/gateway/db/schema_late.rs:373-398`：CREATE TABLE mitm_ca（单行 id=1）+ mitm_whitelist（UNIQUE host_pattern）。
- `schema_late.rs:399-430`：migration 042 加 rule_type 列 + 回填；`seed_default_whitelist_if_empty`（L475）首次空表填 37 条 DEFAULT_RULES + 已配平台 host。
- `src-tauri/src/gateway/mitm/ca.rs`：`load_root_ca`（L184 SELECT）/ `create_and_store_root_ca`（L212 INSERT）/ `set_ca_installed`（L257 UPDATE）/ `set_enabled`（L432 UPDATE）—— 全直 SQL。
- `src-tauri/src/gateway/mitm/whitelist.rs`：`list_whitelist`（L176 SELECT 全表）/ `matches_db`（L200 list+matches_host）。
- `src-tauri/src/commands/mitm.rs`：`mitm_whitelist_add/remove/toggle/import_defaults/clear` 直 SQL（L228-357）；`mitm_whitelist_test_url` 走 list_whitelist。
- `src-tauri/src/gateway/proxy/connect.rs:134`：每 CONNECT 调 `matches_db(&state.db, &host)`。
- `src-tauri/src/gateway/mitm/mod.rs:75`：`MitmState::signer_or_init` 调 `load_root_ca`。
- `src/services/api/mitm.ts`：前端 TS 类型 + invoke 封装（**公共契约层，本次零改**）。

## setting 表模式（参考）

- schema（`schema_early.rs:64`）：`setting (id, scope, key, value TEXT='{}', created_at, updated_at, deleted_at, UNIQUE(scope,key))`，软删模式。
- 访问 API（`db/settings.rs`）：`get_setting(db, scope, key) -> Option<serde_json::Value>`（带内存缓存）、`set_setting(db, SetSettingInput{scope,key,value})`（upsert + invalidate 缓存）、`delete_setting`、`list_setting_keys`、`list_all_settings_raw`（导入导出用）。
- 现有 scope 取值：`app` / `global` / `middleware` / `proxy` / `stats` / `notification` / `hooks` / `script_executor` / `skills` 等。MITM 新增 scope = `mitm`。
- 导入导出（`import_export/`）：setting 走 `SCOPE_SETTING` 统一数组 payload，`scope` 字段自由。MITM 数据进 setting 后**自动随 setting 数组导入导出**（无需专门迁移 import_export）。

## Requirements

### R1 存储模型（2 key）

- R1.1 `scope = "mitm"`，两个 key：
  - `key = "ca"`，value = RootCa 对象 JSON：`{private_key_pem, cert_pem, fingerprint, created_at, enabled, ca_installed}`（字段名与 `RootCa` struct 对齐，camelCase 由 serde 决定——保持 struct 现有序列化风格；实施时 grep RootCa 现有 serde 派生确认）。CA 不存在 → get_setting 返 None（保持 `load_root_ca` 返 `Option<RootCa>` 语义）。
  - `key = "whitelist"`，value = JSON 数组：`[{host_pattern, rule_type, enabled, source, created_at}, ...]`，顺序 = created_at 升序（保现有 list_whitelist ORDER BY created_at 行为）。空数组 `[]` = 无规则（与旧空表等价）。
- R1.2 `RootCa` / `WhitelistEntry` struct **公共签名不变**（仍 `pub`，字段不变），仅其 DB 持久化路径从专属表换 setting。`ca.rs` / `whitelist.rs` 的非 DB 函数（generate_root_ca / sign_host_cert / matches_host / matches_rule / evaluate_host / DEFAULT_RULES / cert_fingerprint_hex 等）**零改**。
- R1.3 `serde`：RootCa 当前未派生 Serialize/Deserialize（手动读列构造）。实施时为 RootCa + WhitelistEntry 加 `#[derive(Serialize, Deserialize)]`（`#[serde(rename_all = "snake_case")]` 与现有字段名对齐——grep struct 字段已 snake_case）。白名单 enabled 用 `serde` 默认行为（bool ↔ JSON bool）。

### R2 ca.rs 重写 DB 层

- R2.1 `load_root_ca(db)`：`get_setting(db, "mitm", "ca")` → `serde_json::from_value::<RootCa>(v)` → Some；None / 解析失败 → None（解析失败 tracing::warn 不炸，与 setting 既有 fallback 一致）。
- R2.2 `create_and_store_root_ca(db)`：生成 CA → `set_setting(db, "mitm", "ca", serde_json::to_value(&ca)?)`。覆盖语义 = upsert（set_setting ON CONFLICT 已实现）。
- R2.3 `set_ca_installed(db, installed)`：load → 改 ca_installed 字段 → set_setting 整对象回写（read-modify-write，单 async fn 内串行，无并发竞争——CA 装信任库是用户低频操作）。
- R2.4 `set_enabled(db, enabled)`：同 R2.3 read-modify-write 改 enabled 字段。
- R2.5 `ensure_root_ca` / `sync_ca_installed_from_system` / `verify_trust_installed` / `enforce_db_file_permissions` 等**不动**（它们调 load/set 函数，间接受益）。

### R3 whitelist.rs 重写 DB 层

- R3.1 `list_whitelist(db)`：`get_setting(db, "mitm", "whitelist")` → `serde_json::from_value::<Vec<WhitelistEntry>>(v)` → 数组（None → 空 Vec）。**移除 ORDER BY**——数组本身即存顺序（写时保 created_at 升序插入）。
- R3.2 `matches_db(db, host)`：`list_whitelist` + `matches_host`（逻辑不变，仅数据源换）。CONNECT 热路径走 get_setting 内存缓存（缓存命中零 DB 往返，比旧 SQL 快）。

### R4 commands/mitm.rs 白名单写操作改 read-modify-write

- R4.1 `mitm_whitelist_add`：load 当前数组 → 校验 host_pattern 不重复（重复 → 静默忽略等价旧 INSERT OR IGNORE，或 Err 提示——**保持等价：重复静默成功**）→ push 新条目（created_at = now，source = "user"）→ set_setting 整数组。
- R4.2 `mitm_whitelist_remove(host_pattern)`：load → filter 掉匹配项 → set_setting。
- R4.3 `mitm_whitelist_toggle(host_pattern, enabled)`：load → map 改匹配项 enabled → set_setting。
- R4.4 `mitm_whitelist_import_defaults`：load → 对 DEFAULT_RULES 每条，数组中无同 host_pattern 则 push（source="default", created_at=now）→ set_setting。统计 imported/skipped 返 ImportDefaultsResult（语义不变）。**幂等**：再跑跳过已存在。
- R4.5 `mitm_whitelist_clear`：set_setting 空数组 `[]`（不 delete_setting——保留 key 便于后续 list 不需判 None；返旧行数 = 清前数组长度）。语义：返删除条数（前端 toast）。
- R4.6 `mitm_whitelist_test_url`：走 list_whitelist（不变）。
- R4.7 **原子性**：read-modify-write 在 command async fn 内串行；set_setting 走 write_conn 单事务。MITM 白名单低频用户操作 + write_conn 全局串行，无竞态。无需额外锁。
- R4.8 校验保留：`valid_rule_type` 4 合法值校验在 add 入口不变；host_pattern trim+lowercase 归一化不变。

### R5 schema migration（数据迁移 + DROP 旧表）

- R5.1 `schema_late.rs` 新增 migration（接现有 migration 序号，grep 现有最大号后续编）：
  1. 读旧 `mitm_ca` 行（id=1）→ 若有 → 构造 RootCa JSON → `INSERT INTO setting (scope, key, value, ...) VALUES ('mitm', 'ca', <json>, ...)`。
  2. 读旧 `mitm_whitelist` 全表 ORDER BY created_at ASC → 构造 JSON 数组 → `INSERT INTO setting ('mitm', 'whitelist', <json array>)`。
  3. `DROP TABLE IF EXISTS mitm_ca;`
  4. `DROP TABLE IF EXISTS mitm_whitelist;`
- R5.2 **幂等**：migration 用 `INSERT OR IGNORE INTO setting`（UNIQUE(scope,key)）—— 已迁过的库重跑不覆盖。DROP TABLE IF EXISTS 天然幂等。
- R5.3 **删旧 CREATE TABLE / 旧 migration 041/042 / seed_default_whitelist_if_empty**：
  - CREATE TABLE mitm_ca/mitm_whitelist 块（L373-398）**移除**（新库不再建这两表）。
  - migration 042（rule_type 回填，L399-429）**移除**（旧库迁完后表已 DROP；新库无表无需回填）。
  - `seed_default_whitelist_if_empty`（L475-511）：**改为 setting 版**——首次空 setting（mitm:whitelist 不存在或空数组）时填 37 DEFAULT_RULES + 已配平台 host（逻辑不变，落点换 setting）。或**移除函数 + 把 seed 逻辑并入 R5.1 migration**（新库首次 migration 时 setting 无 whitelist → 直接 seed 默认 + 平台 host）。**实施时二选一，推荐并入 migration（单源，避免 seed 函数与新表脱节）**。
  - migration 041（旧 seed 调用点 L430）相应移除/改写。
- R5.4 旧库迁移路径完整：旧表数据 → setting JSON → DROP。新库路径：无旧表 → migration 直接 seed 默认白名单到 setting（mitm:ca 首次启用时 ensure_root_ca 写入，不在 migration seed）。
- R5.5 schema_late.rs 的 mitm 相关测试（L895-972 等 `has_mitm_ca` / `has_mitm_whitelist` 断言、`seed_default_whitelist_if_empty` 旧表 seed 测试、旧表 fixture L1010+）**全部更新为新 setting 断言**（`SELECT value FROM setting WHERE scope='mitm' AND key='whitelist'` 含 37 条；`has_mitm_ca` 改判 setting 行存在 / DROP 后 `PRAGMA table_info(mitm_ca)` 无表）。

### R6 测试

- R6.1 `ca.rs` 测试（L730+ 大量 RootCa 生成 / sign_host_cert 单测）：纯逻辑测试不动；涉及 DB 的 `load_root_ca` / `create_and_store_root_ca` / `set_ca_installed` / `set_enabled` 测试改用 setting 验证（写后读对等 / set_setting 后 get_setting 命中）。
- R6.2 `whitelist.rs` 测试：纯匹配引擎测试（matches_host / matches_rule / evaluate_host / DEFAULT_RULES 完整性）零改；DB 相关（import_defaults dedup / clear 复刻 SQL 测试 L416-567）**改为复刻 setting read-modify-write 逻辑**（mock setting 数组而非建 mitm_whitelist 表）。
- R6.3 `commands/mitm.rs` 测试：valid_rule_type / parse_host_from_input 纯函数不动。
- R6.4 `schema_late.rs` migration 测试：新增「旧库有 mitm_ca/mitm_whitelist 行 → migration 后 setting 含对应 JSON + 旧表 DROP」迁移测试（用 `make_legacy_conn_with_group_path` 同模式建含旧表的 fixture conn）。
- R6.5 `test_connect.rs` 的 matches_db 集成测试（L306-332）：走新 setting 路径，验 anthropic.com 命中 / unknown.example miss（语义不变）。

### R7 门禁

- R7.1 `cargo test` 全过（ca / whitelist / commands_mitm / schema_late migration / test_connect）。
- R7.2 `cargo clippy` 无新 warning。
- R7.3 `yarn build`（tsc）过——前端 api/mitm.ts 零改，应无影响。
- R7.4 主仓零改动（worktree 内）。

## Acceptance Criteria

- [ ] scope=mitm 两 key（ca 对象 / whitelist 数组）存储模型落地
- [ ] ca.rs 4 个 DB 函数（load/create/set_ca_installed/set_enabled）走 get_setting/set_setting
- [ ] whitelist.rs list_whitelist/matches_db 走 get_setting
- [ ] commands/mitm.rs 5 个白名单写操作（add/remove/toggle/import_defaults/clear）read-modify-write setting，幂等 + 原子
- [ ] schema migration：旧表数据迁 setting + DROP 两表；新库 seed 默认白名单到 setting
- [ ] 删 CREATE TABLE mitm_ca/mitm_whitelist + 旧 migration 041/042 + seed_default_whitelist_if_empty（或改写）
- [ ] RootCa / WhitelistEntry / 全部 #[tauri::command] 公共签名不变（前端零改）
- [ ] cargo test + clippy 全过；yarn build 过
- [ ] 主仓零改动

## Definition of Done

- 两表数据迁 setting，旧表 DROP，schema 无 mitm_ca/mitm_whitelist 残留
- MITM 子系统全部读写走 setting（get_setting 缓存命中热路径）
- 公共契约层（struct 字段 / command 签名 / 前端 TS）零改
- 单测覆盖迁移路径 + read-modify-write 原子语义
- journal 记录 scope=mitm 命名依据 + seed 并入 migration 决策

## Technical Approach

```
setting 表新增 2 行（scope=mitm）：
  (mitm, ca,        {"private_key_pem":..., "cert_pem":..., "fingerprint":..., "created_at":..., "enabled":..., "ca_installed":...})
  (mitm, whitelist, [{"host_pattern":..., "rule_type":..., "enabled":..., "source":..., "created_at":...}, ...])

ca.rs / whitelist.rs / commands/mitm.rs DB 层：
  读：get_setting(db, "mitm", "<key>") → serde_json::from_value → struct
  写：set_setting(db, {scope:"mitm", key:"<key>", value: serde_json::to_value(&struct)?})
  read-modify-write（set_ca_installed / set_enabled / 白名单 add/remove/toggle/import/clear）：
    load → 改 → set_setting（write_conn 串行，无竞态）

schema migration（接现有最大 migration 号）：
  旧库：SELECT mitm_ca → setting JSON；SELECT mitm_whitelist → 数组 JSON；DROP 两表
  新库：无旧表，直接 seed 37 DEFAULT_RULES + 平台 host 到 setting (mitm, whitelist)
  幂等：INSERT OR IGNORE setting + DROP TABLE IF EXISTS

缓存（自动）：get_setting 命中 db.1.settings；set_setting 调 invalidate_settings_cache。
  → 白名单 add/remove/toggle 后缓存清，下次 matches_db 重读，行为正确。
```

## Decision (ADR-lite)

**Context**：用户要移除两表，数据进 setting。
**Decision**：
1. 2 key 存储模型（ca 对象 + whitelist 数组），非 per-field / per-entry——与现有 `middleware:settings` 单 key 装对象模式一致，最少 setting 行。
2. 迁移旧数据 + DROP 旧表（非保留空表 / 非丢数据）——符合「移除表」指令 + 不破坏已启用 MITM 用户的 CA / 已配白名单。
3. seed_default_whitelist_if_empty 并入 migration（单源）——避免 seed 函数与「无旧表」新库脱节。
4. 白名单写操作 read-modify-write 整数组——保 ordered 语义 + 原子性（write_conn 串行 + set_setting 单事务）。
5. 公共签名全保留（RootCa / WhitelistEntry / 全 command）——前端零改，跨层契约稳定。
**Consequences**：
- 白名单热路径（每 CONNECT matches_db）从 raw SQL 换 get_setting 缓存命中——性能提升（缓存命中零 DB 往返）。
- CA 私钥仍明文存 DB（与旧表同安全模型，0600 文件权限不变）。
- 导入导出：MITM 数据自动随 setting 数组进出（import_export 现有 SCOPE_SETTING 逻辑覆盖 scope=mitm，无需专门改）。
- setting 行数 +2（固定，不随白名单条目增长）。

## Out of Scope

- 改 MITM CA 私钥加密（仍明文，仅存储位置换）
- 改白名单匹配引擎（matches_rule 4 类型不变）
- 改前端 UI / api/mitm.ts 契约
- per-entry 白名单 key（采 2 key 数组模型）
- import_export 专门迁 MITM（随 setting 自动覆盖）

## Technical Notes

- setting scope 命名：`mitm`（与 `app`/`global`/`middleware` 同级，全小写单词）。
- serde rename：grep RootCa / WhitelistEntry struct 字段已 snake_case，`#[serde(rename_all="snake_case")]` 无痛兼容（或默认不 rename，字段名本就匹配）。
- enabled 字段跨层：DB 旧 INTEGER 0/1 → JSON bool。RootCa.enabled / WhitelistEntry.enabled 本就是 Rust bool，serde JSON 自动 bool。读旧表迁移时 `enabled != 0` 转 bool。
- 缓存失效：set_setting 内调 `db.invalidate_settings_cache()` 全清 settings cache。MITM 低频写，可接受。
- 导入导出兼容：旧 export 文件含 mitm_ca/mitm_whitelist 专属段（如有）—— grep `import_export` 确认 MITM 是否有专属段，若有需兼容旧 import（推测 MITM 未进 import_export，因 grep 无命中，待实施确认）。
- 既有 guide：`.trellis/spec/backend/db-conventions.md`（setting 操作规范）+ `.trellis/spec/guides/cross-layer-rules.md`（公共契约层稳定）+ `.trellis/spec/guides/code-reuse-rules.md`（复用 get_setting/set_setting）。
