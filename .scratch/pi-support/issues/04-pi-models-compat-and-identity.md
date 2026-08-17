# 04 — 模型清单、请求兼容开关、客户端身份

**What to build:** 用户在 pi 里按 Ctrl+L 或 `/model` 打开模型列表，看到的是这个 Group 真正能路由到的模型，选中即可用，不会选到打不通的；请求发出后上游不会因为 pi 特有的字段而报错；上游日志里能看出这条请求来自 pi。

三件事合在一张票，因为它们都是「让生成的 provider 的内容变完整」：

**模型清单。** pi 规定非内置 provider 必须自带 `models` 数组才能选模型。取该 Group 各 Platform 的有效模型并集；Group 一个模型都没有时，回落到该协议 preset 的默认候选清单，避免生成一个没有任何模型的废 provider。

**请求兼容。** pi 默认发上游不一定认的东西：每个工具带 eager input streaming 标记；prompt caching 的 `cache_control` 在非官方 baseUrl 下默认带长 ttl；缓存开启时可能带 session affinity 头。两边都兜：生成的 provider 写 `compat` 开关让 pi 不发；aidog 转换层另做容忍，万一 pi 升级后行为变化也不炸。

**客户端身份。** pi 只在一个内置 provider 下才设自己的 User-Agent，自定义 provider 会落到匿名 SDK 默认 UA。aidog 在生成的 provider 的 `headers` 里显式写 `pi (<platform> <release>; <arch>)` 形态的 User-Agent，并在 client-type 表里加对应的 `pi_cli` 条目，两侧对齐。

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] provider 的 `models` 数组来自 Group 有效模型并集，去重
- [ ] Group 无有效模型时回落 preset 默认候选，产出的 provider 仍可用
- [ ] 生成的 provider 带 `compat` 开关，关掉 eager tool input streaming 与长缓存保留
- [ ] 转换层对上述 pi 特有字段做容忍，有测试覆盖「pi 照发不误」的情形
- [ ] provider 的 `headers` 含显式 User-Agent，格式与 pi 自身的 UA 形态一致
- [ ] client-type 表新增 `pi_cli` 条目，8 语言名称与描述齐
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
