# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| popover-smart-layout | 浮窗智能布局 | tray popover 增强: 网格吸附拖拽布局 + 每行独立1/2/3列 + 预制方格尺寸(联动内容富度) + 每卡自定义颜色 + 配置页内嵌实时预览 | 进行中 | 实施 | 25 | 06-20-popover-smart-layout |
| platforms-partial-refresh | 平台管理局部刷新 | AI 平台管理(Platforms 页)增删改任何内容改为局部刷新, 禁全页 reload/全量 load(), 提升 UX | 规划中 | 规划 | 30 | — |
| minimax-coding-plan | MiniMax coding plan 配额 | MiniMax 平台缺 coding plan 配额查询支持, 补齐 quota.rs coding plan 分支 + 前端展示 | 规划中 | 规划 | 20 | — |
| fix-req-b971c6b6 | 修复请求 b971c6b6 | 排查并修复 proxy_log request_id=b971c6b65ce0467e9a7d62f595a84598 暴露的 bug | 已完成 | 收尾 | 100% | 06-20-fix-req-b971c6b6 |
| smartpaste-anticrawl-key | 智能粘贴解析反爬key | 智能添加平台粘贴解析: 反爬中文插在 base64 串中间(如『删掉我再base64解码』)时无法提取 apikey, 需剔中文后拼接再 base64 解码 | 规划中 | 实施 | 20 | 06-20-smartpaste-anticrawl-key |
