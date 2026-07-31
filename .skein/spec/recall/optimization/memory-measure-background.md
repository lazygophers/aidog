---
title: memory-measure-background
layer: recall
category: optimization
keywords: [memory,measure,background,activate,settle,foreground]
status: active
protected: true
---

## 内存量测走纯背景态口径

## 触发场景

内存占用量测时，采用 CPU 量测的 `activate + settle` 两段试图通过前台激活 + 等待稳定来排除用户干扰。但对内存量测而言 activate 后应用只能保住前台 10-20 秒就被 macOS 抢走，导致整轮数据作废（run1/run2 各 4 档全废）。

## 陷阱 & 正解

❌ **陷阱**：内存量测复用 CPU 量测的 activate + settle 口径

```bash
# CPU 量测：activate 前台 + settle 90s 等稳定
activate_app <pid>
sleep 90

# 内存量测时照搬同样流程：期望保持前台 90s
# 实际：activate 后 10-20s 就被系统抢走，数据不可用
```

内存占用不受前后台状态影响（与 CPU 不同），而强制前台反而造成系统抢占、数据噪声倍增。

✅ **正解**：内存量测走纯背景态，launch 后立即让前台给 Finder

```bash
# 内存量测：无 activate，启动后直接隐到后台
/Applications/aidog.app/Contents/MacOS/aidog &
main_pid=$!

# 立即让前台给 Finder（无 activate 调用）
# 全程后台，用户正常使用不受影响

# 延迟 3-5 秒等 launch 完全，然后开始采样
sleep 3
# 开始内存采样（background 态）
```

背景态采样不受用户操作污染，读数稳定可靠。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| 内存+CPU 都用 activate + settle | 内存用背景态（无 activate），CPU 仍保持 activate |
| activate 后期望前台稳定 | launch 后主动让前台给 Finder，全程后台采样 |
| 90s settle 用于内存稳定 | settle 仅 CPU 需要，内存 launch+3s 延迟即可采样 |

## 案例

run1/run2 内存量测全 4 档失效，对比日志发现 activate 后应用被 Finder 抢走。改为背景态启动后，数据立即稳定可用。CPU 量测仍保持 activate + settle（保证前台态下有代表性的稳定数据），两者采样口径分化，各得其所。

## 适用

- Tauri / Electron 应用内存占用基准量测
- 长时间后台内存监控（避免前台抢占）
- 交叉对比前台/后台内存差异时需分别采样
