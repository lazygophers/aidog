# 后端可观测性与 i18n 三处不一致清理 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 消除每个 Tauri command 调用打两条相同 `command invoked` debug 日志的重复 —— 宏已自动发一条, 命令体内又手写了一条, 属宏迁移未清干净的遗留; 重复日志放大 debug 档日志体积并干扰按 command 计数的排查
- [ ] 补齐后端错误消息的西班牙语 —— 前端支持 8 语言含 es-ES, 后端语言枚举只有 7 个变体且解析函数无 es 分支, 西语用户拿到的代理错误消息静默落英文
- [ ] 提升异步任务的 tracing 覆盖 —— 项目已有带 tracing 上下文的 spawn 封装, 但裸 spawn 仍占多数, 这些任务里的日志丢 trace_id, 跨任务串链断裂

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 只动 Rust 后端 (`src-tauri/`), 不改前端、不改 i18n 的前端 locale 文件
- [ ] 删重复日志行时**禁改 command 函数体的其他任何语句**, 只删与宏重复的那一行; 若某处手写行携带了宏没有的额外字段 (如业务参数), 保留该行并在报告中单列
- [ ] 补语言变体只补后端错误消息层, 不碰前端 locale 标签命名空间 —— 该处存在「4 套 locale 命名空间禁统一」的既有硬约束 (Claude CLI 的 region 标签与应用的 script 标签是有意分离的), 禁顺手统一
- [ ] 西语文案由本任务翻译产出, 参照既有语言变体的同一批 key, 不新增 key、不改 key 名
- [ ] spawn 封装替换只做等价替换, 禁借机改任务的并发语义 (禁改 spawn 为 spawn_blocking、禁调整 join/detach 行为)
- [ ] 不碰 `#[tracing::instrument]` 已覆盖的同步路径
- [ ] 三项彼此独立, 任一项受阻不阻塞其余两项

## User Stories
极其详尽地穷举, 覆盖功能各方面 (含边界情况) —— 穷举本身就是逼出边界情况的机械手段:
1. As a 排查线上问题的开发者, I want 每个 command 调用只出现一条 `command invoked` 日志, so that 按 command 名计数时数字就是真实调用次数, 不用先除以 2
2. As a 排查线上问题的开发者, I want 日志里不再有成对重复行, so that debug 档日志体积不被无效行撑大
3. As a 西语用户, I want 代理转发失败时看到西班牙语错误消息, so that 与我在界面上选的语言一致, 而不是突然跳出英文
4. As a 维护者, I want 语言解析函数对 `es-ES` / `es_ES` / `es` 三种写法都归一到同一变体, so that 与既有 7 个变体的容错口径一致
5. As a 维护者, I want 未知 locale 仍回落英文, so that 补变体不改变兜底语义
6. As a 排查跨任务问题的开发者, I want 异步任务内的日志带 trace_id, so that 能把一次请求触发的后台任务与请求本身串起来
7. As a 维护者, I want 替换后异步任务的启动/结束/panic 行为与替换前一致, so that 可观测性改造不引入行为变更
8. As a 维护者, I want 明确知道哪些裸 spawn 是**有意**不加 tracing 的 (如启动期、无请求上下文的常驻任务), so that 覆盖率不追求 100% 而是有判据的

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] 全仓搜 `command invoked` 字面量, 除宏定义处外零命中; 若有保留项, 每一条在报告中列出保留理由 (携带宏没有的额外字段)
- [ ] 随机抽 3 个 command 实际调用, 日志中该字面量各只出现 1 次
- [ ] 语言枚举含西语变体, 解析函数对 `es-ES` / `es_ES` / `es` 三种输入均返回该变体, 有单测覆盖三种写法
- [ ] 未知 locale 仍返回英文变体, 既有兜底单测不变且仍绿
- [ ] 后端错误消息的每个 key 都有西语文案, 无空串、无占位符残留、无直接复制英文
- [ ] 裸 spawn 的替换清单已产出: 每处标注「已替换」或「有意保留 + 理由」, 无未分类项
- [ ] 替换后异步任务的并发语义零变更 (无 spawn→spawn_blocking、无 join/detach 改动), 可由 diff 逐条核验
- [ ] `cargo clippy --workspace --all-targets` 零 warning
- [ ] `cargo test --workspace` 全绿 (已知 flaky 例外: 依赖网络的 quota http 单测, 单跑通过即可)
- [ ] 三项改动分别独立可回滚 (各自成 commit, 不混提交)

## Testing Decisions
什么算好测试 (只测外部行为不测实现细节) / 测哪些模块 / codebase 内的同类测试先例:
- [ ] 语言变体: 测 `from_locale` 的输入→变体映射与消息取词结果 (外部行为), 不测枚举内部布局; 直接扩展该模块既有的解析单测, 沿用同一测试风格
- [ ] 重复日志: 不新增单测 (日志行不是外部契约), 以 grep 断言 + 人工抽样调用为验收手段
- [ ] spawn 替换: 不为替换本身写测试 (等价替换无新行为), 依赖既有测试全绿证明无回归; 替换清单作为可核验产出物
- [ ] 三项均不引入新测试框架、不新增 fixture

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list backend-observability-cleanup`)
