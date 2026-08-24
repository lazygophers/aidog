# 01: 统一引擎基座（Condition Tree + Action Chain）

**What to build:** 代理的中间件改写按新模型工作：一条规则 = 嵌套条件树 + 有序动作链 +
applies_to 过滤器。重建 `middleware_rule` 表（不做兼容：旧列废、conditions/actions/applies_to
JSON 列新），Rust 引擎按树求值 + 按链执行动作（block/classify 终止本链及后续规则、按
priority 累加、空 pattern 兜底全废），classify_error 独立路径折进 classify 动作并把
retryable/override 喂现有重试编排，proxy 挂接点全部改到统一路径。ts-rs 类型重生成。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 表重建后 CRUD/引擎在新模型上编译通过，旧 RuleType/RuleScope/子开关类型消亡
- [ ] 树求值测试：嵌套 ALL/ANY、contains/regex/exact、六种 target、混阶段拒绝
- [ ] 动作链测试：顺序执行、block/classify 终止一切、applies_to 空即全/any-of/priority 累加
- [ ] classify 命中产出 retryable/override_status/override_body，无命中走默认重试
- [ ] ReDoS 防护（size/dfa 上限、编译失败 fail-open）保留并有测试
- [ ] cargo test / clippy 全绿
