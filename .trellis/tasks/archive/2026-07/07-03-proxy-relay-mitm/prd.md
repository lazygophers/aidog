# proxy MITM 解密隧道 P3 — 客户端保官方协议 + AirDog 拦截采集

- **Status**: planning（决策已定，待 design review）
- **Source**: session:claude_96a0dd46-757b-40db-a9f7-4555767078d3
- **Spec**: `.trellis/spec/backend/proxy-connect-relay.md`（P1 契约，P3 需扩展）
- **Research**（继承自 proxy-http-relay task）:
  - `../proxy-http-relay/research/p2-middleware-reuse.md`
  - `../proxy-http-relay/research/mitm-feasibility.md`
  - `../proxy-http-relay/research/scenario-analysis.md`

---

## 背景

第三方会检测/修改用户对 `BASE_URL` 的改动。客户端改 `ANTHROPIC_BASE_URL` 指向 AirDog 可能被检测/拒绝。

诉求：客户端**保持官方协议**（不改 BASE_URL），经 `HTTP_PROXY` 让流量过 AirDog。AirDog MITM 解密 → 拦截/采集/转发 → 绕过第三方 BASE_URL 检测。

## 目标

CONNECT 隧道从 P1 盲转升级为 P3 MITM：解密 → 走 forward_attempt 链 → 重新加密发上游。让 middleware/路由/headers/retry/计费在环境变量代理方案下真正生效。

## 核心流程

```
客户端配 HTTP_PROXY=http://127.0.0.1:<port>（用户人工写，AirDog 不自动写）
  → 发 CONNECT api.anthropic.com:443
  → AirDog 解析 SNI，查白名单
    ├─ 命中白名单 → MITM 路径
    └─ 未命中 → P1 盲转（保留）
  → MITM: 动态签证书（假 CA 签 host）→ 回 200 → 客户端 TLS 握手
  → 解密得明文 HTTP（官方协议）→ 走 handle_proxy_core → forward_attempt 链
  → 连真上游（TLS client，真证书验证）→ 回传（重新加密）
```

## 技术选型

`rustls 0.23` + `rcgen 0.14` + `tokio-rustls 0.26`

## 决策（已落定）

| ID | 决策 | 选定 | 备注 |
|---|---|---|---|
| D1 | 假 CA 装信任库 UX | **Tauri 自动装** | 加 `tauri-plugin-shell` capability + sudo 提权；macOS `security add-trusted-cert` / Windows `certutil -addstore` / Linux `update-ca-certificates` |
| D2 | MITM 范围 | **可配白名单** | 默认 AI host（anthropic.com/openai.com/已配平台 host），用户可加自定义；未命中走 P1 盲转 |
| D3 | 客户端配置 | **保留现状 + 禁自动写 HTTP_PROXY** | sync_settings.rs/codex.rs 自动写 BASE_URL 不动（BASE_URL 路径保规则全生效）；HTTP_PROXY 由用户人工配，AirDog 不自动写 setting |
| D4 | 假 CA 私钥存储 | **DB**（SQLite） | 风险：DB 文件被拷带走私钥；缓解：DB 文件权限 + 私钥条目加密（design 评估） |

## 两路并存语义

- **BASE_URL 路径**（保现状）：CC/Codex 由 sync_settings.rs/codex.rs 自动配，客户端直连 AirDog `/proxy` HTTP 明文 → forward_attempt 全套。零信任成本。
- **HTTP_PROXY MITM 路径**（P3 新增）：需保官方协议的用户人工配 HTTP_PROXY → AirDog MITM → forward_attempt 全套。需装假 CA。

## 🔴 待 design 细化

- 假 CA 生成时机（首次启动 / 配置时）+ 装信任库的跨 OS 命令 + 提权失败兜底
- 白名单存储（DB 表）+ 匹配逻辑（host suffix）
- TLS 层：tokio-rustls accept（假证书）+ client connect（真证书验证上游）
- 明文 Request 灌入 `handle_proxy_core` 的接口（Request 构造）
- HTTP/2 ALPN 协商（client↔AirDog 与 AirDog↔upstream 对齐）
- 私钥 DB 存储：加密方案（如 OS keychain 派生密钥加密私钥，或明文 + 文件权限）

## 范围

- **改**：`connect.rs`（CONNECT 升级 MITM 分流）、新增 TLS 层 + 假 CA 子系统 + 白名单
- **复用**：`handle_proxy_core` + `forward_attempt`（95% 复用，明文 Request 灌入）
- **前端**：白名单配置 UI + 假 CA 安装状态/引导
- **新增依赖**：rustls / rcgen / tokio-rustls + tauri-plugin-shell

## 风险

| 风险 | 缓解 |
|---|---|
| 假 CA 私钥泄露 = 白名单内 HTTPS 被 MITM | DB 私钥加密 + 白名单限范围 + 用户可控 |
| 客户端 cert pinning | research：CC 无 / Codex 推测无 |
| Tauri 自动装 CA 需 sudo，失败兜底 | design：失败回退引导手动装 |
| HTTP/2 协商失配 | ALPN 对齐 |
| 性能（双 TLS） | 评估 |

## 工作量

8–14 人天。分 subtask（design.md 拆分）。

## 不做

- 全量 HTTPS MITM（白名单限 AI host + 用户自定义）
- 移除 BASE_URL 自动配（D3 保留现状）
- P2 元数据补强（P3 MITM 后 forward_attempt 含全套，P2 是子集）
