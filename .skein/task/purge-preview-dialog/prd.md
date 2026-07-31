# 清理失效平台弹窗展示待清理清单与原因 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
- [ ] 「清理失效平台」确认弹窗展示待清理平台的完整清单，每项含平台名与清理原因，用户点确认前能看清会删掉什么
- [ ] 清单数据由新增后端 preview command 提供，与实际删除复用同一套 SQL 筛选条件，杜绝「弹窗列 3 个实际删 5 个」
- [ ] 分组级入口额外区分处置动作：独占本分组 = 永久删除，共享多分组 = 仅移除本分组关联
- [ ] 清单为空时弹窗显示「无失效平台」空态，确认键禁用
- [ ] 用户价值：不可撤销的批量删除操作从盲点确认变为可核对确认
## 边界
- [ ] 范围内：新增 platform_purge_disabled_preview command (Rust) + TS api 封装 + 两处弹窗 UI (PlatformListView.tsx 全局 / GroupListItem.tsx 分组) + 8 语言 i18n key
- [ ] 范围内：清理原因码枚举化 —— 后端返回稳定 reason code (如 auth_failed / expired)，前端 i18n 映射为文案，禁后端拼中文
- [ ] 范围内：分组级 preview 需返回每项处置动作 (delete / unassign)，与 PurgeResult 的 deletedIds/unassignedIds 语义对齐
- [ ] 范围外：不改动实际清理逻辑 purge_auto_disabled_platforms 的筛选条件或删除语义 (纯只读预览 + UI 展示)
- [ ] 范围外：不做 preview 与 purge 之间的 TOCTOU 二次比对 —— 单机桌面应用，两次调用间隔内平台状态变更概率可忽略，不引入版本号/快照机制
- [ ] 范围外：不改按钮常驻置灰逻辑 (需常驻预查，已裁定走弹窗内空态)
- [ ] 约束：弹窗必须 createPortal(document.body)，禁原生 confirm (CLAUDE.md 硬规)
- [ ] 约束：筛选条件必须与 platform_lifecycle.rs 现有 SQL 同源，禁在 preview 里抄第二份条件
## User Stories
极其详尽地穷举, 覆盖功能各方面 (含边界情况) —— 穷举本身就是逼出边界情况的机械手段:
1. As a <actor>, I want <feature>, so that <benefit>

## 验收标准
- [ ] 新增 command platform_purge_disabled_preview 已注册进 startup.rs generate_handler!，接受 group_id: Option<u64>，返回每项含 id / name / reason code / action
- [ ] preview 与 purge 的筛选条件同源 (提取共用 SQL 或共用查询函数)，有测试证明两者返回的平台 id 集合一致
- [ ] 全局入口弹窗列出待删平台名 + 原因文案；分组入口额外区分「永久删除」与「仅移出本分组」两类
- [ ] 清理原因至少覆盖 HTTP 401/403 认证失效 与 已过期 两类，后端返 code 前端 i18n 映射，无硬编码中文出后端
- [ ] 清单为空时弹窗显示空态文案且确认键 disabled
- [ ] 8 个 locale 文件新增 key 全部齐全，scripts/check-i18n.mjs 通过
- [ ] cargo clippy 零 warning、cargo test 通过、yarn build 通过、yarn test 通过
## Testing Decisions
什么算好测试 (只测外部行为不测实现细节) / 测哪些模块 / codebase 内的同类测试先例:
- [ ] 唯一新增接缝：`src-tauri/crates/aidog_core/src/gateway/db/test_platform_lifecycle.rs` 加一条「preview 与 purge 同源」测试 —— 同一 DB 状态下 preview 返回的 id 集合 == purge 实际处理集合 (deletedIds ∪ unassignedIds)，且每项 action 与它实际落到哪个集合一致。这一条同时覆盖「筛选条件不漂移」与「分组独占/共享分流正确」两个最关键风险
- [ ] 先例：同文件 `:133` 已有一键清理三场景用例 (全局全删 / 分组独占删 / 分组共享仅移除关联)，新测试沿用其 DB fixture 与断言风格，不新建测试基建
- [ ] 只测外部行为：断言口径是「哪些平台会被怎么处理」，不断言共用函数的内部签名、SQL 文本或调用次数 —— 重构实现不应弄红测试
- [ ] 不测的部分：前端弹窗列表是纯数据映射 (preview 结果 → 列表行)，无独立逻辑，由 `yarn build` 类型检查 + `scripts/check-i18n.mjs` key 齐全性覆盖，不写组件测试

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list purge-preview-dialog`)
