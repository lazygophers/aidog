# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| group-scoped-usage-stats | group-scoped-usage-stats | — | 已完成 | 收尾 | 100% | — |
| tray-popover-customizable-stats | tray-popover-customizable-stats | — | 已完成 | 收尾 | 100% | — |
| logs-action-col-sticky | Logs 表格操作列固定右侧 | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-13-logs-action-col-sticky |
| popover-opaque-no-white | Popover 圆角外白底消除（透明窗口+不透明本体） | — | 已完成 | 收尾 | 100% | — |
| model-test-coding-plan-fix | Model-test 补齐 coding_plan 处理 + endpoint 选择逻辑 | — | 已完成 | 收尾 | 100% | — |
| coding-plan-reset-countdown | Coding plan 展示重置倒计时 | — | 已完成 | 收尾 | 100% | — |
| list-item-height-relayout | 增加平台/分组列表 item 高度并重排布局 | — | 已完成 | 收尾 | 100% | — |
| swap-window-dimensions | 对调默认窗口长宽 | — | 已完成 | 收尾 | 100% | — |
| logo-prompt-design | AiDog APP logo 提示词设计 | — | 已完成 | 收尾 | 100% | — |
| kimi-coding-client-type-fix | 修复 Kimi Code Plan 预设 client_type (codex_tui→claude_code) | — | 已完成 | 收尾 | 100% | — |
| request-id-copy-button | 请求详情 Request ID 独立复制按钮 | — | 已完成 | 收尾 | 100% | — |
| anthropic-inbound-parse-fail | 修复 anthropic 入站请求解析失败 (ContentBlock 未覆盖类型致 400) | — | 已完成 | 收尾 | 100% | — |
| request-response-middleware | 请求/响应中间件规则引擎 (整流/覆写/脱敏/注入/过滤/敏感词/错误检测) | — | 已完成 | 收尾 | 100% | — |
| mw-rule-engine-core | 中间件 C1 规则引擎基座 (表/模型/CRUD/缓存/作用域/契约) | — | 已完成 | 收尾 | 100% | — |
| mw-inbound-execution | 中间件 C2 入站规则执行 (过滤器/敏感词/脱敏/内容过滤/动态注入) | — | 已完成 | 收尾 | 100% | — |
| mw-outbound-breaker | 中间件 C3 出站规则执行 + 熔断器 + 流式逐块 | — | 已完成 | 收尾 | 100% | — |
| mw-builtin-presets | 中间件 C4 内置预设规则集 (密钥/邮箱/手机 + 默认 error_rules) | — | 已完成 | 收尾 | 100% | — |
| mw-frontend-ui | 中间件 C5 前端 UI + i18n (AppSettings tab + group/platform 嵌入) | — | 已完成 | 收尾 | 100% | — |
| group-scheduling-breaker | Group 智能调度与熔断器 (组内配置 + 默认值) | — | 已完成 | 收尾 | 100% | — |
| gsb-backend | GSB 后端 熔断器+智能调度+指标+集成+契约 | — | 已完成 | 收尾 | 100% | — |
| gsb-frontend | GSB 前端 Platform/Group/全局 UI + i18n | — | 已完成 | 收尾 | 100% | — |
| system-notification | 系统通知模块 (TTS播报/弹窗/收件箱 + Codex&ClaudeCode hook 快捷入口) | — | 已完成 | 收尾 | 100% | — |
| notif-backend | 通知 N1 后端核心 (服务/TTS/弹窗/收件箱/端点/契约) | — | 已完成 | 收尾 | 100% | — |
| notif-hooks | 通知 N2 hook 集成 (脚本生成 + 一键注入 ClaudeCode/Codex) | — | 已完成 | 收尾 | 100% | — |
| notif-frontend | 通知 N3 前端 (设置 UI + 通知中心 + i18n) | — | 已完成 | 收尾 | 100% | — |
| subagent-statusline-dynamic | SubagentStatusLine 首段动态化 (type·status·model 驱动, 替代固定 [Agent·●]) | — | 已完成 | 收尾 | 100% | — |
| subagent-statusline-name | SubagentStatusLine name 段 fallback 对齐 (.label//.name//.id//'?') | — | 已完成 | 收尾 | 100% | — |
| subagent-statusline-debug | 修复 SubagentStatusLine 真实输入失败 (模拟真实 Claude Code stdin 测正确性) | — | 已完成 | 收尾 | 100% | — |
| docs-readme-i18n | 优化 README/docs + 文档地址引入 + README 多语言 | — | 已完成 | 收尾 | 100% | — |
| docs-new-features-i18n | 新功能 docs 补全 (middleware/熔断/通知) × 7 语言 | — | 已完成 | 收尾 | 100% | — |
| docs-mw-zh-en | middleware docs zh+en (独立章节) | — | 已完成 | 收尾 | 100% | — |
| docs-breaker-zh-en | breaker/scheduling docs zh+en (入 groups) | — | 已完成 | 收尾 | 100% | — |
| docs-notif-blocked | notif docs (blocked, 等 system-notification 完成) | — | 已完成 | 收尾 | 100% | — |
| settings-submenu-nav | 设置 tab 重构为侧栏子菜单 | — | 已完成 | 收尾 | 100% | — |
| i18n-locale-unify | i18n 多端 locale 统一 + 缺失补全 | — | 已完成 | 收尾 | 100% | — |
| i18n-frontend-hardcoded | 前端 tsx 硬编码中文 i18n 化 | — | 已完成 | 收尾 | 100% | — |
| coding-tier-depleted-red | 修复 coding plan 配额耗尽(util≥100)配色应为红 | — | 已完成 | 收尾 | 100% | — |
| skills-management | 基于 npx skills 的 skills 管理模块 | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-management |
| i18n-coverage-hardening | i18n 全覆盖硬化: 修复 settings 等裸 key + 自动检查脚本 + spec 规则 | — | 已完成 | 收尾 | 100% | — |
| skills-ui-refine | Skills 模块 UI 修订 (agent 图标化 + scope 默认全局) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-ui-refine |
| skills-list-layout | Skills 页布局修正 (列表=已装/catalog仅搜索出/agent移出筛选) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-list-layout |
| i18n-labelkey-blindspot | i18n labelKey 盲区修复: t(变量) 路径 key 缺失 + check-i18n D 检查 | — | 已完成 | 收尾 | 100% | — |
| menu-scroll-support | 菜单容器滚动支持: 高度不足时显示滚动条 | — | 已完成 | 收尾 | 100% | — |
| skills-unified-toggle | Skills 列表重构 (统一不分agent + per-item启用切换 + 总计样式) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-unified-toggle |
| fix-skills-enable | 修复 skills enable 失败 (path 替代锁文件 source + 前端弹错) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-fix-skills-enable |
| import-export | 导入导出功能（加密单文件 + 自动化导入） | AES-256-GCM 容器 + 7 scope + 逐项冲突 + skills 自动安装 | 已完成 | 收尾 | 100% | — |
| notif-hook-default-inject | Claude Code 默认设置补通知 hook 注入 + 快捷创建 | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-notif-hook-default-inject |
| menu-submenu-collapse | 修复侧栏子菜单展开后无法收起 | — | 已完成 | 收尾 | 100% | — |
| hooks-event-collapse | 修复 Claude Code hooks 单个事件无法收起 | — | 已完成 | 收尾 | 100% | — |
| fix-lint-skills-sync | 修复 make lint: skills_sync 引用过时 source 字段 | — | 已完成 | 收尾 | 100% | — |
| scripts-py-uv | 生成脚本 sh→python+uv(PEP723) + 移到 ~/.aidog/scripts/ | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-scripts-py-uv |
| skills-toggle-ux | Skills 启用/关闭乐观更新 (去全列表刷新闪烁) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-toggle-ux |
| show-version | 页面展示版本信息 | — | 已完成 | 收尾 | 100% | — |
| skills-npx-proxy | npx skills 走 aidog 上游代理 | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-skills-npx-proxy |
| remove-installed-title | Skills 页移除已安装标题 card | — | 已完成 | 收尾 | 100% | — |
| skills-show-desc | Skills 展示每条 desc 字段 | — | 已完成 | 收尾 | 100% | — |
| statusline-py-uv | statusline 脚本 sh→python (golden-output 回归) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-statusline-py-uv |
| skills-uninstall-all | Skills 一键卸载所有平台所有 skills | — | 已完成 | 收尾 | 100% | — |
| skills-align-agents | Skills 对齐两 agent 配置快捷方式 | — | 已完成 | 收尾 | 100% | — |
| skills-enable-all | Skills 一键全启用某 agent | — | 已完成 | 收尾 | 100% | — |
| notif-no-read-state | 通知去已读未读/完成即结束 | — | 已完成 | 收尾 | 100% | — |
| skills-uninstall-single | Skills 单一 skill 卸载功能 | — | 已完成 | 收尾 | 100% | — |
| deep-perf-optimization | 深度性能优化 | — | 已完成 | 收尾 | 100% | — |
| fix-skills-uninstall-noop | 修复单一 skill 卸载无反应 | — | 已完成 | 收尾 | 100% | — |
| perf-backend-hotpath | perf 后端热路径+数据层(问题1/2/3/6) | — | 已完成 | 收尾 | 100% | — |
| perf-frontend-memo | perf 前端 memo(问题4/5) | — | 已完成 | 收尾 | 100% | — |
| fix-skills-modal-portal | Skills modal Portal 化修复定位 | — | 已完成 | 收尾 | 100% | — |
| fix-skills-uninstall-agent-arg | 修复单一 skill 卸载命令缺 -a 参数 | — | 已完成 | 收尾 | 100% | — |
| revert-uninstall-agent-wildcard | 回滚 uninstall_args -a 通配 (无效参数) | — | 已完成 | 收尾 | 100% | — |
| skills-uninstall-result-log | skills_uninstall 加 result debug log 定位 | — | 已完成 | 收尾 | 100% | — |
| skills-load-swr-cache | skills 页加载提速 SWR 缓存 | — | 已完成 | 收尾 | 100% | — |
| skills-uninstall-fs-fallback | uninstall fs 兜底删第三方 symlink skill | — | 已完成 | 收尾 | 100% | — |
| skills-open-no-auto-refresh | skills 开页纯缓存不自动刷新 | — | 已完成 | 收尾 | 100% | — |
| skills-source-grouping | skills 按 owner/repo 来源层级分组展示 | — | 已完成 | 收尾 | 100% | — |
| skills-source-cache-enrich | skills_list_installed 缓存命中后 enrich source 向后兼容 | — | 已完成 | 收尾 | 100% | — |
| skills-uninstall-group | 组级卸载: 卸载整个分组的 skills | — | 已完成 | 收尾 | 100% | — |
| skills-uninstall-case-insensitive | fs_fallback_remove 大小写不敏感匹配 skill 名 | — | 已完成 | 收尾 | 100% | — |
| import-export-theme | 导入导出 UI 适配主题 | — | 已完成 | 收尾 | 100% | — |
| mcp-management | MCP 管理模块 (per-agent 启用切换 + 导入平台 MCP + 删除) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-14-mcp-management |
| logo-transparent-icon-swap-css-theme | Logo 背景透明 + 换图标 + CSS 主题适配补齐 | — | 已完成 | 收尾 | 100% | — |
| mcp-edit | MCP 编辑功能 | — | 已完成 | 收尾 | 100% | — |
| load-recommended-hooks | 修复加载推荐配置时通知 hooks 未自动添加/更新 | — | 已完成 | 收尾 | 100 | — |
| hooks-quick-notify-inject | Hooks 区加快速注入/移除通知 hook 入口 (编辑器可见) | — | 已完成 | 收尾 | 100 | — |
| popup-opaque-bg | 弹窗本体背景 100% 不透明 | — | 已完成 | 收尾 | 100% | — |
| theme-name-i18n | 主题名多语言 i18n | — | 已完成 | 收尾 | 100% | — |
| theme-3axis-system | 主题系统 3 轴化 (style × color × dark/light) + 4 style + 12 palette | — | 规划中 | 规划 | 0% | — |
