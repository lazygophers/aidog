---
layer: recall
created: 1785514813
title: grep-before-write
category: reuse
keywords: [grep,search,verification,refactor,change-audit]
status: active
inclusion: auto
---
layer: recall
created: 1785514813

## 修改前搜索验收清单

## 修改前搜索验收清单

对任何源文件的修改（新增列 / 删除列 / 重命名列）提交前，必须 grep 全库检查「所有读取、初始化、比较该列的代码路径」是否同步改动。忽视此步导致静默 bug（null 检查漏、比较逻辑不对、序列化格式错）是常见误触发。

### 缺陷根因

修改一个共享结构的字段后，代码涉及该字段的**所有地方**必须同步更新。漏掉任何一个读取点会导致：
- JSON 序列化/反序列化不对称
- 数据库查询返回 null / 类型错误
- 业务逻辑的比较/过滤规则失效

### 方法

**修改前**做完整清单，**提交前** grep 验收：

1. **写明修改内容**（新增/删除/重命名什么）
2. **列出所有相关代码路径**：
   - 初始化（构造/默认值/JSON seed）
   - 读取（序列化/查询返回）
   - 写入（INSERT/UPDATE 语句）
   - 业务逻辑（if/match 判定）
3. **每处逐个 grep + 改 commit message 链接之**

### 代码形态

修改 `platform.source_protocol` 字段时：
```rust
// ✓ 改动前准备清单
// 初始化点: gateway/models/platform.rs:42 (Deserialize)
// 读取点:   gateway/db/platform.rs:68 (SELECT source_protocol)
//          gateway/router/candidates.rs:104 (route by source_protocol)
// 写入点:   gateway/db/platform.rs:145 (INSERT/UPDATE source_protocol)
```

验收（提交前）：
```bash
# 清单中每条路径都实际改了
git diff HEAD^ -- gateway/models/platform.rs gateway/db/platform.rs gateway/router/candidates.rs
```

### 反例

```rust
// ❌ 改了数据结构，漏改初始化
pub struct Platform {
    pub extra: String,    // 新增字段
    // ...
}

// 但 JSON seed 只有旧字段
let defaults = r#"{"id":1}"#;
```

## 验收

- [ ] 修改前准备完整的「代码路径清单」文档
- [ ] 清单覆盖：初始化 / 读 / 写 / 业务逻辑四类
- [ ] grep 验收时每条路径都有相应的 git diff
- [ ] commit message 参考 #<code-path> 链接

## 适用

- 修改任何共享结构（SQL 表 / JSON schema / Rust struct 用到多地）
- 特别是跨层结构（Tauri command 参数 / DB model）
- 涉及序列化（serde / SQL）的字段改动

## 关联

[[component-extraction-grep-callsites]] 组件提取时的同步策略
