---
title: task-decomposition-coverage-check
category: domain
keywords: [subtask,PRD,coverage,decomposition,plan]
status: active
inclusion: auto
protected: true
---

## task 分解 → subtask DAG 覆盖检查

## task 分解 → subtask DAG 覆盖检查

### 触发场景

task 分解拆 subtask DAG 时。某次 task 有 7 个明确的目标（PRD），但原拆出的七个 subtask 漏掉其中一条目标（tracing non_blocking），check 阶段才补充 s8。

### 缺陷：计划与 PRD 不映射，漏项直到交付前夕暴露

task 计划常从「代码变化」或「功能模块」角度分解 subtask，但 PRD 目标常从「用户能力」或「质量维度」出发。两套分类体系无显式映射 → subtask DAG 覆盖不全，直到 check 合并时才发现「哦，这条 PRD 目标没人干」。

### 正解：DAG 定版前逐条 PRD 目标映射到 subtask

### MUST 检查清单

- [ ] **列出 PRD 全部目标** —— 来自 PRD/ticket，各条独立编号（如 goal-1 ~ goal-N）
- [ ] **列出 subtask DAG** —— 各 subtask 的目标/产出
- [ ] **映射矩阵**：
  ```
  PRD 目标            | 负责 subtask | 验收点
  goal-1 (feat X)     | s1, s2       | feature 可用
  goal-2 (perf Y)     | s3           | latency < Z
  goal-3 (tracing Z)  | [未映射] ⚠️  |
  ```
- [ ] **闭环**：矩阵无空行 —— 每条 PRD 目标都被至少一个 subtask cover；无孤立 subtask（干了 PRD 外的活）

### 不选别的理由

| 备选 | 否决 |
|---|---|
| check 时才核对（事后确认） | 暴露太晚，补 subtask 会破坏工时估算和依赖关系 |
| 信任 scrum master/PO 记得全部目标 | 人为因素，目标清单无版本控制，容易遗忘 |
| 按模块拆 subtask 就自然覆盖 | 模块分解与 PRD 目标维度不同，两个独立维度，需显式核对 |

### 适用

- 所有有明确 PRD/需求清单的 task
- 特别是跨多个子系统、多人协作的大型 task
