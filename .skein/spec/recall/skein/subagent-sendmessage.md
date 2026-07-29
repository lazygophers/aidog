---
title: subagent-sendmessage
layer: recall
category: skein
keywords: [subagent,sendmessage,return-value,coordinator,message-passing]
status: active
---

## agent 零回传真因 = 未调 SendMessage

## 触发场景

派 subagent（如 researcher / checker 等）时，应答端只写 stdout 文本输出，未调用 `SendMessage` 工具回传结构化数据。主 agent 消费文本作为回传摘要，但因缺轴向数据（无返回 JSON 字段）导致记忆沉淀、下游任务调度等环节失效。

## 陷阱 & 正解

❌ **陷阱**：仅写文本输出，不调 SendMessage 工具

```python
# subagent 应答端
print("发现 3 个问题...")
print("文件 1：...")
print("文件 2：...")
# 就这样 return，无 SendMessage 调用
```

主 agent 只能读 stdout 文本摘要，无法提取结构化字段（findings / recommendations / metadata 等），后续流程无法自动化。

✅ **正解**：除了文本输出外，还需调 SendMessage 工具回传完整 JSON

```python
# subagent 应答端
from tools import SendMessage

findings = [...]
recommendations = [...]

# 先 print 文本总结（供日志审阅）
print("发现 3 个问题...")

# 再调 SendMessage 回传结构化数据
SendMessage(
  tool_name="SendMessage",
  data={
    "status": "done",
    "findings": findings,
    "recommendations": recommendations,
    "needs_main": [...]
  }
)
```

这样主 agent 既有文本摘要供审阅，也有 JSON 轴向数据供后续流程消费。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| 仅 print/echo 文本输出 | print 文本 + 调 SendMessage 回传 JSON |
| 期望主 agent 从文本解析字段 | 结构化数据由 subagent 主动装箱回传 |
| 主 agent "无返回值" → 流程中止 | SendMessage 确保轴向数据到达 coordinator |

## 案例

既有约定记录 3 个实例系统性不回传。根因是这些 subagent 仅写 stdout，未调 SendMessage。修复后 check 类 agent 改为同步调 SendMessage，主 agent 获得结构化回传，记忆沉淀 / 下游任务调度恢复正常。

## 适用

- 所有派 subagent 的场景（researcher / checker / workflow / skill）
- 需要结构化回传给 coordinator 的任务
- 日志审阅 + 自动化流程需要并存的场景
