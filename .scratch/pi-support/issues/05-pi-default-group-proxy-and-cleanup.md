# 05 — 默认分组、出站代理、分组删除清理

**What to build:** 用户把某个 Group 标为 Default Group 之后，直接敲裸 `pi` 就走该组，不必带任何参数；取消标记后恢复原状，而用户自己手设的默认 provider 不会被误删。用户配的出站 HTTP 代理对 pi 生效。删掉一个 Group，pi 的配置里不会留下它的僵尸 provider。

**默认组。** 写进 pi 全局 `settings.json` 的 `defaultProvider`，值为 `aidog-<默认组>`。取消时**只在该键的值是 `aidog-` 开头时才删**，用户手设的值保留 —— 与现有 Codex 默认 profile 移除逻辑同一守卫。

**出站代理。** pi 有原生的 HTTP 代理设置项，比 Codex 那种靠进程环境变量的做法干净，直接写进同一个 `settings.json`。

**写入语义。** merge 而非替换：aidog 只碰 `defaultProvider` 与代理这两个键，用户 `settings.json` 里其余内容（包括 aidog 不认识的键）原样保留。

**分组删除清理。** 按名字前缀扫 `aidog-*` provider，不在存活 Group 集合里的删掉；pi 内置与用户自建 provider 永不触碰 —— 与现有 Codex profile 清理同形。

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] 标记 Default Group 后，pi 的 `defaultProvider` 指向该组的 provider
- [ ] 取消 Default Group 后该键被移除；若用户手设过非 `aidog-` 的值，该值保留
- [ ] 出站代理写进 pi 原生代理设置项
- [ ] 对一份已有无关键的 `settings.json` 做写入，那些键在写入后完好无损
- [ ] 删除 Group 后其 `aidog-<group>` provider 消失
- [ ] 清理过程中 pi 内置 provider 与用户自建 provider 完好无损
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
