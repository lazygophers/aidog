---
title: tauri-applescript-pitfalls
layer: recall
category: platform-macos
keywords: [tauri,applescript,macos,wry,window-control,accessibility,ax-tree]
status: active
inclusion: auto
created: 1722524400
---

## Tauri + wry 窗口调用 AppleScript 的两大坑

## 触发场景

使用 Tauri 2.0 + wry 嵌入式浏览器时，通过 AppleScript 控制窗口（尺寸 / 缩放 / 聚焦）进行自动化测试或性能量测。AppleScript 与 Accessibility 框架交互存在两个隐性合约，违反会导致操作无效或报错。

## 坑 1：launch 后必须先 activate，窗口才进 AX 树

### 陷阱

❌ 直接在 launch 后调用窗口操作（如 tell / count windows），不走 activate：

```applescript
# 错误：launch 立即操作，AX 树未就绪
set windowCount to count windows of application "AiDog"
# 结果：恒返回 0，即使窗口已启动
# 错误信息：无，执行成功但返回值错误
```

**根因**：Tauri 应用的窗口在 Accessibility 框架初始化前，Accessibility 树（AX tree）状态为空。直接 count 会返回 0，极易误判成「Accessibility 未授权」或「窗口未启动」。

### 正解

❌ launch 后，**必须先 `activate` 应用**，迫使应用获得焦点并将窗口注册到 AX 树，再进行其他操作：

```applescript
# 正确：launch → activate → 操作
tell application "AiDog" to activate
delay 0.5  # 等待 AX 树初始化

tell application "AiDog"
    set windowCount to count windows  # 现在返回正确值（通常 1）
end tell
```

### 案例

性能量测 preflight 阶段（s1-preflight），误判应用未启动或 Accessibility 未授权 3 次，原因均为漏掉 activate 步骤。改进后万无一失。

## 坑 2：窗口不支持 `zoomed` 属性，改尺寸用 position + size

### 陷阱

❌ 试图用 `set zoomed to true` 将窗口最大化：

```applescript
# 错误：wry/Tauri 窗口不支持 zoomed
tell application "AiDog"
    set zoomed of window 1 to true  # 报错 -10006（unsupported）
end tell
```

**根因**：macOS 标准应用（Safari / Finder）的窗口对象支持 `zoomed` 属性（绿色放大按钮），但 wry 嵌入式窗口是原生 NSWindow，Accessibility 框架向外暴露的接口**不包含** `zoomed`。

### 正解

✅ 改用 `position` + `size` 直接设置窗口坐标与尺寸，模拟「最大化」：

```applescript
# 正确：计算屏幕尺寸，设置 position 和 size
tell application "AiDog"
    activate
    set {x, y, w, h} to bounds of screen 1  # 屏幕坐标与分辨率
    set position of window 1 to {x, y}
    set size of window 1 to {w, h}
end tell

# 或针对特定测试场景，用固定尺寸
tell application "AiDog"
    set size of window 1 to {2304, 1265}  # 最大化对标尺寸
    set position of window 1 to {0, 0}
end tell
```

### 案例

性能量测最大化对照组需要窗口达到最大尺寸（2304×1265），早期试图用 `zoomed` 失败，改用 `position`+`size` 后成功。

## 应用进程名细节

- AiDog 应用的 Accessibility 进程名是**小写 `aidog`**（非 `AiDog`）
- fork/exec 二进制时应用名采用二进制小写名，AppleScript `tell application` 识别时仍用对外标记名 `"AiDog"`
- `pgrep -x aidog` 核对进程存活时用小写

## 验证清单（性能量测常用）

- [ ] launch 后立即调用 `activate`，wait ≥0.5s AX 树初始化
- [ ] 窗口操作前先 `count windows`，确认非 0
- [ ] 改尺寸用 `position` + `size`，禁用 `zoomed`
- [ ] 最大化对标尺寸 `{2304, 1265}`（满屏前提）
- [ ] 调试时用 `osascript -e 'script'` 单步验证

## 适用

- Tauri 2.0 应用的 AppleScript 自动化（测试 / 量测 / 监控）
- macOS 桌面应用性能测量场景（窗口尺寸变更导致内存对比）
- Accessibility 框架交互时的合约验证
