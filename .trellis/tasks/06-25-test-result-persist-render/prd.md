# 测试结果持久化 + JSON 解析展示 (test-result-persist-render)

## 目标
平台/模型测试的结果（形如 `382ms { "error": { "code": ... } }` 这类「耗时 + 响应体」）：
1. **长久持久化** —— 测试结果不再是临时/闪现态，跨页/重启后仍可查看。
2. **可解析则展示解析后内容** —— 对可解析的、已知结构的 JSON（如 error 对象、usage、message 等），展示**格式化/结构化解析后**的内容，而非原始 code 字符串；不可解析的才回退原始文本。

## 背景 (Explore 须先核实, 勿臆断)
- 测试结果来源：平台卡片「测试」/ ModelTestPanel 的 model_test。
- 已知现状（须先核实）：model_test 早落 `proxy_log`（见记忆 [[platform-last-test-badge]] / [[model-test-proxy-parity]]）；平台卡片有「最近测试」常驻徽章 + 全局事件跨页刷新。但**测试结果正文（响应体 JSON / 错误体）的持久化与解析展示**可能缺失或仅闪现。
- 关键先确认：当前测试结果在哪里产生、当前怎么展示（原始字符串拼 `${ms}ms ${body}`？）、是否已入库、入库在哪张表哪列。

## 需求拆解
### 持久化 (后端可能涉及)
- 测试结果（耗时 + 状态码 + 响应体/错误体 + 时间戳）需持久化，长久可查（复用 proxy_log 已有落库，或最近测试结果落 platform 关联存储 —— **优先复用既有 proxy_log / last-test 机制，禁新造表除非确无去处**）。
- 跨页切换、应用重启后，平台卡片/测试入口仍能展示最近一次测试结果。

### 解析展示 (前端)
- 渲染测试结果时，先尝试 `JSON.parse`：
  - 成功且匹配已知结构（error / usage / message / choices 等）→ 渲染**结构化视图**（key-value / 高亮 / 友好错误信息），而非整坨 raw JSON code 串。
  - 解析失败或未知结构 → 回退展示原始文本（保留现状兜底）。
- 耗时（`382ms`）与正文分离展示，正文按上面规则解析渲染。
- 复用既有格式化工具（`src/utils/formatters.ts`）+ 既有展示组件（`components/shared/`），禁页内重复造格式化逻辑。

## 复用 (Explore 先做)
- 后端：proxy_log 落库 / last-test badge 机制（db.rs / lib.rs / proxy.rs）。
- 前端：Platforms.tsx 测试结果产生与展示处、`utils/formatters.ts`、`components/shared/`。

## 验收标准
- 测试一次后，跨页切换 + 重启应用，最近测试结果仍可见（持久化生效）。
- 已知 JSON（如 error 体）展示为解析后的结构化内容，非原始 code 串。
- 不可解析内容回退原始文本，无崩溃。
- 耗时与正文分离展示。
- `cargo build` + `cargo clippy`(warning 清零) + `cargo test` + `yarn build` + `node scripts/check-i18n.mjs` 全绿（若改动涉及对应层）。
- Rust↔TS 边界字段名对齐（若新增/改 command）。

## 非目标
- 不改测试请求本身的发起逻辑（model_test/proxy parity 不动语义）。
- 不做测试历史的复杂时间线 UI，仅「最近一次结果持久 + 解析展示」。

## 单一交付
单一可验收交付（持久化 + 解析展示），单 worktree。
