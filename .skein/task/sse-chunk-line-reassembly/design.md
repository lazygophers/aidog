# SSE 跨 chunk 行重组 — 详细设计

## 现状

流式转发在 `gateway/proxy/finish.rs` 的 chunk 循环里分两支：

| 分支 | 处理 | 有无本缺陷 |
|---|---|---|
| passthrough（客户端协议 == 上游协议） | 原样 relay 上游字节，不解析 | **无** |
| 协议转换（客户端协议 ≠ 上游协议） | 逐 chunk 调 `adapter::parse_upstream_sse(&text, ...)` 再按客户端协议重新渲染 | **有** |

`parse_upstream_sse` 按 `data: ` 分帧解析。**它对单个 chunk 是无状态的** —— 一条 SSE 事件行若被
chunk 边界切成两半，前半在 chunk A 里没有结束换行、后半在 chunk B 里没有 `data:` 前缀，
两边都不构成合法帧，**双双被丢弃**。客户端就少了这一段内容，且没有任何错误信号。

## 关键：usage 路径早就修过这个，内容路径漏了

同一个循环里 `feed_sse_usage` 的两处调用点，注释白纸黑字写着：

> 跨 chunk 行重组后累计 usage（逐 chunk `.lines()` 会因 `data:` 行被切断而丢 usage）

重组逻辑在 `gateway/proxy/stream.rs` 的 `feed_sse_usage`，用一个 `sse_line_buf: Mutex<String>`
暂存不完整的尾行，下个 chunk 拼上去再切行。

**所以本 task 不是发明新机制，是把已经存在的处理补给漏掉的那条路径。** 这也决定了修法的形状：
复用同一 idiom（尾行缓冲），而不是另起炉灶。

推论：token 计数与 est_cost **本来就是对的**（走 usage 路径），本 task 的红线 2 风险只在于
「改动别把 usage 路径碰坏」，不是「要把它修对」。

## 修法

在转换分支给内容路径接上同型的尾行缓冲：完整行立刻交给 `parse_upstream_sse` 并下发，
不完整的尾行留在 buffer 里等下个 chunk。

三个必须明确的点（不明确就是埋雷）：

1. **完整行必须立即下发** —— 缓冲只能留尾巴，不能攒批。攒批 = 首 token 时延退化 = 压红线 1。
   这条要有用例钉住，不能只靠 code review 看着像。
2. **流末残留** —— 上游断流时 buffer 里可能还有半行。半行本就不是合法帧，解析它只会失败；
   但**静默丢**和**记一条 warn 再丢**是两种行为，要选一个并写进代码注释。倾向后者：
   静默丢正是当前这个 bug 难被发现的原因。
3. **上界** —— 上游若永不发换行，buffer 会无限涨。proxy-hotpath-buffers 的 `s3-push-cap`
   验收里也有一条「`sse_line_buf` remainder 有上界」，说的是同一隐患的 usage 侧。
   两边口径必须一致，否则一个截断一个不截断，行为割裂。

## 与 proxy-hotpath-buffers 的关系（必须串行）

三处交集，改同一批文件：

| 那边的 subtask | 交集 | 处理 |
|---|---|---|
| `s1-utf8-repro` | 已完成，用例落在 `test_stream.rs` | 本 task 用例加在同文件，注意别冲突 |
| `s2-utf8-fix` | 同在转换分支的 chunk 循环 | **串行**：谁先做完谁先合，后做的 rebase |
| `s3-push-cap` | `sse_line_buf` 上界 | 上界口径二选一处实现，另一处引用，禁各写一份 |

排序：`s2-utf8-fix` 先（字节层），本 task 后（行层）—— 字节先正确，行重组才有意义；
反过来则重组出的行里可能已经带着 U+FFFD。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 把转换分支也改成 passthrough | 那是删功能，协议转换本就是产品能力 |
| 让 `parse_upstream_sse` 内部持状态 | 它是 adapter 侧的纯解析函数，被多处调用；塞进可变状态会污染所有调用方，且并发下要加锁 |
| 攒够 N 字节再解析 | 首 token 时延直接退化，压红线 1 |

## 测试接缝 (seam)

复用 `gateway/proxy/test_stream.rs` —— 现有接缝，最高接缝（断言外部可见的下发内容），且只加这一处。

那里已有 usage 侧的同型用例 `feed_sse_usage_reassembles_split_chunk_boundary`：取一条真实 SSE
原文，按字节位切两半分别喂入，断言结果与不切分一致。内容侧用例照抄这个手法，只换断言对象
（下发内容而非 usage）。

切点分三类各一条：`data:` 前缀中间 / JSON 中间 / 多字节字符中间。第三类与 s2-utf8-fix 交集，
本 task 只断言「行不丢」，字符是否乱码归那边 —— 两个 task 各测各的维度，别互相绑死。

「完整行不延迟」那条单独一个用例：喂入「一个完整行 + 一个残行」，断言完整行**立刻**出现在输出里。
比压测便宜、且精确指向要防的退化。
