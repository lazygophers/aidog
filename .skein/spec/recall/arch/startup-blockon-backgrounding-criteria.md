---
title: startup-blockon-backgrounding-criteria
layer: recall
category: arch
keywords: [startup,performance,async,initialization,block_on]
source: src-tauri/src/app_setup.rs
authored-by: skein-spec
created: 1722470400
status: active
related: []
updated: 1722470400
---

## 触发场景
冷启动优化时决定某个 `block_on` 是否能挪到后台 spawn 时。

## 陷阱
检查 block_on 结果是否被**同一 setup 内**后续代码读取/依赖。
- **若初始态（空/默认值）会改变下游业务语义**，即使概率低也须保留同步
- 盲目后台化会导致后续业务逻辑依赖空值、网络功能降级或静默失败

## 正解
判断标准矩阵：

| block_on 产物 | 被后续代码读取？ | 初始态会改语义？ | 决定 | 案例 |
|---|---|---|---|---|
| Db | 是 | 是（后续都依赖） | **保留同步** | app_setup.rs:30 |
| MiddlewareEngine | 是 | 是（empty = fail-open） | **保留同步** | app_setup.rs:189 |
| AppLogSettings | 否 | 否（纯副作用） | **挪后台** | app_setup.rs:111-128 |
| ProxySettings | 否 | 否（可默认启动） | **挪后台** | app_setup.rs:439+ |
| CodingTools | 否 | 否（失败仅 warn） | **挪后台** | 应挪但未确认 |

**启动期必须同步完成**：
1. 数据库打开（所有后续命令依赖）
2. 路由引擎加载（影响 proxy 转发决策）

**可后台化**：
- 日志系统初始化（启动窗口间 tracing 无-op，无功能丧失）
- 设置同步（advisory file 操作，失败无块）
- 代理启动（用户显式点「启动」才生效）

## 检查清单
```bash
# 改动后验证
grep -n "block_on" src-tauri/src/app_setup.rs | wc -l
# 应 ≤ 2（DB 初始化 + engine.reload）

# 查看后台 spawn 个数
grep -n "tauri::async_runtime::spawn" src-tauri/src/app_setup.rs | wc -l
# 应 ≥ 5（log_init + sync_settings + stats_agg + count_tokens + proxy 等）
```

## 案例
commit 150374ec（perf(startup): 启动期 block_on 后台化）：
- 行 30：DB 打开保留 block_on（关键路径）
- 行 189：engine.reload 保留 block_on（转发决策依赖）
- 行 111-148：log_init/sync_settings 挪后台（纯副作用）
- 行 439+：proxy 配置读取挪后台（非启动阻塞）

## 适用
- 冷启动 > 3 秒时优化关键路径
- 启动 span 里新增 block_on 需审视是否真必须同步
- 后台 spawn 需绑定 AppHandle 确保进程退出前 WorkerGuard 存活

## 副作用
- 错误上报时间延后（日志系统晚启 500ms-1s）
- 异步任务内早期异常需额外 tracing 日志（启动窗口短暂无 subscriber）
