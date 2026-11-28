# 「钩子/规则没拦住」类报告：先查会话记录里的实际决策

用户报「没拦截」时，先在会话 jsonl（`~/.claude/projects/<项目>/*.jsonl`）里 grep 对应 tool_use 前后的
`permissionDecision` / `reasonCode`：钩子返回 ask 而后被人点放行，属人为放行，不是规则漏洞。

确认是规则漏洞再改：ask 规则按命令前缀匹配（如 `Bash(mysql:*)`），`docker exec … mysql` 这类嵌套命令
匹配不到，只能靠钩子拦；`src-tauri/defaults/settings/*.json` 改完跑 `yarn check:defaults` 验 JSON。

## hook 事件能否注入/阻断：速查

写预设钩子（`src-tauri/defaults/settings/*.json`）前先对能力选事件：能注入上下文或阻断的只有
PreToolUse（permissionDecision 可 deny/ask）、UserPromptSubmit（additionalContext 可注入）、
SessionStart / PostToolUse 等同步事件；InstructionsLoaded、CwdChanged、Stop 等异步事件的
JSON 输出（含 systemMessage / additionalContext）被 Claude Code **丢弃**，只适合做通知/落盘类副作用。
出处：code.claude.com/docs/zh-CN/hooks「决策控制」节。靠提示文案影响行为的钩子，别挂到异步事件上。
