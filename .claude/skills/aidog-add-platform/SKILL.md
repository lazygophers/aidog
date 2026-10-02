---
name: aidog-add-platform
description: 在 aidog 里加一个新平台或改一个平台的默认配置（base_url / 端点协议 / coding plan / 默认模型 / 余额查询）。registry 化后流程 = ① registry JSON 三件套（platform.json + index.json + models/*.json）② Protocol 枚举 Rust↔TS 双写（aidog_db/src/models/protocol.rs + src/services/api/types/manual.ts，commandcode 2026-10-01 漏过）③ 可选 quota_scripts（数据驱动）。覆盖全套触点与顺序，并避开反直觉陷阱（平台预设住 registry JSON 非前端代码、quota 按 quota_url_match 数据分派、coding 套餐一律独立协议）。触发词：加平台、新增平台、添加平台、改平台默认配置、平台 base_url、平台预设、getDefaultEndpoints、获取余额、查余额、coding plan 配额、coding plan 查询、默认模型、Protocol 枚举、新协议、新增 adapter、supported_protocols、platform.json。
when_to_use: 给 aidog 新增一个平台（下拉里能选、自动填 base_url 和默认模型）；改某平台的默认 base_url/端点/coding plan；给平台接上游余额或 coding plan 配额查询；加一个全新的 wire 协议（anthropic/openai/gemini/typesafe 都不匹配）时
---

# aidog 加平台 / 改平台

给 aidog 新增一个平台，或修改平台默认配置（base_url、端点协议、coding plan、默认模型、余额/配额查询）。本 skill 给出**改哪几处、什么顺序、怎么验证**，并把反直觉陷阱前置。

> 校对于 2026-10-02（registry 化 + crates 拆分后布局）。file:line 以**函数名/符号名**定位为准，行号仅作快速跳转。字段级规范（locale 覆盖 / last_updated 盖戳 / supported_protocols 词表等）真值源是项目 CLAUDE.md「平台默认配置 (registry)」节，本 skill 不复抄。

---

## 0. 认知纠偏（动手前必读）

1. **平台预设住 registry JSON，不在前端、不在 db.rs。**
   真值源 = `src-tauri/defaults/registry/`：`index.json`（平台清单）+ `platforms/<code>/platform.json`（一平台一文件）+ `platforms/<code>/models/<model>.json`（per-platform 模型条目）。前端 `src/pages/Platforms.tsx` / `src/domains/platforms/` **零硬编码预设**：下拉（`buildProtocolsFromPresets`）、显示名（`getProtocolLabelMap`）、品牌色（`getProtocolColorMap`）、端点/默认模型（`getDefaultEndpoints` / `getDefaultModels`）全部 async 从 presets 文档派生（`src/domains/platforms/defaults.ts`，模块级 `docPromise` 单次 RPC 缓存）。**改预设 = 改 JSON，不改前端代码。**

2. **🔴 新平台 code 必须进 Protocol 枚举 Rust↔TS 双写，无任何门禁强制，漏了 = 静默错。**
   - Rust：`src-tauri/crates/aidog_db/src/models/protocol.rs`（`pub enum Protocol`，`#[serde(rename = "<code>")]`；aidog_core 等全部 `use aidog_db::models::Protocol`，无第二份枚举）。
   - TS：`src/services/api/types/manual.ts` 的 `export type Protocol =` 联合类型加 `| "<code>"`，与 serde rename **逐字一致**。
   漏加 Rust 侧 → DB `platform.platform_type` 经 `Protocol::from_db_str`（`protocol.rs:229`）**静默回落 Anthropic** + warn 日志：UI 平台类型名显示错、平台类型相关派生全错，但请求照常转发——全绿门禁也发现不了。2026-10-01 commandcode 即此坑（2026-10-02 修复）。现 `test_registry.rs` 有「registry 平台 code ⊆ Protocol 枚举」门禁兜底，但 TS 侧 `manual.ts` 仍是纯手写（PROTOCOL_LABELS 已改 `Partial`，tsc 不再 exhaustive 强制）。
   ⚠️ 已知存量例外（缺双写、待主会话裁决）：`litellm` / `meta` / `mistral` / `xai`。

3. **Protocol 变体有两类语义**（`protocol.rs` 注释划分）：
   - **wire 协议（可作 endpoint 协议）**：`anthropic / openai / openai_responses / openai_completions / gemini / typesafe / mock`，决定请求体格式 / api_path / SSE 解析，实现在 `src-tauri/crates/aidog_adapter/src/protocols/<name>/`，经 `converter/registry.rs::converter_registry` 注册表分派。
   - **平台类型（仅作平台主协议）**：其余全部（glm/deepseek/openrouter/…），只是身份标签，**不参与 wire 转换**。新平台加的枚举变体几乎总是这一类。
   90%+ 的「加平台」只是又一个 OpenAI/Anthropic 兼容中转/聚合站，wire 复用前 6 个之一，给它 registry 条目 + 枚举双写即可。

4. **coding plan / 余额查询是数据驱动脚本，不在 quota.rs 手写函数。**
   `platform.json` 顶层 `quota_url_match`（base_url 小写子串数组，分派用）+ `quota_scripts`（变体数组，每条 JS 语法脚本，`http.get(url, headers)` / `ctx.apiKey` / 返回 `{success, error, balance?, coding_plan?}`）。执行/物化回落链在 `aidog_db/src/registry.rs`（`resolve_quota_script` / `materialize_quota_script`）。旧「quota.rs 加 query_foo_balance + if url.contains 分派」已废（仅 newapi / devin 特例留在 `aidog_core/src/platform_cmd/quota.rs`）。

5. **coding 套餐一律独立协议**，无 coding_plan JSON 分支：platform.json 顶层 `is_coding_plan: true` 标记，code 独立（如 `glm_coding`）。运行时 endpoint 级 `coding_plan` flag 仍可标。

---

## 1. 路径判定

```
新平台的上游报文格式？
├─ OpenAI / Anthropic / Responses / Completions / Gemini / TypeSafe 兼容（绝大多数）
│     → endpoint.protocol = 对应 wire 协议，base_url 含版本前缀
│     → 【路径 1】零 wire 代码改动
└─ 全新私有 wire 格式（几乎不出现）
      → 【路径 2】新建 ProtocolConverter + 注册表 + 前端两处派生（重活）
```

---

## 2. 路径 1：普通平台（3 硬触点 + 2 可选，缺一即失败）

### ① registry 三件套 — `src-tauri/defaults/registry/`
- `platforms/<code>/platform.json`：`endpoints`（`default` 分支；coding 套餐是独立 code 无分支）+ `models`（default / 可选 peak 分支）+ `model_list`（default，**必须是 models 各分支值集的超集**）+ 品牌字段（`name` 8 locale / `logo_url`（simpleicons slug）/ `color` / `homepage` / `keywords` / `source_urls` 对象 `{docs, pricing}`）+ 可选 `key_prefixes` / `peak` / `quota_url_match` / `quota_scripts`，顶层 `last_updated`。
- `platforms/<code>/models/<model>.json`：每模型一条，`model_id` / `canonical_model` / `supported_protocols`（**必填 ≥1**，值域见 schema）/ `price`（`input` 必填，全数值）等。
- `index.json`：platforms 数组登记 `{code, platform_file, models_dir, models[]}`（漏登记 = 远程同步永远拉不到）。
- 🔴 改完任何 registry json **必跑 `node scripts/bump-registry-last-updated.mjs`** 盖戳（不盖则远程同步按内容比较跳过；共享 checkout 上另有讲究，见 `.claude/rules/shared-worktree.md`）。
- 字段细节以 CLAUDE.md「平台默认配置」+ `schema/` 为准；数据值先官方源验证（memory `registry-verify-before-create`）。

### ② Rust Protocol 枚举 — `src-tauri/crates/aidog_db/src/models/protocol.rs`
在对应段落（国内官方 / 聚合 / 第三方 / 中转）加：
```rust
#[serde(rename = "foo")]   // ★ = registry code = DB 持久值 = TS 字面量，确定后不可改
Foo,
```
加完 `cargo build`：无 `_` 兜底的 exhaustive match 会编译报错指路。当前已知唯一无兜底热点随代码漂移，直接信编译器。有 `_` 兜底的热点：`convert_request` / `parse_sse`（`aidog_adapter/src/converter/`，OpenAI 兼容自动兜底）、`inject_coding_plan_fields`（`aidog_core/src/gateway/proxy/headers.rs:440`，仅注特殊 body 字段才加分支）、`client_type` 派生（`registry.rs::derive_client_type`）。

### ③ TS Protocol 联合类型 — `src/services/api/types/manual.ts`
`export type Protocol =` 加 `| "foo"`，与 ② 的 rename 逐字一致。**无编译强制**（PROTOCOL_LABELS 已 Partial 化），漏了 = 前端到处 `as Protocol` 撒谎，运行时字符串照传、类型检查失效。

### 可选（按需）
- **厂商直连端点锁死**：`Protocol::endpoints_locked()`（`protocol.rs:261` matches! 链）↔ 前端 `ENDPOINTS_LOCKED_PROTOCOLS`（`src/domains/platforms/constants.ts`）**同集双写**（有对称测试锚点），官方端点固定的平台才加。
- **per-platform wire 微调**：`src-tauri/crates/aidog_adapter/src/<platform>/`（如 `glm/openai_chat.rs`）+ `aidog_adapter/src/lib.rs` 声明 mod。仅该平台请求/响应要特调时才建；多数新平台（如 commandcode / tokenrhythm）没有。
- **默认模型**：就是 ① 的 `models.default`（slot 键见 `ModelSlot`），见 `references/default-model.md`。
- **余额 / coding plan 配额**：① 的 `quota_url_match` + `quota_scripts`，见 `references/quota-coding-plan.md`。

> 路径 1 **不需要**动：converter / proxy / router / db / 前端任何 .ts/.tsx。

### 2.5 修改现有平台预设
只改对应 `platform.json` / `models/*.json` + 盖戳，②③ 已存在无需动。**code 一旦发布不可改名**（serde rename = DB 持久值，改名 = 旧库 platform_type 全部回落 Anthropic）。

---

## 3. 路径 2：加新 wire 协议（重活，几乎用不上）

仅当 6 个 wire 协议全不匹配时。除路径 1 外还需：

1. **新建 converter**：`src-tauri/crates/aidog_adapter/src/protocols/<name>/` 实现 `ProtocolConverter` trait（参考 `protocols/openai/` / `anthropic/`）。
2. **注册表**：`aidog_adapter/src/converter/registry.rs::converter_registry` map 加一行（注册表驱动，不再是大 match）。
3. **Protocol 枚举 wire 段**：`protocol.rs` 的「AI 请求协议」段加变体（同 ②，但放 wire 段）。
4. **鉴权头**：`aidog_core/src/gateway/proxy/` 的 upstream headers 构造按协议加分支（anthropic `x-api-key` / gemini `x-goog-api-key` / 默认 Bearer）。
5. **前端端点协议选项**：`src/domains/platforms/constants.ts` 的 `ENDPOINT_PROTOCOLS` + `PROTOCOL_LABELS`（两处同集）。
6. **client_type 缺省派生对称**：Rust `registry.rs::derive_client_type` ↔ TS `src/domains/platforms/defaults.ts::clientTypeForProtocol`（禁单侧改）。

---

## 4. URL 构造铁律

- `base_url` **含版本前缀**（`/v1`、`/api/paas/v4`、`/provider/v1` 等）。
- wire 协议 api_path：OpenAI 系 `/chat/completions`、anthropic `/v1/messages`、openai_responses `/v1/responses`、gemini `/v1beta/...`、typesafe `/systemone`。
- 最终 URL = `base_url + api_path`，**禁止额外拼接**。anthropic 端点 base_url 填到 API 根，openai 端点含 `/v1`。

---

## 5. client_type 陷阱（协议 ≠ 身份）

- **Protocol** = 报文格式；**ClientType** = 模拟哪个客户端（注入 UA / `X-Stainless-*` 指纹头过上游校验）。二者正交。
- 缺省派生：`clientTypeForProtocol`（anthropic → claude_code、openai 系 → codex_tui、其余 default）；例外平台在 endpoint 显式标 `client_type`（如官方 claude_code 直连端点标 default）。
- 🔴 coding plan 上游对 client_type 有**身份白名单**（如 Kimi coding 只接 Claude Code 身份拒 Codex）：配 coding endpoint 的 client_type 要匹配上游实际白名单，别按 wire 协议想当然。

---

## 6. 验证门禁

```bash
node scripts/check-registry.mjs                    # registry 结构 / 词表 / 覆盖率
cd src-tauri && cargo test -p aidog_db             # 含 registry ↔ index 零差集、registry code ⊆ Protocol 枚举门禁
cd src-tauri && cargo clippy                       # warning 清零（项目 CLAUDE.md）
yarn build                                         # tsc
# 冒烟：应用里选新平台 → base_url 自动预填、平台类型名正确（from_db_str 不回落）
```

收尾自检：
- [ ] serde rename ↔ TS 字面量 ↔ registry code 三者**逐字一致**（§0-2）。
- [ ] `index.json` 登记了 platform_file + models 全清单（漏 = 远程同步拉不到）。
- [ ] 所有改过的 registry json 已盖戳（bump 脚本，输出数字 > 0 才算盖了）。
- [ ] `model_list.default` ⊇ `models` 各分支值集。
- [ ] `supported_protocols` 每模型 ≥1 且在词表内。
- [ ] 冒烟过：平台类型名显示正确（不回落 Anthropic）。
- [ ] 厂商直连才动 endpoints_locked（Rust↔TS 双写）。

---

## 反例黑名单（不要做）

1. ❌ 去前端 `Platforms.tsx` / `defaults.ts` 加预设常量 —— 预设住 registry JSON，前端全派生。
2. ❌ 只改 registry JSON 不加 Protocol 枚举双写 —— `from_db_str` 静默回落 Anthropic，UI 平台类型名错（commandcode 2026-10-01 实锤）。
3. ❌ 只改 Rust 枚举不改 `manual.ts` —— TS 联合类型纯手写无强制，类型系统失效。
4. ❌ 改 platform code / serde rename 字符串 —— DB 持久值，改名即旧库全部回落。
5. ❌ base_url 不含版本前缀，或拼完再补 `/chat/completions` —— 双拼接。
6. ❌ coding endpoint 按 wire 协议想当然填 client_type —— 上游有身份白名单。
7. ❌ 手写 quota.rs 查询函数 —— 数据驱动 `quota_scripts`（仅 newapi / devin 特例在代码）。
8. ❌ 机器生成覆盖 registry JSON —— 手维护，脚本只做文本级插入（盖戳/预检）。
9. ❌ 改完 registry 不跑 bump 脚本 —— 远程同步按内容比较跳过，改动永远进不了用户库。

## 相关

- 触点全表：`references/touchpoints-map.md`
- 余额/配额脚本：`references/quota-coding-plan.md`
- 默认模型：`references/default-model.md`
- 数据维护守则：`.claude/rules/registry-data-edit.md`（编辑预检 / 盖戳坑 / 新必填字段三件套）
- 请求链路调试：`aidog-request-inspect` skill
