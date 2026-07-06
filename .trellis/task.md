# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | worktree |
| --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 真实覆盖面全补: vitest 全量统计 + Rust 3 缺口分支 + 前端 39 test (pages/settings/platforms) | 规划中 | — |
| 06-30-group-env-vars | 分组配置支持环境变量设置 | 分组维度支持自定义环境变量注入 (sync 强写 ANTHROPIC_BASE_URL/AUTH_TOKEN 保护) | 已完成 | — |
| platform-last-error-msg | 平台最近错误展示提取error.message | DB 残留旧值 Migration 039 重提 + extract_error_message 已正确 | 已完成 | — |
| 06-30-export-ux-i18n | 导出 UX + i18n | 去「预览导出项」按钮改 debounce 自动展开 + 条目级展示 + setting label 本地化 (app:theme→主题) | 已完成 | — |
| 07-01-platform-429-no-autodisable | 429 不触发自动禁用 | 移除 429-配额从 auto_disable 触发条件 (non_success.rs:68) + spec C1/C3 修订 | 已完成 | — |
| 07-01-test-isolation-fix | 测试隔离治理 | 删真实环境 spawn + HomeGuard 收拢 4→1 + ENV_LOCK 集中 + grep lint 守卫 | 已完成 | — |
| 07-01-07-01-cli-integration-tab | CLI 集成 tab 改名 + 语言设置 | tab「编程工具」→「CLI 集成」(8 locale key+value) + 新增语言设置项 (复用 claude-settings language sync) | 已完成 | — |
| 07-01-07-01-sensenova-platform | 商汤 SenseNova 平台支持 | 加商汤日日新平台 (Protocol/adapter/preset/粘贴识别/quota token plan) | 已完成 | — |
| 07-01-arch-redesign | 全仓架构重设计 | 分包分文件消大文件: 前端 4 巨型 (editors 4609/Platforms 3568/Groups 2195/api 2072) 拆 + 目录重组 + Rust 局部拆 | 规划中 | — |
| 07-01-export-default-check | 导出默认勾选 skills/mcp | 导出 preview 默认勾选 skills+mcp scope (当前默认不勾需手动) | 已完成 | — |
| 07-01-export-ux-revisions | 导出 UX 修订 | setting label 补全 (cc_codex/coding_tools 等裸 key i18n) + 菜单组拆分 (platform/group/group_platform 三子类分开, 非一菜单组平铺) | 已完成 | — |
| 07-01-platform-search-filter | 平台搜索命中只展示命中项 | Platforms 搜索命中平台时只展示命中项, 不连带整组 | 已完成 | — |
| 07-01-export-extra-cleanup | 导出 extra 默认值清理 + 类型修正 | 导出时空 extra ({}) 默认移除 + extra 序列化为 obj 非 str 类型 | 已完成 | — |
| 07-01-aidog-deeplink-share | aidog:// 协议 + URL 导入 + skills/mcp 分享 | 注册 aidog:// 协议 (启动自动) + URL 导入平台/skills/mcp + skills/mcp 独立分享按钮 (复用平台分享逻辑) + 粘贴导入 | 实施中 | .worktrees/07-01-aidog-deeplink-share |
| 07-01-deeplink-platform-url ├ child | D2 平台 URL 导入 | aidog://platform 路径: 分享 base64 → URL → 唤起导入 | 规划中 | — |
| 07-01-deeplink-mcp-share ├ child | D3 mcp 分享 + URL 导入 | mcp 分享按钮 (复用 ShareModal) + aidog://mcp URL 导入 + 粘贴导入 | 规划中 | — |
| 07-01-deeplink-skill-share ├ child | D4 skills 分享 + URL 导入 | skills 分享 (id 列表) + aidog://skill URL 导入 + 粘贴导入 | 规划中 | — |
| 07-01-third-party-context-mgmt-strip | 第三方 anthropic 端点 context_management 致 400 thinking must be passed back | — | 已完成 | .worktrees/07-01-07-01-third-party-context-mgmt-strip |
| batch-add-platform | 批量添加平台 (多 apikey + 分组关联) | — | 已完成 | — |
| opencode-go-baseurl-fix | opencode 协议预设 base_url 缺 /v1 致 fetch-models/推理 404 | — | 已完成 | — |
| third-party-context-mgmt-unconditional-strip | 第三方 anthropic 端点首轮 context_management 致 GLM 1210 | — | 已完成 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-01-third-party-context-mgmt-unconditional-strip |
| stats-platform-dropdown-overlap | Stats 平台下拉平台名重叠不可读 | — | 已完成 | — |
| glm-anthropic-multiturn-1210 | GLM anthropic 端点多轮 tool_use 请求 1210 参数有误 | — | 已完成 | — |
| volces-agent-plan-paste | 火山方舟 agent plan 智能识别缺失 (ark- 前缀 + 圈数字防爬 + /api/plan 端点) | — | 已完成 | — |
| proxy-http-relay | /proxy 支持通用 HTTP 代理 + 无平台/无分组筛选 | — | 已完成 | — |
| stats-logs-filter-unify | Stats/Logs 筛选完全对齐(平台/模型/分组) | — | 已完成 | — |
| logs-detail-copy-buttons | 请求日志详情每个元素一键复制 | — | 已完成 | — |
| settings-managed-marker | settings.json 不写 _aidog_managed marker | — | 已完成 | — |
| paste-base64-recognize | 粘贴 base64 分享文本识别 (MiMo 平台 tp- apikey) | — | 已完成 | — |
| platform-default-models-audit | 阶跃星辰补默认模型 + 全平台模型清单审计 | — | 已完成 | — |
| deeplink-platform-url | D2 平台 URL 导入 | — | 已完成 | — |
| group-priority-check-timing | 平台分组优先级检查时机改确认后触发 | — | 已完成 | — |
| deeplink-mcp-share | D3 mcp 分享 + URL 导入 | — | 已完成 | — |
| deeplink-skill-share | D4 skills 分享 + URL 导入 | — | 已完成 | — |
| aidog-deeplink-share | aidog:// 协议注册 + URL 导入 + skills/mcp 分享 | — | 已完成 | — |
| 07-01-proxy-http-relay-p1 | P1 CONNECT 隧道 + 元数据 + 无平台筛选 | — | 已完成 | — |
| arch-redesign | 全仓架构重设计 - 分包分文件消大文件 | — | 已完成 | — |
| filter-item-height | 分组筛选 item 增高 | — | 已完成 | — |
| recurring-request-error | request 错误链诊断 (3e8b13f0→cb3603ac 必然失败) | — | 已完成 | — |
| body-date-format-rewrite | body 日期格式改写防检测 (CLI集成开关) | — | 已完成 | — |
| cli-lang-select-style | CLI 语言设置 select 样式修正 (label 字号 + select 够用右对齐) | — | 已完成 | — |
| cli-lang-desc-remove | CLI 语言设置删冗余描述 (去 ~/.claude/settings.json · language 行) | — | 已完成 | — |
| diag-42617827 | 诊断 request_id=42617827 错误链根因 | — | 已完成 | — |
| proxy-relay-mitm | proxy MITM 解密隧道 P3 - 客户端保官方协议 + AirDog 拦截采集 | — | 已完成 | — |
| longcat-default-models | Longcat 平台补默认模型列表 | — | 已完成 | — |
| mitm-ca-elevated-install | MITM CA 自动提权安装 (osascript/UAC/pkexec) | — | 已完成 | — |
| mitm-whitelist-clash-rules | MITM 白名单 Clash 规则集全类型支持 + 默认扩充 | — | 已完成 | — |
| mitm-ca-macos-fix | MITM CA macOS osascript 安装失败修复 (手敲 sudo 成功 / osascript 失败) | — | 已完成 | — |
| platform-multi-apikey-ui | 平台添加多 apikey 智能识别批量创建 UI | — | 已完成 | — |
| test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 已完成 | — |
| proxy-http-relay-p2 | proxy /proxy CONNECT 元数据记账 P2 (timeout/熔断/last_error/tracing) | — | 已完成 | — |
| mitm-ca-test-detect | mitm CA 安装失败测试驱动检测 P2 (classifyTrustError 后端化单测 + osascript 语法集成) | — | 已完成 | — |
| mitm-whitelist-import-defaults | MITM 解密白名单导入默认按钮 (只添加去重) | — | 已完成 | — |
| mitm-whitelist-clear-search-test | MITM 解密白名单 一键清空/搜索/URL 命中测试 | — | 已完成 | — |
| connect-readbuf-flush-fix | CONNECT 隧道 read_buf flush 写错对象致 TLS 握手 RST | — | 已完成 | — |
| mitm-whitelist-add-ruletype | MITM 白名单添加规则时选匹配方式 (domain/suffix/keyword/ipcidr) | — | 已完成 | — |
| rustls-crypto-provider-install | rustls CryptoProvider 未 install_default 致 MITM TLS panic | — | 已完成 | — |
| unmatched-fallback-default-group | 未匹配分组 fallback 默认分组记录统计不报错 | — | 已完成 | — |
| coding-tools-tab-rename-dedup-lang | 编程工具 tab 改名 + claudeTab 语言配置去重 | — | 已完成 | — |
| h2-passthrough-stream-cancel | MITM 直通转发 HTTP/2 响应流 CANCEL | — | 已完成 | — |
| mitm-h2-cancel-real-rootcause | MITM h2 stream CANCEL 真根因 (env proxy 已修仍 CANCEL) | — | 实施中 | — |
| sql-log-full-content | SQL 日志输出完整原始内容而非占位符 | — | 已完成 | — |
| proxy-trace-id-header | proxy 响应头注入 X-AiDog-Trace trace-id (debug 模式 MITM/非 MITM 都加) | — | 已完成 | — |
| app-log-to-file-always | 应用日志启用时强制输出到文件 (无视 debug 模式) | — | 已完成 | — |
| trace-id-log-format | trace-id 注入日志格式让 header↔日志可 grep | — | 已完成 | — |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
| /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-05-mitm-h2-cancel-real-rootcause | 07-05-mitm-h2-cancel-real-rootcause | trellisx-start |
| forward-http-absolute-form | forward proxy 扩展支持 absolute-form HTTP 转发任意 host | — | 已完成 | — |
| log-format-field-loss | 日志格式器 MsgCollector 丢非 message event 字段 (fn/req/dur/sql 全丢) | — | 已完成 | — |
| delete-platform-keep-empty-group | delete_platform 保留空组不连带 force_delete_group 孤儿 auto 组 | — | 已完成 | — |
