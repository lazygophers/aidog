# 日志菜单拆分: 测试/余额单独菜单 — PRD

## 目标
测试日志 + 余额查询日志各单独侧栏菜单, 点入显对应 source 日志。RequestLog (请求日志, test+quota 合并视图) 保持现状不动。

## 背景
- proxy_log 表统一, `source_protocol` 区分: `test`(model test) / `quota`(余额查询) / 各协议(代理转发)
- 现状: Logs(代理转发 exclude test/quota) + RequestLog(test+quota, TypeFilter all/test/quota)
- 用户需求: 测试/余额从侧栏直接可见 (独立菜单), RequestLog 保留

## 边界
- **前端 only**: 复用 `request_log_list` 后端 (source 过滤已有), 无后端改
- **RequestLog 不动**: 保持 test+quota 合并 + TypeFilter
- 新菜单复用 RequestLog 组件, 参数化

## 设计
RequestLog 加 props:
- `defaultSource?: "test" | "quota"` — 入口预设 source
- `lockSource?: boolean` — 锁定隐藏 TypeFilter

侧栏 BASE_NAV 加 2 项 (section: nav.section.proxy):
- `test-log` (icon logs, labelKey nav.testLog)
- `quota-log` (icon logs, labelKey nav.quotaLog)

App.tsx 渲染: `<RequestLog defaultSource="test" lockSource />` / `<RequestLog defaultSource="quota" lockSource />`

## 验收标准
- [ ] 侧栏新增「测试日志」+「余额日志」2 菜单
- [ ] 测试日志 → 仅 source=test (无 TypeFilter)
- [ ] 余额日志 → 仅 source=quota (无 TypeFilter)
- [ ] RequestLog 保持现状 (test+quota + TypeFilter)
- [ ] i18n 8 语言 (nav.testLog / nav.quotaLog + page 标题)
- [ ] yarn build 过 + check:i18n 过

## 索引
- task.json: `.skein/task/log-menu-split/task.json`
- 规模小, 设计含本文件
