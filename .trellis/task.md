# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| upstream-gzip-decompress | 修复非流式 gzip 上游响应未解压致 token/成本=0 + 日志乱码 | — | 已完成 | 收尾 | 100% | .worktrees/06-17-upstream-gzip-decompress |
| passthrough-upstream-resp-headers | 非流式回客户端透传上游响应头(选择性剔除压缩/长度/逐跳头) | — | 进行中 | 实施 | 30% | .worktrees/06-17-passthrough-upstream-resp-headers |
