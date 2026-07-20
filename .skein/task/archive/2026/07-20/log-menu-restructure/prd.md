# 日志菜单重构: 代理/请求双菜单 + 日志统计组 — PRD

## 目标
日志侧栏从 5 项 (request-log/test-log/quota-log/logs/stats) 简化重组为清晰双菜单架构, 消除 label 错位 (nav.logs 原标"请求日志"实为代理转发)。

## 最终菜单结构
| section (label) | 菜单 | label |
|---|---|---|
| overview (概览) | home | 首页 |
| **proxy → platform (平台)** | platforms | AI 平台 |
| | cli-proxy | CLI 代理 |
| **logStats (日志统计, 新)** | stats | 使用统计 |
| | logs | **代理日志** (原"请求日志", 代理转发 exclude test/quota) |
| | request-log | **请求日志** (原"请求记录", test+quota 合并) |
| | notifications | 通知中心 |
| extension (扩展) | skills / mcp | |
| system (系统) | settings / about | |

## 改动清单
1. **App.tsx BASE_NAV**:
   - 删 test-log / quota-log 项
   - logs/request-log/notifications/stats section `nav.section.observe` → `nav.section.logStats`
   - platforms/cli-proxy section `nav.section.proxy` → `nav.section.platform`
2. **RequestLog.tsx**: 删 dead props (defaultSource/lockSource/lockedType, log-menu-split b3acb01e 加, 删菜单后无 caller) — 回滚参数化, 回到 b9388102 test+quota 合并视图
3. **i18n 8 locale**:
   - `nav.logs`: "请求日志"→"代理日志"
   - `nav.requestLog`: "请求记录"→"请求日志"
   - `nav.section.proxy`: "代理"→"平台"
   - `nav.section.observe`: 删
   - `nav.section.logStats`: 加"日志统计"
   - `nav.testLog`/`nav.quotaLog`: 删 (菜单已删)

## 边界
- **纯前端 + i18n, 无后端改**。cli_proxy 分流后端现状已满足 (代理转发 source=协议名→代理日志; test/quota source→请求日志; cli_proxy_provider_id 额外维度不影响 source 分流)
- Logs 页 (代理日志) 后端过滤 `exclude_sources=["test","quota"]` 不动
- RequestLog 页 (请求日志) 后端过滤 `sources=["test","quota"]` 不动

## 验收标准
- [ ] 侧栏: 平台(AI 平台+CLI 代理) / 日志统计(使用统计+代理日志+请求日志+通知中心)
- [ ] test-log / quota-log 菜单消失
- [ ] Logs label="代理日志", 内容=代理转发日志 (不变)
- [ ] RequestLog label="请求日志", 内容=test+quota 合并 + TypeFilter (回滚 lockSource)
- [ ] `yarn build` 过
- [ ] `check:i18n` 8 locale 对齐过
- [ ] 无 dangling key 引用 (nav.section.observe / nav.testLog / nav.quotaLog)

## 索引
- task.json: `.skein/task/log-menu-restructure/task.json`
- 前身: log-menu-split (b3acb01e, 加 test/quota 菜单 — 本次回滚菜单项保留合并视图)
