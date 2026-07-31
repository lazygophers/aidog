# SKEIN recall 规则索引 (章节粒度: 一行一条规则)

类目: arch(25), build(23), db(2), domain(39), ops(8), optimization(35), skein(24) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/auto-fix-downgrade-33.md#关联 | arch | 关联 | agent,handler,branch,platform,wire,sse | auto | - | active / →trellis-04 | dashmap-sharding (session 映射) [[trellis-04]] (enum 变体同步) |
| arch/auto-fix-downgrade-33.md#判定：分支 vs wire | arch | 判定：分支 vs wire | agent,handler,branch,platform,wire,sse | auto | - | active | / 特征 / wire 层 / handler 分支 / /------/---------/-------------… |
| arch/auto-fix-downgrade-33.md#反例 | arch | 反例 | agent,handler,branch,platform,wire,sse | auto | - | active | ❌ 新 agent 平台塞 wire 层 → adapter 改到吐血 ❌ 分支内做多候选 retry → agent … |
| arch/auto-fix-downgrade-33.md#触发场景 | arch | 触发场景 | agent,handler,branch,platform,wire,sse | auto | - | active | 新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 sessio… |
| arch/auto-fix-downgrade-33.md#适用 | arch | 适用 | agent,handler,branch,platform,wire,sse | auto | - | active | agent-as-LLM 平台接入（Mock/ClaudeCode/Devin/Factory） |
| arch/auto-fix-downgrade-33.md#陷阱-正解 | arch | 陷阱-正解 | agent,handler,branch,platform,wire,sse | auto | - | active | - **陷阱**: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段、候选切… |
| arch/auto-fix-downgrade-34.md#关联 | arch | 关联 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active / →auto-fix-downgrade-35,cross-db-subquery-handle-selection | [[cross-db-subquery-handle-selection]] (跨库读两阶段) [[auto-fix-d… |
| arch/auto-fix-downgrade-34.md#反例 | arch | 反例 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式） ❌ 只 gr… |
| arch/auto-fix-downgrade-34.md#触发场景 | arch | 触发场景 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 ha… |
| arch/auto-fix-downgrade-34.md#适用 | arch | 适用 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | DB 拆库迁移、表访问点归属审计 |
| arch/auto-fix-downgrade-34.md#陷阱-正解 | arch | 陷阱-正解 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | - **陷阱**: 只查 `call_*_traced` chokepoint → 漏掉 `.write_conn()`… |
| arch/auto-fix-downgrade-34.md#验收命令 | arch | 验收命令 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ```bash # 1. wrapper 形式 grep -rn "call_platform_traced\/call… |
| arch/auto-fix-downgrade-35.md#关联 | arch | 关联 | dedup,空字段,key,数据丢失,合并 | auto | - | active / →shadcn-infra-32 | [[shadcn-infra-32]] (数据清理) |
| arch/auto-fix-downgrade-35.md#反例 | arch | 反例 | dedup,空字段,key,数据丢失,合并 | auto | - | active | ❌ (provider.source_segment, provider.base_url) 其中 base_url 全… |
| arch/auto-fix-downgrade-35.md#正解 | arch | 正解 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup key 选择优先级： 1. **业务唯一键**(user_id / email / name) — 最稳 2… |
| arch/auto-fix-downgrade-35.md#测试 | arch | 测试 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 构造 N 个对象(该字段全空但其余不同)，dedup 后必须保留 N 个(非合并为 1)。 |
| arch/auto-fix-downgrade-35.md#触发场景 | arch | 触发场景 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy ke… |
| arch/auto-fix-downgrade-35.md#适用 | arch | 适用 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup / 去重 / 合并逻辑、数据导入解析 |
| arch/auto-fix-downgrade-35.md#陷阱 | arch | 陷阱 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 字段设计为空(待后续回填 / 占位)但被用作 dedup key → N 个对象共享同一空值 → HashSet 全撞 … |
| arch/auto-fix-downgrade-38.md#MUST 流程 | arch | MUST 流程 | enum,serde,db,migration,rust,panic | auto | - | active | 1. 写 migration: DELETE FROM table WHERE enum_column = 'delet… |
| arch/auto-fix-downgrade-38.md#关联 | arch | 关联 | enum,serde,db,migration,rust,panic | auto | - | active / →shadcn-infra-32,trellis-04 | [[shadcn-infra-32]] (locale 清理) [[trellis-04]] (TS ↔ Rust en… |
| arch/auto-fix-downgrade-38.md#反例 | arch | 反例 | enum,serde,db,migration,rust,panic | auto | - | active | ❌ 先删代码再 migration → migration 期间所有访问 panic ❌ 只改 TS 未改 Rust e… |
| arch/auto-fix-downgrade-38.md#硬约束 | arch | 硬约束 | enum,serde,db,migration,rust,panic | auto | - | active | **删 serde 落库的 enum 变体前必须先 migration DELETE DB 旧值**，否则代码中 `fr… |
| arch/auto-fix-downgrade-38.md#触发场景 | arch | 触发场景 | enum,serde,db,migration,rust,panic | auto | - | active | 删 serde 落库的 enum 变体时。 |
| arch/auto-fix-downgrade-38.md#适用 | arch | 适用 | enum,serde,db,migration,rust,panic | auto | - | active | serde enum 变体删除、DB schema enum 迁移、前后端 enum 同步 |
| build/rule-06.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | converter 双向转（source→wire 请求 + wire→source 响应）与 endpoint 选择解… |
| build/rule-06.md#关联 | build | 关联 | - | auto | - | active | - |
| build/rule-06.md#反例 | build | 反例 | - | auto | - | active | - ❌ 误判：endpoint 层限制只许选同协议 → converter 能力已就绪，endpoint 无需自我限制 … |
| build/rule-06.md#案例 | build | 案例 | - | auto | - | active / →rule-07,rule-55 | - endpoint-cross-protocol-fallback task：converter 5×5 已就绪，en… |
| build/rule-06.md#适用 | build | 适用 | - | auto | - | active | - 所有新增 wire protocol 的变更 - endpoint 跨协议回退扩展 - converter 双向转换… |
| build/rule-07.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | is_valid_wire_protocol gate 触发（502）说明 endpoint 选择失败（matched_… |
| build/rule-07.md#关联 | build | 关联 | - | auto | - | active | - |
| build/rule-07.md#反例 | build | 反例 | - | auto | - | active | - 只修白名单而未修 select → 新协议仍 502（根因未除） - 误判为 endpoint 配置缺 protoc… |
| build/rule-07.md#案例 | build | 案例 | - | auto | - | active / →rule-05,rule-54 | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| build/rule-07.md#适用 | build | 适用 | - | auto | - | active | - 所有 502 route fail 场景 - is_valid_wire_protocol gate 触发 - en… |
| build/rule-61.md#关联 | build | 关联 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active / →rule-63 | [[rule-63]] |
| build/rule-61.md#案例 | build | 案例 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - arch-deepen-2：迁移函数后 clippy 无新输出，touch 才触发重编检查 |
| build/rule-61.md#正解 | build | 正解 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改源文件后跑 clippy 前，先 `touch` 该文件强制重编： ```bash touch src-tauri/… |
| build/rule-61.md#触发场景 | build | 触发场景 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改后再跑 `cargo clippy` 判断 warning 数时。 |
| build/rule-61.md#适用 | build | 适用 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - 验证 clippy 改动效果 - 高频编译场景 - 持续集成前检查 |
| build/rule-61.md#陷阱 | build | 陷阱 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 同命令第二次跑输出为空（命中编译缓存），易误判「0 warning」实际仍有。 |
| build/rule-63.md#关联 | build | 关联 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active / →rule-61 | [[rule-61]] |
| build/rule-63.md#案例 | build | 案例 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - arch-deepen-2 c3-commands batch 3：commands_tray/commands_s… |
| build/rule-63.md#检查 | build | 检查 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | ```bash # 检查迁移后是否仍能编译通过 cargo build -p aidog_core  # 应无 env!… |
| build/rule-63.md#正解 | build | 正解 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 迁移代码到新 crate 后，给**新 crate 补等价的 build.rs**，重新定义环境变量。 |
| build/rule-63.md#触发场景 | build | 触发场景 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 用 `env!("XXX")` 的代码从一个 crate 迁移到另一个 crate 时。 |
| build/rule-63.md#适用 | build | 适用 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - 任何用 env!() 的代码跨 crate 迁移 - workspace 多 crate 场景 - build.rs… |
| build/rule-63.md#陷阱 | build | 陷阱 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | `cargo:rustc-env=` 在 build.rs 中定义的环境变量**只对定义它的 crate 生效**，跨 … |
| db/sqlite-cache-residency-probe-method.md#SQLite 页缓存常驻量的直接探针方法 | db | SQLite 页缓存常驻量的直接探针方法 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active | - |
| db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针 | db | 页缓存常驻量探针 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active / →measure-window-exclusive-env,sqlite-cache-measurement-traps,sqlite-read-cache-config | ### 方法  用 `heap --addresses 'malloc[5k]'` 的 5KB 块数作为 SQLite … |
| domain/bundled-models-fallback.md#关联 | domain | 关联 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active / →rule-66,time-tiers-apply-idiom | [[time-tiers-apply-idiom]] [[rule-66]] |
| domain/bundled-models-fallback.md#反例 | domain | 反例 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | ```rust // ❌ 启动 seed （版本冲突、IO 阻塞） #[init] async fn on_startu… |
| domain/bundled-models-fallback.md#触发场景 | domain | 触发场景 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | 只读配置数据（models.json 价格表、platform-presets.json）需在 DB 为空或未同步时兜底… |
| domain/bundled-models-fallback.md#路径计算 | domain | 路径计算 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | `include_str!` 相对路径**从当前 .rs 文件出发**（不是 Cargo.toml 所在目录）： - `… |
| domain/bundled-models-fallback.md#适用 | domain | 适用 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | - 只读配置（定价表、平台预设、常量列表） - 冷启动不依赖 RPC / 版本同步 - DB 可能暂时为空、滞后同步的场… |
| domain/bundled-models-fallback.md#陷阱 ❌ vs 正解 ✅ | domain | 陷阱 ❌ vs 正解 ✅ | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | **陷阱1**：启动时 seed DB - ❌ `fn seed_models()` 启动期间 INSERT bundl… |
| domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查 | domain | PRD 验收标准与约束互容性检查 | PRD,acceptance,constraint,compatibility,plan | auto | - | active | - |
| domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查 | domain | PRD 验收标准与约束互容性检查 | PRD,acceptance,constraint,compatibility,plan | auto | - | active / →mock-platform-bypasses-forward-pipeline | ### 触发场景  task plan 阶段定下验收标准（如「phys_footprint 下降」）和技术约束（如「仅用… |
| domain/rule-51.md#关联 | domain | 关联 | protocol endpoint converter platform_type | auto | - | active / →rule-05,rule-53 | [[rule-05]] [[rule-53]] |
| domain/rule-51.md#关键不变量 | domain | 关键不变量 | protocol endpoint converter platform_type | auto | - | active | endpoint 协议 = converter 模块支持的格式（convert_request + parse_sse） |
| domain/rule-51.md#反例 | domain | 反例 | protocol endpoint converter platform_type | auto | - | active | - 把 glm/kimi/sensenova 当作 endpoint 协议 → 转换时 panic/未实现 - 误以为有… |
| domain/rule-51.md#案例 | domain | 案例 | protocol endpoint converter platform_type | auto | - | active | - converter-reasoning-content task：5 协议是 N×N 互转矩阵的锚点 - glm/k… |
| domain/rule-51.md#触发场景 | domain | 触发场景 | protocol endpoint converter platform_type | auto | - | active | - endpoint 协议层只 5 种（anthropic/openai/openai_responses/openai… |
| domain/rule-51.md#适用 | domain | 适用 | protocol endpoint converter platform_type | auto | - | active | - converter 模块扩展（新增 wire protocol） - N×N 协议互转设计（真值源） - 平台接入时… |
| domain/rule-51.md#陷阱-正解 | domain | 陷阱-正解 | protocol endpoint converter platform_type | auto | - | active | - ❌ 混淆：以为所有 Protocol 枚举值都是「协议」 - ✅ 区分：仅 5 个可作为 endpoint 协议参与… |
| domain/rule-52.md#关联 | domain | 关联 | reasoning thinking anthropic signature converter | auto | - | active / →rule-52,rule-53 | [[rule-53]] [[rule-52]] |
| domain/rule-52.md#决策背景 | domain | 决策背景 | reasoning thinking anthropic signature converter | auto | - | active | - TrueFoundry/LiteLLM #8927 调研佐证：第三方 reasoning 无 signature -… |
| domain/rule-52.md#反例 | domain | 反例 | reasoning thinking anthropic signature converter | auto | - | active | - 强行出 thinking 块 → CC 多轮交互时 400/empty or malformed - 空 reaso… |
| domain/rule-52.md#实现 | domain | 实现 | reasoning thinking anthropic signature converter | auto | - | active | - openai/response.rs:13：reasoning_content 被忽略，不影响 content/to… |
| domain/rule-52.md#触发场景 | domain | 触发场景 | reasoning thinking anthropic signature converter | auto | - | active | - 第三方（deepseek/sensenova/glm）reasoning_content 纯文本无 signatur… |
| domain/rule-52.md#适用 | domain | 适用 | reasoning thinking anthropic signature converter | auto | - | active | - 所有第三方 → anthropic 跨协议转换 - reasoning 扩展字段处理（未来第三方新增非标准字段） |
| domain/rule-52.md#陷阱-正解 | domain | 陷阱-正解 | reasoning thinking anthropic signature converter | auto | - | active | - ❌ 方案 A（标准协议）：出 thinking 块 → signature 风险 - ✅ 方案 B（务实方案）：re… |
| domain/rule-53.md#关联 | domain | 关联 | converter NonStreamResponse parse render protocol | auto | - | active / →rule-52,rule-54 | [[rule-52]] [[rule-54]] |
| domain/rule-53.md#反例 | domain | 反例 | converter NonStreamResponse parse render protocol | auto | - | active | - 点对点设计：新增协议时改 N 处 → O(N²) 维护成本 - 无中间归一：无法跨协议组合（如 openai→gem… |
| domain/rule-53.md#案例 | domain | 案例 | converter NonStreamResponse parse render protocol | auto | - | active | - converter-reasoning-content：5×5 互转矩阵用 NonStreamResponse - … |
| domain/rule-53.md#覆盖范围 | domain | 覆盖范围 | converter NonStreamResponse parse render protocol | auto | - | active | - 当前：openai → anthropic 真转换（convert_response） - 其余组合：回退透传（re… |
| domain/rule-53.md#触发场景 | domain | 触发场景 | converter NonStreamResponse parse render protocol | auto | - | active | - N 协议互转设计选择：内部归一（路 A）vs 点对点（路 B） - O(N) parse + render vs O… |
| domain/rule-53.md#设计决策 | domain | 设计决策 | converter NonStreamResponse parse render protocol | auto | - | active | 路 A（内部归一）： 1. 上游响应 → parse → NonStreamResponse（归一） 2. NonStr… |
| domain/rule-53.md#适用 | domain | 适用 | converter NonStreamResponse parse render protocol | auto | - | active | - converter 模块扩展（新增协议/转换组合） - N×N 互转矩阵设计（converter-reasoning… |
| domain/rule-53.md#陷阱-正解 | domain | 陷阱-正解 | converter NonStreamResponse parse render protocol | auto | - | active | - ❌ 路 B：点对点 N×N 函数 → 新增协议需加 N 个函数 - ✅ 路A：NonStreamResponse 作… |
| domain/rule-55.md#关联 | domain | 关联 | - | auto | - | active | - |
| domain/rule-55.md#分层不变量 | domain | 分层不变量 | - | auto | - | active | - 回退仅在普通平台生效：普通平台允许跨协议回退（降低 502 率） - coding 平台永不落非 coding：步骤… |
| domain/rule-55.md#反例 | domain | 反例 | - | auto | - | active | - ❌ 误判：coding 平台也跨协议回退 → 破坏 401 防护 - ❌ 误修：只修普通平台回退，忘了 coding… |
| domain/rule-55.md#案例 | domain | 案例 | - | auto | - | active / →rule-06,rule-07 | - endpoint-cross-protocol-fallback task：普通平台步骤 4 泛化（同协议 > op… |
| domain/rule-55.md#触发场景 | domain | 触发场景 | - | auto | - | active | - 普通平台 endpoint 选择时协议不匹配（如 anthropic 入站 + 仅 openai endpoint）… |
| domain/rule-55.md#适用 | domain | 适用 | - | auto | - | active | - endpoint.rs select_endpoint_for_protocol 修改 - 跨协议回退逻辑扩展 - … |
| domain/rule-55.md#陷阱-正解 | domain | 陷阱-正解 | - | auto | - | active | **陷阱**: 误以为跨协议回退可应用于所有平台类型，或回退优先级混乱。  **正解**: 普通平台步骤 4 泛化为三级… |
| domain/task-decomposition-coverage-check.md#task 分解 → subtask DAG 覆盖检查 | domain | task 分解 → subtask DAG 覆盖检查 | subtask,PRD,coverage,decomposition,plan | auto | - | active | ### 触发场景  task 分解拆 subtask DAG 时。某次 task 有 7 个明确的目标（PRD），但原拆… |
| domain/task-decomposition-coverage-check.md#task 分解 → subtask DAG 覆盖检查 | domain | task 分解 → subtask DAG 覆盖检查 | subtask,PRD,coverage,decomposition,plan | auto | - | active | - |
| ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单 | ops | 空闲期唤醒源 6 分类清单 | wakeup,timers,scheduler,sources,profiling,static-analysis,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env | 空闲期 CPU 唤醒源分 6 类，静态 rg 检索无遗漏（src-tauri + src）。  / 分类 / 频率 / … |
| ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推 | ops | 日志队列 capacity 定值方法：从采样均值反推 | logging,queue,capacity,tuning,p99 | auto | - | active | - |
| ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推 | ops | 日志队列 capacity 定值方法：从采样均值反推 | logging,queue,capacity,tuning,p99 | auto | - | active / →hot-path-buffers | ### 触发场景  应用日志流量稳定后需定值日志队列 capacity（mpsc channel），既要不丢日志（缓冲充… |
| ops/stack-attribution-profiling-methodology.md#栈归因用法 | ops | 栈归因用法 | profiling,stack-trace,attribution,instruments,xctrace,methodology,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env,webkit-jit-warmup-trap | **定理**：静态检索定时器只能估出量级（因周期、触发条件、执行成本都是猜），无法判断是否真在稳态 CPU 占比中命中。… |
| ops/test-data-isolation-constraint.md#性能测试数据隔离约束 | ops | 性能测试数据隔离约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | - |
| ops/test-data-isolation-constraint.md#测试数据隔离硬约束 | ops | 测试数据隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | 性能量测或功能验证时需要用特定数据库（如缩小库、污染库等）。  ### 硬约束  - **禁移动/重命名用户的真实库文件… |
| ops/test-data-isolation-constraint.md#量测脚本 HOME 环境隔离硬约束 | ops | 量测脚本 HOME 环境隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | - |
| ops/test-data-isolation-constraint.md#量测脚本 HOME 环境隔离硬约束 | ops | 量测脚本 HOME 环境隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active / →"$HOME" == "$HOME_REAL",tmp | ### 扩展约束：禁污染用户真实数据目录  前置约束禁止移动用户真实库文件，但仍需隔离 **整个数据目录**（不仅是单个… |
| optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据 | optimization | 空闲 CPU 基线数据 | baseline,measurement,xctrace,process,webkit,profiling,cpu | auto | - | active / →idle-wakeup-sources-inventory,measure-window-exclusive-env,webkit-jit-warmup-trap | 基于 xctrace Time Profiler 实测（2026-07-31，30s 采样窗口）。四进程占比： - **… |
| optimization/idle-cpu-stack-sampling.md#反例（错误模式） | optimization | 反例（错误模式） | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 仅 grep 定时器列表 / grep 列表 + `sample`… |
| optimization/idle-cpu-stack-sampling.md#案例 | optimization | 案例 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | grep 找到 5 个定时器，工作量推算应占 CPU 1-1.5%。但实测 3.0% 稳态，缺口 1.5% 无法追溯。用… |
| optimization/idle-cpu-stack-sampling.md#空闲 CPU 归因必须靠栈采样 | optimization | 空闲 CPU 归因必须靠栈采样 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - |
| optimization/idle-cpu-stack-sampling.md#触发场景 | optimization | 触发场景 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | 性能分析中发现应用稳态 CPU 占用 3.0%，但静态代码检索只能找到 60s×1 + 300s×1 + 24h×3 共… |
| optimization/idle-cpu-stack-sampling.md#适用 | optimization | 适用 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - 稳态 CPU 3% 以上但代码检索无法解释的场景 - 长时间后台进程 CPU 诊断 - 定时任务链效应分析（A 定时… |
| optimization/idle-cpu-stack-sampling.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | ❌ **陷阱**：仅用静态代码检索（grep）列举定时器  ```bash # 搜索所有定时器 grep -r "set… |
| optimization/measure-footprint-pid-matching.md#measure.sh 同 label 跨 run 文件混淆 | optimization | measure.sh 同 label 跨 run 文件混淆 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | - |
| optimization/measure-footprint-pid-matching.md#反例（错误模式） | optimization | 反例（错误模式） | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / `glob footprint-${label}-*-*.txt`… |
| optimization/measure-footprint-pid-matching.md#案例 | optimization | 案例 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | 实测显示某指标（graphics 等）跳到物理上限 2 倍，对比 size-curve-raw.txt 确认该档 TOT… |
| optimization/measure-footprint-pid-matching.md#触发场景 | optimization | 触发场景 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | 性能量测脚本 `measure.sh` 按 label 重复运行（如多轮对比测试）时，旧 run 的 footprint… |
| optimization/measure-footprint-pid-matching.md#适用 | optimization | 适用 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | - `measure.sh` 同 label 重复运行（对比 baseline 常见） - 任何大块临时数据依赖文件名去… |
| optimization/measure-footprint-pid-matching.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | ❌ **陷阱**：glob 匹配所有同 label 的 footprint 文件，不区分 run  ```bash # … |
| optimization/measure-window-exclusive-env.md#环境互斥约束 | optimization | 环境互斥约束 | profiling,performance,measurement,environment,cargo,yarn,exclusive | auto | - | active / →idle-cpu-baseline-xctrace,webkit-jit-warmup-trap | Profiling（采样、trace 录制）与后台编译（cargo/yarn build）占用机器资源竞争。同步触发导致… |
| optimization/measure-window-multi-probe.md#判据 | optimization | 判据 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | CPU/内存稳态采样，只在采样前打一次前台确证（如 `lsappinfo front`）不够——采样窗口内应用可能中途失… |
| optimization/measure-window-multi-probe.md#案例 | optimization | 案例 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | `.scratch/perf-200mb/assets/results/cpu-s7-after-run3.txt`（8… |
| optimization/measure-window-multi-probe.md#正解 | optimization | 正解 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | 稳态采样窗口内必须**多点探针**（如每 15s 一次），全程确证前台/目标态未漂移，而非仅窗口前一次性确证。另需注意 … |
| optimization/measure-window-multi-probe.md#适用 | optimization | 适用 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | CPU/内存稳态性能采样，尤其涉及应用前台/背景态切换、GUI 应用量测场景。 |
| optimization/measure-window-multi-probe.md#量测 regime 自证必须窗口内多点探针 | optimization | 量测 regime 自证必须窗口内多点探针 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | - |
| optimization/measure-window-multi-probe.md#陷阱 | optimization | 陷阱 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | 实测：run3 采样前确证前台，但 60s 窗口末端已漂回终端，读数被稀释成 8.2%（前台+背景混合值）。同实例钉死前… |
| optimization/memory-measure-background.md#内存量测走纯背景态口径 | optimization | 内存量测走纯背景态口径 | memory,measure,background,activate,settle,foreground | auto | - | active | - |
| optimization/memory-measure-background.md#反例（错误模式） | optimization | 反例（错误模式） | memory,measure,background,activate,settle,foreground | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 内存+CPU 都用 activate + settle / 内存用… |
| optimization/memory-measure-background.md#案例 | optimization | 案例 | memory,measure,background,activate,settle,foreground | auto | - | active | run1/run2 内存量测全 4 档失效，对比日志发现 activate 后应用被 Finder 抢走。改为背景态启动… |
| optimization/memory-measure-background.md#触发场景 | optimization | 触发场景 | memory,measure,background,activate,settle,foreground | auto | - | active | 内存占用量测时，采用 CPU 量测的 `activate + settle` 两段试图通过前台激活 + 等待稳定来排除用… |
| optimization/memory-measure-background.md#适用 | optimization | 适用 | memory,measure,background,activate,settle,foreground | auto | - | active | - Tauri / Electron 应用内存占用基准量测 - 长时间后台内存监控（避免前台抢占） - 交叉对比前台/后… |
| optimization/memory-measure-background.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | memory,measure,background,activate,settle,foreground | auto | - | active | ❌ **陷阱**：内存量测复用 CPU 量测的 activate + settle 口径  ```bash # CPU … |
| optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测三大陷阱 | optimization | SQLite 页缓存量测三大陷阱 | sqlite,measurement,profiling,memory,phys_footprint,noise | auto | - | active | 实测 SQLite 默认 cache_size 与各档位定值方案时踩过的坑。  ### 陷阱一：内存计量工具选错  **… |
| optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测陷阱 | optimization | SQLite 页缓存量测陷阱 | sqlite,measurement,profiling,memory,phys_footprint,noise | auto | - | active | - |
| optimization/webkit-jit-warmup-trap.md#WebContent JSC JIT 热身陷阱 | optimization | WebContent JSC JIT 热身陷阱 | webkit,jsc,jit,warmup,profiling,sampling,trap,cpu | auto | - | active / →idle-cpu-baseline-xctrace | WebContent 进程中 JSC JIT 热身阶段（启动后数分钟）vs 稳定态（运行 45+ 分钟）的 CPU 占比… |
| optimization/webkit-xpc-helper-process-bounds.md#反例（错误模式） | optimization | 反例（错误模式） | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 用 ppid 反查归属 / 编制硬闸：期望 WebContent×… |
| optimization/webkit-xpc-helper-process-bounds.md#案例 | optimization | 案例 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | 多轮量测发现某档进程数突增（期望 4，实际 6-8），发现混入了飞书/Safari 的 WebKit helper。改用… |
| optimization/webkit-xpc-helper-process-bounds.md#触发场景 | optimization | 触发场景 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | WebKit 内嵌浏览器在 Tauri 应用中运行时，`ppid`（父进程 ID）恒为 1（launchd），`ps -… |
| optimization/webkit-xpc-helper-process-bounds.md#进程编制核验硬闸替代动态反查 | optimization | 进程编制核验硬闸替代动态反查 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | - |
| optimization/webkit-xpc-helper-process-bounds.md#适用 | optimization | 适用 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | - Tauri / Electron 等嵌入 WebKit 的桌面应用性能量测 - 多窗口场景排查进程组织 - 交叉应用… |
| optimization/webkit-xpc-helper-process-bounds.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | ❌ **陷阱**：用 ppid / ps args / procinfo 反查进程归属  ```bash # ppid … |
| skein/coding-plan-utilization-calib-fix-27.md#task 查重: 同模块非重复, 先看 PRD 边界互引 | skein | task 查重: 同模块非重复, 先看 PRD 边界互引 | skein,dedup,task-boundary,prd | auto | - | active | dedup/查重判定重叠维度前, MUST 先看两 task 的 PRD 边界条款是否已显式互相引用切割 (如双向标注对… |
| skein/decision-documentation.md#实测推翻设计假设时的处理范式（留痕+不硬凑） | skein | 实测推翻设计假设时的处理范式（留痕+不硬凑） | planning,execution,hypothesis-testing,decision-logging,design-vs-reality | auto | - | active | 当 task 执行过程中发现「planning 写的验收文本与 exec 实测结果矛盾」时，按以下范式处理：  **模式… |
| skein/parallel-subtask-prop-contract.md#3.5 并行契约（S2/S3 同时跑，锁死边界） | skein | 3.5 并行契约（S2/S3 同时跑，锁死边界） | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ### 文件划分（禁止跨界改动） - **S2 负责**：`PlatformEditForm.tsx`（给 Models… |
| skein/parallel-subtask-prop-contract.md#关联 | skein | 关联 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active / →dirty-float-hour-normalization,form-level-tz-state-sharing | [[dirty-float-hour-normalization]] · [[form-level-tz-state-s… |
| skein/parallel-subtask-prop-contract.md#反例 / 常见错误 | skein | 反例 / 常见错误 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | / 错误                            / 为什么错                      … |
| skein/parallel-subtask-prop-contract.md#案例 | skein | 案例 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | - time-models-timezone task (design.md §3.5) — S2/S3 并行，prop… |
| skein/parallel-subtask-prop-contract.md#正解：planning 阶段锁定 prop 契约（硬约束，关键） | skein | 正解：planning 阶段锁定 prop 契约（硬约束，关键） | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ### MUST 在 design.md 明确标记文件分工  ```markdown |
| skein/parallel-subtask-prop-contract.md#落地 checklist | skein | 落地 checklist | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ```bash # 集成前逐项验证 # 1. 文件分工 git log --oneline time-models-ti… |
| skein/parallel-subtask-prop-contract.md#触发场景 | skein | 触发场景 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | 两个或多个 subtask 需要同时改造同一组件树中的多个文件（例如 S2 改 `PlatformEditForm.ts… |
| skein/parallel-subtask-prop-contract.md#适用 | skein | 适用 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | - 并行多个 subtask 改造同一组件树的不同部分 - 跨团队开发中需要接口预协商的场景（prop 签名即"API … |
| skein/parallel-subtask-prop-contract.md#陷阱：未锁定 prop 契约导致运行时 BAD_REQUEST / TS 类型错 | skein | 陷阱：未锁定 prop 契约导致运行时 BAD_REQUEST / TS 类型错 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | > S2 和 S3 分别并行改造组件树的不同部分，但 S2 声明的 prop 接收端签名（如 `ModelsMatrix… |
| skein/parallel-subtask-prop-contract.md#验证场景 | skein | 验证场景 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | 1. S2 提交：`usePlatformForm.ts` 新增 `windowsTz` state，design 文档… |
| skein/subagent-hook-scope.md#subagent hook 禁写主仓报告文件 | skein | subagent hook 禁写主仓报告文件 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | - |
| skein/subagent-hook-scope.md#反例（错误模式） | skein | 反例（错误模式） | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 派时未限制产物路径 / 明确 `工作目录: research/`，… |
| skein/subagent-hook-scope.md#案例 | skein | 案例 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | 派 researcher 调研某模块时，它直接在主仓根目录产生 `findings.md` 和 `recommendat… |
| skein/subagent-hook-scope.md#触发场景 | skein | 触发场景 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | 派 researcher / workflow subagent 时，如果在 hook（如 subagent 中断返回）… |
| skein/subagent-hook-scope.md#适用 | skein | 适用 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | - 派遣 researcher / workflow / skill 等 data-producing subagent… |
| skein/subagent-hook-scope.md#陷阱 & 正解 | skein | 陷阱 & 正解 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | ❌ **陷阱**：派 researcher 时不限制产物路径，允许 hook 中写报告文件  ```python # 派… |
| skein/subagent-sendmessage.md#agent 零回传真因 = 未调 SendMessage | skein | agent 零回传真因 = 未调 SendMessage | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | - |
| skein/subagent-sendmessage.md#反例（错误模式） | skein | 反例（错误模式） | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 仅 print/echo 文本输出 / print 文本 + 调 … |
| skein/subagent-sendmessage.md#案例 | skein | 案例 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | 既有约定记录 3 个实例系统性不回传。根因是这些 subagent 仅写 stdout，未调 SendMessage。修… |
| skein/subagent-sendmessage.md#触发场景 | skein | 触发场景 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | 派 subagent（如 researcher / checker 等）时，应答端只写 stdout 文本输出，未调用 … |
| skein/subagent-sendmessage.md#适用 | skein | 适用 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | - 所有派 subagent 的场景（researcher / checker / workflow / skill） … |
| skein/subagent-sendmessage.md#陷阱 & 正解 | skein | 陷阱 & 正解 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | ❌ **陷阱**：仅写文本输出，不调 SendMessage 工具  ```python # subagent 应答端 … |
