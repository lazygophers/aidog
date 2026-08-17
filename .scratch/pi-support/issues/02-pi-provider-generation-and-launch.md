# 02 — 一个分组跑通 pi：provider 生成 + 启动命令

**What to build:** 用户在 Group 卡片上点「复制 pi 启动命令」，粘进终端，pi 真的通过 aidog 路由到该 Group 的上游。这是整个 pi 支持的 tracer bullet —— 打通端到端，后续票在此之上加维度。

aidog 为每个 Group 在 pi 的全局 `models.json` 里生成一个名为 `aidog-<group_key>` 的 pi Provider：`baseUrl` 指向本地代理，`apiKey` 直接写 Group 名，`authHeader` 置 true，于是 pi 发 `Authorization: Bearer <group>`，aidog 据此路由。不注任何环境变量，因此 pi 的 `auth.json` 无法覆盖（它按 provider id 索引，`aidog-*` 不与内置 id 冲突）。详见 `docs/adr/0001-pi-group-mapping-via-custom-providers.md`。

本票只做 anthropic 一种线路协议，`baseUrl` 用代理根地址**不带版本后缀**（pi 的 Anthropic SDK 自己补路径；pi 官方文档此处示例是错的，以其源码常量为准）。多协议留给 03。

aidog 只写自己的 `aidog-*` provider，pi 的内置 provider 与用户自建 provider 原样保留。

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] 后端有一个纯函数，输入 Group 集合与代理信息，输出完整的 pi 配置文件内容；写盘是它外面一层薄壳
- [ ] 每个 Group 恰好产出一个 `aidog-<group_key>` provider，命名可预测
- [ ] Group 名落在 `apiKey`，`authHeader` 为 true
- [ ] anthropic 协议下 `baseUrl` 不带版本后缀
- [ ] 已存在的 `models.json` 中，pi 内置与用户自建 provider 在写入后完好无损
- [ ] 前端有一个纯函数产出 pi 启动命令，Group 名做 shell 引号转义，Group 的环境变量以前置 export 注入
- [ ] Group 卡片出现「复制 pi 启动命令」入口，与现有 Claude / Codex 入口并列
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
