# Implement — 火山方舟 CodingPlan 双端点智能粘贴识别 + openai 默认配置修复

## 执行编排
单交付，触点集中在前端 2 文件（`platformPaste.ts` 解析 + `Platforms.tsx` preset/applyPaste），存在 hosts 派生依赖（base_url 改 → hosts 自动派生）。**单 subagent 串行执行**（不拆并行）。worktree 隔离，与 Task A（engine.py + coding_plan.rs）文件零交集，并行安全。

## 触点清单（精确到行，须先复现确认）
1. **bug2 — doubao preset**（`src/pages/Platforms.tsx:233-238` `getDefaultEndpoints` doubao 分支）
   - 新增标准 `openai` 端点：`base_url: "https://ark.cn-beijing.volces.com/api/coding/v3"`，client_type 用 openai 客户端身份（核对 `defaultClientForProtocol("openai")` / 既有 openai preset 惯例，**非 codex_tui**）。
   - **保留** 现有 anthropic（/api/coding，claude_code）+ openai_responses（/api/coding/v3，codex_tui）。
   - endpoint 顺序：核对其他双端点平台（如 glm/qianfan:242-247）惯例，保持一致。
2. **bug1 — 双 base_url 不塌缩**。先复现：粘贴含两条 URL 的文案，断点/日志看 `extractBaseUrls` + `guessProtocol`（`platformPaste.ts:266-294`）输出 + `applyPaste`（`Platforms.tsx:1631-1647`）endpoint 去重结果。确认塌缩点（两条 `unknown→openai` 在 :1638 findIndex 去重覆盖）。修复方向二选一（复现后择优，见 prd 范围）：
   - (a) 增强 `guessProtocol`（:270）`/v\d+` 泛化 + 火山 host `/api/coding`→anthropic host 感知；权衡是否误伤其他平台。
   - (b) matchPlatform 命中多端点 coding preset 时，按 host+path 最长子串映射 pasted base_urls → preset endpoints（复用 hosts 派生，对齐记忆 `volces-dual-endpoint-substring-match`）。
   - 优先不破坏既有平台粘贴行为（glm/qianfan 等双端点回归须过）。
3. **hosts 派生同步**（`src/pages/Platforms.tsx:405-423` 从 getDefaultEndpoints 派生 `p.hosts`）
   - 新增 openai 端点后确认 doubao hosts 仍含 `ark.cn-beijing.volces.com/api/coding/v3`，matchPlatform 最长子串仍命中 doubao（不被其他平台抢）。禁手写 hosts。
4. （核实）`defaultClientForProtocol`（grep 定位）确认 openai 协议默认 client_type 正确。
5. 相关单测：grep `matchPlatform`/`guessProtocol`/`doubao` 在测试文件，若有须同步并通过；火山双端点新增回归用例（粘贴文案 → 两独立 endpoint）。

## 验收命令
- `yarn build`（tsc + vite，前端类型/构建）必须全绿。
- 若有前端测试脚本覆盖 platformPaste，跑通；无则在 implement 报告说明手工复现验证步骤。
- 复现验证：模拟粘贴火山分享文案，确认表单出现 anthropic(/api/coding) + openai(/api/coding/v3) 两独立 endpoint。

## 失败处理
- 方案 (a) 泛化 `/v\d+` 触发其他平台粘贴回归失败 → 退 (b) host 感知映射。
- 火山官方 openai client_type 不确定 → 返回标 `需要: 火山方舟标准 openai 端点的 client_type`，main 转达用户。
- glm/qianfan 等既有双端点平台粘贴回归 → 必须保持原行为，不得为火山牺牲其他平台。
