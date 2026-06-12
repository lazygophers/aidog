# 代理请求流

## 完整请求链路

```
1. 入站请求到达 Axum proxy (127.0.0.1:7860)
   ↓
2. 协议检测 (parse_incoming_request)
   - /v1/chat/completions → openai_chat
   - /v1/completions → openai_completions
   - /v1/responses → openai_responses
   ↓
3. 分组匹配 (router.rs)
   - 按模型名匹配分组
   - 无匹配 → 默认分组
   ↓
4. 平台选择
   - 顺序 / 随机 / 加权策略
   - 检查平台启用状态
   ↓
5. 模型映射链
   - 分组级映射 → 平台级映射 → 原始值
   ↓
6. 协议转换 (convert_request)
   - 入站协议 → 内部格式 → 出站协议
   - URL = base_url + provider_api_path()
   ↓
7. 上游请求
   - HTTP 客户端发送
   - SSE 流式透传（逐 chunk 转发）
   - 超时保护（可配置，默认 300s）
   ↓
8. 故障转移
   - 429 / 5xx / 超时 → 尝试下一个平台
   - 全部失败 → 返回错误
   ↓
9. 后处理
   - 日志记录（三级控制）
   - Token 提取
   - 费用估算（resolve_price 回退链）
   - est_cost 持久化
   ↓
10. 响应返回客户端
```

## SSE 流式处理

AiDog 透明转发 SSE 流：
1. 上游返回 SSE headers → AiDog 立即转发 headers 给客户端
2. 每收到一个 SSE chunk → 立即转发给客户端
3. 上游关闭 → AiDog 关闭客户端连接

## 超时级联

- 上游请求超时：可配置（默认 300s）
- 超时后触发故障转移（如果组内有下一个平台）
- 全部超时 → 返回 504 Gateway Timeout

## 日志记录时机

- 请求开始时：记录请求头、模型名
- 请求完成时：记录 Token 用量、est_cost、延迟
- 三级控制：master switch / user request / upstream request
