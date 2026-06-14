# Implement Plan: 导入导出子系统

## 子任务拆分（parent + 4 children）

### A. 加密容器 `import_export/container.rs`（基础，串行先行）
- [ ] 加 `Cargo.toml` 依赖：aes-gcm / sha2 / hmac / rand / base64
- [ ] `gateway/import_export/mod.rs`：模块入口
- [ ] `container.rs`：
  - `const MAGIC = *b"ADGX"; const SALT = [...]; const VERSION = 1;`
  - `fn pad() -> [u8;32]` = SHA256(MAGIC||VERSION||SALT)
  - `pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String>`
  - `pub fn decrypt(file: &[u8]) -> Result<Vec<u8>, String>`（HMAC 校验 → GCM 解密）
  - 单测：encrypt → decrypt 往返；篡改 hmac 失败；篡改 ciphertext 失败

### B. 导出后端（A 完成后，可与 C 并行）
- [ ] `collect.rs`：
  - `pub async fn collect(scopes: &[Scope], db: &Db) -> Result<Payload, String>`
  - 各 scope 收集器：platform / group / group_platform / setting / codex_global + codex_profiles（读 `~/.codex/*`）/ claude_code_group_settings（读 `~/.aidog/settings.*.json`）/ model_price / skills（锁文件 + npx list）
- [ ] `lib.rs` commands：
  - `export_to_file(scopes, path)` → collect → serialize → encrypt → fs::write
- [ ] manifest.checksum 计算（SHA256 明文，不含 checksum 字段）

### C. 导入后端（A 完成后，可与 B 并行）
- [ ] `apply.rs`：
  - `pub async fn preview(file_bytes, db) -> Result<ImportPreview, String>`：解密 → 解析 → 扫描冲突
  - `pub async fn apply(payload, decisions, db) -> Result<ImportReport, String>`：事务内逐 scope 应用
  - 顺序：codex_global/profiles → claude_code_group_settings → group → platform → group_platform → setting → model_price → skills（外键依赖序）
  - 文件类先 `.bak` 备份
- [ ] `lib.rs` commands：`import_read_file(path)` / `import_apply(path, decisions)`

### D. Skills 自动化（C 的子流程，随 C）
- [ ] `skills_sync.rs`：
  - `pub fn export_skills() -> Vec<SkillExportEntry>`（复用 `skills::list_installed` + 锁文件 source + per-agent enable）
  - `pub fn import_skills(entries: &[SkillExportEntry]) -> Result<ImportReport, String>`
  - 对每条：`npx skills add <source> -s <name> -a <slug> [-g] -y`，再按 enabled 状态调 enable/disable
  - scope 严格匹配原（user→-g）
  - 单条失败不阻塞（收集到 report.errors）

### E. 前端 UI（B + C + D 完成后）
- [ ] `src/pages/ImportExport.tsx`（新页面）或 Settings 新 tab
  - 导出区：7 个 scope checkbox + 导出按钮 → save dialog → `invoke('export_to_file')` → toast
  - 导入区：选文件 → `invoke('import_read_file')` → 显示 manifest + 冲突清单 → 逐项决策弹窗 → `invoke('import_apply')` → report toast
- [ ] `services/api.ts`：类型 + invoke 封装
- [ ] `App.tsx` 侧栏加入口（或 Settings tab 注册）
- [ ] i18n 7 语言（zh/en/ar/fr/de/ru/ja）+ check-i18n.mjs 过

## 执行顺序

```
Step 1 (串行): A
Step 2 (并行组): B + C(含 D)
Step 3 (串行): E
Step 4: cargo test + cargo clippy + yarn build + check-i18n
Step 5: trellis-check
Step 6: trellisx-finish.py
```

## 关键约束（继承 CLAUDE.md / memory）
- URL 不拼、proxy settings 走 `setting` 表（[[proxy-log-settings]]）
- skills 全 npx，锁文件只读（[[skills-management-module]]）
- codex 走 codex.rs CodexSettings（[[codex-config-subsystem]]）
- group stats 不导出（运行期聚合）
- Rust warning 必清（[[warnings-are-issues]]）
- i18n 两类 key 全覆盖（[[frontend-i18n-coverage]]）

