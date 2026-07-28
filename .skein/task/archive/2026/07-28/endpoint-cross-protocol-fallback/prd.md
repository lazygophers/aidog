# endpoint 跨协议回退: 无同协议端点时回退 openai 经 converter 转换 — PRD (主入口)

## 目标
- [ ] 普通平台 source 无同协议 endpoint 时回退 openai(优先)/ 首个非 source 可用 endpoint
- [ ] converter 自动双向转: 请求 source→wire, 响应 wire→source (上轮 5×5 已就绪, 无新开发)
- [ ] coding 平台不落非 coding 端点 (401 防护不变)
- [ ] is_valid_wire_protocol gate 不再对回退请求触发 (回退保证合法)
## 边界
- 改: src-tauri/crates/aidog_core/src/gateway/proxy/endpoint.rs select_endpoint_for_protocol 普通平台步骤 4 扩展
- 测: endpoint.rs test 覆盖 anthropic/gemini/openai_completions 入站 + 仅 openai endpoint 回退
- 不动: converter (5×5 已覆盖), forward.rs gate, UA 透传分支, coding 平台逻辑
## 验收标准
- [ ] anthropic 入站 + 仅 openai endpoint 普通平台 → 回退 openai endpoint, converter 转, target_protocol=openai 落库
- [ ] gemini 入站 + 仅 openai endpoint → 回退 openai
- [ ] coding 平台 (has_coding_ep) 无同协议 endpoint → 仍走步骤 1/2, 不落非 coding 端点 (回归测试)
- [ ] openai_responses 入站无 responses endpoint → 回退 openai (现有行为, 回归)
- [ ] 同协议 endpoint 存在 → 直发不转换 (现有行为, 回归)
- [ ] cargo test aidog_core 全绿, clippy 0 本项目 warning
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list endpoint-cross-protocol-fallback`)
