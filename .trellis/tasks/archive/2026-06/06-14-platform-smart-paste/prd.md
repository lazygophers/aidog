# 平台添加智能识别（剪贴板粘贴解析）

## Goal

添加平台页加「智能识别」按钮 → 弹窗自动读剪贴板内容到多行框 → 解析出 base_url / 平台 / apikey 并在下方展示 → 用户确认后填入表单。降低从论坛杂乱文案（含图片计数/楼层/日期噪声、key 中混入防爬汉字、base64 编码）手动抠字段的成本。

## What I already know

- 添加表单态：`Platforms.tsx` `name/protocol/baseUrl/apiKey` + setters + `resetForm`；表单头 button group（line ~1936）。`protocol` 实为 `platform_type`（preset value）。
- preset 列表 `PLATFORM_PRESETS`（`{value,label,keywords}`，line 40-88），`DEFAULT_ENDPOINTS` 映射 preset→{protocol,base_url}（line 128+）。关键字匹配用「空格分词 substring」惯例。
- 无 Tauri clipboard 插件 → `navigator.clipboard.readText()` best-effort + try/catch，失败回退手动粘贴（textarea 始终可编辑）。
- 无前端 test runner（仅 tsc）；解析逻辑做纯函数便于推理/手测。

## 样例覆盖（必须全过）

1. 小米 MIMO：`tp-czhy...` key、`Compatible with Anthropic API protocol:https://token-plan-cn.xiaomimimo.com/anthropic`、双 base_url（/anthropic + /v1）。
2. 小米 MIMO 2：`兼容 openai 接口：URL/v1` + `兼容 Anthropic 接口：URL/anthropi`（末尾截断少 c，需容错）。
3. 防爬汉字：`tp-c771gx1phd68ca436oc防止脚本汉字直接删除6q6s1s3mkrwunyo7mbisdvpm0drzo` → 剔除中间 CJK 得纯 key。
4. kimicode：`url："https://api.kimi.com/coding/";` + `API_KEY=sk-kimi-...` 两个 key。
5. base64 编码的 apikey → 自动解码（前缀不匹配但 base64 解出合法 key）。

## Requirements

- **入口**：添加平台表单（`!editing`）头部加「智能识别」按钮 → 打开弹窗。
- **弹窗**：多行 textarea，打开时 best-effort 读剪贴板填入；内容变更实时解析；下方展示解析结果。
- **解析字段（仅 3 类）**：
  - **apikey**：识别 `sk-/sk-ant-/sk-kimi-/tp-` 等前缀 token；剔除混入 CJK 字符；base64 启发式解码（无前缀但 base64 解出合法 key 串则用解码值）；`API_KEY=/秘药：/key:` 等赋值模式兜底。
  - **base_url**：http(s) URL 提取，剥尾部标点/引号；按邻近文案/路径标协议（/anthropic→anthropic、/v1|openai→openai）。
  - **平台**：对全文做空格归一化后逐 preset keyword substring 匹配（含域名如 xiaomimimo→mimo）。
- **多值处理（决策）**：多个 apikey → 全部列出**单选**；多个 base_url → 全部列出**单选**；无 preset 匹配 → **不改平台选择**。
- **应用**：选定后填入表单——匹配到 preset 则 `setProtocol` + 可选 `setName(label)`；`setBaseUrl(选中)`；`setApiKey(选中)`。无 preset 时只填 base_url/apikey。

## Acceptance Criteria

- [ ] 5 个样例粘贴后 apikey/base_url/平台 至少各识别出正确候选（样例3 key 剔汉字、样例4 出 2 key 单选、样例5 base64 解码）。
- [ ] 弹窗打开 best-effort 读剪贴板；拒绝/失败不报错，textarea 可手动粘贴。
- [ ] 应用后表单字段正确写入；无 preset 时平台选择不变。
- [ ] `yarn build`（tsc + vite）green；无新 warning。
- [ ] i18n：新增 UI 文案走 t() + 8 locale key（沿用项目 check-i18n 防线）。

## Out of Scope

- models 列表识别（用户明确只要 base_url/平台/apikey）。
- 余额/配额、balance_base_url 等其它字段。
- 编辑既有平台时的识别入口（仅新增场景）。

## Technical Approach

新增纯函数 `src/utils/platformPaste.ts`：`parsePlatformPaste(text, presets) → {apiKeys[], baseUrls[{url,protocol}], platform|null}`。弹窗组件内联或 `src/components/platforms/`；Platforms.tsx 注入 preset 列表 + 选中回调填表单。base64：candidate 不匹配已知前缀但 `^[A-Za-z0-9+/]{20,}={0,2}$` 且 atob 解出含已知前缀 → 用解码值。

## Decision (ADR-lite)

- 多 key / 多 base_url：全列出单选（避免自动选错，样例存在多值）。
- 无 preset：不改平台选择（用户手动选，避免误判中转/自定义）。
- base64：启发式自动（无需用户开关）。
