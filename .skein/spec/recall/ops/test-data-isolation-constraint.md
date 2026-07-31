---
title: test-data-isolation-constraint
category: ops
keywords: [testing,data,isolation,database,measurement,real-data]
status: active
inclusion: auto
---

## 性能测试数据隔离约束

## 测试数据隔离硬约束

性能量测或功能验证时需要用特定数据库（如缩小库、污染库等）。

### 硬约束

- **禁移动/重命名用户的真实库文件**（如 ~/.aidog/log.db） —— 移动后如失败无法恢复用户数据，超出最小风险授权解读
- **需隔离数据时改用独立数据目录**：
  - 创建临时目录（如 `/tmp/test-data/log.db`）
  - 复制真实库到临时目录（`cp` 非移动）
  - 在量测脚本中指向临时目录（env 或程序参数）
  - 量测完毕删临时数据

### 背景

某次量测为造 100MB 库环境，把用户 log.db 移到 /tmp 后移回。虽然最终 `quick_check ok` 验证无损，但**移动用户真实数据文件已超出「禁未经授权破坏性操作」的可容许范围**。后续应显式写入测试协议中。

### 验证

采用 `quick_check` 命令行工具验证数据完整性（SQLite built-in，`PRAGMA integrity_check`）。
