# PRD — 平台计费类型识别默认 coding plan（智能粘贴自动识别补全）

## 背景

社区分享帖常含「计费类型（PRO/coding 套餐）+ 过期时间」，用户期望**智能粘贴一键识别并默认填好**，无需手动勾选。用户实测 MiMo PRO 分享帖（`token-plan-cn.xiaomimimo.com`，「6.27 到期」）报 **「没有识别到过期时间」**。

用户决策（AskUserQuestion）：**智能粘贴自动识别** —— coding-plan 平台自动默认 `coding_plan=true`，过期时间自动填入并可见。

## 现状核查（planning 已查证）

| 链路 | 文件:行 | 状态 |
|---|---|---|
| coding_plan preset 标记 | Platforms.tsx:29-47（glm/kimi/minimax/qianfan/xiaomi_mimo `codingPlan:true`） | ✅ |
| 解析透传 codingPlan | platformPaste.ts:296-332 matchPlatform 返回 codingPlan | ✅ |
| xiaomi_mimo token-plan host 升级 coding 变体 | platformPaste.ts:504-511 | ✅ |
| applyPaste 写 coding_plan | Platforms.tsx:1629 `handleProtocolChange(value, codingPlan)` | ✅ |
| 过期时间解析 | platformPaste.ts extractExpiryAt（DATETIME_RE 匹配「6.27」+ 语义词「到期」+ 60 字符门槛） | ✅ |
| SmartPasteModal 展示过期 | SmartPasteModal.tsx:135/325-334/365 | ✅ |
| **applyPaste 写过期到表单** | **Platforms.tsx:1650 只 `setExpiresAt`，漏 `setExpiryEnabled(true)`** | ❌ **断点** |
| 表单渲染过期字段 | Platforms.tsx:3184 仅 `expiryEnabled===true` 才渲染 datetime-local | （依赖上一行 toggle） |
| 字段名一致 | api.ts:206 `expires_at:number` / platform.rs:187 `expires_at:i64` | ✅ |

**根因**：`coding_plan` 自动识别**已完整可用**；唯一断点是 `applyPaste` 设 `expiresAt` state 但未同步置 `expiryEnabled=true`，表单 toggle 默认 OFF → datetime-local 隐藏 → 用户进表单看不到识别到的过期日期 → 误判「没识别」。

## 目标

智能粘贴识别到过期时间时，表单**自动展示**该过期日期（toggle 自动 ON），与 coding_plan 自动识别行为对齐，形成「计费类型 + 过期时间」一键识别完整闭环。

## 范围（用户决策：扩大到 coding_plan 泛化）

### coding-plan 识别两套机制（现状）
- **机制 A — host 子串匹配**（matchPlatform，platformPaste.ts:306-322）：coding 变体 preset 带独立 coding host（如 `token-plan-cn.xiaomimimo.com` / `open.bigmodel.cn/api/coding`），最长子串胜出 → `codingPlan:true`。**已通用**：任何带 coding host 的 preset 自动生效。
- **机制 B — token 前缀兜底**（platformPaste.ts:506-512）：纯 token 粘贴无 base_url 时 host 匹配触不到 coding host，靠 apiKey 前缀（mimo 为 `tp-`）升级到 coding 变体。**当前硬编码仅 xiaomi_mimo**。

### 必做
1. **泛化机制 B（核心）**：把 xiaomi_mimo 硬编码的 token-前缀升级改为**数据驱动**，覆盖所有有 coding 变体的平台。
   - 为 coding 变体 preset 增 `codingKeyPrefixes?: string[]` 字段（Platforms.tsx PROTOCOLS + platformPaste.ts PastePresetRef 同步），承载该平台 coding-plan 专属 token 前缀。
   - 升级逻辑泛化：matchPlatform 命中**非 coding 变体**时，若存在同 `value` 的 coding 变体且任一 apiKey 命中其 `codingKeyPrefixes` → 升级到 coding 变体。删除 `value === "xiaomi_mimo"` 特判，xiaomi_mimo 改为数据项（`codingKeyPrefixes: ["tp-"]`）。
   - **research 子步**：逐个 coding-plan 平台（glm/kimi/minimax/minimax_en/qianfan/xiaomi_mimo）查证其 coding-plan token 是否有**专属前缀**（区别于普通版 key）。有专属前缀 → 填入数据；无法区分（coding 与普通 key 同形）→ 不填前缀（仅靠机制 A host 匹配，记入 check 报告）。前缀须有依据（官方文档/分享样本/现有代码），无依据不臆造。
2. **修 expiry toggle 断点**：`applyPaste`（Platforms.tsx:1650）识别到 `expiresAt>0` 时同步 `setExpiryEnabled(true)` + 更新旧注释（原「保持 toggle OFF」翻转为「自动 ON」）。
3. **测试覆盖**：platformPaste.test.ts 补——① 泛化后每个有前缀的平台「纯 token 粘贴 → codingPlan=true」断言；② 「粘贴 MiMo PRO 文案 → coding_plan=true + expiresAt 同时识别」；③ 回归原 xiaomi_mimo `tp-` 用例不退化。applyPaste 的 expiryEnabled state 写入若无组件测试设施则以 parser 契约兜底，限制记入 check 报告。

### 不做（明确排除）
- 不改后端 Rust 链路（字段名已一致，无需动）。
- 不改 SmartPasteModal 展示逻辑（展示正常）。
- 不臆造无依据的 token 前缀（无依据的平台保持仅机制 A）。

## 验证

- `yarn build`（tsc + vite）零错误。
- 新增/现有前端测试通过（platformPaste.test.ts）。
- 手工验证：粘贴 MiMo PRO 分享文案 → SmartPasteModal 显示 coding plan 变体 + 过期时间 → 点「填入表单」→ 表单 coding_plan 端点已选 + 过期时间 toggle ON 且显示 6.27 日期。
- `cd src-tauri && cargo build`（确认未误伤后端，预期无改动）。
- 若有 `scripts/check-i18n.mjs` 涉及新 key 则跑（本任务预期无新文案 key）。

## 资源

- 改：`src/utils/platformPaste.ts`（PastePresetRef 增 codingKeyPrefixes 字段 + 泛化升级逻辑 506-512）
- 改：`src/pages/Platforms.tsx`（PROTOCOLS preset 增 codingKeyPrefixes 数据 + applyPaste:1650 expiry toggle）
- 测试：`src/utils/platformPaste.test.ts`（泛化前缀 + coding_plan/expiresAt 联合 + 回归断言）
- 参考（不改）：`src/components/platforms/SmartPasteModal.tsx`、`src/services/api.ts`、`src-tauri/src/gateway/models/platform.rs`
- 现有 memory：`xiaomi-mimo-token-plan-no-api`（MiMo token-plan 背景）、`platform-smart-paste-parser`、`extractexpiryat-false-positive-fallback`

## 依赖

无外部依赖。含 research 子步（查各平台 coding token 前缀）→ 实现 → 测试，顺序串行，单交付单 worktree。

## 风险/取舍

- `expiryEnabled` 注释原写「粘贴识别填入 expiresAt 但保持 toggle OFF（用户手动启用）」是**旧设计意图**，本任务按用户最新决策（自动识别可见）翻转为「自动 ON」，须同步更新该注释避免误导。
- applyPaste 缺组件级测试设施，state 写入断言可能无法直接覆盖；以 parser 契约测试兜底，限制记入 check 报告。
