# Devin v3 API Session 生命周期调研

> 调研目标: 为 aidog「chat/completions ↔ Devin session」协议转换层提供技术依据。
> 数据源: agent-reach 可用, 但本任务纯文档抓取用 curl 直拉 docs.devin.ai 的 Mintlify `.md` 源 (最权威, 含 OpenAPI YAML)。所有事实带 docs URL + schema 片段。
> 覆盖缺口声明: POST `/sessions/{id}/messages` 与 DELETE terminate 的 v3 `.md` 源 Mintlify 返回 `null` (疑似页面 gate), 已用 v1 同义端点 schema + navigation + common-flows 交叉佐证, 并标 `推测:`。

---

## 1. Session 创建 — `POST /v3/organizations/{org_id}/sessions`

来源: https://docs.devin.ai/api-reference/v3/sessions/post-organizations-sessions

**请求 body** (application/json):

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `prompt` | string | ✅ | 给 Devin 的任务指令 |
| `advanced_mode` | enum\|null | ❌ | `analyze` / `create` / `improve` / `batch` / `manage` (即"agent mode") |
| `attachment_urls` | string<uri>[]\|null | ❌ | 附件 URL, 每项长 1–2083 |
| `bypass_approval` | bool\|null | ❌ | 跳过 safe-mode 动作审批 |
| `child_playbook_id` | string\|null | ❌ | 子 session playbook |
| `create_as_user_id` | string\|null | ❌ | 归属到某人类用户 (需 `ImpersonateOrgSessions` 权限) |
| `knowledge_ids` | string[]\|null | ❌ | 关联 knowledge notes |
| `max_acu_limit` | integer\|null | ❌ | 本 session ACU 消费上限 (计费硬闸门) |
| `playbook_id` | string\|null | ❌ | 使用的 playbook |
| `repos` | string[]\|null | ❌ | 仓库 |
| `secret_ids` | string[]\|null | ❌ | secret 引用 |
| `session_links` | string[]\|null | ❌ | |
| `session_secrets` | object[]\|null | ❌ | `{key, value, sensitive}` 内联 secret |
| `structured_output_schema` | object | ❌ | JSON Schema Draft 7, ≤64KB, 自包含无外部 $ref — 终态产出结构化结果 |
| `tags` | string[]\|null | ❌ | |
| `title` | string\|null | ❌ | |

**注意: 无 `model` 字段** — Devin 不暴露底层 LLM 模型选择, 只有 `advanced_mode` 控制任务类别。

**响应 200** (关键字段):
```jsonc
{
  "session_id": "devin-...",      // 创建即可拿到的会话 ID
  "status": "new",                 // 初始状态 (create 响应 enum 含 creating, get 不含)
  "status_detail": "working",      // running 时: working/waiting_for_user/waiting_for_approval/finished
  "url": "https://app.devin.ai/sessions/devin-...",
  "created_at": 123, "updated_at": 123,
  "org_id": "...",
  "acus_consumed": 0,
  "pull_requests": [{"pr_state":"...","pr_url":"..."}],
  "structured_output": {},         // 终态才有
  "is_advanced": false, "is_archived": false,
  "parent_session_id": null, "child_session_ids": [],
  "playbook_id": null, "service_user_id": null, "user_id": null,
  "tags": [], "title": null
}
```
权限: `ManageOrgSessions` (创建) + `ImpersonateOrgSessions` (用 `create_as_user_id` 时)。

---

## 2. Session 状态轮询 — `GET /v3/organizations/{org_id}/sessions/{devin_id}`

来源: https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session

`devin_id` 即 session_id (前缀 `devin-`)。

**`status` 状态机** (get 端点 enum):
| status | 含义 | 是否终态 |
|---|---|---|
| `new` | 刚创建 | 否 |
| `claimed` | 已分配 agent | 否 |
| `running` | 执行中 | 否 |
| `resuming` | 唤醒中 | 否 |
| `exit` | 正常完成 | ✅ 终态 |
| `error` | 出错 | ✅ 终态 |
| `suspended` | 挂起 | ✅ 终态 (可被唤醒) |

(创建响应 enum 多一个 `creating` 中间态, get 端点不返回。)

**`status_detail`** (仅 get/list 有, 细化状态):
- status=`running` → `working` (干活) / `waiting_for_user` (要输入) / `waiting_for_approval` (safe-mode 等审批) / `finished` (任务做完, 即将转 exit)
- status=`suspended` → `inactivity` / `user_request` / `usage_limit_exceeded` / `out_of_credits` / `out_of_quota` / `no_quota_allocation` / `payment_declined` / `org_usage_limit_exceeded` / `total_session_limit_exceeded` / `error`

**关键判定 (转换层用)**:
- 可继续轮询: `new` / `creating` / `claimed` / `running` / `resuming`
- **终态 = `exit` ∪ `error` ∪ `suspended`** (官方 common-flows Python 示例 line 127: `if status in ("exit","error","suspended"): break`, 来源 https://docs.devin.ai/api-reference/common-flows.md)
- "结果就绪" 更精细: `status_detail == "finished"` 即 Devin 已产出最终回复, 紧接着会转 `exit`。

响应还含: `category` (用例分类 bug_fixing/feature_development/...), `origin` (webapp/api/cli/...), `structured_output`, `subcategory`。

---

## 3. Session 输出获取

Devin 不直接给"一行 completion text", 有三条产出通道, 转换层需择一/组合:

### (a) Messages 流 — `GET /v3/organizations/{org_id}/sessions/{devin_id}/messages`
来源: https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session-messages

cursor 分页 (`after` cursor, `first` 1–200 默认 100), 按时间顺序。**SessionMessage schema**:
```yaml
SessionMessage:
  required: [event_id, source, message, created_at]
  properties:
    event_id: string        # 去重 key
    source: enum [devin, user]
    message: string         # 消息正文
    created_at: integer     # Unix 秒
```
转换层取最终回复 = 取最后一条 `source==devin` 的 message (或 status_detail==finished 后拉全量)。

### (b) Structured Output
创建时传 `structured_output_schema` (JSON Schema Draft 7), 终态后 session 对象的 `structured_output` 字段返回经验证的对象。适合把 Devin 当"函数"调用, 强类型拿结果。来源同 §1 create 端点。

### (c) Artifacts
`GET .../sessions/{devin_id}/attachments` 列文件 (output.py / 日志 / 截图), 每项 `{attachment_id, name, url}`, url 直下。来源: common-flows.md "Downloading session attachments"。

### (d) Insights (事后分析, 非实时输出)
`GET .../sessions/{devin_id}/insights` — 含 `num_devin_messages`, `num_user_messages`, `session_size` (xs/s/m/l/xl), `analysis` (AI 生成 timeline / action_items / issues)。来源: https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session-insights.md 。转换层一般用不到, 配额核算可参考 `acus_consumed`。

---

## 4. 流式支持 — **否 (无原生 SSE / WebSocket)**

**结论: Devin v3 API 不提供任何 server-push 流式端点。** 进度只能轮询。

依据:
1. Sessions 端点全集 (navigation, 来源 get-session 页左侧 nav): List / Create / Get / List messages / Send message / List attachments / Terminate / Archive / tags / insights — **无一 stream/SSE/eventsource 字眼**。URL: https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session
2. 官方 common-flows 的"实时跟踪"做法就是轮询: `while True: GET session; if 终态 break; sleep(10)` (common-flows.md line 120-130), 以及 `GET .../messages` 拉消息。
3. 企搜"Devin API streaming SSE" 无官方 v3 流式端点命中; 第三方 gist 提"real-time token streaming"指 Devin 网页 UI 体验, 非 REST API 能力。

**对转换层的影响 (关键)**:
- 用户传 `stream:true` 时, aidog **无法**做真正的 token-by-token SSE 转发。
- 可行的伪流式方案 (权衡, 不拍板):
  - 方案 A: 轮询 messages, 把新出现的 `source==devin` message 切块包装成 chat SSE chunk `data: {choices:[{delta:{content}}]}` 推给客户端, 终态发 `[DONE]`。chunk 粒度 = 轮询周期, 不是真 token 流。
  - 方案 B: `stream:true` 直接拒绝/降级为非流式, 等 exit 后一次性返回 full message。
  - 方案 C: 先返一个"任务已派发, 查看 session.url" 的占位回复 (把 Devin 当异步 agent 而非 LLM)。
- 此分歧需 main 转达用户拍板。

---

## 5. 多轮对话语义 — **支持续聊 (在同一 session 追加消息)**

Devin session 是长生命周期 agent 会话, 不是 one-shot。

**端点**: `POST /v3/organizations/{org_id}/sessions/{devin_id}/messages`
- 来源: navigation "Send a message to a session" (get-session 页 nav); v1 同义端点 `POST /v1/sessions/{session_id}/message` schema 作交叉佐证。
- `推测:` v3 `.md` 源 Mintlify 返回 null (页面 gate), 但 nav + v1 行为高度可信。请求 body 推测为 `{"message": "<string>"}` (v1 PostMessageParams 即此), 可能还有 attachment 字段。
- v1 文档原文 (https://docs.devin.ai/api-reference/v1/sessions/send-a-message-to-an-existing-devin-session.md): *"Send a message to an active Devin session. The session must be in a running state to receive messages. Returns null on success, or a detail message if the session is already suspended."*

**约束**:
- session 必须处于 `running` (或可被唤醒的 `suspended`/`exit`?) 才能收消息 — `推测:` 非运行态发消息会返 detail 错误。
- 闲置会自动 sleep (约 0.1 ACU 闲置即睡), sleep 不计费, 发消息可唤醒 (来源: https://docs.devin.ai/admin/billing/usage.md "Sleep and idle behavior")。

**对转换层的语义映射 (权衡)**:
- chat 多轮映射有两种选择 (需 main 裁):
  - 映射 A: 同一 chat conversation → 同一 Devin session, 后续 user turn 走 POST messages (符合 Devin 原生多轮语义, 省 ACU, 但要维护 chat_id ↔ devin_id 映射 + 唤醒逻辑)。
  - 映射 B: 每个 chat request → 新建 Devin session (无状态, 简单, 但丢失 Devin 的累积上下文, 费 ACU)。
- 多数 chat/completions caller 期望无状态, 映射 B 更直白; 但 Devin 的价值在长任务上下文, 映射 A 更发挥能力。此为设计分歧, 标交 main。

---

## 6. model / mode 选择

- **无底层 LLM `model` 参数** (§1 已述)。Devin 是完整 agent, 不暴露换模型。
- **`advanced_mode`** (create body, 可选): `analyze` / `create` / `improve` / `batch` / `manage` — 控制 agent 任务类别 / 行为模式。来源: create 端点 schema。
- `playbook_id` / `child_playbook_id` — 用预制 playbook (可视为"模式模板")。
- `推测:` mode 枚举的精确语义文档未展开, 需实测或查 playbook 文档。

对转换层: chat 请求里的 `model` 字段无法映射到 Devin model; 可忽略, 或映射到 `advanced_mode` (如 code 请求 → `create`), 此映射规则归 main 设计。

---

## 7. 计费模型 — **ACU (Agent Compute Units)**

来源: https://docs.devin.ai/admin/billing/usage.md , https://docs.devin.ai/admin/billing/self-serve.md , https://docs.devin.ai/api-reference/v3/consumption/organizations-consumption-daily.md

- **单位**: Enterprise = ACU (订单额度); Self-serve = 计划配额 + 预付 on-demand credit (1 credit = 1 ACU 等价美元值)。
- **计费维度**: 按 Devin 实际干活量 (动作数/复杂度 + VM 时间 + 带宽), **不是 token, 不是时长**。Windows session 比 Linux 贵 ~9%。
- **不计费**: 等用户响应 / 等测试跑 / clone 仓库期间; sleep 期间 0 消费。
- **session 级用量**: session 对象的 `acus_consumed` 字段 (实时累计); create 时的 `max_acu_limit` 是硬上限。
- **status_detail 欠费态**: `usage_limit_exceeded` / `out_of_credits` / `out_of_quota` / `no_quota_allocation` / `payment_declined` / `org_usage_limit_exceeded` / `total_session_limit_exceeded`。

**用量查询端点 (配合 aidog quota)**:
- `GET /v3/organizations/{org_id}/consumption/daily` — 按日 ACU, 含 `acus_by_product` (devin/cascade/terminal/review 分项), `total_acus`。需 `ViewOrgConsumption` 权限。日界 = 太平洋时间 0 点 (08:00 UTC)。
- `GET .../consumption/daily/sessions` — 单 session 日消耗 (来源 llms.txt: consumption-daily-sessions.md)。
- `GET .../sessions/{id}/insights` 也能拿单 session `acus_consumed`。

对转换层: chat 响应的 usage 字段可填 `acus_consumed` (但语义非 token, 需 UI 标注); aidog quota 可周期性拉 consumption/daily 同步。`推测:` 实时余额端点文档未见, 可能只能靠 consumption 累计反推余额。

---

## 8. 限流 — 429

来源: overview.md "Error handling", common-flows.md error handler 示例 line 436。

- HTTP 429 = rate limit exceeded, 语义"wait and retry"。
- `推测:` 官方文档**未明确**具体 rate limit 数值, 也**未文档化** `X-RateLimit-*` / `Retry-After` 响应头 (overview 只列了状态码, 没列 header)。需实测抓 429 响应头确认有无 Retry-After。
- 对转换层: 实现 429 → 指数退避重试; 给客户端返 429 时透传。

---

## 9. 错误码

来源: https://docs.devin.ai/api-reference/overview.md (Error handling 段), authentication.md troubleshooting。

| 码 | 场景 |
|---|---|
| 200 | 成功 |
| 201 | 资源创建 |
| 400 | 请求参数无效 |
| 401 | API key 缺失/失效/格式错 (如 MCP 用了 legacy `apk_` key) |
| 403 | 权限不足 (service user 缺该端点所需 RBAC 角色); v2 端点需 Enterprise Admin |
| 404 | 资源不存在 / 无权访问 |
| 422 | 校验错误 (响应体 `HTTPValidationError.detail[]`, 各端点 OpenAPI 都标了 422) |
| 429 | 限流 |
| 500 | 服务端错误 |

RBAC 权限矩阵 (各端点页 "Permissions" 段): 创建=`ManageOrgSessions`, get/list/insights=`ViewOrgSessions`, consumption=`ViewOrgConsumption`, impersonate=`ImpersonateOrgSessions`。

---

## 转换层设计要点 (汇总, 供 main brainstorm)

1. **生命周期**: POST /sessions (拿 devin_id) → 轮询 GET /sessions/{id} 到终态 (exit/error/suspended) → GET /messages 取最后 devin 消息 → 包成 chat completion response。终态判定 = `status ∈ {exit,error,suspended}` (官方示例口径)。
2. **streaming 硬伤**: 无原生流式, `stream:true` 只能伪流式 (轮询 messages 切块发 SSE chunk) 或降级非流式。**需用户拍板**。
3. **多轮语义**: Devin 原生支持同 session 续聊 (POST messages), 但 chat caller 多为无状态。映射 A (同 chat→同 session) vs 映射 B (每请求新 session) **需用户拍板**。
4. **model 不可映射**: chat 的 `model` 字段无对应, 可忽略或映射 `advanced_mode`。
5. **usage**: 用 `acus_consumed` (非 token), quota 同步走 consumption/daily。
6. **终止**: 客户端断连时 `推测:` 可 DELETE /sessions/{id} terminate 止血省 ACU (端点在 nav 列出, v3 `.md` 源 null, slug 推断 `delete-organizations-session`, 需实测)。
7. **org_id 必填**: 所有 v3 organization 端点 path 都要 `org_id` (前缀 `org-`), aidog 平台配置需存 org_id + cog_ key 两个值。

## 需要 (需 main 转达用户 / 实测补齐)

- 需要: streaming 伪流式 vs 降级非流式 — 用户拍板 (§4)。
- 需要: 多轮映射 A vs B — 用户拍板 (§5)。
- 需要: POST `/sessions/{id}/messages` 的 v3 精确 schema (v3 .md 源 gate, 仅 v1 佐证 body={message}) — 实测 curl 抓真实响应补齐。
- 需要: DELETE terminate 的精确端点 path + 行为 (nav 有, schema 缺) — 实测。
- 需要: 429 响应头有无 `Retry-After` — 实测。
- 需要: `advanced_mode` 各枚举的实际行为差异 — 文档未展开, 需实测或查 playbook 文档。
- 需要: 实时余额查询端点 (若存在) — 文档未见, 待确认 self-serve 是否只能累计反推。
