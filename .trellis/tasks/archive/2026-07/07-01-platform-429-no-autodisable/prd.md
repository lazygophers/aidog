# PRD — 429 不触发平台自动禁用

> 来源: 用户 /trellisx-flow「429 不应该自动禁用」。当前 spec `backend/platform-error-handling.md` C1/C2 + `non_success.rs:67` 把 429-配额耗尽当 auto_disable 触发码（同 402）。

## 目标
429 不再触发 `set_platform_auto_disabled`（无论配额耗尽还是限流）。仅改 auto_disable 触发，**保留** classify_429 熔断分类（C3）+ failover 重试语义。

## 改动（最小改）

### 代码
- `src-tauri/src/gateway/proxy/non_success.rs:67`：auto_disable 条件删 `|| is_429_quota_exhausted`
  - 改前: `if code == 401 || code == 403 || code == 402 || is_429_quota_exhausted`
  - 改后: `if code == 401 || code == 403 || code == 402`
- 同步更新 :64-66 注释（429-配额不再 auto_disable，仅 401/403/402 单次禁用）
- **保留** classify_429 + is_429_quota_exhausted 用于 :58 熔断分类（429-配额不计熔断 record_ignored，429-限流计 record_failure）— C3 表不变
- **保留** :58 `code == 429 && !is_429_quota_exhausted` 熔断判定逻辑不变
- **保留** :92 429 retryable（failover 换候选）不变

### spec
- `.trellis/spec/backend/platform-error-handling.md` C1：触发码列表移除「429 配额耗尽」条；验证 grep 同步改（删 `|| is_429_quota_exhausted`）
- C3 表：429-配额行 auto_disable 列 「是」→「否」（熔断仍 record_ignored）
- C2 classify_429 **保留**（仍用于 C3 熔断分类，非 auto_disable）
- C4 purge / C5 last_error **不动**（429-配额不 disable 后不入 auto_disabled 状态，purge 谓词无影响；last_error 仍记录）

### 测试
- `src-tauri/src/gateway/proxy/test_retry.rs`：若有断言「429-配额 → auto_disabled」的集成测试，改为断言「不 disable + record_ignored + failover」；classify_429 单测（classify_429_quota_exhausted / classify_429_rate_limit）保留（熔断仍用）
- grep `is_429_quota_exhausted` / `auto_disabled` 全测试文件确认无残留断言 429 disable

## 验收
1. `cd src-tauri && cargo test` 绿（含 test_retry.rs / test_platform_lifecycle.rs）
2. `cd src-tauri && cargo clippy` 零 warning（is_429_quota_exhausted 仍有引用，非死代码）
3. `grep -n 'code == 401 || code == 403 || code == 402' src-tauri/src/gateway/proxy/non_success.rs` 命中（无 `|| is_429_quota_exhausted`）
4. spec C1 grep 验证同步更新
5. 429-配额场景：平台不 auto_disabled，仍 failover 换下个候选

## 非目标
- 不改 classify_429 marker 列表（C2 保留）
- 不改熔断逻辑（C3 record_failure/record_ignored 分类不变）
- 不改 purge（C4）/ last_error（C5）
- 不加配置开关（用户选「仅移除 auto_disable」非「可配置」）

## 风险
- 429-配额平台反复试探拖慢请求（spec 原设计意图）— 用户已确认接受此权衡（要 429 不禁用）
- 并行 task test-isolation-fix 改测试文件，本 task 改 non_success.rs/retry.rs/test_retry.rs/spec — 文件集不相交，finish merge 不冲突
