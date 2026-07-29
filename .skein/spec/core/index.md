# Core 规则索引

硬约束 (必须遵守，后续必踩)。SessionStart hook 常驻注入。

| 类目 | 名称 | 描述 |
|---|---|---|
| build | [shadcn-infra-02](core/build/shadcn-infra-02.md) |  |
| domain | [resolve-price-now-ms-convention](core/domain/rule-66.md) | resolve_price 末位参数 now_ms 传值约定，违反导致定价口径分裂 |
| domain | [peak-multiplier-symmetry](core/domain/rule-67.md) | estimate 链中余额扣减+手动预算必须同步乘 peak 倍率，防止前后端不一致 |
| i18n | [rule-04](core/i18n/rule-04.md) |  |

**统计**: 4 条
