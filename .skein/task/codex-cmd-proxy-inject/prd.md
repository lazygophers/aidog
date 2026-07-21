# Codex 启动命令注入代理 env — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] Codex CLI 无 config.toml 级 proxy 字段（官方 issue #4242/#6060 未实现），其出站代理靠 process env。在「复制 Codex 启动命令」时，把 Coding 设置代理卡片写入 claude settings.json env 段的 HTTP_PROXY/HTTPS_PROXY/ALL_PROXY/NO_PROXY 自动追加为前置 `export` 语句，使 codex 子进程继承代理设置。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内：2 个 caller（GroupListItem.tsx 复制菜单 + GroupEditPanel.tsx 复制按钮）合并代理 envVars；新建 domains/groups/proxy-env.ts（PROXY_ENV_KEYS 常量 + loadProxyEnvVars + useProxyEnvVars hook）；CodingToolsSettings.tsx 复用共享常量去重。
- [ ] 范围外：不改 buildCodexCommand 纯函数签名（已支持 envVars）；不动 group.env_vars 用户数据；不为代理冲突做 UI（全局代理覆盖 group 级，YAGNI）。
- [ ] 约束：合并顺序 [...group.env_vars, ...proxyVars]（全局代理 export 在后，shell 后者覆盖前者，保证代理生效）；代理 envVars 读失败静默（不阻塞命令复制，fallback 到无代理）。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn build 通过
- [ ] scripts/check-i18n.mjs 通过
- [ ] yarn test 全过
- [ ] 新建 domains/groups/proxy-env.ts 导出 PROXY_ENV_KEYS / loadProxyEnvVars / useProxyEnvVars
- [ ] GroupListItem 复制 Codex 命令含代理 export（claude env 配置后）
- [ ] GroupEditPanel 复制 Codex 命令同上
- [ ] CodingToolsSettings.tsx 改用共享 PROXY_ENV_KEYS（去重，无行为变化）

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list codex-cmd-proxy-inject`)
