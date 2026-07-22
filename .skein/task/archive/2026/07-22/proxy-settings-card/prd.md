# Coding 设置添加代理设置卡片 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 在 AppSettings「Coding 设置」tab 新增「代理设置」卡片，支持配置 HTTP_PROXY / HTTPS_PROXY / ALL_PROXY / NO_PROXY 四个出站代理环境变量，写入 Claude Code CLI 的 settings.json `env` 段，使 Claude Code 启动时自动注入 process env 走指定代理。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内：CodingToolsSettings.tsx 加代理卡片（4 输入 + 即时保存 on blur），写 claude env（复用 writeClaudeConfigField）；i18n 8 locale 全覆盖。
- [ ] 范围外（非目标）：Codex config.toml 代理同步。Codex CLI 无原生 proxy config 字段（官方 issue #4242/#6060 未实现），codex 侧代理改由「复制启动命令」注入 env，属另一功能点，本任务不做。
- [ ] 约束：遵循 runCommit 乐观翻转 + dirtyRef 并发态模板；数值格式化沿用 utils/formatters（本卡片为 URL 字符串无格式化需求）；与现有 5 项卡片视觉一致（glass-surface）。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn build 通过（tsc + vite 编译无误）
- [ ] scripts/check-i18n.mjs 通过（8 locale codingTools.proxy.* 全覆盖）
- [ ] yarn test 全过（无回归）
- [ ] 代理卡片 4 输入 onChange 更新本地 draft，onBlur 触发 writeClaudeConfigField 写 claude env.HTTP_PROXY/HTTPS_PROXY/ALL_PROXY/NO_PROXY；空值删除键；trim 后无变化不触发写入
- [ ] 失败回滚 + 常驻错误态（复用 runCommit revert/error 路径）

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list proxy-settings-card`)
