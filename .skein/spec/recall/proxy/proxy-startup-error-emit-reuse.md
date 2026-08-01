---
layer: recall
created: 1722470400
title: proxy-startup-error-emit-reuse
category: proxy
keywords: [proxy,startup,error,event,emit,notification,autostart]
status: active
inclusion: auto
---

## 无前端窗口路径的启动失败可见性

## 触发场景

代理启动失败且无前端窗口可显示（自启动、后台启动）时，需要让用户能看到失败信息（系统通知 / 托盘菜单状态）。

## 陷阱

新建独立通知链路（新 command、新事件、新事件监听），导致：
- 跨 Rust↔TS 新增接线点、测试覆盖缺口
- 与既有的「app 事件 → 前端 listener 链」重复
- 硬编码错误文案 @ Rust 侧，i18n 维护分散

## 正解

复用既有能力：
1. **事件机制**：`app.emit("proxy-start-failed", {error, port, ...})` 复用已有的 `app.emit()` 基础设施，无需新造事件系统
2. **托盘刷新**：复用既有的 `tray-refresh` 事件与 listener，启动失败后触发刷新即可在菜单中显示状态，无需新增 tray 刷新链路
3. **文案单源**：在 i18n locale 定义错误文案（8 语言），Rust 仅做错误分类，不硬编码文案

## 案例

**正例**（proxy-port-no-drift 实现）：
```rust
// Rust 侧返回错误分类，不含文案
if matches!(err, ProxyBindError::AddrInUse(port)) {
  app.emit("proxy-start-failed", json!{"reason": "addr_in_use", "port": port})?;
  app.emit("tray-refresh", ())?; // 复用既有 tray 刷新机制
}
```

```typescript
// TS 侧从 locale 取文案，组织 UI（system notification / error toast）
listen('proxy-start-failed', (event) => {
  const msg = t(`proxy.start_failed.${event.payload.reason}`, {port: event.payload.port});
  // 系统通知 + 错误条展示
});
```

## 适用

- 任何后台启动失败的可见性需求
- 跨 Rust↔TS 错误通知
- 与既有事件系统的交互

## 关联

（参考 core/cross-layer/tauri-ts-boundary-contract，Tauri 事件模式）
