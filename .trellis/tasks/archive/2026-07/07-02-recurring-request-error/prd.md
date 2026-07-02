# PRD — request 错误链诊断（反复必然失败）

## 现象（用户 2026-07-02）
- 错误请求 `request_id=cb3603ac00044b6297b6cff8b40dfb9e`
- 每次这样的错误前**必**跟一条 `request_id=3e8b13f0c74741b28ee106f32f63f68d`（前驱）
- "似乎必然出现错误"——有规律可复现
- "反反复复很多次"——高频复发，疑似已知模式未根治

## 目标
定位根因（前驱 3e8b13f0 与错误 cb3603ac 的因果链）→ 提供修复方案 → 用户确认后才改。

## 阶段
1. **research（本步）**：定位 SQLite DB 路径 → 查两条 request_id 的 proxy_log 完整记录（status / error / url / model / source_protocol / target_protocol / tokens / cost / duration / 时间戳）→ 扩查历史是否多组"前驱→错误"对（grep 类似模式）→ 读相关代码（proxy 链路 / converter / 重试 / streaming）定位根因
2. **grill 用户确认根因 + 修复方案**（main 据 research 结果 AskUserQuestion）
3. **exec**：用户确认后派 trellis-implement 修复
4. **check + finish**

## 非目标
- 不臆测根因（必须有 DB/代码证据）
- 未经用户确认禁改码

## 验收（research 阶段）
- 两条 request_id 的完整 proxy_log 字段
- 前驱→错误的因果假说（有代码/日志证据，非猜测）
- 历史同类模式数量（"反复多次"量化）
- 修复方案候选（≥1，附代价）

---

## 扩展（2026-07-02，d3a0ce30 第二类根因）

### 新现象
- `request_id=d3a0ce30d53040d9aabcf199f48fe4c2` = **502 / upstream 200 / is_stream=1**
- 上游 GLM 返 200 + SSE content-type 但流无内容秒断 → proxy peek 判空流 → failover 无下家（platform 38 唯一候选）→ 502
- 历史：全库 1301 条同类空流 502，glm 组近 2 天 109 条 502 为主体

### 两类根因（独立）
1. **cb3603ac（hoist）— proxy bug**：漏 stream → hoist 误触 → GLM 1210。修复方案 B/D/E（用户上轮选先 curl 验证）
2. **d3a0ce30（空流）— GLM 间歇 + 单候选**：proxy peek+failover 正确，502 是真实兜底。**非 proxy bug**

### 扩展目标
- 空流 502 先取证（peek_buf 落库）再判方案，禁盲改
- 两类根因独立修复，互不阻塞

### 空流取证盲区
- peek_buf 未持久化 → DB 无 GLM 真实首块内容
- 1301 条空流 502 全是 28 字节占位（response_body）

### 验收（扩展）
- 空流 502 根因有 DB/代码证据（非猜测）
- 取证后给出方案候选（≥1，附代价）

## grill 弱点修复（2026-07-02）

### 本轮 exec 锁定（轴 G）
**仅做空流取证 subtask**（单一交付）。hoist 修复待 curl 数据回来独立轮次，避开同改 forward.rs 冲突。

### 空流取证交付（轴 B 明确）
- 位置：`src-tauri/src/gateway/proxy/forward.rs:497` `retry_on_empty_2xx!("200 but empty/invalid stream")` 前
- 动作：把 `peek_text` 截断 4KB 写入 `log.response_body`（替代 28 字节占位 `"200 but empty/invalid stream"`）
- 截断策略（轴 F）：4KB 上限，超出尾部加 `…[truncated N bytes]`，避免 64KB peek 撑大 DB 行
- 非流式同理（forward.rs:444 `retry_on_empty_2xx!("200 but empty/invalid body")`）— 把 resp_str 截断落库

### 验证（轴 C）
- 改码后 unit test：peek_text 截断格式断言 + 落库字段非空
- `cargo test` 全绿（retry.rs 现有 classify_stream_first 测试不回归）
- `cargo clippy` 零 warning
- 不依赖 GLM 复现（取证被动，下次间歇触发自动落库）

### 失败模式（轴 F）
- peek_text 截断前若含敏感头：proxy 已脱敏（log_settings gate），无新增风险
- response_body 列 TEXT 无硬限，4KB 截断后单行增量可控

## hoist 修复方案锁定（2026-07-02，用户裁定 B）

### 决策
跳 curl（DB 双证据已充分），方案 **B：漏 stream 跳 hoist**。

### 实现
- 位置：`src-tauri/src/gateway/proxy/forward.rs:272`
- 当前：`if !is_stream { hoist_mid_messages_system(&mut req_body); }`
- 改为：`if chat_req.stream == Some(false) { hoist_mid_messages_system(&mut req_body); }`
- 语义：仅「客户端显式 stream=false」才 hoist；漏发 stream（is_none）跳 hoist（视同流式不规整，避免大上下文 body 被 hoist 后 GLM 1210）

### 影响分析
- 本例（cb3603ac 漏 stream）：跳 hoist → body 原样透传 → GLM 接受（与 SUCCESS 676bdbfc stream=true 同构）
- 历史 9 条 no_stream+system 救场案例：若它们是**显式 stream=false** → 仍 hoist（零回归）；若是漏 stream → 跳 hoist（可能回归，但作者注释 9/9 是 no_stream，DB cb3603ac 也是漏 stream 致 is_stream=false，需 DB 复核 9 条的 stream 字段）

### 验收（hoist）
1. forward.rs:272 门控改为 Some(false) 判定
2. cargo test 全绿（现有 hoist 测试不回归 + 新增漏 stream 跳 hoist 测试）
3. cargo clippy 0 新 warning
4. DB 复核：历史 9 条 no_stream+system 救场案例的 stream 字段（显式 false vs 漏发），评估回归风险

## hoist 拆出（2026-07-02 用户裁定）

方案 B DB 复核推翻前提（全库零显式 stream=false + hoist 非因）。详见 research/hoist-reevaluation.md。

**本 task 不修 hoist**。hoist 真因深挖（cb3603ac vs 471c14 顶层 system 块数差异）拆另开 task。

## 本 task 实际交付（finish 范围）
1. 两类根因诊断（hoist 链 + 空流 502 链）完整定位 — research 3 份
2. 空流 502 取证改码（commit 24883d2e，checked ✓）— peek_buf 落库
3. hoist 因果重评估结论沉淀（research/hoist-reevaluation.md）— 推翻 §4.2 假设，指明真因方向
