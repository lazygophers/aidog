# Implement — sub2api 导入 + 导入 auto-group 执行编排

> 配套 `prd.md`。本文件给 exec agent 的施工蓝图：改动面 / 依赖顺序 / 共享文件互斥 / 单测。
> 全程复用 cc-switch 异源导入框架，**不另造写入路径**。
> **本任务两部分**：(A) sub2api 导入全链；(B) sub2api + cc-switch 两路导入都加 auto-group toggle + 共享 ensure-group-and-attach（prd §0 / §5）。

## 0. 五要素

- **目标**：(A) 导入导出新增「从 sub2api 导入平台配置」核心映射 only；(B) 给 sub2api + cc-switch 导入加 auto-group（toggle 默认开，sub2api→`sub2api` 组 / cc-switch→`cc-switch` 组）。
- **产出**：后端 `sub2api.rs` 解析器 + 命令 + apply 接入 + **共享 `ensure_group_and_attach`** + `ccswitch_import` 接 auto_group；前端 `Sub2ApiImport`（双入口+预览下拉+toggle）+ `CcSwitchImport` 加 toggle + api 封装 + ImportExport 入口 + platform 映射；单测；i18n key。
- **验证**：见 prd §9（cargo clippy/test、yarn build/check:i18n、手动导入冒烟 + auto-group 生效/关闭）。
- **资源**：见 §1 文件清单（参照源 + 新增 + 改动）。
- **依赖**：后端解析 DTO 先定 → 前端消费（DTO 字段名是 Rust↔TS 契约，记忆 `aidog-add-platform-skill` 双写）；`ensure_group_and_attach` 后端共享 → sub2api/cc-switch 两路命令都接（串行改 lib.rs）。

## 1. 改动面

### ① 后端（先行，DTO 是契约源头）

| 文件 | 操作 | 说明 |
| --- | --- | --- |
| `src-tauri/src/gateway/import_export/sub2api.rs` | **新建** | 参照 `ccswitch.rs:1-468`。含 `Sub2ApiPayload` / `Sub2ApiAccount` DTO（`#[serde(rename_all="camelCase")]` 对前端，`#[serde(rename=...)]` 对 sub2api 源字段）+ `parse(json_text) -> Result<Sub2ApiReadResult>` + `import(platform_payload, decisions, auto_group, db)` 复用 `super::apply::apply`（照抄 `ccswitch.rs:443-468` 的 `import`，仅改 `source_machine: "sub2api-import"` + 多 `auto_group: bool` 入参，见下 §1.② auto-group 接法） |
| `src-tauri/src/gateway/import_export/apply.rs` | 改 | **新增共享 `ensure_group_and_attach(db, group_name: &str, platform_names: &[String])`**（sub2api/cc-switch 两路共用）。复用既有 `upsert_group_row`（`apply.rs:410`）+ `relink_group_platform`（`apply.rs:583`，名→id 解析 + `group_platform` ON CONFLICT 幂等 INSERT，`apply.rs:607-610`）。或更简：**不新增 fn，改在各 import 路径把 `group` 行 + `group_platform` name 对注入 `Payload`**（`apply_db` 已遍历 `payload.group` `apply.rs:309` + `payload.group_platform` `apply.rs:359` 处理），由 apply 现成流程建组+关联。⚠️ 见 §6「需要:」重复导入 name 解析歧义 |
| `src-tauri/src/gateway/import_export/ccswitch.rs` | 改 | `import` 签名加 `auto_group: bool`（`ccswitch.rs:443`）；auto_group 为 true 时按 §1.② 注入 `cc-switch` 组 + group_platform 对（或调 `ensure_group_and_attach("cc-switch", platform_names)`）。`source_machine` 维持 `"cc-switch-import"` |
| `src-tauri/src/gateway/import_export/mod.rs` | 改 | `pub mod sub2api;`（L13-17 区）+ `pub use sub2api::{Sub2ApiReadResult, Sub2ApiAccount};`（L19 区，与 ccswitch re-export 并列） |
| `src-tauri/src/lib.rs` | 改 | (1) 新增 2 命令：`sub2api_parse(json_text: String) -> Sub2ApiReadResult`（无需 db State；参照 `ccswitch_read` L2193-2199 但入参是文本不是路径）+ `sub2api_import(platform_payload, decisions, auto_group, db)`（照抄 `ccswitch_import` L2204-2216 + 加 `auto_group: bool` 入参）。(2) **改 `ccswitch_import`（L2204-2216）签名加 `auto_group: bool` 入参 + 透传** `ccswitch::import(..., auto_group, &db)`。(3) **sub2api 2 命令加进 `invoke_handler!` 注册列表**（L4156-4157 区，紧跟 `ccswitch_*`） |

### ② auto-group 接法（部分 B 核心，sub2api/cc-switch 共用）

`apply::apply` 不触发 `platform_create` 命令级 auto-group 副作用（记忆 `import-apply-bypasses-platform-create`），故必须**显式建组+关联**。已查实 apply 现有可复用原语：

- `upsert_group_row(db, group_key, effective_name, row)`（`apply.rs:410`）：按 group_key 查重 upsert；group_key 留空时 row 走 db create 逻辑生成 `gk_<32hex>`（记忆 `group-name-group-key-split`）。
- `relink_group_platform(db, g_name, p_name)`（`apply.rs:583`）：按 **name** 解析 group_id + platform_id（注意：函数形参名为 `group_key` 实按 `WHERE name=?` 查，`apply.rs:591`），`INSERT ... ON CONFLICT(group_id,platform_id) DO UPDATE`（`apply.rs:607-610`，**关联天然幂等**，不重复建关联）。
- `apply_db` 已按序处理 `payload.group`（`apply.rs:309`）→ `payload.platform`（`apply.rs:337`）→ `payload.group_platform`（`apply.rs:359`）。

**推荐接法（最小改动）**：auto_group=true 时，在各 import 函数构造 `Payload` 时：
1. `payload.group = vec![ json!({"name": "sub2api"}) ]`（无 group_key → upsert 生成；幂等：同名重复导入只一个组，因 group 按 group_key 查重——注意首次无 group_key 会每次新建，**需 ensure 按 name 查已存在组**，见 §6「需要:」）。
2. `payload.group_platform = platform_names.iter().map(|n| ["sub2api".into(), n.clone()]).collect()`。

> ⚠️ **两处歧义须 exec 决断（已列 §6「需要:」）**：(a) group 幂等——`upsert_group_row` 按 group_key 查重，注入无 group_key 时每次导入会新建同名组；要按 name ensure 须先查 `SELECT id FROM "group" WHERE name=?` 命中则复用、未命中再 create（这正是建议新增 `ensure_group_and_attach` 而非纯 payload 注入的理由）。(b) platform 重复——`relink_group_platform` 按 name 取首行 platform_id（`apply.rs:596-602` query_row 取第一条），重复导入同名平台时关联解析到哪一行不确定。

后端 DTO 设计要点（**实现时先对照 `account_data.go` DataAccount struct json tag 确认精确字段名**）：

```rust
// 对 sub2api 源 JSON 解析（字段名 = sub2api json tag）。
#[derive(Deserialize)]
struct RawSub2ApiPayload {
    #[serde(rename = "type")] r#type: String,   // 必须 == "sub2api-data"
    #[serde(default)] accounts: Vec<RawAccount>,
    // proxies / version / exported_at 不解析（丢弃）
}
#[derive(Deserialize)]
struct RawAccount {
    name: String,
    platform: String,                            // anthropic/openai/gemini/...
    #[serde(default)] credentials: RawCredentials,
    // type/extra/proxy_key/concurrency/priority/rate_multiplier/expires_at/notes 全不解析
}
#[derive(Deserialize, Default)]
struct RawCredentials {
    #[serde(default)] api_key: Option<String>,
    #[serde(default)] base_url: Option<String>,
}

// 透传给前端的 DTO（camelCase）。
#[derive(Serialize)] #[serde(rename_all="camelCase")]
pub struct Sub2ApiAccount {
    pub name: String,
    pub platform: String,           // 原始值，前端做 Protocol 映射
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}
#[derive(Serialize)] #[serde(rename_all="camelCase")]
pub struct Sub2ApiReadResult { pub accounts: Vec<Sub2ApiAccount> }
```

`parse()`：`serde_json::from_str::<RawSub2ApiPayload>` → 校验 `r#type == "sub2api-data"`（否则 `Err("非 sub2api 导出文件（type != sub2api-data）")`）→ map RawAccount → Sub2ApiAccount（api_key/base_url 空串过滤为 None）。

### ③ 前端（依赖后端 DTO）

| 文件 | 操作 | 说明 |
| --- | --- | --- |
| `src/services/api.ts` | 改 | (1) 新增 `Sub2ApiAccount` / `Sub2ApiReadResult` interface（对齐后端 camelCase，参照 `CcswitchReadResult` L1792-1796）+ `sub2apiApi = { parse, import }`（参照 `ccswitchApi` L1798-1808，`parse: (jsonText) => invoke("sub2api_parse",{jsonText})`、`import: (payload,decisions,autoGroup)=>invoke("sub2api_import",{platformPayload,decisions,autoGroup})`）。(2) **改 `ccswitchApi.import`（L1805-1807）加 `autoGroup: boolean` 入参 + 透传** `invoke("ccswitch_import",{platformPayload,decisions,autoGroup})` |
| `src/utils/sub2apiMatch.ts` | **新建** | 比 `ccswitchMatch.ts` 简单：`mapPlatformToProtocol(platform): {protocol, recognized}`（lowercase+trim → anthropic/openai/gemini 直映射，未识别返回 `openai` + `recognized:false`）+ `sub2apiAccountToPlatformJson(account, protocolOverride?, baseUrlFallback)` 复用 `getDefaultEndpoints` 构造 endpoints（同 `ccswitchMatch.ts:82-104` `buildMatch`：base_url 缺失取 preset 默认）+ 产 Platform JSON（形态照抄 `ccProviderToPlatformJson` L231-260，models 恒 `{}`、extra 恒 `""`） |
| `src/components/settings/Sub2ApiImport.tsx` | **新建** | 参照 `CcSwitchImport.tsx`，但**去掉 detect/pickDir 本地探测**，改为：**双入口**（文件选择 Tauri dialog 读文件文本 + textarea 粘贴，两者都给——评审决策 ①）→ `sub2apiApi.parse` → 预览列表（每账号一行：name / **Protocol 下拉可改**——评审决策 ② / base_url / api_key 脱敏 + 未识别徽标）→ 勾选 + **「加入分组」toggle（默认 `autoGroup=true`，评审决策 ③）** → `sub2apiAccountToPlatformJson` → `sub2apiApi.import([], [], autoGroup)` → 展示 ImportReport。`onReport` 回调签名同 CcSwitch（`(r:ImportReport)=>void`） |
| `src/components/settings/CcSwitchImport.tsx` | 改 | **加「加入分组」toggle**（默认 `autoGroup=true`，新 useState）+ 导入调用处 `ccswitchApi.import(payload, ds)` → `ccswitchApi.import(payload, ds, autoGroup)`（`CcSwitchImport.tsx:181`）。toggle UI 放确认导入按钮附近，文案 i18n key `ccswitch.autoGroup`（开=「导入后自动建/加入「cc-switch」分组」） |
| `src/components/settings/ImportExport.tsx` | 改 | L30 import 旁加 `import { Sub2ApiImportSection } from "./Sub2ApiImport";`；L298 `<CcSwitchImportSection .../>` 下方挂 `<Sub2ApiImportSection onReport={(r)=>{ setReport(r); reloadFromDB().catch(()=>{}); }} />` |
| `src/locales/*.json`（7 语言）| 改 | 新增 sub2api 导入区块文案 key（标题/说明/选文件/粘贴/解析/未识别徽标/导入/结果/**加入分组 toggle**）+ cc-switch 新增 `autoGroup` toggle key。**7 语言全补**（记忆 `frontend-i18n-coverage`），跑 `yarn check:i18n` 验证 |

## 2. 依赖与执行顺序

```
后端 ①
  (a) apply.rs: ensure_group_and_attach（共享，先做——sub2api/ccswitch 两路都依赖）
  (b) sub2api.rs DTO + parse + import(auto_group) + mod.rs
  (c) ccswitch.rs import 加 auto_group + lib.rs 命令（sub2api_* 新增 + ccswitch_import 改签名 + 注册）
        │  DTO/命令字段名 = Rust↔TS 契约
        ▼
前端 ②
  api.ts interface/封装（sub2apiApi 新增 + ccswitchApi.import 加 autoGroup）
   → sub2apiMatch.ts → Sub2ApiImport.tsx（双入口+下拉+toggle）
   → CcSwitchImport.tsx（加 toggle） → ImportExport.tsx → locales
```

**先后端、后前端**：DTO camelCase 字段名 + 命令入参（含新 `autoGroup`）定下来前端才能写对 interface（错位 = invoke 返回字段 undefined / 参数名不匹配静默失败，记忆边界审计高频 bug）。**`ensure_group_and_attach` 先于两路 import 完成**（共享依赖）。

## 3. 共享文件冲突（串行化）

以下文件被多处改，**同一 agent 串行改，禁并行**：

- `src-tauri/src/gateway/import_export/apply.rs`：新增 `ensure_group_and_attach`（或确认 payload 注入路径）—— sub2api/ccswitch 两路共享，**最先改**。
- `src-tauri/src/lib.rs`：加 sub2api 2 命令 + 改 `ccswitch_import` 签名 + 注册列表（同一文件 ≥4 处编辑）。
- `src-tauri/src/gateway/import_export/ccswitch.rs`：`import` 签名加 auto_group + auto-group 接入（与 sub2api.rs import 同模式）。
- `src/services/api.ts`：加 sub2apiApi + 改 ccswitchApi.import（紧邻 ccswitch 段）。
- `src/components/settings/ImportExport.tsx`：import + 挂载 2 处。
- `src/locales/*.json`：7 个文件各加同结构 key（sub2api 区块 + ccswitch.autoGroup）。

新建文件（`sub2api.rs` / `sub2apiMatch.ts` / `Sub2ApiImport.tsx`）无冲突，可独立写。

## 4. 单测

后端 `sub2api.rs` `#[cfg(test)] mod tests`（参照 `ccswitch.rs:472-533`）：

1. `parse_valid` — 合法 sub2api-data JSON（含 anthropic/openai/gemini 各一账号）→ accounts 数量 + 字段正确。
2. `parse_rejects_wrong_type` — `type:"other"` → Err。
3. `parse_rejects_malformed` — 非法 JSON → Err。
4. `parse_missing_base_url` — credentials 无 base_url → `base_url: None`（回退在前端，后端只透传 None）。
5. `parse_missing_api_key` — 无 api_key → `api_key: None`。
6. `parse_drops_extra_fields` — 含 proxy_key/concurrency/extra/proxies → 解析成功且这些字段不出现在 Sub2ApiAccount。

后端 auto-group `apply.rs`（或 sub2api.rs/ccswitch.rs import 集成测，参照现有 db 测用例建临时库）：

7. `ensure_group_creates_when_absent` — 组不存在 → 按 name 建组（生成 group_key）+ 关联给定 platform_ids/names。
8. `ensure_group_idempotent` — 同名组已存在 → **不重复建组**，仅 attach（关联 `group_platform` ON CONFLICT 幂等，不重复关联）。
9. `import_auto_group_false_skips` — auto_group=false → 不建组、不 attach（仅 apply 平台）。

前端映射（若项目有前端测试基建则加；当前 package.json 无 test 脚本 → 改为在组件内 inline 校验 + 手动冒烟）：`mapPlatformToProtocol` 三正例 + 一未识别兜底；`sub2apiAccountToPlatformJson` base_url 缺失走预设默认。

## 5. 验证清单（exec 收尾跑）

```bash
cd src-tauri && cargo clippy   # 0 warning（记忆 warnings-are-issues）
cd src-tauri && cargo test     # 含新 sub2api 单测
cd /Users/luoxin/persons/lyxamour/aidog && yarn build
node scripts/check-i18n.mjs    # 或 yarn check:i18n，验 7 语言 key 对齐
```

手动冒烟：
- sub2api：构造样例 JSON（anthropic+openai+gemini+未知 platform 各一）→ 文件入口 + 粘贴入口各导一次 → 验平台落库、未识别兜底 openai 且下拉可改、base_url 缺失走预设、extra 为空。
- auto-group：toggle 开导入 → 验导入平台属 `sub2api`（cc-switch 路径属 `cc-switch`）分组；toggle 关导入 → 验不建组、平台未关联任何组。
- cc-switch 回归：原本地探测导入流程不破，auto_group 默认开按 `cc-switch` 建组。

## 6. 遗留疑问（标「需要:」由 main 转达用户）

> 评审决策 ①②③ 已定（见 prd §0 决策反映），下列仅剩 **实现层歧义** 待确认：

- **需要:** auto-group × always-INSERT 重复导入的去重策略？同一份导出**重复导入**时：(i) **group 幂等**——`upsert_group_row` 按 group_key 查重，payload 注入无 group_key 会每次新建同名组 → 须改为「按 name 先查已存在组命中则复用」(即建议的 `ensure_group_and_attach` 显式 ensure，而非纯 payload 注入)；(ii) **platform 关联歧义**——platform always INSERT 产生多行同名平台，`relink_group_platform` 按 name 取首行 platform_id（`apply.rs:596-602`），重复导入时关联落到哪一行不确定。
  - **推荐**：(i) 必做 ensure-by-name（否则重复导入堆同名空组，体验差）；(ii) **接受重复**——关联 = 本次导入新建的 platform_id 集合，不做跨次平台去重（平台去重 = 改 apply always-INSERT 语义，超本任务范围，记忆 `platform-name-not-unique-import`）。若要更干净，备选「按 name+base_url 软去重平台」需单独评估、不在本任务做。
  - **影响 §1.② 接法选型**：若确认要 ensure-by-name，则采用「新增 `ensure_group_and_attach`（先 SELECT group by name → 命中复用/未命中 create → relink）」而非「纯 Payload 注入」。exec 据本决策落地。

---

### 附：参照源 file:line 索引

- 后端框架：`ccswitch.rs:443-468`（import 复用 apply，**改加 auto_group**）、`mod.rs:13-19`（mod + re-export）、`mod.rs:54-80`（Payload，含 `group` / `group_platform` 字段——auto-group 注入用）、`apply.rs:6-8`（platform 不参与冲突 always INSERT）。
- **auto-group 复用原语**：`apply.rs:302-365`（`apply_db` 遍历 group→platform→group_platform）、`apply.rs:309-334`（group upsert 流程）、`apply.rs:359-365`（group_platform name 对处理）、`apply.rs:410`（`upsert_group_row` 按 group_key 查重）、`apply.rs:583-621`（`relink_group_platform` 名→id 解析 + `group_platform` ON CONFLICT 幂等 INSERT，L591 group 按 name 查 / L596-602 platform 按 name 取首行 / L607-610 INSERT）。
- 命令：`lib.rs:2193-2199`（ccswitch_read）、`lib.rs:2204-2216`（ccswitch_import，**改加 auto_group 入参**）、`lib.rs:4156-4157`（ccswitch_* 注册，sub2api_* 紧跟其后）。
- 前端：`api.ts:1792-1796`（CcswitchReadResult DTO）、`api.ts:1798-1808`（ccswitchApi，`import` L1805-1807 **改加 autoGroup**）、`ccswitchMatch.ts:82-104`（buildMatch 取预设 base_url）、`ccswitchMatch.ts:231-260`（Platform JSON 形态）、`CcSwitchImport.tsx:56-181`（组件骨架，**L181 import 调用改加 autoGroup**）、`ImportExport.tsx:30,298`（入口挂载）。
- Protocol 枚举：`models.rs:5-16`（anthropic/openai/gemini serde 值）、`Platforms.tsx:20-23,95-99`（PROTOCOLS / getDefaultEndpoints）。
- sub2api 源：`Wei-Shaw/sub2api` · `backend/internal/handler/admin/account_data.go`（DataAccount struct，**实现时以 json tag 为准**）；导出 API `GET /api/v1/admin/accounts/data`。
