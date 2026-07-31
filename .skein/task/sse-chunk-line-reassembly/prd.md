# SSE 跨 chunk 行重组 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
- [ ] 修复协议转换分支下，被 chunk 边界切断的 SSE 事件行两侧都不完整而被整段丢弃 —— 客户端收到的响应内容静默缺一段，用户看不出来
- [ ] 复用已有的跨 chunk 行重组 idiom（usage 累计路径早已做了这件事），不新造第二套缓冲机制
- [ ] 用户价值：长响应 / 大 chunk 场景下流式输出不再随机丢句子

## 边界
- [ ] 范围内：协议转换分支（客户端协议 ≠ 上游协议）中逐 chunk 解析上游 SSE 的调用点，给它接上行重组缓冲
- [ ] 范围内：证明「切断行内容不丢」的用例，需覆盖「切在 `data:` 前缀中间」与「切在 JSON 中间」两种切法
- [ ] 范围内：流末尾残留 buffer 的处置（上游断流时 buffer 里还有半行 → 定义行为，禁静默丢）
- [ ] 范围外：**不动 passthrough 分支** —— 它原样 relay 上游字节，不经上游 SSE 解析，无此缺陷
- [ ] 范围外：不动 usage 累计路径（那里已有重组，本 task 是把同一处理补给内容路径）
- [ ] 范围外：不动 UTF-8 字节边界缺陷（归 proxy-hotpath-buffers 的 s2-utf8-fix）—— 两者同文件，根因与修法不同，需串行改
- [ ] 范围外：不改协议转换语义与下发报文格式
- [ ] 约束：token 计数与 est_cost 不得受影响（红线 2）—— usage 走独立路径，改动不得干扰它
- [ ] 约束：首 token 时延不得变差（红线 1）—— 重组只应缓冲不完整的尾行，完整行必须立即下发，禁攒批

## User Stories
极其详尽地穷举, 覆盖功能各方面 (含边界情况):
1. As a 用户, I want 流式响应的每一句都完整送达, so that 我不会在长回答里读到断掉的句子却毫无察觉
2. As a 用非上游原生协议客户端的用户, I want 协议转换不吞内容, so that 换客户端不改变我拿到的答案
3. As a 用户, I want 上游断流时半行残留有确定行为, so that 不会因半行触发 panic 或吞掉最后一句
4. As a 开发者, I want 缓冲有上界, so that 上游发一个永不换行的超长流不会把内存吃光
5. As a 开发者, I want 完整行立即下发, so that 加了缓冲不把流式变成攒批、首 token 时延不退化

## 验收标准
- [x] 存在一条就红的用例证明：SSE 事件行被 chunk 边界切断时，转换分支下发给客户端的内容缺失该行
- [x] 修复后该用例转绿，且内容与不切分时逐字节一致
- [x] 切在 `data:` 前缀中间、切在 JSON 中间两种切法各有覆盖
- [x] 完整行不被缓冲延迟下发（有用例证明，非压测）
- [x] 流结束时 buffer 残留有明确处置且不 panic
- [x] 缓冲有上界，触发上界时行为已定义（与 proxy-hotpath-buffers 的 sse_line_buf 上界处置保持一致口径）
- [x] passthrough 分支未被改动（有 diff 佐证）
- [x] 同一组 mock 请求改动前后 token 数与 est_cost 逐条一致
- [x] cargo clippy --workspace 零 warning、cargo test --workspace 全绿
- [x] 全程只用 mock 平台与分组，记录中可核验无真实上游调用

## Testing Decisions
什么算好测试 (只测外部行为不测实现细节) / 测哪些模块 / codebase 内的同类测试先例:
- [ ] 主接缝复用现成的：`gateway/proxy/test_stream.rs` 已有 usage 路径的同型用例 `feed_sse_usage_reassembles_split_chunk_boundary`（同一条 SSE 原文按字节位切两半分别喂入）。内容路径用例沿用其构造手法，不新建测试基建
- [ ] 只测外部行为：断言口径是「下发给客户端的内容 == 不切分时的内容」，不断言缓冲区内部状态、不断言解析函数被调用几次 —— 换实现不该弄红测试
- [ ] 边界用例按切点分类穷举：切在 `data:` 前缀中、切在 JSON 中、切在多字节字符中（末者与 s2-utf8-fix 有交集，本 task 只需保证行不丢，字符不乱码归那边）
- [ ] 首 token 时延不退化这条：用「喂入一个完整行 + 一个残行，断言完整行立刻出现在输出里」的用例覆盖，比压测便宜且更精确
- [ ] 不写压测：内容正确性由单测覆盖，延迟由上面那条用例覆盖，跑 50 路并发压测对本 task 无额外信息量

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list sse-chunk-line-reassembly`)
