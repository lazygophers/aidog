# mock 压测能力补全 — PRD (主入口)

## 目标
- [ ] 补齐 mock 平台作为压测基准所缺的能力，让 50 路并发流式压测的数据不被 mock 自身的实现瑕疵污染。
- [ ] 消除 proxy/mock.rs:96-104 的每请求 platform 写连接 —— budgets 为空时短路，不进 tokio-rusqlite 单后台线程。这同时是真实转发路径的优化（非 mock 专属）。
- [ ] 把 delay_ms 拆成 ttft_ms（首包延迟）与 inter_chunk_ms（chunk 间隔）两个独立旋钮，现状二者共用同一值（proxy/mock.rs:22 与 :113-118）。
- [ ] 加 error_rate 概率化错误注入，现状 error_mode 是确定性单值（proxy/mock.rs:32），做不到「5% 请求 429」。
- [ ] 同步补 src/domains/platforms/MockConfigEditor.tsx 的对应字段与 8 语言 i18n key，用户明令「完善 MOCK 时要同时完善对应的前端展示」。
## 边界
- 只动 mock 协议路径与 manual_budget 的空短路，禁改任何真实上游协议的转发语义。
- manual_budget 短路必须是纯粹的「无配额则不进写连接」，配额存在时行为逐字不变 —— 这是计费路径，压红线 2。
- 新增字段全部可选，缺省值必须让现有 mock 平台配置行为零变化（delay_ms 保留为兼容入口：ttft_ms/inter_chunk_ms 未设时回落 delay_ms）。
- error_rate 只做概率注入，不新增 error_mode 枚举值。
- 禁动 adapter/mock/response.rs 与 stream.rs 的协议格式化逻辑（5 种 source_protocol 的响应形状）。
- 不做「无并发级观测埋点」「timeout 600s 可配」「chunk 字节大小控制」三项 —— 内存/CPU 压测用不上，YAGNI。
## 验收标准
- [x] budgets 为空的 mock 请求不再触及 platform 写连接（trace 或计数器证明，非推断）。
- [x] 配额存在时，manual_budget 扣减结果与改动前逐条一致。
- [x] ttft_ms=800 / inter_chunk_ms=30 配置下，实测首包与 chunk 间隔符合设定（±20%）。
- [x] delay_ms 单独设置时行为与改动前一致（向后兼容）。
- [x] error_rate=0.05 跑 200 次请求，429 比例落在 5%±3%。
- [x] MockConfigEditor 三个新字段可编辑、可保存、重开表单回显正确。
- [x] scripts/check-i18n.mjs 全绿（8 语言新 key 齐）。
- [x] cargo clippy --workspace 零 warning + cargo test --workspace 全绿 + yarn build 通过。
- [x] 50 路并发 mock 流跑 5 分钟无 panic、无请求失败（除 error_rate 注入的）。
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list mock-loadgen-capability`)
