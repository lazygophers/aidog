# PRD — 火山方舟 agent plan 智能识别缺失

> 用户报 (/trellisx-flow + 粘贴文案): 火山方舟 agent plan 分享文案智能识别失败, 未识别为火山平台。文案含:
> - `https://ark.cn-beijing.volces.com/api/plan` (Anthropic 兼容)
> - `https://ark.cn-beijing.volces.com/api/plan/v3` (OpenAI 兼容)
> - apikey: `ark-9a②96aed-4c0e-474c-9c09-49⑤8⑨1a00fef-7c3c6` (圈数字 ②⑤⑨ 防爬, "换成 1 以此类推")

## 根因 (main 调研, 3 层)

### A 主因: ark- 前缀不识别

`src/utils/platformPaste.ts:53` `KEY_PREFIXES = ["sk-ant-", "sk-kimi-", "sk-or-", "sk-proj-", "sk-", "sk_", "tp-"]` — **不含 `ark-`**。火山引擎 apikey 前缀 `ark-` (如 ark-9a96...) 不被前缀锚定 token 抽取 → 弹窗 apikey 字段空。

正则 (line 63): `(sk-ant-|...|tp-)[...]{12,}` 同步加 `ark-`。

### B: 圈数字防爬不剔除

`src/utils/platformPaste.ts:58` `CJK_RE = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯]/gu` — 全角区段 `　-〿`(U+3000-303F) + `＀-￯`(U+FF00-FFEF)。**圈数字 ①②③ (U+2460-247F) 不在此区段** → stripCjk 不剔除 → key 含 ②⑤⑨ 无效。

修: CJK_RE 加 `①-⓿` (Enclosed Alphanumerics, 含 ①-⑳ 圈数字 + 圈字母)。

### C: agent plan 端点 preset 缺失

`src/pages/Platforms.tsx:235-245` doubao preset endpoints 全是 `/api/coding` (coding plan), **缺 `/api/plan` (agent plan)**:
```
{ protocol: "anthropic", base_url: "https://ark.cn-beijing.volces.com/api/coding", ... }
{ protocol: "openai", base_url: "https://ark.cn-beijing.volces.com/api/coding/v3", ... }
```
matchPlatform host 子串匹配 (line 335-337 `urls.some(u => u.includes(hl))`): 用户 URL `/api/plan` 不命中 preset host `/api/coding`。keyword "火山" 兜底命中 doubao (文案有"火山方舟"), 但 **base_url 提取的 `/api/plan` 无对应 endpoint → 拿到默认 `/api/coding` 端点 (错误)**。

agent plan 是火山新套餐, 端点路径 `/api/plan` (区别 coding plan 的 `/api/coding`)。需:
- preset endpoints 加 `/api/plan` (anthropic) + `/api/plan/v3` (openai) 变体
- hosts 派生含 `/api/plan`, matchPlatform 双端点最长子串胜出 (复用 [[volces-dual-endpoint-substring-match]] 机制)
- 评估: agent plan 与 coding plan 是否同 preset (doubao) 区分, 或 codingKeyPrefixes/新字段区分 (agent plan key 前缀也是 `ark-`, 与 coding 同形 → 靠 base_url path 区分, 同 preset 多端点)

## scope

1. **platformPaste.ts**:
   - KEY_PREFIXES 加 `ark-` (前缀锚定 + 正则)
   - CJK_RE 加 `①-⓿` (圈数字防爬)
2. **Platforms.tsx doubao preset**:
   - endpoints 加 `/api/plan` (anthropic) + `/api/plan/v3` (openai/openai_responses) 变体
   - hosts 派生覆盖 `/api/plan`, 双端点最长子串匹配
3. **测试** (platformPaste.test.ts):
   - ark- 前缀 key 抽取
   - 圈数字 ②⑤⑨ stripCjk 剔除
   - agent plan 文案 (`/api/plan` + `/api/plan/v3` + ark- key + 圈数字) 全流程识别为 doubao + 正确端点
   - 既有 coding plan (`/api/coding`) 测试不回归

## 验收

1. 粘入用户报文案 → 弹窗识别为火山引擎 (doubao), apikey 抽出 `ark-9a96...` (圈数字剔除), base_url 提取 `/api/plan` + `/api/plan/v3`, 端点正确
2. 既有 coding plan 文案识别不回归 (端点仍 `/api/coding`)
3. `yarn build` + `check-i18n` 全绿
4. platformPaste.test.ts 新测试全绿 (含圈数字 + ark- + agent plan)

## 非目标

- 不改 matchPlatform 核心算法 (仅补 preset endpoints + 前缀/防爬)
- 不加新 preset (agent plan 归 doubao 同 preset, 靠 path 区分端点)
- 不处理 "5小时/周额度" 等文案噪声 (与识别无关)

## 风险

- agent plan 与 coding plan key 同前缀 `ark-`, 靠 base_url path 区分; 若用户只粘 key 无 base_url → 无法区分 coding vs agent (接受, 默认 coding, 用户手选)
- 圈数字 U+2460-247F 仅含 ①-⑳, 超过 20 的圈数字 (U+2468+) 在 ①-⓿ 内也覆盖 (Enclosed Alphanumerics 全区段)

## 调度

- bug fix, 槽位 2/2 (deeplink-share parent D1 在跑, 文件集不相交 platformPaste/Platforms vs Cargo/lib.rs/App.tsx, 可并行)
- 单 bug-hunt agent, 定位已明 (main 调研), 直接实现 + 测试
