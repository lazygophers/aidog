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
| import-export | 导入导出功能（加密单文件 + 自动化导入） | — | 进行中 | 规划 | 0% | — |
