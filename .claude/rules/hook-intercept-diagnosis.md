# 「钩子/规则没拦住」类报告：先查会话记录里的实际决策

用户报「没拦截」时，先在会话 jsonl（`~/.claude/projects/<项目>/*.jsonl`）里 grep 对应 tool_use 前后的
`permissionDecision` / `reasonCode`：钩子返回 ask 而后被人点放行，属人为放行，不是规则漏洞。

确认是规则漏洞再改：ask 规则按命令前缀匹配（如 `Bash(mysql:*)`），`docker exec … mysql` 这类嵌套命令
匹配不到，只能靠钩子拦；`src-tauri/defaults/settings/*.json` 改完跑 `yarn check:defaults` 验 JSON。
