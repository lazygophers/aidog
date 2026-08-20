---
layer: recall
created: 1722470400
title: error-type-seam-not-string-match
category: test
keywords: [testing,error,assertion,seam,regression,string-match]
status: active
inclusion: auto
---

## 错误类型判别而非字符串匹配（回归防线）

## 触发场景

根因可能复现的 bug（如配置污染、状态漂移），需要写一条回归防线测试钉住该根因的行为。

## 陷阱

断言错误信息的**整句文案**：
```rust
// ❌ 脆断：文案一改就要改测试
assert!(err.to_string().contains("port 9890 is already in use"));
```
- 文案会随 i18n / UI 改动而改动
- 测试变成对实现细节的硬编码
- 测试通常晚于重构发现，失去防线作用

## 正解

断言错误的**可判别特征**（类型 / 枚举 / 前缀 / 是否含字段）而非整句文案：
```rust
// ✅ 稳健：文案改不影响测试，只要错误分类逻辑不变就通过
assert!(matches!(err, ProxyBindError::AddrInUse(port) if port == 9890));
```

同时断言与错误无关的不变量（本 bug 的根因）：
```rust
// ✅ 钉住根因 2：启动失败后设置值不被改写
let settings_after = load_settings();
assert_eq!(settings_after.proxy.port, 9890);  // 用户设的值
```

## 测试接缝选择

三层规则：
1. **优先复用现有接缝**，不新建测试基建
2. **取最高接缝**（越靠外部行为越好），如 HTTP 返回值 > 内部函数返回值
3. **越少越好**，理想 = 1 个接缝覆盖一个根因

## 案例

**proxy-port-no-drift 回归防线**（`gateway/proxy/test_bind.rs`）：
```rust
#[test]
fn test_bind_addr_in_use_not_retry() {
    // 接缝 = 启动函数返回值 + 启动后设置值
    
    // 1. 先占住目标端口（外部资源）
    let listener = TcpListener::bind("127.0.0.1:9890").unwrap();
    
    // 2. 调启动，预期绑定 9890 → 失败（端口被占）
    let err = proxy_start(9890).await.unwrap_err();
    
    // 3. 断言错误类型是端口占用，不是其他绑定失败
    assert!(matches!(err, ProxyBindError::AddrInUse(9890)));
    
    // 4. 钉住根因 2：设置值未被改写
    let settings = load_settings().await;
    assert_eq!(settings.proxy.port, 9890);  // 原值保持
    
    drop(listener);  // 释放端口
    
    // 5. 重试应成功（确保失败是一过性，非永久性）
    let result = proxy_start(9890).await;
    assert!(result.is_ok());
}
```

## 适用

- 所有根因可复现的 bug 修复
- 需要区分不同失败模式的测试
- 跨 crate / 跨 i18n 的行为测试

## 关联

（本规则体现「回归防线」与「接缝选择」通用原则）
