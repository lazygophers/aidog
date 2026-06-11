# PRD: New API 平台支持

## 背景

New API（基于 One API 的开源 LLM 网关）是广泛使用的第三方中转平台。用户使用标准 API key 发起模型请求，但 **查询余额需要独立的凭据**（session token / access token），与请求用的 API key 不同。

## 需求

1. 添加 `newapi` 平台类型（Protocol 变体）
2. 仅支持 OpenAI 协议（endpoint 固定 openai）
3. 余额查询使用 **独立 API key + 用户 ID**（存储在 `platform.extra`）
4. 前端编辑/新建时显示额外字段："余额查询 Key"、"用户 ID"

## 技术调研

### New API 余额接口（OpenAI 兼容路径）

两个请求组合计算余额：

| 接口 | 路径 | 说明 |
|------|------|------|
| 订阅额度 | `GET /dashboard/billing/subscription` | 返回 `hard_limit_usd`（总额度 USD） |
| 使用量 | `GET /dashboard/billing/usage` | 返回 `total_usage`（单位 0.01 USD） |

**请求头**（两个接口相同）：
```
Authorization: Bearer <balance_api_key>
New-Api-User: <user_id>
Content-Type: application/json
```

**subscription 响应**：
```json
{
  "object": "billing_subscription",
  "hard_limit_usd": 100.0,
  "soft_limit_usd": 100.0,
  "system_hard_limit_usd": 100.0,
  "access_until": 1640995200
}
```

**usage 响应**：
```json
{
  "object": "list",
  "total_usage": 2500.0
}
```

计算：`remaining = hard_limit_usd - total_usage / 100`

> 来源：https://doc.newapi.pro/api/fei-account-billing-panel/

### CC Switch 实现参考

CC Switch 提供 "New API 模板"，使用 `accessToken` + `userId` 参数，调用 `/api/user/self`。
我们选用 `/dashboard/billing/subscription` + `/dashboard/billing/usage` 路径，因为：
- OpenAI SDK 兼容格式，解析更稳定
- 直接返回 USD 金额，无需 `/500000` 换算

> 来源：cc-switch `docs/user-manual/zh/2-providers/2.5-usage-query.md`

## 数据设计

### `platform.extra` JSON 结构

```json
{
  "newapi": {
    "balance_api_key": "sess-xxxxx",
    "user_id": "123"
  }
}
```

前端已有的 `parseMockConfig` / `serializeMockConfig` 模式复用，新增 `parseNewApiConfig` / `serializeNewApiConfig`。

### 不需要 DB migration

`extra` 是自由 JSON 文本列，无需改表。

## 实现计划（Subtasks）

### ST1: Rust 后端 — Protocol + quota 查询

**文件**：
- `src-tauri/src/gateway/models.rs` — 添加 `NewApi` Protocol 变体
- `src-tauri/src/gateway/quota.rs` — 添加 `query_newapi_balance()`

**详情**：
1. `models.rs`: 添加 `#[serde(rename = "newapi")] NewApi`
2. `quota.rs`:
   - 新增 `parse_newapi_extra(extra: &str) -> Option<(String, String)>` 解析 balance_api_key + user_id
   - 新增 `query_newapi_balance(base_url, balance_api_key, user_id)`
     - GET `{base}/dashboard/billing/subscription` → `hard_limit_usd`
     - GET `{base}/dashboard/billing/usage` → `total_usage`
     - 计算 remaining = hard_limit_usd - total_usage / 100
     - 返回 `PlatformQuota { balance: BalanceInfo { remaining, total, used, currency: "USD" } }`
   - `query_quota()` 入口新增：当 `base_url` 含已知 newapi 域名 → 走 newapi 查询
   - 同时需要修改 `query_quota` 签名或新增 `query_quota_with_extra(base_url, api_key, platform_type, extra)` 使 extra 可传入

**注意**：当前 `query_quota(base_url, api_key)` 无法接收 extra。需要：
- 方案 A：扩展签名为 `query_quota(base_url, api_key, extra)` — 但现有调用点需改
- 方案 B：新增 `query_quota_ex(base_url, api_key, platform_type, extra)` — 向后兼容
- **选择方案 B**

**验证**：单元测试 mock HTTP 响应

### ST2: Tauri command 适配

**文件**：`src-tauri/src/lib.rs`

- 新增 `platform_query_quota_ex(platform_id)` command，从 DB 读 platform 获取 base_url / api_key / extra / platform_type，调用 `query_quota_ex`
- 现有 `platform_query_quota` 保持不变（向后兼容）
- 注册新 command

### ST3: 前端 TS 类型 + API

**文件**：`src/services/api.ts`

- `Protocol` type 添加 `"newapi"`
- `NewApiConfig` interface: `{ balance_api_key?: string; user_id?: string }`
- `parseNewApiConfig(extra: string): NewApiConfig`
- `serializeNewApiConfig(extra: string, config: NewApiConfig): string`
- `quotaApi.queryEx(platformId: number)` 新增

### ST4: 前端 UI — 平台编辑表单 + quota 刷新

**文件**：`src/pages/Platforms.tsx`

1. 平台类型选择器添加 "newapi" 选项
2. 当 `protocol === "newapi"` 时显示额外字段：
   - "余额查询 Key"（text input）
   - "用户 ID"（text input）
3. 保存时序列化到 `extra.newapi`
4. quota 刷新改用 `quotaApi.queryEx(p.id)`（对 newapi 平台）
5. 全量加载 quota 也需要适配

### ST5: Logo + i18n

**文件**：
- `src/assets/platforms/newapi.svg` — 如果有 logo
- `src/locales/*.json` — 添加 newapi 相关 i18n key

### ST6: 端到端测试

- 新建 newapi 平台 → 填写 balance key + user id → 保存 → 刷新 quota → 显示余额
- 验证 OpenAI 协议转发正常

## 关键约束

- `base_url` 含版本前缀（如 `/v1`），余额接口路径为 `/dashboard/billing/...`，最终 URL = `base_url.strip_version() + /dashboard/billing/...`
- 余额查询的 key **不是** 主 API key，不混用
- `hard_limit_usd` 单位 USD；`total_usage` 单位 0.01 USD（即 cents）
