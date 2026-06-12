# PRD: 系统级 Proxy 设置 + 平台级开关

## 目标

在系统设置页添加 proxy 配置（类型/地址/端口/认证），在每个平台添加"是否启用代理"开关。

## 设计

### 系统级 Proxy 配置

存储在 DB `setting` 表 scope=`"proxy"`, key=`"proxy_client"`：

```json
{
  "enabled": false,
  "type": "socks5",        // "socks5" | "http" | "https"
  "host": "127.0.0.1",
  "port": 7890,
  "username": "",
  "password": "",
  "dns_over_proxy": true   // SOCKS5 时 DNS 走代理解析 (socks5h)
}
```

### 平台级 Proxy 开关

在 `platform` 表新增字段或利用 `platform.extra` JSON 存储。建议复用 `extra` 零迁移：

```json
{
  "proxy_enabled": true    // null/true = 跟随系统, false = 不走代理
}
```

默认 `null`/缺省 = 跟随系统 proxy 配置。

## 范围

### 后端 (Rust)

1. 新增 `ProxyClientSettings` struct + CRUD commands
2. `platform.extra` 新增 `proxy_enabled` 字段
3. 封装 `build_http_client(db, platform)` 函数：
   - 读系统 proxy 配置
   - 读平台 `proxy_enabled`
   - 若都启用 → reqwest ClientBuilder `.proxy()`
   - SOCKS5: `reqwest::Proxy::all(format!("socks5h://host:port"))` (dns_over_proxy=true) 或 `socks5://`
   - HTTP/HTTPS: `reqwest::Proxy::all(format!("http://host:port"))`
   - 认证: `.proxy.basic_auth(username, password)`
4. Cargo.toml 添加 reqwest `socks` feature
5. 替换所有 6 处 `reqwest::Client` 构建点

### 前端 (React)

1. 系统设置 tab: proxy 配置区 (启用开关 + 类型选择 + 地址端口 + 认证输入)
2. 平台编辑/创建: 添加"使用代理"开关
3. i18n: 7 种语言

## 涉及 reqwest Client 构建点

| 文件 | 函数 | 用途 |
|------|------|------|
| proxy.rs:701 | handle_proxy | 主代理转发 |
| proxy.rs:1115 | handle_passthrough | CC 透传 |
| quota.rs:92 | http_client | 余额查询 |
| price_sync.rs:61 | fetch_price_table | 价格表拉取 |
| lib.rs:439 | platform_fetch_models | 模型列表拉取 |
| lib.rs:578 | 另一 client | 其他 API |

## 验证

- `cargo build` 通过 (需 reqwest socks feature)
- `tsc --noEmit` 通过
- SOCKS5 proxy 设置后，上游请求实际走代理 (抓包验证)
- 平台级 proxy_enabled=false 时，该平台不走代理
- 无 proxy 配置时行为与改动前完全一致
