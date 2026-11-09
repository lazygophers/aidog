# AiDog

AiDog 是本地 AI 网关：把 Claude Code / Codex 等客户端的请求按分组路由到各平台，统一记账与统计。

## Language

### Routing

**平台 (Platform)**:
一个上游 API 供应商实例，由用户创建（base_url + apikey 等），归属某个协议类型。
_Avoid_: 供应商（那是 registry 里的协议概念）

**协议 (Protocol)**:
平台类型的枚举值，既是 wire 协议名也是平台别名（如 anthropic / openai / glm_coding），Rust 与 TS 双写。
_Avoid_: 通道、线路

**透传平台 (Passthrough Platform)**:
不识别协议、不转换内容的平台类型：请求原样打到用户指定的 base_url，仅注入用户定义的认证头。usage 尽力解析，费用恒不估算。
_Avoid_: 直连（那指客户端直连上游）

**分组 (Group)**:
客户端接入的单元，密钥即分组名；决定请求在哪些平台间路由与重试。
_Avoid_: 账号

**密钥通道 (Key Channel)**:
客户端把分组密钥放进请求的一个位置：一个 header 名或一个 URL 参数名。一个请求可同时带多个通道，任一通道的值等于某分组密钥即定组。
_Avoid_: 认证头（那指 aidog 发给上游的头）

**模型槽 (Model Slot)**:
平台上按档位配置的候选模型集合，共 8 槽（`default` / `sonnet` / `opus` / `haiku` / `gpt` / `jev` / `embedding` / `rerank`），路由按 source_model 匹配。`jev` 槽服务 Decision Request。透传平台不吃模型槽（通配命中）。
_Avoid_: 模型列表（那是下拉候选清单）、model alias、tier

**模型链 (Model Chain)**:
一个模型槽里有序排列的模型名列表；首个为主模型，其余为备选，按顺序使用。
_Avoid_: 模型列表（下拉候选清单）、fallback 平台链

**时段档 (Time Model Rule)**:
平台上按时段生效的一整套模型配置：内置槽位（含 embedding / rerank）与各槽启用开关。时段命中时整套替换平台的默认模型配置，档里没填的槽不回落默认配置；自定义槽不属于时段档。
_Avoid_: 时段规则、高峰模型（那是 preset 的 peak 分支）

**同平台换模型 (Same-Platform Model Switch)**:
一次请求在某平台上因模型类错误失败后，改用该槽模型链中的下一个模型重试，而不离开该平台。
_Avoid_: 平台重试、故障转移（那是换平台）

**尝试 (Attempt)**:
一次请求里向某个「平台 + 模型」组合发出的一次上游调用。每次尝试前按当时状态重新选组合；失败的组合（模型类错误）或平台（服务类错误）在本次请求内不再被选。一次请求的尝试总数有上限（分组「最大尝试次数」，1–10）。
_Avoid_: 重试次数（旧口径只数平台）

**路由决策快照 (Routing Decision Snapshot)**:
一次请求路由时对全部候选平台的评估结果：每平台一个原因标签（选中/排除原因）+ 最终顺序与位置依据，随该请求的 proxy_log 落库。回答「为什么选它、为什么不是别的」。
_Avoid_: 决策日志（那指 tracing 内存日志）、blocked_reason（那只是整组失败的单因）

**est_cost**:
单请求估算费用，落 proxy_log。已知价取 registry 模型价，缺价走 fallback 单价；透传平台例外恒 0。
_Avoid_: 账单（那是上游真实计费）

**限频策略 (Rate Limit Policy)**:
一组上限规则（并发 / RPM / TPM，平台级 + 模型级，可带时段窗口）。registry 内置的叫「内置策略」（`platform.json` 的 `rate_limit_policies`，只填官方值、带出处）；平台 `extra.rate_limits` 里的叫「平台限频配置」。
_Avoid_: 限频快照（那是另一回事，见下）、额度窗口（`extra.quota_windows`，不属本概念）

**限频快照 (Rate Limit Snapshot)**:
数据库 `platform.rate_limit` 列，上游响应头里读到的余量数据，只读展示，不参与限流决策。
_Avoid_: 限频策略（那是用户 / registry 声明的上限规则）

**Client**:
A program that sends requests to aidog: AI coding CLIs, IDE plugins, language SDKs, and HTTP tools. Identified by User-Agent against a two-level vocabulary (category → variant).
_Avoid_: agent, tool, harness

**Decision Request（决策请求）**:
A proxied request in the decision wire format (TypeSafe `/v1/systemone`: a state plus typed questions, answered with choices, yes/no probabilities, or rubric scores). Routed only to Platforms that support decisions; chat requests never go to decision-only Platforms.
_Avoid_: classification request, judge call

**Default Group**:
The one Group whose configuration aidog merges into a Client's own global config file, so the user
can launch that Client bare and still be routed.
_Avoid_: primary group, active group

### pi integration

**pi Provider**:
An entry aidog generates in pi's `models.json`, named `aidog-<group>`, that points pi at the local
proxy for one Group. pi's own built-in providers are untouched.
_Avoid_: pi profile, pi config entry

**Touchpoint**:
One dimension along which aidog integrates a Client — installation, config generation, launch
command, skills, icon, and so on. Used to check a new Client for parity against existing ones.
_Avoid_: integration point, surface, feature

### Middleware

**Middleware Rule**:
One proxy rewrite rule: a Condition Tree, an Action Chain, and an Applies To filter. It affects
only the request sent upstream and the body returned downstream — never log recording, with
one exception: text replaced by a Reversible Mask is logged in placeholder form on every side.
_Avoid_: rule type, filter rule (the old 8-type names)

**Condition Tree**:
A nested ALL/ANY boolean combination whose leaves each match one target (request_body /
request_headers / response_body / response_headers / status / model) with contains/regex/exact.
All leaves in one rule must belong to the same phase (request-side or response-side).
_Avoid_: condition list, matcher

**Action Chain**:
An ordered sequence of actions (mask / block / warn / inject / override / classify) executed when
the tree matches. block and classify are terminal: they stop the chain and all later rules.
_Avoid_: action type

**Applies To**:
The filter on a rule — platforms, groups, models — that decides where it runs. Empty means all;
multiple values mean any-of; rules stack by priority. Replaces the old three-level cascade.
_Avoid_: scope, scope_ref

**Reversible Mask**:
The mode of the mask action that replaces each matched span in the request with a placeholder and
automatically restores the original text in the response returned to the client — one request-side
rule does both. The other mode, Fixed Mask, replaces with fixed text and never restores.
_Avoid_: pseudonymize, privacy scrub, 隐私脱敏 as a separate feature

**Builtin Rule**:
A rule shipped by aidog. It can only be enabled or disabled — never edited or deleted. Upgrades
overwrite its content by name while preserving the user's disabled state.
_Avoid_: default rule, seed rule

**Failed Rule**:
A leftover rule from the old 8-type model that could not be translated. It is shown as failed in
the list for the user to delete manually.
_Avoid_: broken rule, invalid rule

### Model Info（模型信息）

**Registry（注册表）**:
内置平台/协议/模型条目的数据真值源（`src-tauri/defaults/registry/`），随应用发布、远程可同步、人工维护：一个平台一个文件夹，内含该平台的平台描述文件与模型文件，顶层一个索引文件。取代旧的单文件 `models.json` 与 `platform-presets.json`。
_避免_: catalog、presets（指新结构时）、knowledge base

**Registry Index（注册表索引）**:
Registry 顶层索引文件，列出全部平台（名称、code、文件位置）。远程同步先读它，再决定拉取哪些文件。
_避免_: manifest、目录枚举

**Model Entry（模型条目）**:
一个平台对一个模型的完整视图：价格、能力、上下文限制、内置工具剔除、版本链位置。同一底层模型在每个平台各有一条 Model Entry，相互独立——定价、能力、入参都可能不同。
_避免_: model（有歧义——请明确说 Model Entry 或 Canonical Model）

**Canonical Model（规范模型）**:
模型的内部统一身份，用于跨平台聚合该模型的各条 Model Entry（比价、模型维度 tab 聚合）。区别于平台真实 `model_id`（线上请求实际用的名字）。
_避免_: 默认模型 id、官方名

**Capability（能力）**:
模型能力的一个维度：text、vision、tool_use、reasoning、audio、video、embedding、rerank、decision，以及生成类（text_to_image、image_to_image、image_edit、text_to_video、image_to_video、video_to_video、video_edit）。每条 Model Entry 携带一组能力（取代旧的单值 `modality`）。`decision` 模型不生成文字，只对给定问题返回选择 / 是否概率 / 等级分，因此不含 text。
_避免_: modality

**Version Chain（版本链）**:
一个平台内模型的家族迭代谱系（如 glm-4.5 → glm-4.6），记录在每条 Model Entry 的 family / version / predecessor 字段里。
_避免_: 别名演进史（同 id 不同时期重新指向，刻意不追踪）

**Official Pricing（官方定价标记）**:
价格记录上的 per-platform 标记，意为「这条价格是厂商自营价」——同一模型可以在一个平台是官方价、在另一个平台是转售价。
_避免_: 官方平台（标记属于价格，不属于平台）

**Builtin Tool Exclusion（内置工具剔除）**:
已知某 Model Entry 不支持的 Claude Code 内置工具黑名单；缺省表示全部支持。描述模型本身，与 proxy 层的 `builtin_tool_compat` 兼容 hack 无关。
_避免_: builtin_tool_compat（那是代理层 hack，不是模型能力）

**Peak Price（分时价格）**:
per-model 的分时段绝对价。平台 peak 窗口命中时优先使用；未命中或缺失时回落平台倍率，再回落默认价。
_避免_: peak multiplier（倍率是平台级回落机制，不是模型价格）

### Quota（配额查询）

**Quota Script（配额脚本）**:
platform.json 顶层 `quota_scripts` 数组里的一条自包含 JS 脚本，查询一个平台的余额 / Coding
Plan 配额：内部多次调上游接口再汇总，返回一份完整 PlatformQuota。执行走 boa 沙箱（仅
http.get/post 出站、统一系统代理 + 落日志）；用户参数按 `requires` 声明存 `platform.extra`，
脚本经 `ctx.extra` 读。取代旧的 per-platform Rust 查询实现（ADR 0007）。
_避免_: quota.rs、custom query script（那是旧的泛型用户脚本入口，机制已统一）

**Quota Script Variant（配额脚本变体）**:
同一平台协议下的一个部署形态对应一条 Quota Script（如 new-api 的各种 fork 各一条）。用户
在平台设置里选定变体（id 存 `extra.quota_script_id`，缺省数组首条、id 失效回落首条），
保存时单条正文物化进 platform 行的 `quota_script` 列执行；用户自定义脚本合成为伪变体
（custom 正文优先于 id 选中）。远程同步只更新 registry 变体清单，不自动换已物化脚本。
_避免_: deployment、provider preset

## 统一日志（卡顿/故障定位入口）

前后端同文件：`~/.aidog/logs/aidog.<YYYY-MM-DD-HH>.log`，按小时分割，按 `retention_hours`（默认 24）自动清理，设置页可调（`app_log_settings_get/set`，file_enabled/level/retention_hours）。前端 console（error/warn/info/debug）与未捕获异常由 `src/utils/frontendLog.ts` 入口 patch 转发 `app_log_frontend` 命令，Rust 侧 tracing `target=frontend` 落同一 appender。定位卡顿：复现时段内 grep `frontend`（前端渲染异常）+ `trace_id`（代理请求链）+ `ERROR`（后端），打包给 AI：`tar czf aidog-logs.tgz -C ~/.aidog logs`。_避免_: appender、webview console

**请求模型 (Requested Model)**:
客户端请求里带的模型名，存 `proxy_log.model`；经路由映射后实际发给上游的名字是「实际模型」（`proxy_log.actual_model`）。
日志页两者各一个下拉，可同时过滤（AND）。_避免_: 原始模型（旧 UI 叫法）

### 直接模型转发

**直通模型（Direct Model）**:
平台级声明（`platform.extra.direct_models`）的一组模型 id：分组请求带这些 id 时，只在声明
它的平台中按现有优先级 failover，转发保留请求的模型原名（跳过槽位改写）；声明平台全不可用
回 503（`blocked_reason: direct_model_unavailable`，落 proxy_log 审计）。声明即生效，不查
registry 收录（聚合站自定义模型名可配）。整条直通链只在组内任一平台声明了直通模型时启用，
未声明的组行为与现状一致。_避免_: 直接模型（需求口语）、直连模型

## 推荐配置预设

**推荐配置预设（Recommended Preset）**:
一份可被设置页「加载推荐配置」选用的配置模板，随应用打包，一份一个文件、以文件名为 id，带一个显示名；Claude Code、Codex、Pi 各有自己的一组。不同于 Registry（平台/模型数据）与 provider preset（平台协议预置条目）。
_避免_: 推荐配置文件（指单份时）、settings 模板

**开源默认预设（OSS Preset）**:
id 为 `oss` 的推荐配置预设，只含通用隐私/安全项，不依赖本机工具、不放宽权限；任何构建都必带，且是首次兜底与 hooks 默认值的取值来源。
_避免_: 公开版、发版版

**组织预设（Organization Preset）**:
作者所在组织（lazygophers）自用的推荐配置预设，含个人工具链与偏好，进 git 管理但在 GitHub 构建中不打包。
_避免_: 个人版、私有预设

## 对外接口

**管理接口（Management API）**:
第三方程序通过 HTTP 调用 aidog 命令的入口，能力与界面上的操作等同（读配置、改平台、查日志、看统计等），附带一路实时事件推送。只接受本机连接，由设置里一个开关统一启停。
_避免_: 管理面（旧称）、RPC、开放 API

**Local API**:
挂在代理端口上、供本机 hook / statusline 脚本使用的少数专用端点（查分组信息、发本机通知），以分组名鉴权。不同于管理接口：它只服务 aidog 自己生成的脚本。
_避免_: 本地接口、管理接口

**转发接口（Forwarding API）**:
客户端把 AI 请求交给 aidog、由 aidog 按分组路由到平台的那些端点（如 chat completions、messages、embeddings）。以分组名作为调用凭据。
_避免_: 代理接口、上游接口

**密钥信息（Secret Field）**:
任何拿到即可冒用身份或解密数据的值：平台 API 密钥、入站密钥通道配置、管理接口凭据、通知渠道的 token 与 webhook、备份/导出口令及含密钥的导出包、MITM 证书私钥。分组名不算。管理接口对密钥信息只写不读。
_避免_: 敏感信息（范围更宽）、凭证

## 桌面形态

**界面（GUI）**:
桌面应用里用户看得见、能操作的主窗口及其页面。可以随时关掉再打开，关掉时不占用界面内存。
_避免_: 前端（指代码层时另说）、主程序

**内核（Core）**:
界面关掉后仍在后台运行的那部分 aidog：代理转发、托盘、通知、定时任务。退出内核才算完全退出 aidog。无头内核（独立的内核版程序）是同一内核不带界面单独运行的形态。
_避免_: 后台、服务端

**托盘常驻（Tray-resident）**:
界面已关闭、只有内核在运行的状态：屏幕底部的 Dock / 任务栏和顶部应用菜单都不显示，只能从托盘、aidog:// 链接或再次启动应用重新打开界面。
_避免_: 静默模式（那是启动选项「静默启动」）、最小化、隐藏窗口

## 调度与统计

**路由模式（Routing mode）**:
分组选平台的固定策略，分组设置里二选一或多选一的单一值。共 8 种：故障转移（按优先级逐个回退，即 magpie 的 order）、负载均衡、健康感知、会话粘性、严格轮换（rotate，候选间轮流，重启后从头开始）、用量最少（usage，近 24 小时成功请求少的优先）、余量最多（pace，套餐剩余额度多的优先，无数据时回落用量）、智能模式（smart，最快重置的优先，无数据时回落健康感知）。
_避免_: 调度模式、路由策略（同一概念混用多个名字）

**会话（Session）**:
一次连续对话产生的一组请求，由请求里自带的会话标识归组；没有标识的请求不属于任何会话，不进会话统计。会话标题取该会话第一条用户请求的开头几个字；没记录原始请求内容时显示未命名会话。
_避免_: 对话（统计口径里不用）、session id（泛指任何字符串标识，会话归组用的是会话键）

**会话键（Session key）**:
请求里提取出来的会话标识本身，每个请求一个值，同一次对话的请求共享同一个值。
_避免_: 会话 ID、trace id

## 代理日志与 MITM 观测

**代理日志**（Proxy Log）：
AI 请求的账本。每条记录一次 AI 请求的模型、token、费用与收发内容。例外有二：发往已知 AI 平台、但未能解密的**未解密隧道**也记一条，只有元数据，用来提示有 AI 费用没算进去；由**网关代执行**应答、没有发往任何平台的请求不进代理日志，只进**工具日志**。
_Avoid_: 请求日志（指两样东西时）、连接日志

**MITM 观测记录**（MITM Observation Record）：
经过 AiDog 代理的每一个连接的元数据：目标地址、是否解密、状态、耗时、上下行字节数。非 AI 连接只出现在这里，不进代理日志。
_Avoid_: mitm 日志、旁路日志

**未解密隧道**（Undecrypted Tunnel）：
带着分组凭据经过 AiDog、但目标地址不在 MITM 解密名单里的连接。AiDog 只原样转发字节，看不到内容。
_Avoid_: 盲转（未带分组凭据的普通转发也叫盲转）、opaque

## 工具

**工具（AiDog Tool）**:
AiDog 自己提供、由 AiDog 执行的能力：网页搜索、抓网页、生图、生视频。客户端可通过 MCP 调用，也可由网关代执行时内部使用。配置集中在「扩展 → 工具」页。
_避免_: MCP（指第三方 MCP 服务器管理）、插件

**网关代执行（Gateway-executed server tool）**:
请求里带只有 Anthropic 官方能执行的服务端工具（网页搜索、抓网页），而选中的平台不是 Anthropic 官方时，AiDog 不调用模型，自己执行一次该工具并按官方结果格式应答。
_避免_: 拦截、模拟搜索、本地搜索

**工具按需加载模拟（Tool search emulation）**:
选中的平台不是 Anthropic 官方时，AiDog 替上游实现 Claude Code 的「工具按需加载」：标记为延迟加载的工具先不发给上游，只发对话中已被点名加载的那些。
_避免_: 工具搜索（Claude Code 客户端里的 ToolSearch 工具同名）、展开

**搜索引擎（Search Engine）**:
网页搜索的一个结果来源，分两类：免 key 引擎（AiDog 直接抓搜索页或调用免 key 公开接口）与 key 服务（需填 API key 的搜索服务商）。每个引擎可单独开关。
_避免_: provider（只指 key 服务时可用）、搜索源

**工具日志（Tool Log）**:
工具每次执行的账本：哪个工具、由谁触发（网关代执行或 MCP 调用）、所属分组、用了哪些引擎、各返回几条、耗时、成败。搜索词与结果内容受「记录用户请求原始信息」开关控制。保留天数单独设置。
_避免_: 搜索日志、代理日志
