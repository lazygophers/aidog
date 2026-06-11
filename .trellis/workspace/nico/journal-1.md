# Journal - nico (Part 1)

> AI development session journal
> Started: 2026-06-09

---



## Session 1: DB schema v2 规范化重构

**Date**: 2026-06-11
**Task**: DB schema v2 规范化重构
**Branch**: `master`

### Summary

10 条 DB 规范破坏式重构: 表名单数/uint64自增PK(proxy_log除外uuid去连字符)/ms时间戳/每表软删除deleted_at/禁NULL默认值/protocol→platform_type/复合表加代理PK/删model_mappings内联group JSON/独立一次性迁移脚本. 后端(models/db/router/proxy/lib)+前端(api.ts/pages)全对齐, schema测试11绿, 真库~/.aidog/aidog.db迁移成功, BUG-1(JOIN列歧义)修复, simplify质量重构8项. 后续: ORM评估/软删除统一封装/auto_from_platform改INTEGER FK.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `5bb8743` | (see git log) |
| `979f096` | (see git log) |
| `a05960a` | (see git log) |
| `48d3ebe` | (see git log) |
| `f9eaaed` | (see git log) |
| `60eebde` | (see git log) |
| `348ff28` | (see git log) |
| `c980dd7` | (see git log) |
| `ce26867` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: 添加 mock 平台类型

**Date**: 2026-06-11
**Task**: 添加 mock 平台类型
**Branch**: `master`

### Summary

新增 Protocol::Mock 平台类型: 路由到 mock 平台不转发真实上游, 本地按入站协议(anthropic/openai/openai_completions/openai_responses/gemini)生成可控假响应(非流式+流式SSE). 三层配置覆盖(请求body.mock>message role映射>platform.extra), error_mode(none/http_error/429/timeout) + delay_ms + 假token. 配置存platform.extra零schema变更. 前端MockConfigEditor. 拦截点proxy.rs handle_mock仅matches!(Mock)不影响现有平台. 22单测全绿, spec沉淀mock-platform.md.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `dae1c10` | (see git log) |
| `d3c2188` | (see git log) |
| `448570b` | (see git log) |
| `73fa042` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: Claude Code 订阅平台（纯透传）

**Date**: 2026-06-11
**Task**: Claude Code 订阅平台（纯透传）
**Branch**: `master`

### Summary

新增 Protocol::ClaudeCode 平台类型, 纯透传 relay: 路由到 CC 平台原样转发客户端请求到 base_url, 不转换 body/header/不注入认证(客户端自带订阅 OAuth). into_parts 前捕获 orig method/uri/headers; handle_passthrough 剔 Host+Content-Length 保留 Authorization, 流式+非流式 1:1 relay, proxy_log 正常记+token 尽力解析. 不调 convert_request/build_upstream_headers. 6 透传单测+spec claude-code-passthrough.md. 后续: 分组路由 AI 平台拖动排序(独立 task) + endpoints 前端不展示 bug 待查.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7b21f33` | (see git log) |
| `3d3593b` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 4: 修复 endpoints 前端不展示

**Date**: 2026-06-11
**Task**: 修复 endpoints 前端不展示
**Branch**: `master`

### Summary

根因: parse_endpoints serde_json::from_str.unwrap_or_default() 反序列化 endpoints 数组时, 单个元素未知 client_type='anthropic'(ClientType enum 无此变体) 致整个数组解析失败, 静默返空 → endpoints 全丢. 修: deserialize_client_type_lenient(未知值回退 Default) field-level deserialize_with + 回归测试. cargo test 40 绿. 注: 修复因多窗口并行同文件被别窗口 pricing commit(540b912)卷入, 未独立 commit.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `540b912` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 5: 平台 quota 展示区分 + 统一刷新

**Date**: 2026-06-11
**Task**: 平台 quota 展示区分 + 统一刷新
**Branch**: `master`

### Summary

Platforms 卡片: coding plan/余额(quota) 独立 glass-surface 分组+「额度」标签, 与 usage 统计 badge 视觉/位置区分; quota 内联刷新图标(↻ spin), 统一刷 balance+coding_plan(quotaApi.query 合查) + loading + 错误 toast, mock/claude_code 隐藏; i18n zh/en. 纯前端, tsc 0.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `c504d7b` | (see git log) |
| `961c9ad` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 6: quota 请求驱动预估增量更新

**Date**: 2026-06-11
**Task**: quota 请求驱动预估增量更新
**Branch**: `master`

### Summary

请求完成后 tokio::spawn 后台预估增量更新余额(resolve_price 原子自减)+coding plan(Kimi 精确 limit/remaining; GLM/MiniMax 方案B Δutil/Στoken 拟合, 冷启动不预估, reset 丢样本); 5min/100次校准真查覆盖; 禁持锁跨 await(std Mutex), Arc<Db> 重构; platform+4列 migration 004; 前端展示 est+预估/实测标识+刷新校准. cargo test 50 绿, C3 全 PASS. 发现既有 bug: 流式 proxy_log token=0(待独立 task).

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `6c16d15` | (see git log) |
| `8e12f70` | (see git log) |
| `b2090d7` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 7: 平台 quota 系统托盘展示

**Date**: 2026-06-11
**Task**: 平台 quota 系统托盘展示
**Branch**: `master`

### Summary

platform +show_in_tray/tray_display(互斥单平台), migration 005; set_tray_platform 单事务互斥; build_tray_menu 展示选定平台 quota(复用 est_* 预估值, balance 💳/coding 🪙%), macOS tray.set_title cfg 守卫+menu item; 后台 spawn 预估后 app.emit(tray-refresh)→主线程 listen refresh(线程安全); 前端 enabled 平台 tray 开关+余额/coding 二选一+互斥. cargo test 50 绿, C3 全 PASS. 注: 多窗口 commit race 致后端代码被别窗口 commit 卷走但功能完整.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `d1f3ae6` | (see git log) |
| `466db11` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 8: 修复流式 proxy_log token=0

**Date**: 2026-06-11
**Task**: 修复流式 proxy_log token=0
**Branch**: `master`

### Summary

流式分支 upsert_log 在 axum 消费 stream 前执行致 tokens_acc=0; 在 [DONE] 闭包(est_fired 守卫)再 upsert 最终 token(INSERT OR REPLACE 覆盖)+status; 非流式不动. cargo test 50 绿.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `8cdd188` | (see git log) |
| `27d99c2` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 9: 分组平台拖动排序

**Date**: 2026-06-11
**Task**: 分组平台拖动排序
**Branch**: `master`

### Summary

Groups 关联平台列表 HTML5 native 拖拽重排(dragIndex/reorder + ⠿手柄 + 视觉); saveEdit 按顺序设 priority(i+1) 复用; 后端 set_group_platforms/ORDER BY priority 无改. tsc 0.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `69fa530` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 10: 托盘两行小字展示

**Date**: 2026-06-11
**Task**: 托盘两行小字展示
**Branch**: `master`

### Summary

tray 有值: 隐 logo + 两行(平台名/余额)纯文字无 emoji; coding 显剩余%(双信号 tray_display||tiers, 冷启动剩100%) balance 显总余额; 9pt 小字(NSStatusItem attributedTitle via with_inner_tray_icon+ns_status_item+objc2, set_title 无字号API); 垂直居中 lineHeight+baselineOffset. 待用户 GUI 验字号/垂直. 后续: 可配 tray 面板(多平台+排序+设置页)重构渲染.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `a726869` | (see git log) |
| `96e7340` | (see git log) |
| `bbd4b9c` | (see git log) |
| `10e743e` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete
