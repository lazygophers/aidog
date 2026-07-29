# tokenizer 峰值与零散常驻清扫 — PRD (主入口)

## 目标
- [ ] 把 count_tokens 的本地 BPE 估算改惰性 —— 现状 count_tokens.rs 在上游透传之前无条件跑，claude-cli 每次发对话前都打该端点，等于把 HF tokenizer 初始化绑死在正常路径
- [ ] 改后触发条件从每次 count_tokens 降到上游不支持该端点的 fallback 分支，绝大多数用户永不初始化 glm-4.json 19MB / qwen2.json 6.7MB 对应的 40-120MB 堆
- [ ] 去掉 platform-presets.json 的重复解析 —— peak_hours.rs 与 defaults_sync.rs 各持一份 OnceLock Value，同一份 105K 源文本解析两遍
- [ ] 删 coding_plan.rs 的死代码 —— presets() 标 allow(dead_code)，生产无调用点，注释自承当前无 Rust 路由消费
- [ ] 给 MITM 的 cert_signer 证书缓存与 suspects 表加界 —— 二者是全仓仅有的两个真无界容器，只在开 MITM 后增长
- [ ] AGG_DEDUP_CAP 从 8192 降到合理值
## 边界
- token 计数与费用精度不得下降（红线 2）—— 惰性化只改何时算，不改算什么与怎么算；禁降级为估算或纯依赖上游 usage
- 上游成功返回 count_tokens 结果时用上游值，这是现状行为，本 task 只是不再多余地本地也算一遍
- 不动 tokenizer.rs 的 pick_encoding 选型逻辑与四个单例本身
- 不动转发主路径的报文处理（归 proxy-hotpath-buffers）
- MITM 证书缓存加 evict 后重签只影响该 host 首次连接，不得影响已建立连接
- 一切验证只用 mock 平台与分组
## 验收标准
- [x] 存在用例证明：上游 count_tokens 成功时不再触发本地 BPE 估算；上游失败时 fallback 仍给出与改动前一致的 token 数
- [x] 对 glm / qwen 模型走 count_tokens 且上游成功的场景，进程 phys_footprint 不出现 HF tokenizer 初始化对应的跃升
- [x] 上游失败 fallback 路径的 token 计数结果与改动前逐条一致
- [x] platform-presets.json 在进程内只解析一份，用计数或 trace 证明
- [x] coding_plan.rs 的死代码已删且全 workspace 编译通过
- [x] cert_signer 缓存有明确上界，达界时 evict 行为已定义；suspects 表有 sweep 机制
- [x] AGG_DEDUP_CAP 调整后，重复终态去重仍正确（有用例覆盖）
- [x] cargo clippy --workspace 零 warning、cargo test --workspace 全绿
- [x] 全程只用 mock 平台与分组
- [x] 清场完成
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list tokenizer-residency-trim`)
