---
title: cross-task-artifact-retention
name: cross-task-artifact-retention
description: 砍 subtask 前先 grep 下游 task 依赖 — 临时产物清场须通过引用链检查
layer: recall
keywords: [task,依赖,清场,artifact,subtask,可行性]
created: 1785564340
inclusion: auto
---

## cross-task-artifact-retention

## 砍 subtask 前先 grep 下游 task 依赖 — 临时产物清场须通过引用链检查

当对 task 内部 subtask 进行删除或变更（如基于新的实测发现改变原 task 范围）时，必须检查下游 task（deps 中提及的 task）的验收标准或关键步骤是否依赖那些被砍 subtask 的产出。过早删除会在下游启动时才炸出来。

## 陷阱：假设已证伪后主动砍 subtask，但下游 task 口径写死依赖它的产出

window-default-size task 场景：

**旧假设**："窗口面积与内存线性相关，删 maximized:true（改默认 1026×759）可降 graphics memory"

**实测证伪**（run3 实测，2026-07-29）：面积涨 3.7 倍，TOTAL 反而降低 ± 95MB 噪声范围，无可信拟合式。

**砍掉的 subtask**：s3-preflight / s4-static-audit / s5-measurement / s6-verify 中涉及验证「删 maximized:true 后默认窗口 1026×759 非最大化」的改动。

**隐患**：下游 task `perf-final-verification` 的 s1-preflight 冒烟验证依赖 `.scratch/perf-200mb/assets/results/` 下 22 个采样文件（window-default-size s1-s2 产物），以及 `window-size-measure-protocol.md` 的量测协议。这两样原本应由 window-default-size 的清场步骤（s6 最后）负责保留/清理，现在 s3-s6 被砍了，清场决策就失效了。

## MUST 砍 subtask 前检查下游 deps

```bash
# 1. 查出下游 task（在 .skein/task.json 中有 deps 或 blocks 关联）
grep -l '"window-default-size"' .skein/task/*/task.json
# 输出：.skein/task/perf-final-verification/task.json

# 2. 逐条读下游 task 的 PRD/验收标准，grep 上游产物名称
grep -i "window-default-size\|size-curve-raw\|protocol.md" \
  .skein/task/perf-final-verification/prd.md \
  .skein/task/perf-final-verification/task.json

# 3. 若发现引用，则砍 subtask 前必须明确：
#    a) 移交明确时点（在下游 task 哪个 subtask 中执行清场/删除）
#    b) 交接手在下游 task 文档中有对应动作记录
```

## 正解：明确移交清场时点与责任

当主动砍掉可能影响下游的 subtask 时，在当前 task 的验收记录中标明：

```markdown
## 清场/交接记录

| 产物 | 涉及文件 | 处置 | 移交时点 |
|---|---|---|---|
| 采样数据 | `.scratch/perf-200mb/assets/results/` 22 个文件 | 移交清场 | perf-final-verification s1-preflight 冒烟验证完成后 |
| 量测协议 | `window-size-measure-protocol.md` | 须保留 | perf-final-verification 全程 |
| 临时脚本 | `assets/run-size-curve.sh` 等 | 移交/可删 | 明确时点 |
```

关键：

1. **明确「移交」vs「删除」的时点**——不能只说「移交给下游」，要说「在下游的哪个 subtask 之后」
2. **交接手需在下游 task 的起始文档中有对应记录**——perf-final-verification 的 prd/task.md 要明确提及「window-default-size 产物清场规定在 s1-preflight 后执行」
3. **做验收时逐条对账**——清场 checklist 逐项核对，不能只标「已移交」

## 反例（错误做法）

| 错做法 | 后果 | 正做法 |
|---|---|---|
| 砍 subtask 后不记录清场交接 | 下游 task 启动才发现产物被删，冒烟失败 | 同步更新当前 task 与下游 task 的清场 checklist |
| 只说「移交」不说时点 | 下游 task 不知道何时可安全删除 | 「移交 perf-final-verification s1-preflight 完成后」 |
| 删除是「之后再说」的悬案 | 下游 task 完成后产物仍堆积 | 在 checklist 中明确规定在下游的哪个 subtask 执行 |

## 验收

- [ ] 所有被砍 subtask 的产出在 task 清场记录中逐条列举
- [ ] 产出标记为「移交」时，明确提及下游 task 名和执行时点（subtask 编号）
- [ ] 下游 task 的 prd/task.md 中有对应的「清场责任」段落
- [ ] 若执行了早期清场，下游 task 冒烟验证不失败

## 适用

- 任何主动砍掉 subtask 或改变 task scope 的情况
- 跨 task 依赖存在的场景（尤其是长流程的性能/基础设施任务）

## 实例

task 场景：window-default-size 因实测证伪（窗口面积与内存无线性关系）砍掉 s3-s6，22 个采样文件需移交 perf-final-verification 的 s1-preflight 冒烟验证后清场（.scratch/perf-200mb/assets/results/）
