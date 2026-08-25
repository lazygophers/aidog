# aidog

aidog is a desktop control plane for AI coding CLIs. It runs a local proxy that routes a CLI's LLM
requests to whichever upstream provider the user has configured, and it writes the configuration
files those CLIs read so the user never edits them by hand.

## Language

### Routing

**Group**:
A named routing target. A CLI authenticates to the local proxy with the group name as its bearer
token, and the proxy picks an upstream from the group's platforms.
_Avoid_: profile, workspace, account

**Platform**:
One configured upstream LLM provider — a base URL, a key, a protocol, and a model set.
_Avoid_: provider, vendor, endpoint

**Protocol**:
The wire format a platform speaks (`anthropic`, `openai`, `glm_coding`, …). Determines both request
translation and which URL suffix the proxy appends.
_Avoid_: API type, format, wire API

**Client**:
An AI coding CLI that aidog configures and proxies for — currently Claude Code, Codex, and pi.
_Avoid_: agent, tool, harness

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
only the request sent upstream and the body returned downstream — never log recording.
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
随应用发布、人工维护的平台与模型知识库：一个平台一个文件夹，内含该平台的平台描述文件与模型文件，顶层一个索引文件。取代旧的单文件 `models.json` 与 `platform-presets.json`。
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
模型能力的一个维度：text、vision、image_gen、tool_use、reasoning、audio、video、embedding。每条 Model Entry 携带一组能力（取代旧的单值 `modality`）。
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
