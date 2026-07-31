# SKEIN 规则库总索引

三层: **core** 常驻注入 (SessionStart) · **recall** 按需召回 (planning `recall <query>`) · **external** 外部参考 (纯手动 CLI 检索, 不入 hook)。

| layer | 条数 | 类目分布 | 索引 |
|---|---|---|---|
| core | 4 | domain(4) | [core/index.md](core/index.md) |
| external | 0 | - | [external/index.md](external/index.md) |
| ops | 4 | sqlite(4) | [ops/index.md](ops/index.md) |
| recall | 659 | arch(116), build(62), cross-layer(12), db(23), domain(93), encoding(4), frontend(100), git(6), i18n(24), ops(5), optimization(36), proxy(41), reuse(6), shadcn(49), skein(24), style(18), test(12), testing(13), theme(5), ts-rust-boundary(10) | [recall/index.md](recall/index.md) |
