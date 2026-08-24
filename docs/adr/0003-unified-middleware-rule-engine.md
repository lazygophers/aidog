# ADR 0003: 统一中间件规则引擎（条件树 + 动作链）取代 8 类 RuleType

日期: 2026-08-24
状态: accepted

## 背景

aidog_middleware 原有 8 类 RuleType（request_filter / sensitive_word / redaction / content_filter /
dynamic_injection / response_override / rectifier / error_rule），每类独立的 type-specific config、
独立的 type_enabled 子开关、入站/出站各一套固定执行顺序（inbound.rs:17 / outbound.rs:26），
规则作用域是 CSS 级联式三级就近覆盖（platform 盖 group 盖 global，非累加）。

## 决策

1. **单一规则模型**：一条规则 = 嵌套条件树（ALL/ANY 递归，叶子 target+field+match_type+pattern）
   + 有序动作链（mask/block/warn/inject/override/classify）+ Applies To 过滤器
   （platforms/groups/models 数组，空=全部，规则间按 priority 累加执行）。
2. **废除三级就近覆盖**：统一模型下「就近覆盖」无法定义，改为过滤器数组累加语义。
3. **显式化，无隐藏兜底**：废除「content_filter 空 pattern 用内置检测器」「error_rule 空 pattern
   = 任意非 2xx」等隐式行为，一切条件显式书写。
4. **不做兼容迁移**：直接重建 `middleware_rule` 表；旧规则无法翻译的在列表标记为 Failed Rule
   引导用户手动删除；内置规则按 name 强制覆盖。
5. **Builtin Rule 只可启停**：不可编辑、不可删除；升级 seed 按 name upsert 覆盖内容、保留用户
   停用状态。
6. **error 进链**：classify 作为动作链内终止性动作（category/retryable/override_status/override_body），
   消掉非 2xx 独立分类路径。
7. **混阶段拒绝**：一条规则内条件叶子必须同属 request 侧或 response 侧。
8. **子开关删除**：只留总开关 + 每条规则 enabled。
9. **内置检测器迁出为 Builtin Rule**：AI token、邮箱、手机号（大陆+明确国际格式）、DB/Redis
   连接串与 key=value 密钥——只匹配特征明确的模式，拒绝高误伤模式。
10. **编辑器双模式**：递归组卡片为主 + DSL 源码模式；树 JSON 是唯一存储真值，DSL 是前端视图。
11. **流式日志完整记录**（并入本 feature）：聚合完整 SSE 落 proxy_log，废 `[stream]` 占位；
    断流也落已聚合部分；终态判定改显式 done 标记列。中间件不影响日志记录。

## 后果

- 流式（SSE）下动作降级：block 仅在首块转发前生效；mask/override 逐块替换（跨块漏匹配维持
  已知限制，滑窗后续）；其余动作流式侧不适用。
- `MiddlewareSettings.type_enabled`、`RuleType`、`RuleScope`、`MatchType`/`pattern`/`action` 单值
  列、`classify_error` 独立路径、「一键导入默认」前后端入口全部移除。
- ts-rs 生成类型与 `MiddlewareRules.tsx` 表单随模型重建。

## 备选方案

- 双引擎过渡（旧类型旧引擎继续跑）：长期双轨维护成本最高，弃。
- 固定双层 ALL/ANY（不嵌套）：实现更简，用户要求完整嵌套表达力，弃。
- 保留三级就近覆盖：统一模型下语义不可定义，弃。
