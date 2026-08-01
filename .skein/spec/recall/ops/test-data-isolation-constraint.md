---
title: test-data-isolation-constraint
category: ops
keywords: [testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp]
status: active
layer: recall
inclusion: auto
protected: true
created: 1785560294
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

## 量测脚本 HOME 环境隔离硬约束

## 量测脚本 HOME 环境隔离硬约束

### 扩展约束：禁污染用户真实数据目录

前置约束禁止移动用户真实库文件，但仍需隔离 **整个数据目录**（不仅是单个文件）。

### 硬约束

- **量测脚本必须 `HOME` 重定向**：设置 `HOME` 环境变量指向隔离临时目录，禁止使用 `~` 展开到用户真实 home
  - 根本原因：Tauri 应用从 `~/.aidog/` 读取数据，若 loadgen 进程 `HOME` 与主进程相同，所有 fs 操作（读库、写日志、创建缓存）都会污染用户真实数据
  - 历史教训：本仓某轮压测未隔离 `HOME`，导致 26614 行测试日志（占全库 98%）写入用户真实 `~/.aidog/log.db`，清理时需完整备份后删除
- **脚本起始处硬校验 `HOME` 隔离**：
  ```bash
  case "$HOME" in
    "$HOME_REAL"|/tmp) echo "Error: HOME not redirected — refusing to run" >&2; exit 1;;
    /tmp/*) ;;
    *) echo "Error: HOME not in /tmp — refusing to run" >&2; exit 1;;
  esac
  ```
  禁靠「人工检查参数」，用代码强制校验

### 实施

```bash
export HOME_REAL="$HOME"
export HOME="/tmp/aidog-test-$$"
mkdir -p "$HOME/.aidog"

# 硬校验
case "$HOME" in /tmp/*) echo "✓ HOME isolated";; *) echo "✗ FAIL"; exit 1;; esac

# 量测脚本
# ... loadgen 命令 ...

# 清理
rm -rf "$HOME"
```

### 为什么这比「禁移动真实库」更关键

- 移动库是**一处高风险操作**，风险集中、易察觉
- 忘记隔离 `HOME` 是**持续污染**，百次操作累积，才察觉时已写入数万行噪声
- 一处 hardcoded path（`~/.aidog`）贯穿整个应用（settings、logs、db、cache），任何 fs 操作都隐含依赖 `HOME` 值

### 验证

- [ ] 脚本起始硬校验 `HOME` 重定向（退出码 1 拒绝错误配置）
- [ ] 量测前后运行 `quick_check` 确认用户真实库无新写入（对比 mtime 或行数）
- [ ] 复现用例：两个隔离 `HOME` 的并行 loadgen 进程，验证各自在独立 `/tmp/aidog-test-*` 中写入，不互相污染
