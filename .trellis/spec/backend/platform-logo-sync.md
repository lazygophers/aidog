---
updated: 2026-07-07
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Platform Logo Sync (三路 fallback)

何时被读: 改 `src-tauri/src/gateway/logo_sync.rs` / 协议 logo 缓存策略 / 新增 logo 数据源时
谁读: main / sub-agent
不遵守的代价: 三路顺序错乱致清空缓存后首屏回退首字母圆圈 / env proxy 递归环 hang / 0 字节响应污染缓存致永久 miss / 扩展名分裂浏览器拒渲染。

---

## 三路 fallback 顺序 (MUST, 首成功即止)

固定顺序, **禁重排**, 见 `sync_one_into`:

1. **simpleicons CDN** — 仅当 protocol 配 `logo_url` (=simpleicons slug, 如 `"anthropic"`, **非 URL**)。
   URL = `https://cdn.simpleicons.org/<slug>`, 默认返 PNG。
2. **厂商 favicon** — 从 `homepage` 提取域名 → `https://{domain}/favicon.ico`。
3. **clearbit logo api** — `https://logo.clearbit.com/{domain}` (末路; 隐私: clearbit 知用户访问品牌)。

- 路 2/3 依赖 `homepage` 域名; `extract_domain` 返 None → 直接返败, **禁跳过域名提取强试 clearbit**。
- 三路全败 → **不写缓存** (留空), 前端 fallback 首字母圆圈 (`ProtocolLogo.tsx`)。

## 缓存契约 (MUST)

- 缓存路径 `~/.aidog/logos/<protocol_id>.png` (`logo_cache_path`), 前端 `convertFileSrc` 消费。
- **命中判定 = 文件存在 + `metadata().len() > 0`** — 空文件视 miss 重下 (`sync_all_logos` / `sync_one_logo` 均查)。
- **统一 `.png` 扩展名** — simpleicons/clearbit 返 PNG, favicon 返 ICO; 后者强存 `.png`, 浏览器仍可渲染 (magic number 而非扩展名嗅探)。禁按 content-type 分扩展名。
- **`write_if_nonzero` 拒 0 字节响应** — 0 字节视失败, 三路都返空时不污染缓存 (防错误响应永久卡 miss)。

## HTTP client (MUST)

- **MUST 复用 `build_http_client_system`** (非 `build_http_client`) — logo 同步是系统向外部 CDN 请求, **禁 env proxy**。
  反直觉陷阱见 [http-client-forward.md](./http-client-forward.md): 读 `HTTPS_PROXY` env 若指向代理自身 → CONNECT 隧道无限递归 → h2 stream CANCEL。logo 同步虽非 forward 上游, 同理禁 env proxy 防用户环境变量递归。
- 超时 (20s connect / 10s read) 由 `build_http_client_system` 参数传入, 禁单造 client。

## presets JSON 读取 (MUST)

- `read_local_presets_json` 优先级: `~/.aidog/platform-presets.json` (运行时同步版本) → 缺失/损坏/空回退 `include_str!("../../defaults/platform-presets.json")` bundled。
  **同 `commands/defaults.rs::get_defaults_json` 优先级**, 但本模块独立 `include_str!` (避免循环引用, 不调 get_defaults_json)。
- `extract_protocols` 取每 protocol 的 `logo_url` (slug) + `homepage`; 字段缺失 `unwrap_or("")` 兜底, 不崩。
- 解析失败 **log warn 后 return** (sync_all_logos) / debug log return (sync_one_logo), **不抛错不阻塞 app 启动**。

## 入口

- `sync_all_logos(db, app_data_dir)` — 后台批量同步 (app 启动 / 手动触发); 遍历 presets 全 protocol, 命中 skip, miss 异步下。
- `sync_one_logo(db, app_data_dir, protocol_id)` — 前端懒加载 miss 时单点调; 命中 skip。

两入口共享 `sync_one_into` 三路逻辑, **禁在入口重复实现 fallback**。

## 验收基准 (可复用)

- [ ] 清空 `~/.aidog/logos/` 后, 有 `logo_url` 的 protocol 命中路 1; 无 slug 有 homepage 命中路 2; 都无 → 空, 前端首字母圆圈
- [ ] 三路全败时 `~/.aidog/logos/<id>.png` 不存在 (不被 0 字节污染)
- [ ] favicon (ICO) 存为 `.png` 浏览器正常渲染
- [ ] `cargo test` extract_domain / write_if_nonzero / logo_cache_path 三测过

## 验证命令

```bash
# 三路 URL 模板存在且顺序
grep -n "cdn.simpleicons.org\|/favicon.ico\|logo.clearbit.com" src-tauri/src/gateway/logo_sync.rs

# 命中判定 (size>0) 两入口都有
grep -n "meta.len() > 0" src-tauri/src/gateway/logo_sync.rs

# client 复用 (禁 env proxy)
grep -n "build_http_client_system" src-tauri/src/gateway/logo_sync.rs

cargo test --lib extract_domain write_if_nonzero logo_cache_path
```

## 关联

- [http-client-forward.md](./http-client-forward.md) — build_http_client `no_proxy()` 契约 (本模块用 _system 变体同源禁 env proxy)
- 前端消费: `src/domains/platforms/ProtocolLogo.tsx` + `useProtocolLogo.ts` (convertFileSrc + miss 调 sync_one_logo)
