# platform 表 schema 清理

## 目标
精简 `platform` 表：删除 `auto_group` 持久列，把 3 个 breaker 配置列收进 `extra` JSON。降低表宽、把"非核心/可选配置"归并进 extra。

## 用户决策（已澄清）
1. **auto_group**：自动建默认分组是**创建时一次性**判断，不需持久化；编辑平台时不再判断是否自动建组。
   - 删 `platform.auto_group` 列 + Platform 存储字段。
   - 保留 `create` 的 transient 输入 `auto_group`（默认 true）：创建时据它决定是否 `create_auto_group_for`。
   - 删 `update_platform` 里基于 auto_group 的 reconcile 逻辑（lib.rs:220-229）。
2. **breaker_failure_threshold / breaker_open_secs / breaker_half_open_max** → 移进 `extra` JSON 的嵌套对象 `extra.breaker = {failure_threshold, open_secs, half_open_max}`。
   - 保留现有语义：**0 / 缺省 = 继承全局默认**（`effective_thresholds` 逻辑不变，只改取值来源）。

## 影响面（已调研 blast radius）
**后端**
- `gateway/models.rs`: Platform struct 删 auto_group 字段；breaker 3 字段从 struct 列字段改为经 extra 解析（保留 typed 访问，如 `Platform::breaker()` 或解析 helper）。
- `gateway/db.rs`: COLS / COLS_PREFIXED 列清单删 4 列；row 映射（590/1795）；INSERT/UPDATE（623/787）；**Migration（新版本号）**: 把现有行 breaker 3 列值 backfill 进 extra.breaker，再 DROP 4 列（auto_group + 3 breaker）。SQLite DROP COLUMN（3.35+）或表重建，exec agent 定。
- `gateway/router.rs`: `effective_thresholds` 改从 extra.breaker 取（语义不变）。
- `gateway/import_export/apply.rs`: breaker 字段导入路径同步（现读列 → 读 extra）。
- `lib.rs`: create 保留 transient auto_group 建组；删 update 的 auto_group reconcile。

**前端**
- `services/api.ts`: Platform TS 类型删 auto_group（如有）；breaker 字段类型从顶层移进 extra（或经 extra 访问）。
- `pages/Platforms.tsx`: 平台编辑页 breaker 3 字段编辑器读写改走 extra；auto_group 开关——创建表单保留（transient 输入），编辑表单移除（编辑不再判断）。
- `components/settings/SchedulingSettings.tsx`: breaker 全局默认设置不变（那是 settings KV scope=scheduling，非 platform 列），核对不受影响。
- `utils/ccswitchMatch.ts`: 引用 breaker 字段处同步。

## 验收标准
- `platform` 表不再有 auto_group / breaker_* 4 列。
- 现有平台的 breaker 配置经 migration 无损迁入 extra.breaker（旧值保留）。
- 创建平台时 auto_group=true 仍自动建默认分组；=false 不建；编辑平台不再触发建组判断。
- 熔断 `effective_thresholds`（platform 覆盖全局默认）语义不变，单测通过。
- 前端平台编辑 breaker 三字段读写正常（经 extra）；创建表单 auto_group 开关正常。
- 门禁全绿：`cargo build && cargo clippy && cargo test`（含 router/db breaker 相关单测）+ `yarn build` + `check-i18n`。
- 导入导出（.aidogx）含 breaker 配置的平台往返一致。

## 失败处理
- Migration backfill 或 DROP COLUMN 在旧 SQLite 不支持 → 改表重建（CREATE new + 拷贝 + DROP + RENAME），保数据。
- breaker 经 extra 后 effective_thresholds 取值边界（缺 extra.breaker / 缺单键）→ 当 0（继承全局），单测覆盖。
- 导入旧格式（breaker 在顶层）兼容：apply 读取时顶层与 extra 双兜底。

## 执行载体
单一 exec subagent 串行执行（全程触 db.rs/models.rs 共享文件，不可并行拆分）。worktree 隔离。check subagent 验证。
