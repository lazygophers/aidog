# PRD: 协议转换优先透传原始协议

## 背景

当前代理对协议转换处理 (research/01-04 确认):
- 入站协议按 path 推断 (`detect_source_protocol` proxy.rs:1487, 调用 proxy.rs:578)
- 出站协议由**端点匹配**决定 (proxy.rs:629-641): 遍历 `platform.endpoints` 找匹配入站协议的端点, 无匹配才回退 `platform_type` 默认协议
- **但命中端点后仍无条件** `convert_request` (proxy.rs:742) 重序列化, 构成 `from_* → to_*` 有损往返
- 唯一真字节透传 = `Protocol::ClaudeCode` 平台 (handle_passthrough proxy.rs:1185), 强绑平台类型而非协议条件
- 原始请求量 (method/uri/headers/bytes) 已在 body 消费前 clone 保存 (proxy.rs:522), 可复用
- `Platform.endpoints: Vec<PlatformEndpoint>{protocol, base_url, client_type, coding_plan}` (models.rs:337) 多协议端点能力齐备

**问题**: 即使入站协议 = 目标平台支持的协议, 仍走有损格式往返转换, 浪费 + 失真。

## 目标

入站请求协议若被目标平台**显式声明支持** (endpoints 含该协议端点), 走**逻辑透传**: 跳过协议格式转换 (messages 结构 from_*→to_*), 直接用原始请求体转发。仅当平台**未显式声明**入站协议端点时, 才 `convert_request` 转成平台支持的协议。

## 决策 (已确认)

- **"平台支持该协议"判定 = 显式 endpoint 声明**。仅当 `platform.endpoints` 里有协议 == 入站协议的端点才算支持。**不**按 platform_type 隐含归类 (不引入 `Protocol::wire_kind()`, 不做方案 C)。
- 走方案 A: 端点命中入站协议 → 透传该端点。

## 透传语义 (关键: 非纯字节透传)

同协议透传**不等于** ClaudeCode 那种纯字节透传。即便协议格式不转, 以下仍**必须保留** (research/04 风险点):
1. **model 名 remap**: 请求体 `model` 字段按 group model_mappings 替换 (路由模型映射不能失效)
2. **鉴权改写**: 注入目标平台 api_key / OAuth, 去掉客户端原始鉴权
3. **URL 构造**: `base_url + provider_api_path` (端点的 base_url), 遵守 URL 约束 (base_url 含版本前缀, 禁重复拼接)
4. **coding_plan 注入**: 若端点 coding_plan=true 的注入逻辑保留
5. **usage 计费提取**: 响应侧 usage 仍需提取 (用于 est_cost / 统计), 不能因透传丢失

→ 实现 = "同协议短路格式转换", 保留上述旁路改写。可能仍需 parse 请求体改 model 字段 + 序列化, 但**跳过 messages/tools 等协议结构转换** (convert_request 的有损部分)。或对 model 字段做轻量 JSON patch 避免全量重序列化。

## 范围

- 后端: proxy.rs (出站协议决策 proxy.rs:629-742 加"同协议短路"分支; 响应侧 parse_sse proxy.rs:998 同协议短路 + usage 提取保留) + converter.rs (可能抽出"仅 model remap 不转格式"路径) + models.rs (若需端点能力查询辅助)
- 文档: 收尾修订 `.wiki/architecture/protocol-adapter.md:47-53` 过时描述 (漏端点匹配 + 新增透传优先逻辑)

## 非目标

- 不引入 platform_type 隐含协议归类 (wire_kind)
- 不改 ClaudeCode 现有纯字节透传路径
- 不改入站协议推断逻辑 (detect_source_protocol)
- 不改 endpoints 数据结构 / 前端编辑入口

## 风险 (research/04)

- URL 前缀重复 (透传用端点 base_url, 禁再拼版本前缀)
- 鉴权差异 (各协议鉴权头不同: Anthropic x-api-key vs OpenAI Bearer)
- 匹配粒度 (协议精确字符串比对, 大小写/变体)
- model remap 在不全量重序列化下改 model 字段
- coding_plan 注入与透传共存
- usage 计费: 响应透传时仍要提取 token (与"流式日志兼容"task 的 usage 提取交叉, 注意一致)

## 验收标准

- 入站协议 = 平台显式端点协议时: 不走 convert_request 格式转换, 请求体 messages 结构原样转发
- model remap / 鉴权 / URL / coding_plan / usage 提取 5 项在透传下全部正确保留
- 入站协议无匹配端点时: 仍正确 convert_request 转平台默认协议 (回归不破)
- 响应侧同协议短路 + usage 正确提取 (est_cost 不丢)
- cargo build + yarn tsc 0 error 无新增 warning; 现有 test 通过 + 补透传分支测试
- `.wiki/architecture/protocol-adapter.md` 描述修订准确

## 编排

单一交付 (协议透传优化), 单 worktree。**依赖**: 与"流式日志兼容"task 同改 proxy.rs 响应分支 (usage 提取), **必须等流式日志 task commit/merge 后再 start 本 task**, 避免 worktree 基线冲突 + usage 提取逻辑双改冲突。串行顺序: 颜色 → 日志 → 协议。
