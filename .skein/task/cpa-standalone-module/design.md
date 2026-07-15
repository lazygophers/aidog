# CPA 独立模块重构 — 详细设计

## 命名 (已锁)
- 模块/菜单: `cli-proxy` (CLIProxyAPI 本意, 与旧 cpa 切割)
- 新 protocol serde: `cli-proxy` → Rust `Protocol::CliProxy`
- 新表: `cli_proxy_provider`
- 新 crate: `commands_cli_proxy`
- 新前端页: `CliProxy.tsx`, 菜单 id `cli-proxy`

## 架构

### 数据模型

**新表 `cli_proxy_provider`** (schema migration):
```sql
CREATE TABLE cli_proxy_provider (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  wire_protocol TEXT NOT NULL,   -- openai_responses / gemini / anthropic / openai (实际 wire, 对应 ENDPOINT_PROTOCOLS)
  base_url TEXT NOT NULL,
  api_key TEXT NOT NULL DEFAULT '',
  models TEXT NOT NULL DEFAULT '[]',  -- JSON array, 继承给引用平台
  extra TEXT NOT NULL DEFAULT '{}',   -- JSON: prefix/headers/oauth_type/auth_dir 等
  status TEXT NOT NULL DEFAULT 'active',  -- active / disabled
  group_id INTEGER,               -- 可选归属分组
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
```

**Protocol::CliProxy** (protocol.rs 新增变体):
- serde rename `cli-proxy`
- platform 表存一行 `platform_type="cli-proxy"`, extra 存 `{"cli_proxy_provider_id": <id>}`
- 这类平台: platform.models 字段忽略(UI 只读), platform.base_url/api_key 字段忽略, 真实配置从 provider 拉
- platform.status / group / endpoints(空, 路由时注入) 仍走 platform 表

### 路由 (candidate resolve)

**不变**: select_candidates 仍查 platform 表, selection/ordering 不改。

**新增**: candidate resolve 阶段(`candidates.rs::resolve_effective_models` 同层或 candidate 构建处), 若 `platform.platform_type == CliProxy`:
1. 从 extra 读 `cli_proxy_provider_id`
2. join `cli_proxy_provider` 表拉 wire_protocol / base_url / api_key / models
3. 注入 candidate 的 endpoint: `{ protocol: provider.wire_protocol, base_url: provider.base_url, api_key: provider.api_key }`
4. effective_models = provider.models(覆盖 platform.models)
5. provider.status == disabled 或 provider 不存在 → candidate 排除(同 platform.status=disabled 语义)

**转发**: candidate endpoint 已注入真实 wire/base_url/key, forward/converter 走通用路径(wire = provider.wire_protocol), 零特判。

### 统计

- proxy_log 仍按 platform_id 聚合(cli_proxy 平台行)
- provider 表不参与统计聚合
- "干净统计": 删旧 cpa-* 平台后, proxy_log 孤儿 platform_id 兜底显示"已删除平台"(Stats.tsx:316 platform_id=0 模式扩展, 或 query_stats 对不存在 platform_id 显示空名)
- group 维度统计: cli_proxy 平台入 group 后, proxy_log.group_name 记录, 聚合照常

### 前端

**新页 `CliProxy.tsx`** (src/pages/CliProxy.tsx):
- provider 列表(表格: name / wire / base_url / status / 关联平台数)
- 新增/编辑 provider 表单(name/wire/base_url/key/models/extra)
- 测试按钮(调 test command, 临时用 provider 配置发探测请求)
- 导入按钮(复用旧 parser 逻辑: 选 CLIProxyAPI config 文件/auth-dir → 解析 → 批量建 provider)
- 删除按钮(检查关联平台, 提示解绑或级联)

**菜单**: App.tsx BASE_NAV 加 `{ id: "cli-proxy", labelKey: "nav.cliProxy", section: "nav.section.proxy" }`, 渲染分支加 CliProxy 页。

**PlatformEditForm 入口**: 新建态加"从 cli-proxy 添加"按钮 → 弹 provider 选择器 → 选后建 cli_proxy 平台行(platform_type=cli-proxy, extra 存 provider_id, models 只读显示 provider.models)。

**模型区只读**: formSections 渲染 platform.protocol == cli-proxy 时, models 区显示"继承自 provider X"(只读列表), 禁用编辑。

### parser 复用

旧 `cpa_import/parser.rs`(解析 CLIProxyAPI config.yaml/auth-dir/zip/dir)逻辑迁新 crate `commands_cli_proxy/src/import.rs`, 产出 `CliProxyProvider`(非旧 MappedPlatform)。mapper 逻辑(resolve_protocol 段→wire)改为段→wire_protocol 映射。

## 数据流

```
[provider 配置] 
  cli-proxy 页 CRUD → commands_cli_proxy → cli_proxy_provider 表

[导入 CLIProxyAPI 配置]
  cli-proxy 页导入 → commands_cli_proxy::parse_config (迁旧 parser) → 批量建 provider

[AI 平台引用 provider]
  PlatformEditForm "从 cli-proxy 添加" → 选 provider → commands_cli_proxy::create_cli_proxy_platform
  → platform 表插一行 platform_type="cli-proxy", extra={cli_proxy_provider_id}

[代理转发]
  client 请求 → resolve_group → select_candidates (platform 表, 含 cli-proxy 行)
  → candidate resolve: cli-proxy 平台 → join provider 表拉 wire/base_url/key/models 注入 endpoint
  → forward (走 provider.wire_protocol 通用路径) → 上游
  → proxy_log (platform_id = cli-proxy 平台 id, target_protocol = provider.wire)
```

## 关键取舍

### A. provider 不独立成路由候选源
- 候选源仍 platform 表。provider 是配置库, 被 platform(cli-proxy 类型)引用。
- 避免 router 感知"第二候选源"(research D6 最大架构改动点), 路由层改动控制在 candidate resolve 注入。
- 一个 provider 可被多 platform 引用(不同 group / 不同 status)。

### B. 模型从 provider 继承, 平台侧不可选
- cli-proxy 平台 models 字段忽略, effective_models = provider.models。
- 用户改模型去 cli-proxy 页改 provider, 所有引用平台自动生效。
- 符合"模型不可选择, 路由走新模块"需求。

### C. 旧 parser 迁移复用
- parser.rs 解析 CLIProxyAPI 配置是核心资产, 迁新模块作"导入"入口。
- mapper.rs 段→Protocol 改为段→wire_protocol(去 cpa-* 协议, 直接到 wire)。

### D. 旧 cpa-* 数据删除(不迁移)
- 用户原话"现有的 cpa-xxx 的平台全部移除"。
- migration: `DELETE FROM platform WHERE platform_type LIKE '"cpa-%'`(platform_type 存 JSON 带引号)。
- proxy_log 历史保留(platform_id 孤儿, 统计兜底)。

### E. proxy_log + stats_agg 历史同步清(用户授权)
- migration 顺序:
  1. 查旧 cpa 平台 id 集: `SELECT id FROM platform WHERE platform_type LIKE '"cpa-%'`(platform_type 存 JSON 带引号)
  2. `DELETE FROM proxy_log WHERE platform_id IN (<旧 id 集>)`
  3. `DELETE FROM stats_agg_hourly WHERE platform_id IN (<旧 id 集>)` (若有 platform_id 列, subtask s4 确认 schema)
  4. `DELETE FROM platform WHERE platform_type LIKE '"cpa-%'`
  5. 删 Protocol enum 4 cpa-* 变体(此时 DB 已无 cpa-* 行, 无 panic 风险)
- 破坏性不可逆, 用户明确授权("同步清历史")。成本/用量历史全丢, 换代码+数据层干净。

## 改动文件

### 新增
- `src-tauri/crates/aidog_core/src/gateway/db/cli_proxy.rs` — provider db 模块
- `src-tauri/crates/aidog_core/src/gateway/models/cli_proxy.rs` — CliProxyProvider struct + serde
- `src-tauri/crates/commands_cli_proxy/` — 新 crate(Cargo.toml + lib.rs + commands + import.rs 迁旧 parser)
- `src/pages/CliProxy.tsx` — 前端新页
- `src/services/api/cliProxy.ts` — api 封装
- `src/services/api/types/cliProxy.ts` — 类型定义

### 修改
- `src-tauri/crates/aidog_core/src/gateway/models/protocol.rs` — 加 CliProxy 变体 + 删 4 cpa-* 变体
- `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs` — candidate resolve 加 cli-proxy 分支
- `src-tauri/crates/aidog_core/src/gateway/db/schema*.rs` — 加 cli_proxy_provider 表 migration + 删旧 cpa 平台 migration
- `src-tauri/crates/aidog_core/src/gateway/adapter/converter/request.rs` — 删 4 cpa-* arm
- `src-tauri/src/lib.rs` + startup.rs — 注册新 crate commands + 删旧 3 cpa command
- `src-tauri/defaults/platform-presets.json` — 删 4 cpa-* 条目 + 加 cli-proxy preset(可选, 内部类型)
- `src/App.tsx` — BASE_NAV 加 cli-proxy 菜单 + 渲染分支
- `src/pages/platforms/PlatformEditForm.tsx` — 加"从 cli-proxy 添加"入口
- `src/pages/platforms/formSections.tsx` — cli-proxy 平台模型区只读渲染
- `src/locales/*.json`(8) — 新 nav.cliProxy + cliProxy.* 键, 删 platform.cpaImport.*

### 删除
- `src-tauri/crates/aidog_core/src/gateway/cpa_import/` — 整目录(parser.rs/mapper.rs/mod.rs)
- `src-tauri/crates/commands_platform/src/cpa_import.rs` — 整文件
- `src/components/platforms/CpaImportModal.tsx` — 整文件(624 行)
- `src/pages/platforms/platformPasteApply.ts` 内 applyCpaToForm / runBatchCreateFromCpa 段
- `src/services/api/platforms.ts` 内 cpaImportApi 段
- `src/services/api/types/part4.ts` 内 MappedPlatform/CpaSkipReason/CpaImportParseResult/CpaBatchFailure/CpaBatchReport(整文件或相关段)

## subtask 拆分 (调度落 task.json)

| sid | 名称 | 范围 | deps |
|---|---|---|---|
| s1 | provider 表 + db 模块 + migration | aidog_core: schema + db/cli_proxy.rs + models/cli_proxy.rs | - |
| s2 | Protocol::CliProxy + 路由 resolve | protocol.rs 加变体 + candidates.rs cli-proxy 分支 | s1 |
| s3 | commands_cli_proxy crate | 新 crate: CRUD + test + create_cli_proxy_platform + import(迁旧 parser) | s1, s2 |
| s4 | 删旧 cpa-* 后端 + 数据 migration | 删 cpa_import 模块 + 4 变体 + converter arm + presets + 删旧平台 migration | s2 |
| s5 | 前端 CliProxy 页 + 菜单 + i18n | CliProxy.tsx + api + 类型 + App.tsx 菜单 + i18n | s3 |
| s6 | 前端 PlatformEditForm cli-proxy 入口 + 模型只读 | "从 cli-proxy 添加"按钮 + formSections 只读 | s3 |
| s7 | 删旧 cpa-* 前端 | 删 CpaImportModal + apply 链 + api + 类型 + union + i18n | s5, s6 |

DAG: s1 → s2 → s3; s4 依赖 s2(并行 s3); s5/s6 依赖 s3(并行); s7 依赖 s5+s6。

## 待 grill 澄清
1. ~~命名~~ → 已锁 cli-proxy
2. ~~proxy_log 历史~~ → 已锁同步清(用户授权)
3. s4 删旧平台 migration 时机: 升级时自动跑(app 启动 migration 幂等模式, 默认)
4. provider 关联平台删除策略: 禁删提示解绑(安全默认, 级联删平台风险大)
5. cli-proxy 平台单 provider 引用(YAGNI, 多 provider 聚合负载均衡暂不做)
