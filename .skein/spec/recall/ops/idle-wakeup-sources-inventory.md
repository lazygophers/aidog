---
title: 空闲期唤醒源 6 分类清单（静态检索）
layer: recall
category: ops
keywords: [wakeup,timers,scheduler,sources,profiling,static-analysis,cpu]
status: active
inclusion: auto
protected: true
---

## 空闲期唤醒源 6 分类清单

空闲期 CPU 唤醒源分 6 类，静态 rg 检索无遗漏（src-tauri + src）。

| 分类 | 频率 | 空闲触发 | 检验方法 |
|---|---|---|---|
| tokio interval / sleep | 60s(DB) / 300s(托盘) / 24h(维护) | 是 | `rg "interval\|sleep\("`, 排除 test |
| CSS 无限动画 | 常驻（0.9-3s 周期） | 是 | `globals.css` 5 个动画 + popover 1 个，无 `document.hidden` gate |
| 前端 setInterval 轮询 | 0 处 | 否 | `rg "setInterval" src/` 无匹配，只有一次性 setTimeout |
| FS watcher | 0 处 | 否 | `rg "notify\|RecommendedWatcher"` 无匹配，无 `notify` crate 依赖 |
| 网络心跳 | 0 处 | 否 | reqwest 池无 keepalive 配置；accept 阻塞 epoll；无出站探测 |
| SQLite 维护 | 24h（VACUUM/checkpoint） | 否 | `wal_autocheckpoint` 走默认值，写触发非定时 |

### 精细化尝试

- 查 `gateway/backup/scheduler.rs` 逻辑：60s tick 是硬唤醒（sleep 在判定之前），`enabled=false` 不阻止唤醒，仅内部 `maybe_backup` 快速返回
- 审 `app_setup.rs:428-450` 托盘刷新：仅两个触发源（tray-refresh 事件 + 300s coarse tick），event 驱动时空闲为零
- CSS 动画分布：主窗口 5 个（shimmer/spin/pulseGlow/statusPulse/flowBorder）+ popover 1 个 statusPulse，隐藏窗口常驻

### 关联

[[idle-cpu-baseline-xctrace]] 栈归因验证了前 3 类均落框架底噪 / [[measure-window-exclusive-env]] 清单取样环境可控
