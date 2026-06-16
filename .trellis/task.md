# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| _（无活跃任务）_ | — | — | — | — | — | — |
| platform-smart-paste | 平台添加智能识别: 剪贴板粘贴解析 base_url/平台/apikey(base64自动解码) | — | completed | finish | 100 | .trellis/worktrees/06-14-platform-smart-paste |
| readme-redesign | README 重写: 安装/使用详化 + 功能核对 + 视觉重设计 | — | 已完成 | 收尾 | 100% | — |
| readme-7lang-sync | README 同步 7 语言 | — | 已完成 | 收尾 | 100% | — |
| about-module | 关于模块: 展示完整版本信息 + GitHub 信息 | — | completed | finish | 100 | .trellis/worktrees/06-14-about-module |
| docs-api-i18n | docs/api 多语言适配 | — | 已完成 | 收尾 | 100% | — |
| version-cicd-updater | .version 唯一版本源 + 发布 CICD + 自动更新对接 | — | completed | finish | 100 | .trellis/worktrees/06-14-version-cicd-updater |
| license-agpl3 | 开源协议改为 AGPL-3.0 | — | completed | finish | 100 | .trellis/worktrees/06-14-license-agpl3 |
| silent-launch-depends-autolaunch | silent-launch 依赖 autolaunch: autolaunch off 时隐藏并默认关闭 | — | 已完成 | 收尾 | 100% | — |
| dependabot-3-esbuild | 修复 dependabot 安全警报 #3: esbuild 0.27.7 → 0.28.1 (GHSA-gv7w-rqvm-qjhr) | — | 已完成 | 收尾 | 100% | — |
| dependabot-1-glib-eval | 评估 dependabot 安全警报 #1: glib GHSA-wrw7-89jp-8q8g (tauri 上游阻塞) | — | 已完成 | 收尾 | 100% | — |
| skills-catalog-install | Skills 页新增「搜索安装」子视图: catalog 搜索 + 选 agent 安装 | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-15-skills-catalog-install |
| notify-project-name | 通知中心展示项目名: cwd basename 作为 project 注入 inbox/popup | — | 已完成 | 收尾 | 100% | — |
| skill-detail-view | 已装 skill 详情查看: SKILL.md 渲染 + 关联文件浏览 (只读) | — | 已完成 | 收尾 | 100% | .trellis/worktrees/06-15-skill-detail-view |
| notify-ux | 通知模块易用性增强: 每类型默认模板 + 一键注入 hook 到所有分组 | — | 已完成 | 收尾 | 100% | — |
| notify-test-buttons | 通知设置加独立 TTS/弹窗测试按钮 (与类型测试解耦) | — | 已完成 | 收尾 | 100% | — |
| fix-model-test-mock | 修复 model_test 对 Mock 平台返回 builder error (502) | — | 已完成 | 收尾 | 100% | — |
| notify-mac-system | macOS 通知默认走系统通知 (osascript display notification 替代 tauri-plugin) | — | 已完成 | 收尾 | 100% | — |
| subagent-statusline-fix | subagent statusline 展示修复 (诊断+定位) | — | 已完成 | 收尾 | 100% | — |
| readme-badges | README + docs 添加 LINUX DO 社区徽章及常见 GitHub 徽章 | — | 已完成 | 收尾 | 100% | — |
| skills-catalog-mismatch | Skills 搜索结果与 npx skills ls --json 不一致诊断+修复 | — | 已完成 | 收尾 | 100% | — |
| hide-notify-menu-when-disabled | 通知关闭时隐藏通知中心菜单 | — | 已完成 | 收尾 | 100% | — |
| disable-default-hooks-when-notif-off | 通知关闭时禁用默认注入通知Hook开关 | — | 已完成 | 收尾 | 100% | — |
| notif-center-settings-link | 通知中心页加快捷入口跳设置 | — | 已完成 | 收尾 | 100% | — |
| fix-tts-backend-init | 修复TTS后端初始化失败 | — | 已完成 | 收尾 | 100% | — |
| huashu-nuwa-desc-empty | huashu-nuwa desc空不显示分隔符 | — | 已完成 | 收尾 | 100% | — |
| fix-model-test-mock | Mock 平台 model_test 走本地生成响应 (补 Mock 分支, 不再 502) | — | 已完成 | 收尾 | 100% | — |
| fix-notif-template-stale | 通知文案变更未及时生效 | — | 已完成 | 收尾 | 100% | — |
| fix-notif-no-popup | 通知 notification_test 命令无系统弹窗 (osascript 绝对路径根治) | — | 已完成 | 收尾 | 100% | — |
| fix-skills-install-claude-code | skills 安装 claude code 未生效 (spawn npx 注入 HOME) | — | 已完成 | 收尾 | 100% | — |
| notif-perm-guidance | macOS 通知授权分层引导: 启动 request_permission + 设置页深链系统通知 + 签名公证文档 | — | 已完成 | 收尾 | 100% | — |
| notif-empty-show-template | 通知模板为空时展示默认模板而非空/英文兜底（后端 render + 前端预览） | — | 已完成 | 收尾 | 100% | — |
| remove-notif-custom | 移除通知 Custom 类型（4→3，未知入站 type 兜底 task_complete） | — | 已完成 | 收尾 | 100% | — |
| notif-template-presets | 通知模板多预设快捷选择（不可变预设 + 手选可改不污染 + 禁空内容） | — | 已完成 | 收尾 | 100% | — |
| claude-hook-notify | claude code hook 事件通知：可配置多事件触发系统通知 | — | 已完成 | 收尾 | 100% | — |
| notif-per-hook-only | 通知模块重构：移除按类型配置，仅保留逐 Hook 事件（每事件独立 启用/TTS/弹窗/专属模板+专属入参） | — | 已完成 | 收尾 | 100% | — |
| notif-hook-sound-toggle | 逐 Hook 事件加提示音(sound)独立开关 | — | 已完成 | 收尾 | 100% | — |
| groups-copy-baseurl-apikey | Groups 列表页加复制代理 base_url + 每 item 复制 api_key(group_name) | — | 已完成 | 收尾 | 100% | — |
| proxy-support-models-endpoint | 代理支持 /v1/models 模型列表端点（透传到分组所选平台上游） | — | 已完成 | 收尾 | 100% | — |
| daily-update-check | 每日检测更新并提醒用户（tauri updater 前端对接） | — | 已完成 | 收尾 | 100% | — |
| responses-api-endpoints | 核查并支持 Responses API 全端点(create 转换 + get/cancel/compact 透传) | — | 已完成 | 收尾 | 100% | — |
| stats-model-filter-size | 使用统计模型下拉筛选 item 过窄不可读 | — | 已完成 | 收尾 | 100% | — |
| stats-platform-filter-mismatch | 使用统计平台筛选语义错配筛空 | — | 已完成 | 收尾 | 100% | — |
| home-dashboard | 新增首页总览仪表盘（侧栏首项+默认落地, 复用现有主题/组件） | — | 已完成 | 收尾 | 100% | — |
| stats-model-filter-size-v2 | 模型下拉筛选 item 仍过小 (上次改动力度不足) | — | 已完成 | 收尾 | 100% | — |
| stats-regression-and-item-size-v3 | stats 回归修复(ambiguous col) + 模型筛选 item 尺寸 v3 | — | 已完成 | 收尾 | 100% | — |
| stats-trend-hourly-by-preset | 请求趋势按 preset 联动 granularity (today→hourly) | — | 已完成 | 收尾 | 100% | — |
| stats-filter-trigger-fontsize-v4 | 模型筛选触发按钮字号 12 过小 (前3次改错位置) | — | 已完成 | 收尾 | 100% | — |
| stats-cache-rate-over-100 | 缓存率公式错误可超 100% (cache/input→cache/(input+cache)) | — | 已完成 | 收尾 | 100% | — |
| home-request-trend | 首页加请求趋势图（今日 hourly buckets 轻量 SVG） | — | 已完成 | 收尾 | 100% | — |
| stats-trend-chart-missing | 请求趋势图不显示 (query_stats buckets 空/回归未重启诊断) | — | 已完成 | 收尾 | 100% | — |
| mcp-add-entry | MCP 主动添加入口 (手动新建 server) | — | 已完成 | 收尾 | 100% | — |
| home-trend-line | 首页请求趋势改曲线图(SVG 折线/面积替柱状) | — | 已完成 | 收尾 | 100% | — |
| home-trend-24h | 首页请求趋势改最近24小时(滚动窗口+文案) | — | 已完成 | 收尾 | 100% | — |
| mcp-enabled-not-effective | MCP 配置启用后未生效 | — | 已完成 | 收尾 | 100% | — |
| home-trend-smooth | 首页趋势曲线改平滑(Catmull-Rom 贝塞尔, 替直折线) | — | 已完成 | 收尾 | 100% | — |
| stats-model-filter-only-recorded | 使用统计模型筛选仅含有记录的模型 | — | 已完成 | 收尾 | 100% | — |
| stats-trend-curve-granularity | Stats 请求趋势改曲线+分钟/5分钟粒度自动降级 | — | 已完成 | 收尾 | 100% | — |
| menu-ia-redesign | 菜单与导航信息架构重构 | — | 进行中 | 规划 | 0% | — |
| request-headers-not-recorded | 请求头未记录诊断修复 (request_id=729d085103e246b1bc34888527541117) | — | 已完成 | 收尾 | 100% | — |
| linuxdo-badge-acknowledgement-only | LINUX DO 徽章移至致谢区 (去顶部重复) | — | 已完成 | 收尾 | 100% | — |
| license-badge-agpl-fix | License badge/section 修正为 AGPL-3.0 (实际协议) | — | 已完成 | 收尾 | 100% | — |
| docs-nav-api-dedup | docs 顶部栏 API 接口重复/缺失修复 (_nav.json) | — | 已完成 | 收尾 | 100% | — |
| ci-checkout-node24 | 升级 CI actions/checkout 到 Node 24 兼容版（消除 Node 20 deprecation） | — | 已完成 | 收尾 | 100% | — |
| tray-column-dead-code | 清除 TrayColumn macOS 专属字段在非 macOS 的 dead_code warning | — | 已完成 | 收尾 | 100% | — |
| release-ci-github-actions | 优化 release CI 减少 GitHub Actions 计费分钟 | — | 已完成 | 收尾 | 100% | — |
| release-ci-cache-on-failure | release CI 失败也缓存 (cache-on-failure) | — | 已完成 | 收尾 | 100% | — |
| updater-pubkey-gitignore | updater 新签名密钥对 (改 pubkey + gitignore) | — | 已完成 | 收尾 | 100% | — |
| ci-actions-node-24 | CI actions 升级到 Node 24 兼容版 | — | 已完成 | 收尾 | 100% | — |
| release-v0-1-0 | 优化 release 发布文本 (模板+v0.1.0) | — | 已完成 | 收尾 | 100% | — |
| model-info-source | 模型信息中心化: 移除旧价格同步, 改 GitHub JSON 为唯一信源 (price/max_tokens/context) | — | 已完成 | 收尾 | 100% | — |
| pricing-python | Python 定价工程 + data/models.json 首版 (scripts/pricing/ uv + 11 scraper + Makefile 单入口) | — | 已完成 | 收尾 | 100% | .worktrees/06-16-pricing-python |
| pricing-rust-sync | Rust: migration 008 max_*列 + 同步源换 GitHub raw + db max_* + 移除旧 price_sync/upsert/delete + import_export 移除 model_price scope | — | 已完成 | 收尾 | 100% | — |
| pricing-frontend | 前端 PricingTab 改造: 移除手动编辑/删除 + 加 max_tokens/context 列 + 文案 GitHub + api.ts + i18n 7 语言 | — | 已完成 | 收尾 | 100% | — |
| max-tokens-cap | max_tokens 入站 parse(anthropic/gemini) + router 转换前裁剪(仅超上限裁, 未传不注入) | — | 已完成 | 收尾 | 100% | — |
| blackhole | 熔断候选空回退透传(单平台不blackhole) | — | 已完成 | 收尾 | 100% | — |
