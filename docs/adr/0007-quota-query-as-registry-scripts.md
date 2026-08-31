# ADR 0007: 配额查询从 Rust 硬编码迁入 registry 脚本

日期: 2026-09-01
状态: accepted

## 背景

平台余额 / Coding Plan 配额查询原是纯 Rust 代码：每平台一个 `quota.rs`（HTTP 调用 + 解析），
`Protocol`·base_url 双 dispatch，`capability.rs` 硬编码能力开关，newapi / devin 参数靠
`PlatformExtra` 结构体预定义字段。三条痛点：

1. **fork 无法数据化**：New API 系 fork（done-hub / VoAPI 等）接口形状各异，「newapi」一个
   Rust 实现罩不住所有部署，用户只能等 aidog 发版写新代码。
2. **发版依赖**：上游改接口（端点、字段名、单位换算）或出新变体，都必须改代码发版；
   registry 的远程同步机制（ADR 0004）对查询逻辑完全使不上。
3. **平台月级腐化**：查询逻辑与模型价格同源腐化，但价格已走 registry 同步、查询没有。

## 决策

1. **quota_scripts 结构**：platform.json 顶层 `quota_scripts: [{id, name(8 locale),
   requires:[{key,label(8 locale)}], returns:{balance,coding_plan,mcp,tiers[]}, script}]`。
   `script` 为自包含 JS 文本（boa 引擎 `run_custom_query` 求值，注入 `http.get/post` +
   `ctx.{baseUrl,apiKey,extra}`），内部可多次调上游再汇总（两步依赖查询如 newapi
   token usage → 用户余额可表达）；同族协议正文直接复制到各自文件，无跨文件引用机制。
   `requires` 声明的参数值存 `platform.extra.<key>`，脚本经 `ctx.extra.<key>` 读
   （newapi / devin 存量嵌套家 `extra.newapi.*` / `extra.devin.*` 嵌套优先、顶层兜底，
   保存时前端顶层写值 + 镜像写旧嵌套家，两侧恒等）。
2. **物化列**：platform 行加 `quota_script TEXT NOT NULL DEFAULT ''`。registry 源文件保持
   多变体全量，DB 只存物化后的单条可执行正文；**仅用户操作（选变体 / 保存平台）时物化**，
   远程同步只更新 registry 数据、不自动换已物化脚本（防远程改坏用户正在用的查询）。
3. **执行时 custom-wins 回落链**（`registry::resolve_quota_script`）：物化列非空 → 用之；
   否则 `extra.quota_custom_script`（用户手写）非空 → 用之；否则按 `extra.quota_script_id`
   选中变体（id 失效回落首条，UI 显示已回落）；无任何脚本 → None（调用方回落 base_url
   启发式或 Unsupported）。物化规则与之一致：custom 正文非空时覆盖 id 选中。
4. **错误文案前缀映射约定**：引擎出站单点产 `HTTP {status}: {body}`（截 500 字）/
   `JSON parse: {e}` / 裸网络错误串；脚本自身 throw 的消息约定加平台前缀（`Network: ` /
   `Parse: ` 等），等价测试只锁前缀不锁全文。改前缀 = 破坏等价测试与 UI 排障语义，
   动前须三处（引擎 / 脚本 / 测试）同改。
5. 用户自定义脚本运行时合成伪变体（补 id + name，无 requires / returns 元数据）与
   registry 变体并列同一列表；能力入口（卡片是否渲染刷新按钮）由前端从 registry 数据
   派生（`platformHasQuotaScript` 同步索引），不消费 Rust 能力开关——原 `quota_config_for`
   / `capability.rs` 死链已随本迁移删除。
6. 执行统一入口 `run_quota_script`：行在 → 按行协议走脚本（协议是权威）；行协议无脚本
   回落 base_url 启发式（关键词 → registry code → 首条变体，零配置探测路径）。
   `query_quota_newapi` / `query_quota_devin` 保留为稳定签名薄委托。

## 后果

- 平台维护者给平台加配额查询 = 改 registry JSON（盖 last_updated 戳），走远程同步分发，
  不发版；fork 变体注册成多 script 条目，用户下拉选即可。
- 新增脚本进信任面：registry 经 jsDelivr 同步本就是既定信任模型，脚本跑在 boa 沙箱、
  出站仅 `http.get/post` 两函数且统一走系统代理 client + 落 `proxy_log`
  （group_key `[quota:script]`），未新增能力面。
- 同族脚本正文逐文件复制（如 glm 族 4 份），上游改接口需同步改多份——与 ADR 0004 的
  per-platform 独立性换自包含的取舍一致。
- 已物化脚本不随远程同步自动更新：上游修复要用户重选变体（或清 `extra.quota_script_id`
   触发重物化）才生效，是防自动换坏查询的刻意代价。
- JS 错误信息排障性弱于 Rust 调用栈，靠前缀约定 + `[quota:script]` 日志行补偿。

## 备选方案

- 声明式 JSON 脚本形态（endpoint + 字段路径映射）：表达不了两步依赖与条件分支，弃。
- 远程同步后自动重物化：远程 registry 一旦出错会静默打断用户正在用的查询，弃
  （改为用户主动重选）。
- 保留 Rust 实现作兜底：双实现等价维护成本高于收益，一次性全迁后删除。
