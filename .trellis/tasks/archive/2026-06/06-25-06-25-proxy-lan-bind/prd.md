# PRD — 默认代理支持局域网访问

## 背景 / 问题
代理服务器当前硬编码绑定 loopback：`gateway/proxy/mod.rs:194`
```rust
let addr = std::net::SocketAddr::from(([127, 0, 0, 1], actual_port));
```
仅本机可连。需求：让代理默认监听局域网（`0.0.0.0`），使同局域网其他设备（手机 / 另一台电脑）也能用本机 aidog 作代理。

## 用户决策（已通过 AskUserQuestion 确认）
1. **开启方式 = 设置项开关，默认开**。AppSettings 加开关；默认 `bind_lan=true`（绑 `0.0.0.0`）；用户可关回 `127.0.0.1`。
2. **端点保护 = 全部端点对 LAN 开放，靠 Bearer 鉴权**。不加 peer IP 守卫。
   - 已核实 `/api/group-info`（group_info.rs）与 `/api/notify`（notify.rs:42-49）均有 `Authorization: Bearer <group_key>` 鉴权且校验 group 存在 → LAN 暴露可接受。
   - `/proxy` 转发端点本就靠 group_key 鉴权。

## 范围（MVP）
- ProxySettings 增 `bind_lan: bool` 字段（serde default = true）。
- `start_proxy` 按 `bind_lan` 选 `0.0.0.0` 或 `127.0.0.1`。
- 新增 command `proxy_set_bind_lan(enabled)`：持久化 + 自动重启代理使生效（绑定地址只在 bind 时读取，必须重启）。
- 前端 AppSettings 加开关 + 调用，开关切换后重启代理。
- i18n 全语言补 key（src/locales/*.json，实测文件数为准）。

## 非目标
- 不做指定网卡 / 白名单 IP（仅 0.0.0.0 全开 vs 127.0.0.1）。
- 不在 UI 展示本机 LAN IP（可后续增强）。
- 不改 sync_settings 写入客户端配置的 base_url（本机仍用 localhost；LAN 客户端手动填本机 IP）。

## 安全说明（落档，非阻塞）
默认开 LAN = 升级后存量用户代理默认暴露到局域网。两个内部端点 + 转发端点均有 group_key Bearer 鉴权兜底，无未鉴权端点暴露。健康端点 `/` `/proxy` 返回固定 JSON 无敏感数据。符合用户显式选择。

## 验收标准
- `bind_lan=true`：同局域网另一设备可访问 `http://<本机IP>:<port>/proxy`（带合法 group_key）。
- `bind_lan=false`：仅本机可连，外部连接被拒。
- 开关切换即时生效（重启代理）。
- `cargo build` / `cargo clippy`（零 warning）/ `cargo test` 全绿。
- `yarn build` 通过；`check:i18n` 无缺 key（若有该脚本）。
- 存量用户（DB 无 bind_lan 字段）反序列化 default=true，不崩。
