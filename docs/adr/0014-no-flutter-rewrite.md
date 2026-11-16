# ADR 0014: 不换 Flutter，维持 React 前端 + Tauri 壳 + 无头内核

## 背景

2026-09-02 完成换栈调研（票 01–09，全文曾在 `.scratch/stack-migration/DECISION.md`，该目录已清理）。用户有三个痛点：内存超 1 GB、CPU 飙升、空闲时也有 CPU 占用。调研要回答的问题是：换 Flutter 能否解决，以及是否同时满足低资源无界面模式、Linux、Web 三个诉求。

## 决策

1. **不换 Flutter**。前端保持一份 React，桌面外壳保持 Tauri。
2. **另出一个无头内核二进制**，多挂 `/rpc/<命令>` + SSE 路由，浏览器访问同一份 React 前端即为 Web 界面。该方向后续落地为 [ADR 0010](0010-management-api-on-proxy-port.md)（管理接口挂代理端口，路径 `/api/rpc/<命令>`、`/api/events`）。
3. 痛点按与换栈无关的低成本优化处理，不借换栈解决。

## 理由

1. 真空闲 CPU 实测约 0.03%（测量须并列记录窗口内新增请求数，只取请求数为 0 的窗口），换栈治不了「空闲 CPU」痛点。
2. 吞吐上限 1311 RPS，真实负载 0.064 RPS，余量约 20000 倍，换栈的性能收益不可观测。
3. `invoke` 调用收口率 99.0%（209 个调用点中 207 个在 `src/services/api/`），换传输层只需改少量 import 与一个 ipc 文件，不必重写前端。
4. 内存痛点中 WebView 占峰值约 80%，只与 UI 框架有关；换 Flutter 需重写 28 个 Radix 组件、图表、CodeMirror、dnd-kit、RTL，与「像素级一致」硬约束冲突。

## 已作废的依据

以下两条在调研早期被当作依据，实测后均不成立，不得再引用：

- 「Flutter 空闲 CPU 有约 6% 架构地板」：本机 Flutter 3.47.0 + Impeller + macOS 实测 0.05%，该说法来自 2020–2023 年 Skia 时代的 Linux / Windows issue。
- 「Impeller blur 退化是最大风险」：当时 Liquid Glass 样式不使用 `backdrop-filter`（纯色填充 + 细边 + 阴影）；该风险对本项目不适用。2026-10-24 UI 风格已改为观测台 G3，本条更无关。

## 后果

- Flutter 相关代码与规则已全部删除，仓库不再保留 Flutter 目录。
- 内存问题通过与换栈无关的优化处理（如减少常驻窗口、控制日志库体积）。

## 备选方案

- **换 Flutter 重写前端**：落选，理由见「理由」1–4。复活条件：做完全部低成本优化后，主 WebView 430–580 MB 的内存占用仍是不可接受的痛点。

## 来源

私有记忆 `aidog-stack-decision-no-flutter`（2026-09-02 裁决）；调研原文已随 `.scratch` 清理删除，上述数字未在本次整理中重测。
