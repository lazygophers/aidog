# PRD: README 重写 (zh-CN)

## 目标

重写 `README.md` (zh-CN), 使最终用户能直接读懂: 是什么 / 怎么装 / 怎么用 (含客户端接入)。重点: 安装详化 + 使用详化 + 功能核对补全 + 视觉重设计。先 zh-CN, 验收后同步 7 语言。

## 现状锚点

- `README.md` (zh, 5.0K) 已有基础结构 (hero/badges/功能10项/三步上手/curl/技术栈/开发/文档/多语言)。
- 安装仅一句"下载安装包", 无各平台步骤/首启权限/系统要求。
- 使用"三步上手"简化, 缺 Claude Code/Codex 具体接入命令。
- 功能列表缺新模块: MCP/skills/系统通知/hooks/导入导出/coding plan/价格同步/手动预算/主题3轴。
- 截图素材已存在: `screenshots/{dashboard,add-platform,settings}.png`。
- 权威源: `docs/docs/zh/getting-started/{installation,quick-start}.mdx` + `proxy/codex-integration.mdx` + `settings/claude-code.mdx`。
- release 地址: 用户要求用 `https://github.com/lazygophers/aidog/releases/latest`。
- docs 站: `https://lazygophers.github.io/aidog/zh/`。

## 设计 (结构)

```
<div hero center>
  🐕 logo (emoji) + AiDog
  tagline: 统一管理你的 AI API 网关
  一句话定位 (强化: 本地桌面 · 无需上云 · 50+ 平台)
  badges (docs / release latest / license / platforms)
  语言切换行
</div>

## 截图 (dashboard.png, 全宽)

## 它解决什么问题 (3-4 痛点 → AiDog 方案, 对话式)

## 核心功能 (分类 grid: 网关 / 监控 / 编程助手 / 个性化, 各列要点)
  - 网关: 多平台聚合(50+预设)/智能分组(failover+load balance)/模型映射/协议互转/熔断调度/中间件规则引擎
  - 监控: 用量统计(token+费用估算)/余额查询/Coding Plan配额/请求日志(三级)/手动预算
  - 编程助手: Claude Code/Codex 原生集成/MCP管理(per-agent)/Skills管理(npx)/系统通知(TTS+弹窗+收件箱+hooks)
  - 个性化: 主题3轴(9 style×12 palette×亮暗)/8语言(含ar RTL)/导入导出(.aidogx加密容器)/托盘/状态栏脚本

## 安装 (各平台详化)
  ### 系统要求 (表格)
  ### macOS (.dmg + Gatekeeper 右键打开)
  ### Windows (.msi + SmartScreen)
  ### Linux (.deb / .AppImage + 命令)
  下载入口: releases/latest (突出)

## 快速上手 (3 步 + 截图 add-platform.png)
  1. 添加平台 (截图)
  2. 配置客户端 → 代理地址
  3. 验证 (curl)
  代理地址: http://127.0.0.1:9876/proxy/v1

## 客户端接入 (详化, tab 式)
  ### Claude Code (ANTHROPIC_BASE_URL env / 一键导出 ~/.claude.json / settings.ClaudeCode tab)
  ### OpenAI Codex (~/.codex/config.toml base_url / settings.Codex tab)
  ### 任意 OpenAI 兼容客户端 (curl 示例)
  > Key 可填任意值, AiDog 用配置的真实 Key (group 认证: Authorization Bearer <group_name>)

## 功能截图 (settings.png)

## 从源码构建 (开发向)
  前置依赖 + yarn tauri dev/build + cargo

## 技术栈 (表格)

## 文档 (链接网格: 快速开始/平台/分组/代理/统计/中间件/设置/API)

## 多语言 (8 README 表格)

## License (MIT)
```

## 视觉规约

- hero 用 `<div align="center">` 包裹 (GitHub md 渲染居中)。
- logo 用 🐕 emoji (无 SVG 内联, 简洁)。
- badges 用 shields.io: docs / release(latest) / license / stars。
- 截图用相对路径 `screenshots/dashboard.png` (repo root 已有)。
- 功能用分类 + 子列表, 非超长扁平列表。
- 表格美化 (系统要求 / 技术栈 / 多语言 / 客户端接入对比)。

## 内容锚点 (避免与 docs 冲突)

- 端口 9876 (`src-tauri/src/lib.rs:331` `port: 9876`)。
- 代理地址 `http://127.0.0.1:9876/proxy/v1` (Base URL 含 /v1, 自动追加 /chat/completions)。
- 安装步骤权威: `docs/docs/zh/getting-started/installation.mdx`。
- 客户端接入权威: `docs/docs/zh/proxy/codex-integration.mdx` + `settings/claude-code.mdx`。
- release latest: `https://github.com/lazygophers/aidog/releases/latest`。
- 功能模块权威: MEMORY 索引 (gateway/导入导出/中间件/熔断调度/通知/MCP/skills/主题3轴/价格同步)。

## 验收标准

- [ ] README.md 重写, 含 hero/截图/痛点/功能分类/安装详化(3平台)/快速上手/客户端接入(Claude Code+Codex+curl)/源码构建/技术栈/文档/多语言。
- [ ] 安装用 `releases/latest` 地址 (非 /releases)。
- [ ] 功能列表覆盖所有现模块 (含 MCP/skills/通知/hooks/导入导出/coding plan/价格同步/手动预算/主题3轴)。
- [ ] 客户端接入含 Claude Code (env + 导出) + Codex (config.toml) + curl 具体命令。
- [ ] 截图引用 `screenshots/{dashboard,add-platform,settings}.png` (相对路径, 文件存在)。
- [ ] 链接有效 (docs 站 + release latest + 各 README)。
- [ ] 仅改 README.md (zh-CN), 不动其他 7 语言 (留后续 task)。

## 风险

- **功能过载**: 列太多致阅读疲劳。缓解: 分类 grid + 每类 ≤ 6 子项, 突出核心。
- **截图尺寸**: GitHub md 大图致首屏臃肿。缓解: 截图放功能段后, 非最顶 (hero 后用文字痛点引入)。
- **接入命令过时**: Claude Code/Codex 配置文件路径变。缓解: 以 docs 为权威, README 精炼版 + 链接 docs 详页。
