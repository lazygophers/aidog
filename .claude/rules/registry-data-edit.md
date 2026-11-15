# registry 数据维护：编辑脚本预检与验证输出读取

registry 数据轮次（r98-r105）暴露的两类流程缺陷。适用于一切「文本级编辑 JSON + 跑
check-registry 验证」的维护轮。

## 编辑脚本先预检，不盲插

文本级插入（不做 JSON round-trip，保科学计数法字面）在这轮两次撞上既有脏形状断言失败：

- 目标键已存在且**重复**（aihubmix `claude-3-opus-20240229.json` 双 `"capabilities"` 键，
  后值覆盖前值，行为碰巧一致但属数据错误）；
- 同文件同键出现两处（双 `official`），插入锚点匹配到的不是预期位置。

2026-09-24 起就有同族教训（price 块尾无逗号两种插入形状「已在多轮重犯」，见
`workflows/registry-data-alignment.md` 第 8 轮）。规则：

1. **批量插入前先预检每份目标文件**：目标键已存在 → 跳过并列入报告，不盲插不覆盖；
   重复键、锚点不唯一 → 单独处理，禁止放宽断言硬闯。
2. **同一插入形状问题重犯两次以上，修法是把预检写进脚本**（给 check-registry 加重复
   JSON 键检测 / 写公共编辑工具），不是靠下一轮 agent 记性好。
3. 插入后 `git diff` 逐文件确认改动只落在目标键，防止锚点误匹配绞坏邻近字段
   （第 8 轮曾绞坏 14 文件后重建）。

## 验证输出先看格式再 grep

check-registry 的覆盖率行是中文模板 `必补字段 ${name} 覆盖率 ${pct}%`
（`scripts/check-registry.mjs:367`），且仅在低于门禁时作为 failure 打印。用想当然的
pattern（英文 `coverage` 之类）grep 输出零命中，被当成「输出里没有覆盖率信息」→
漏读了本轮验证结论。规则：

1. **grep 零命中只说明 pattern 不匹配，不说明信息不存在。** 下「输出没有 X」的结论前，
   先去脚本源码 grep 打印模板（`console.log`/`console.error` 的字符串）确认实际格式。
2. 验证命令输出不长（百行级）就整读一遍再 grep；长输出先 `wc -l` 分段，确认每段都过眼
   或每段都有对应 grep。

## index.json 单独变更时盖戳脚本静默零盖（2026-09-30）

bump-registry-last-updated.mjs 的循环里 index.json 被 `continue` 跳过
（scripts/bump-registry-last-updated.mjs:100-105），只在**其它数据文件至少
一个被盖**时才推高 index 的全局 last_updated。单独改 index.json（重排平台
清单、改 pricing_only）→ 脚本输出「0 个文件已盖戳」且 exit 0，看似成功实际
没盖——index 自身的顶层 last_updated 没变，远程同步按内容比较跳过。此时手动
把 index.json 顶层 last_updated 改成当前 Unix 秒（等价操作），或顺带改任一
其它数据文件再跑脚本。

判脚本干了什么以输出数字为准：「0 个文件已盖戳」= 本次没盖，不是成功。

## 清单类测试逐条 panic：先收集全量失败再动手（2026-09-30）

修 platform slug 白名单测试，每修一个 panic 就暴露下一个（cline → zdotai），
循环三轮才绿。包含/排除名单、键集对比这类清单断言失败时，先把门禁改成一次
输出全部不匹配项（或临时收集再断言），看全了再一次性修，不逐条试错。

2026-11-09 数据补齐轮二次实锤，且主因升级为门禁写法本身：`every_model_has_an_official_platform`
逐条 panic，首报只见 1 条违规，修成收集式后一次见 12 条。补强：**新写清单断言直接写成
收集全量式**（循环内收集违规清单、循环尾一次 panic 打印全部），不写逐条 panic 的门禁。

## 新必填字段上线 = 三件套（2026-10-01 supported_protocols 轮，c74507db1）

给 model/platform 条目加一个 schema 必填字段时，三件事缺一不可，一次 session 内做齐：

1. **schema required**：`schema/model.schema.json` 加字段定义 + 塞进顶层 `required` 数组
   （值域词表用 enum 硬编码）。
2. **lint 硬错**：`check-registry.mjs` 加对应规则（缺失 / 空值 / 词表外值都算 failure，
   不是 warning）——只有 schema required 没有 lint，存量文件漏填要到运行时才炸。
3. **存量全量回填**：写脚本按可推断的缺省值批量补（如 supported_protocols =
   该平台 endpoints.default 的 protocol 并集），已知例外手改；回填本身走本文件
   「编辑脚本先预检」条——文本级插入、锚点唯一性预检、`git diff` 逐文件核对。

词表在 schema 与 lint 各写一份（对称设计），加新协议值时两处都要改——check-registry
顶部注释里的「与 platform.schema.json 对称」在 devin 值上已经不严格成立（devin 是
per-model 值、不是 endpoint 协议），新增第三种这类专用值时先想清楚它属于哪个词表。

先例：supported_protocols（c74507db1）、price.free（显式免费标记，同日 jev 轮）。

## 跨分支合并冲突的 registry JSON 解法（2026-11-15 PR #63 轮）

feature 分支与 master 各自盖过戳后再合并，registry JSON 必冲突：master 侧是
bump 脚本推高的时间戳，分支侧是功能字段（本例 17 个 platform.json 的
holiday_calendar + index.json 顶层 last_updated）。解法统一：

1. **功能字段保留分支侧，`last_updated` 取两边最大**——冲突点只有时间戳与
   功能字段的交叠，功能内容永不丢；两边都只动时间戳的文件（index.json）取 max。
2. 解完跑 `bump-registry-last-updated.mjs`（把解后时间戳推到当前）+
   `check-registry` 门禁，随 merge commit 一起交付。
3. 同 merge 里 master 新增的测试夹具构造点不必然要补新字段：字段带 serde
   默认值时夹具照常编译（本例 billing.rs 自动合并成功，cargo check EXIT=0
   证实，派发时预期的「手工补夹具两行」是多余的）。夹具要不要补，以
   cargo check 实测为准，不按预期预改。

## 非 registry 数据目录豁免盖戳约定：先证明灌种层不存在，再写 ADR（2026-11-15 holidays / ADR 0013）

「新必填字段上线 = 三件套」与 last_updated 盖戳约定默认适用于 registry 数据
（有 DB 灌种机制参与）。新数据目录（如编译期内置的 `holidays/` 日历）偏离该
约定（无 last_updated、无 schema）之前，先 grep 证明豁免成立：

1. 灌种层证据三查：build.rs 是否编译期 `include_str!`（`aidog_db/build.rs:84-102`
   静态表）→ 内存缓存层是否读 DB（`registry.rs:42`）→ schema 目录 grep 该
   目录名零命中。三查都成立 = 无任何 DB 灌种机制可参与，盖戳纯增重。
2. 豁免成立 → 给该目录的更新机制写一行真值源说明并落 ADR（本例 ADR 0013：
   改 JSON → rerun-if-changed 重编 → check-holidays 门禁），审查质疑时引
   ADR 应答；不成立 → 照常进三件套。
3. 判据：**有没有消费者经 DB 读它**，不是「是不是 JSON 数据」。134 份文件
   全量盖 last_updated 是审查提议与落选方案，理由记进 PR 回复与 ADR。

## 外部取证的失败与地址语义（2026-11-14 节假日日历轮）

1. **搜索无结果 / 403 / 404 / DNS·TLS 失败 ≠ 官方未公布。** 缺年、缺依据类
   缺口保留「未知」状态列进缺口清单，不写成「官方未发布」；凭记忆或候选日期
   补齐消除警告是更大的错误。同一条外部检索路线连败多轮（DDG 202、Bing/Google
   零结果、内置搜索 400）即停，冻结阻塞与缺口清单，不再重复同类取证。
2. **地址语义三不等**：注册地址 ≠ 邮寄地址（本例 opencode_zen：原写入依据的
   2443 Fillmore St 是条款里仲裁 opt-out 邮寄地址，复抓条款原文发现无法律
   实体名、适用法特拉华，US-CA 默认当场撤）；软件厂商地址 ≠ 自托管平台的
   运营主体所在地（litellm「install on your servers」→ 不设厂商所在地默认）。
   「主体所在地」类取证要拿到法律实体名 + 注册/隐私政策的注册地双证；拿不到
   就不设默认，不拿邮寄地址、厂商地址顶替。

## 摘要型抓取工具的输出是假设不是事实（2026-10-15）

registry-refresh 轮：webReader（MCP 网页摘要）第二次抓 docs.bigmodel.cn 套餐页
时摘出 "GLM-5.4 (beta)"，同页 raw curl 零命中；第一次摘要正确——同工具同页，
第一次对不构成第二次对的证据。

- AI 摘要型抓取产物（webReader / WebFetch 的小模型回答）**只用来定位，不用来
  取值**。落库前必须有 raw 一手（curl 原文）或双源对照；两次摘要互相印证不算双源。
- 同模式先例：ui-color-bug-verify.md「静态推理是假设」、shared-worktree.md
  r102「agent 外部值主会话一手核实」。

## models 目录批量增删后必须同步 index.json 清单（2026-10-15）

per-platform `models/*.json` 的增删不自动反映到 `index.json` 的平台 `models`
清单。check-registry 能发现不一致但无修复工具，registry-refresh 轮靠手写一次性
脚本修复。

- 批量增删模型文件后，同轮同步 `index.json` 对应平台的 `models` 数组，再跑
  `node scripts/check-registry.mjs`；两处 diff 属同一任务文件集，盖戳照常
  （`node scripts/bump-registry-last-updated.mjs`）。
- 2026-10-15 尚无 sync 脚本；若日后加了，以脚本为准删本节。

## 大规模落库的分工：写入 agent 不碰共享文件，收尾统一（2026-11-07 审计轮）

1155 文件落库轮（commit 04abaddcb）跑通的分工：两个写入 agent 并行（文件集
不相交，各管自己的平台目录），**不碰 `index.json`、不 commit**——index.json
同步 + 盖戳 + 门禁 + commit 由收尾 agent 在两个写入 agent 都完成后统一做。
全程零 index.lock 事故、零盖戳误伤（既有规则「无参盖戳会盖到别人的脏文件」
自然失效：脏文件集里没有并行的写入 agent）。

规则：写入型 agent 要并行时，共享文件（index.json、盖戳、commit）收口给一个
收尾 agent；写入 agent 的改动面限定各自目录，边界三行写进派发 prompt。这是
shared-worktree.md「凡写主 checkout 的 agent 串行派」在 registry 大轮上的
例外形态：文件集不相交才并行，共享文件永远单点收口。

2026-11-09 数据补齐轮再验证并扩成四段形态：取证 agent 产值文件（`.scratch/<feature>/research/`
下逐平台 json）→ 主会话抽核（24 条回源验证）→ 写入 agent 按文件集并行（platform.json /
models / schema+代码三分）→ 收尾 agent 单点收口（逐档回填、盖戳、门禁、分批 commit）。
两轮均全程零 index.lock、零盖戳误伤。

## schema 拦截 ≠ 数据没丢：按 schema 删键前 grep 读取层（2026-11-07 审计轮）

「新必填字段上线 = 三件套」是加字段方向；本条是反向的删字段 / 禁键对称件。
model.schema.json 禁 `context_tier` 档内 `cache_write` / `cache_write_1h`
（additionalProperties:false 拦截），check-registry 门禁**全绿**——但
`price_resolve.rs:163-171` 的 `overlay_prices` 显式读这两键：按 schema 删掉
gpt-6.1-terra 系档内 cache_write 价（2.5e-6）后，运行时行为从「档内价覆盖
base」静默变成「继承 base 价」。门禁绿 + schema 是契约，挡不住 schema 与
读取层错位导致的数据静默删除。

规则：按 schema 为契约删数据键、或改 schema 禁键之前，先 grep Rust / TS
读取层有无显式读该键的消费者（`grep -rn '<键名>' src-tauri/crates/*/src/`）；
有 → 先拍板（扩 schema 保留，或确认「删除 = 运行时行为变更」并留复活条件），
不静默删。落选的「扩 schema 保留」要带复活条件写进报告遗留（本轮遗留㉑即此
写法）。

## 取证值文件的形状 = 落库目标字段的形状（2026-11-09 数据补齐轮）

aihubmix 14 条图像档价，取证 agent 记的是区间串（「0.1395–0.2479 $/MP（720p/1080p）」），
拍板阶段够用、落库阶段不够——无法按 `unit_tiers` 逐档回填，收尾 agent 被迫重取证一遍。
规则：

1. **派取证任务时把目标 schema 字段的档结构写进派发 prompt**（字段名、单位口径、
   档位维度），值文件按最终可落库的逐档值记录。
2. 判据：值文件每条数据能直接抄进 schema 字段；抄不进 = 取证未完成，不算结票。
3. 区间串 / 摘要串（「$0.03–0.05/MP」级）在拍板阶段合法，落库阶段不算值。

## 批量上量前 grep 全量反断言（2026-11-09 数据补齐轮）

test_registry.rs 躺着「除 4 个 GLM 平台外其余平台不得有 rate_limit_policies」式全量
反断言（单票时期写的），批量给 14 个平台落数据后必红。规则：

1. 「样本扩大」类批量数据轮派单前，先 grep 门禁里的全量反断言
   （`grep -nE '不得|只允许|除.*外|NO_[A-Z_]+' src-tauri/crates/aidog_db/src/test_registry.rs`
   与 `scripts/check-registry.mjs`），命中名单写进派发 prompt。
2. 数据扩大后反断言同轮同步：扩白名单，或改写成正向断言（本轮 commit 85d69bb75
   「限频断言改正向」即此），不等门禁红再修。

## 外部数值落库前量级核对（2026-11-09 数据补齐轮）

aliyun rerank 档官方 TPM 4.98e9 超过 u32 上限 4.29e9，`RateLimitCaps` 首版 u32
装不下，靠 cargo test 抓住后改 u64（`registry.rs:248` 字段注释即此出处）。规则：

外部官方数值（TPM / RPM / 上下文窗口 / 价格）落库前，对目标字段做一次量级核对：
grep struct 定义看整数宽度（u32 上限 4.29e9，u64 约 1.8e19），官方最大值超限 →
先改字段类型再落库，禁截断 / 估算塞入。机械拦截候选：check-registry lint 加数值
范围检查（未做，属代码改动，需要时开票）。

## 审计口径先对门禁源码，摘要数字不算工作清单（2026-11-13 defaults 审计轮）

审计 agent 按文件路径比对 model_list 与 models 目录，报出 63 条「死引用」，3/4 平台
（cline/cline_pass/novita/atlascloud）为误报——真值定义在代码注释里：模型文件名只求
稳定唯一，引用比对的键是文件内 `model_id`（`src-tauri/crates/aidog_db/src/registry.rs:5`
注释 + `scripts/check-registry.mjs:69-101` walkModels 按 model_id 收集）。check-registry
基线 EXIT=0 本身就是「门禁口径下无此问题」的线索，被当成「门禁覆盖不到」跳过了。

1. **审计报大量违规而 check-registry 基线全绿时，先怀疑审计口径与门禁口径不一致**：
   下手前读门禁脚本与真值定义处的代码注释，确认比对键（文件名 / 文件内字段 / index
   清单）与审计脚本一致。同一批结论里真缺口（openrouter 72 条）与误报混排，靠口径
   核对逐平台分拣，不整批采信也不整批否掉。
2. **agent 摘要数字（「约 940 条」）不是工作清单**：两波取证都复现不出（全量盘点脚本
   实际 3944 格）。审计摘要数字只用于拍板阶段叙事；进入执行段前跑全量盘点脚本复现
   一次，对不上以盘点脚本输出为准。
3. **取证 agent 报「取值 N 格」按可落库口径复核**：字符串原文（"128K"）、基数不明的
   数占大半时，实际可落库格数远小于 N——派取证任务时统计口径写成「干净数值（基数
   明确、可直接抄进 schema 字段）」，模糊值单列不计入已取证（「取证值文件的形状 =
   落库目标字段的形状」条的统计侧对称件）。

## 推测的 model_id 禁写入 registry（2026-08-30 用户拍板，2026-11-15 提升自私有 memory）

用户在审计拍板中对「sensenova 推测 id 3 条」选 skip：未经验证的 model_id 不得写入 registry——registry 是计费与路由的真值源，推测 id 会污染两者。

- 建 / 补条目前先取官方文档，或两个独立第三方目录（如 llmgateway + openclaw）确认确切 `model_id`；Meta dev 域对瑞士 IP 地区封锁时走 GitHub 目录双源。验证到位后再写，产出附出处；查不到就跳过并列入遗留，不写估算值。
- 写 registry 数据文件用 Write / Edit，Bash heredoc 批量写会被权限分类器拦。
- 摘要型抓取只用于定位的约束见上文「摘要型抓取工具的输出是假设不是事实」。

## K/M 窗口基数现状：1024 基与 1000 基并存（2026-11-13 现状，2026-11-15 记入）

`context_window` 存量值中 1024 基（如 131072）与新落的 1000 基（如 128000）并存；拍板只管新值，存量未改写。改写存量需单独拍板（见 wayfinder-session.md §16 口径统一类须带「存量怎么办」子题）。来源：`.scratch/memory/2026-11-13.md` 遗留④。
