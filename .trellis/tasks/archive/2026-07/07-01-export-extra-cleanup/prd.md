# PRD — 导出 extra 默认值清理 + 类型修正

> 用户请求 (/trellisx-flow): 分享 (导出 .aidogx) 时, platform 行的 `extra: "{}"` 应默认移除 (默认值), 且 extra 类型预期 obj 非 str。
> brainstorm 第二批扩 scope: 全字段默认值审计 + 分享场景运行时数据清。

## 目标
导出 payload 的 platform 项清洗 (分享=给别人, 不泄露个人使用数据 / 不带默认值噪音):

### 三层清洗 (用户定: L3 + status 不导出)
1. **extra** (核心): 空 (`{}`/`""`) 移除; 非空时序列化为 JSON object 非 string
2. **配置空值省略**: models (全 None) / available_models (空 []) / endpoints (空 []) 空时导出行省略字段
3. **运行时不导出** (分享场景, 个人数据): `auto_disabled_until` / `auto_disable_strikes` / `expires_at` / `deleted_at` / `est_balance_remaining` / `est_coding_plan` / `last_real_query_at` 字段一律不进导出 payload
4. **status / enabled 不导出** (用户决定: 分享不带原用户启用/禁用意图, 导入回 default)

## 现状 (main + research 审计)
- `Platform` struct (`src-tauri/src/gateway/models/platform.rs:157`): extra: String (TEXT), 多数字段已有 `#[serde(default)]`
- 导出: `collect.rs:37` → `db::list_platforms` → serde 全字段序列化 → 含运行时 + extra `"{}"`
- 导入: `apply/db_rows.rs:161` `json_str(row,"extra")` 按 string 取; `effective_extra_with_breaker` 兼容旧 breaker
- 导入缺失字段回退: Platform 多字段 `#[serde(default)]` 保证缺失回默认; `enabled` 无 default 标注 → 需加

## scope
1. **导出 collect.rs / Platform 导出序列化**:
   - extra: 空值 skip + 非空 string→obj parse 后序列化 (custom serialize 或 collect 阶段 transform)
   - 运行时 7 字段 + status + enabled: `skip_serializing` 不进 payload
   - 配置空值 (models/available_models/endpoints): `skip_serializing_if = "is_empty"` 类
2. **导入 apply/db_rows.rs**:
   - extra: 兼容新格式 (obj) + 旧格式 (string) 双向 (parse: 若 Value::Object 直接用, 若 String 原样)
   - 缺失字段 (运行时/status/enabled) 回退 `#[serde(default)]` (PlatformStatus::Enabled / false)
3. **前端 ImportExport.tsx**: preview 显示 extra obj (非裸 `"{}"`); 缺失字段不显示

## design 决策 (见 design.md)
- extra obj 化: collect 阶段 transform (非改 struct serde), 避免 Platform model 字段类型 breaking
- skip 策略: `#[serde(skip_serializing)]` 固定字段 vs collect 阶段白名单 (见 design)
- 导入兼容矩阵: 旧 .aidogx (全字段) + 新 (清洗后) 双向可导入

## 验收
1. 导出 .aidogx: 空 extra 平台无 extra 字段; 非空 extra 为 obj; 无运行时 7 字段; 无 status/enabled
2. 导入: 旧格式 (全字段 string extra) + 新格式 (清洗 obj extra) 均可导入, 数据无损, 缺失字段回默认
3. 前端 preview: extra obj 展开, 无运行时字段行
4. `cargo test` (test_collect/test_db_rows/test_conflicts 更新) + `cargo clippy` + `yarn build` 全绿
5. breaker 迁入逻辑 (`effective_extra_with_breaker`) 适配 obj 格式

## 非目标
- 不改 DB schema (extra TEXT)
- 不改 Platform Rust 字段类型 (仍 String, collect 阶段 transform)
- 不改 mock/newapi/breaker 配置逻辑 (仅序列化层)

## 风险
- 导入兼容: 旧 .aidogx 全字段仍可导入 (Platform `#[serde(default)]` 已覆盖大多, enabled 需补)
- breaker: `effective_extra_with_breaker` 假设 extra string, obj 格式需适配
- 跨层: collect 序列化 ↔ apply 反序列化 ↔ 前端 preview 三侧对齐
- est_* / auto_* 导入缺失: 导入方拿到 default (0 / 空字符串), 平台像"全新" — 符合分享语义

## 调度
- subagent 编排, 跨层 3 文件 (collect.rs / apply/db_rows.rs / ImportExport.tsx) + Platform struct 微调
- 撞 arch-redesign planning (同文件), arch 未动 src, 本 task 先行
