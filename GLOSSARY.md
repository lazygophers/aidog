# AiDog

AiDog 是本地 AI 网关：把 Claude Code / Codex 等客户端的请求按分组路由到各平台，统一记账与统计。

## Language

**平台 (Platform)**:
一个上游 API 供应商实例，由用户创建（base_url + apikey 等），归属某个协议类型。
_Avoid_: 供应商（那是 registry 里的协议概念）

**协议 (Protocol)**:
平台类型的枚举值，既是 wire 协议名也是平台别名（如 anthropic / openai / glm_coding），Rust 与 TS 双写。
_Avoid_: 通道、线路

**透传平台 (Passthrough Platform)**:
不识别协议、不转换内容的平台类型：请求原样打到用户指定的 base_url，仅注入用户定义的认证头。usage 尽力解析，费用恒不估算。
_Avoid_: 直连（那指客户端直连上游）

**分组 (Group)**:
客户端接入的单元，密钥即分组名；决定请求在哪些平台间路由与重试。
_Avoid_: 账号

**模型槽 (Model Slot)**:
平台上按档位（default 等 5 槽）配置的候选模型集合，路由按 source_model 匹配。透传平台不吃模型槽（通配命中）。
_Avoid_: 模型列表（那是下拉候选清单）

**registry**:
内置平台/协议/模型条目的数据真值源（`src-tauri/defaults/registry/`），远程可同步，手维护。

**est_cost**:
单请求估算费用，落 proxy_log。已知价取 registry 模型价，缺价走 fallback 单价；透传平台例外恒 0。
_Avoid_: 账单（那是上游真实计费）
