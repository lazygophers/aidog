# Research: hoist 因果重评估（方案 B 前提崩塌 + 真因待查）

- **Date**: 2026-07-02
- **来源**: hoist 方案 B 实施前 DB 复核（subagent），推翻 research/error-chain-diagnosis.md §4.2 假设
- **只读诊断，未改码**

---

## 1. 方案 B 前提崩塌

方案 B（漏 stream 跳 hoist，显式 stream=false 保留）门控 `chat_req.stream == Some(false)`。

**DB 实证（全库）**：**零显式 stream=false**。客户端单向字段：
- 流式：显式发 `stream:true`
- 非流式：**省略字段**（is_none），从不发 false

→ 方案 B 门控**永假** = 无条件禁用 hoist = `forward.rs:587 fn hoist_mid_messages_system` 变死代码。

## 2. research §4.2 因果假设被推翻

原假设（error-chain-diagnosis.md §4.2）：漏 stream → is_stream=false → hoist 跑 → GLM 1210。

**DB 实证**：
- 18 条 GLM 1210 失败：**全是漏 stream，hoist 跑了仍 1210**
- cb3603ac（漏 stream，hoist 跑）→ 1210
- 471c14…（漏 stream，hoist 跑，7 system 提顶层）→ **200 成功**

两案例**都漏 stream、都 hoist 跑**，一失败一成功。**hoist 非决定因素**。

## 3. 非流式 PASS 群体（bigmodel anthropic, is_stream=0, status=200）

总 1150；漏发 stream 266；显式 stream=false = **0**。

266 条漏发非流式 PASS 含 role=system-in-messages 的 5 条（理论 hoist 目标）：

| id | n_msgs | req sys-in-msgs | upstream sys-in-msgs | hoist 行为 |
|---|---|---|---|---|
| ca357 | 112 | 25 | 25 | no-op（req==upstream） |
| 32873 | 14 | 3 | 3 | no-op |
| 7bd13 | 41 | 12 | 12 | no-op |
| ab264 | 2 | 1 | 1 | no-op（首轮无 assistant） |
| **471c14** | 54→47 | 7 | 0 | **真 hoist（7 条合并顶层）** |

5 条里 4 条 hoist no-op（top-level system 客户端自带，messages 原序），不靠 hoist 也 PASS。**仅 471c14 是真 hoist 救场**，且它也漏 stream。

## 4. 真因待查（cb3603ac vs 471c14 差异）

| 维度 | cb3603ac（1210 失败） | 471c14（200 成功） |
|---|---|---|
| stream | 漏发 | 漏发 |
| hoist | 跑（40→0 in messages，顶层 2→42 块） | 跑（7→0，顶层 +7） |
| messages 数 | 231 | 54 |
| role=system in messages | 40 | 7 |
| 顶层 system 块数（hoist 后） | **42** | **9** |

**推测**：GLM 对顶层 system 数组块数有上限（42 超限 → 1210，9 OK）。待验证。

## 5. 结论 + 后续（拆另开 task）

- hoist 非本 bug 决定因素，方案 B/D/E 修门控均无效
- 真因在 cb3603ac vs 471c14 结构差异（顶层 system 块数上限？messages 数？body 大小？）
- 本 task 不修 hoist，结论沉淀。hoist 深挖拆另开 task，数据驱动定位真因
- 验证方向：构造不同顶层 system 块数 body，curl GLM 端点测阈值

## Files Found

| File | 证据 |
|---|---|
| `src-tauri/src/gateway/proxy/forward.rs:272` | hoist 门控（方案 B 未实施） |
| `src-tauri/src/gateway/proxy/forward.rs:587` | hoist_mid_messages_system 实现 |
| DB ~/.aidog/aidog.db | 18 条 1210 + 5 条非流式 PASS 含 sys-in-msgs |
