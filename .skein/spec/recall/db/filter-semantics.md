---
name: filter-semantics
title: filter-semantics
layer: recall
category: db
keywords: [db,rule,spec]
created: 1725080438
inclusion: auto
---


## 排斥列默认过滤需明确确认为产品设计意图

当 task 涉及「默认排斥某类请求」的过滤逻辑时（如 Logs 主页默认隐藏 test/quota 请求），确认这是**产品设计意图**而非可优化的冗余判断。

**检查清单**（logs-query-ipc-slimming s3）：

1. **排斥值是否真的被写入**（grep 数据源）
   - test 协议来自 `src-tauri/crates/aidog_core/src/ai_tools_cmd/model_test.rs:157`（模型测试面板）
   - quota 协议来自 `src-tauri/crates/aidog_core/src/gateway/quota/http.rs:187`（自动探测轮询）
   - 两者都是日常操作触发的正常路径，非边角场景

2. **UI 层默认值与后端行为一致**（diff 两侧）
   - 前端恒发 `["test","quota"]`（`useLogsFilters.ts:39`）
   - 后端恒对非空 exclude_sources 生效过滤（`proxy_log.rs:564`）
   → 一致性 ✓，无需对齐改动

3. **验证目的**（查注释/PRD）
   - Logs 主页注释：「已迁到 RequestLog 新页」（产品分离）
   - 这是**故意的架构选择**，不是可删掉的恒真判断

**结论**：
- 不跳过该谓词；不建索引试图「优化掉」它（治标，真正应该优化的是算法——改 COUNT 为 LIMIT+1）
- 若设计说「可跳过恒发谓词」而实测发现并非恒真，设计前提错误，研究文档中明确标注「前提证伪」

**痕迹**：
- 设计.md 的方案条目本身基于错误前提，并不修改设计.md（保持原始文档），仅在 s3-predicate.md research 文档中加注说明结论

**适用**：任何涉及「默认过滤条件可否优化」的 task

---

