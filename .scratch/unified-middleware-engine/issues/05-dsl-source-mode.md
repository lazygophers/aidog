# 05: DSL 源码模式（树 JSON 之上的视图）

**What to build:** 规则编辑器可切换到 DSL 源码模式直接写条件表达式（AND/OR/() + 匹配算子），
与组卡片双向同步；树 JSON 是唯一存储真值，DSL 解析失败时禁止保存和切回卡片模式并给出
定位提示。DSL 解析器只在前端，Rust 不感知。

**Blocked by:** 04 条件树组卡片编辑器

**Status:** done

- [x] 树 → DSL、DSL → 树往返一致（round-trip 测试）
- [x] 非法 DSL 阻止保存/切换并显示错误位置
- [x] 组件测试覆盖切换、错误态
- [x] yarn build / yarn test 全绿
