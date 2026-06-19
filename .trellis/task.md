# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| openai-pricing-scraper | OpenAI 一手定价 scraper 接入 developers.openai.com | — | 已完成 | 收尾 | 100% | — |
| xiaomi-coding-plan | 小米 coding plan 配额支持 | — | 已完成 | 收尾 | 100% | — |
| platform-presets-fill | 补齐所有平台预设 base_url 与默认模型 | — | 已完成 | 收尾 | 100% | .worktrees/06-17-platform-presets-fill |
| ccswitch-no-name-dedup | cc-switch 导入禁按 platform name 去重 (name 非唯一) | — | 已完成 | 收尾 | 100% | — |
| xiaomi-coding-variant | 小米 MiMo coding plan 平台变体 | — | 已完成 | 收尾 | 100% | .worktrees/06-17-xiaomi-coding-variant |
| platform-model-list | 平台内置模型列表供下拉选择 | — | 已完成 | 收尾 | 100% | .worktrees/06-17-platform-model-list |
| openai-apikey-header | openai 协议 api-key 头鉴权适配 (小米 token-plan) | — | 已完成 | 收尾 | 100% | .worktrees/06-17-openai-apikey-header |
| pricing-full-coverage | Pricing scraper 全平台覆盖 (7 first-party 平台补一手价) | — | 已完成 | 收尾 | 100% | — |
| merge-groups-platforms | 分组内嵌进平台页(侧栏只留平台) | 侧栏移除groups项只留platforms;Groups.tsx重构为GroupsEmbedded内嵌组件;Platforms列表视图顶部植入分组段+平台列表项加所属分组badge(N:N);onGroupsChanged回调刷新归属;后端不动 | 已完成 | 收尾 | 100 | .worktrees/06-18-merge-groups-platforms |
| group-full-platform-cards | 分组展开显示完整可展开平台卡片 | GroupsEmbedded分组展开区从badge tag改为完整PlatformCard(同Platforms页),复用PlatformCard+per-platform state(quota/usage/expanded/test);PlatformCard点选就地展开详情;分组头点击改为展开非进编辑;抽共享hook/组件 | 已完成 | 收尾 | 100% | — |
| group-platform-drag | 分组平台拖拽排序与跨组移动 | 分组展开区平台卡片支持拖拽: 组内重排(group_platform.priority) + 跨组拖拽(源组移除+目标组添加)。后端加 reorder/move 命令; 前端 Groups.tsx 重构为多容器 dnd-kit DnD(单外层 DndContext + 每分组 SortableContext droppable) | 已完成 | 收尾 | 100% | — |
| platform-schema-cleanup | platform 表 schema 清理: 删 auto_group + breaker_* 移入 extra | — | completed | 收尾 | 100 | — |
| batch-test-ux | 批量测试: 弹窗可中途关闭 + 并行数改3 | — | completed | 收尾 | 100 | — |
| platform-list-progressive-load | AI 平台列表页分阶段异步加载优化 | — | 已完成 | 收尾 | 100% | — |
| platform-card-last-test-badge | platform-card-last-test-badge | — | 已完成 | 收尾 | 100% | — |
