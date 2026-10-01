# Flutter 测试环境依赖：内核二进制缺失的批量假红

2026-10-01 轮：worktree 与主 checkout 均无 `aidog-kernel` 二进制时，`flutter test`
假红 98 个——大量 page widget 测试真起内核子进程（`flutter/lib/src/transport/kernel_process.dart`
的 `_findKernelBinary`，`flutter/test/platform_integration_test.dart` /
`kernel_integration_test.dart` 一族），二进制缺失时逐个报
`Bad state: aidog-kernel not found`。构建后 98 → 9 真。

## 判据（先于「代码红」判定）

批量红且集中 page/widget 测试时，**先 grep 报错原文再判代码问题**：

```
flutter test 2>&1 | grep -c "aidog-kernel not found"
```

- 命中数量 ≈ 红的总数 → 环境性假红，不是代码回归，别逐个修测试。
- 真红在剩余里，构建二进制后重跑再判。

## 修复（三选一）

1. 构建开发用二进制：`cargo build -p aidog_kernel`（src-tauri 目录），产物落
   `src-tauri/target/{debug,release}/aidog-kernel`，测试按向上搜索路径找到。
2. 已有二进制在别处：`export AIDOG_KERNEL_BIN=<绝对路径>`。
3. CI / 新 clone 的 worktree 首跑 flutter test 前，先确认上面两条之一成立。

查找逻辑真值源：`flutter/lib/src/transport/kernel_process.dart:218-267`
（env `AIDOG_KERNEL_BIN` → 可执行文件旁 → 向上找 `src-tauri/target/{release,debug}/`）。
