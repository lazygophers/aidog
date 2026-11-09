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
