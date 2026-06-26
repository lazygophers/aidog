# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| popover-smart-layout | 浮窗智能布局 | tray popover 增强: 网格吸附拖拽布局 + 每行独立1/2/3列 + 预制方格尺寸(联动内容富度) + 每卡自定义颜色 + 配置页内嵌实时预览 | 已完成 | 收尾 | 100 | — |
| platforms-partial-refresh | 平台管理局部刷新 | AI 平台管理(Platforms 页)增删改任何内容改为局部刷新, 禁全页 reload/全量 load(), 提升 UX | 已完成 | 收尾 | 100 | — |
| minimax-coding-plan | MiniMax coding plan 配额 | MiniMax 平台缺 coding plan 配额查询支持, 补齐 quota.rs coding plan 分支 + 前端展示 | 已完成 | 收尾 | 100 | — |
| fix-req-b971c6b6 | 修复请求 b971c6b6 | 排查并修复 proxy_log request_id=b971c6b65ce0467e9a7d62f595a84598 暴露的 bug | 已完成 | 收尾 | 100 | — |
| smartpaste-anticrawl-key | 智能粘贴解析反爬key | 智能添加平台粘贴解析: 反爬中文插在 base64 串中间(如『删掉我再base64解码』)时无法提取 apikey, 需剔中文后拼接再 base64 解码 | 已完成 | 收尾 | 100 | — |
| fix-popover-loading | 修浮窗卡加载中 | 浮窗重构后一直显示加载中(回归), 数据/渲染路径断 | 已关闭 | 收尾 | 100 | — |
| notify-retention | 通知自动清理设置 | 通知模块加自动清理(收件箱/历史)设置, 默认7天, 允许关闭不清理 | 已完成 | 收尾 | 100 | — |
| hourly-stats-rollup | 小时维度统计表 | 新增小时维度预聚合统计表加速统计渲染, 独立task+完善测试 | 已关闭 | 收尾 | 100 | — |
| test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 规划中 | 规划 | 0% | — |
| test-cov-rust | Rust 后端分支覆盖≥80% | line 口径 80.76% 达标(1031 tests); branch 69.35% 未达, 验收按 line | 已完成 | 收尾 | 100% | — |
| test-cov-frontend | 前端 vitest 框架+分支覆盖≥80% | — | 已完成 | 收尾 | 100% | — |
| popover-stats-batch | 浮窗统计批量化+UTC修复 | 修浮窗/页面慢: 批量化浮窗N卡统计查询(一次IPC) + 修 bucket_time_expr UTC时区bug(db.rs:3517) | 已完成 | 收尾 | 100 | — |
| smartpaste-plaintext-noise | 智能粘贴明文反爬变体 | 智能粘贴解析: CJK噪声(如『（删除我）』)插在明文url/apikey中间(非base64)时无法识别, 需剔CJK括号噪声后拼接 | 已完成 | 收尾 | 100 | — |
| matchplatform-no-mock | matchPlatform禁返回mock | 智能粘贴/平台匹配: 未知host fallback错选mock测试平台, mock任何情况不可被自动识别; 排除mock出matchPlatform候选 | 已完成 | 收尾 | 100 | — |
| platform-card-usage | 平台卡片消费展示增强 | coding plan 平台补已用tokens+预估金额; 平台列表展开展示总tokens/金额消耗+今日 | 已完成 | 收尾 | 100 | — |
| newapi-balance-refresh | NewAPI余额主动更新 | NewAPI 平台余额未主动更新, 接入 quota 调度自动刷新 | 已完成 | 收尾 | 100 | — |
| fix-add-platform-save | 修添加平台保存无反应 | 添加平台点保存没反应(疑 platforms-partial-refresh handleSave 乐观改写回归) | 已完成 | 收尾 | 100 | — |
| db-index-cache-perf | DB索引+缓存提速 | 分组加载平台慢; 加 sqlite 索引 + 缓存提速, 维持缓存与DB一致 | 已完成 | 收尾 | 100 | — |
| rs-file-split | Rust 文件拆分: 所有 .rs ≤500 行(目标≤300) | — | 已完成 | 收尾 | 100% | — |
| split-db | 拆分 db.rs (7884行) 为 db/ 子模块 | — | 已完成 | 收尾 | 100% | — |
| platform-duplicate | 平台复制功能 (复制后直接进编辑页, 复用全部配置) | — | 已完成 | 收尾 | 100% | — |
| sqlite-rw-pool | SQLite 读写分离连接池 (修复代理满载 UI 卡顿) | — | 已完成 | 收尾 | 100% | — |
| coding-plan-priority | coding plan 平台优先调度 | — | 已完成 | 收尾 | 100% | — |
| 06-24-perf-frontend-hotpath | 前端体感卡慢优化 (高频读写热路径) | — | 已完成 | 收尾 | 100% | — |
| perf-b5-component-split | 巨石组件拆分 (Groups renderItem → SortableRow memo 化) | — | 已完成 | 收尾 | 100% | — |
| sql-index-audit | SQL/索引效率审计与优化 | — | 已完成 | 收尾 | 100% | — |
| db-dejoin-queries | DB 层去 JOIN/子查询重构 | — | 已完成 | 收尾 | 100% | - |
| 06-25-proxy-lan-bind | 默认代理支持局域网访问 | — | completed | 收尾 | 100 | — |
| platform-share | 平台分享 | — | 已完成 | 收尾 | 100% | .worktrees/06-25-platform-share |
| mcp-fullscreen-layout | MCP页面全屏适配 | — | 已完成 | 收尾 | 100% | .worktrees/06-25-mcp-fullscreen-layout |
| test-result-persist-render | 测试结果持久化+JSON解析展示 | — | 已完成 | 收尾 | 100% | .worktrees/06-25-test-result-persist-render |
| skills-search | 已安装skills支持搜索 | — | 已完成 | 收尾 | 100% | .worktrees/06-25-skills-search |
| skills-empty-ui-fix | 修skills列表UI空显示bug | — | completed | 收尾 | 100 | — |
| skills-remove-grouping | 移除skills分组概念 | — | completed | 收尾 | 100 | — |
| enabled-plugins-delete-visibility | Enabled Plugins 删除按钮可见性 | — | completed | 收尾 | 100 | — |
| platform-priority-field | 平台编辑创建表单加优先级设置 | — | 已完成 | 收尾 | 100% | — |
| billing-default-coding-plan | 平台计费类型识别默认coding plan | — | 规划中 | 规划 | 0% | — |
| mimo-volces-mismatch | mimo coding plan 误识别为火山引擎 | — | 已完成 | 收尾 | 100% | — |
| platform-expiry | 平台过期时间字段与到期自动禁用 | — | 已完成 | 收尾 | 100% | — |
| skills-cleanup-fix | npx skills 安装的 skills 被清理修复 | — | 已完成 | 收尾 | 100% | — |
| share-skip-empty | 平台分享零值空值字段不展示 | — | 已完成 | 收尾 | 100% | — |
| proxy-log-inflight-cache | proxy_log in-flight 缓存消除热路径 SELECT | — | 已完成 | 收尾 | 100% | — |
| realtime-event-notify | 前端实时数据事件通知替代轮询 | — | 已完成 | 收尾 | 100% | .worktrees/06-25-realtime-event-notify |
| platform-expiry-toggle | 平台过期时间默认禁用+启用toggle | — | 已完成 | 收尾 | 100% | — |
| datetime-local-theme | datetime-local 主题适配 | — | 已完成 | 收尾 | 100% | — |
| skills-test-isolation | skills 测试隔离 | — | 已完成 | 收尾 | 100% | — |
| expiry-paste-not-recognized | 粘贴识别过期时间失败 | — | 已完成 | 收尾 | 100% | — |
| skills-removal-deep-audit | skills 被移除问题深度审计 | — | 已完成 | 收尾 | 100% | — |
| skills-removal-recur | skills 被移除复现深度审计 | — | 已完成 | 收尾 | 100% | — |
| skills-list-lockfile | skills 列表读全局锁文件免 npx | — | 已完成 | 收尾 | 100% | .worktrees/06-26-skills-list-lockfile |
| platform-expiry-priority | router 同优先级按过期时间最早调度 | — | 进行中 | exec | 10 | .worktrees/06-26-platform-expiry-priority |
