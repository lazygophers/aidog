# 修 mimo coding plan 误识别为火山引擎

## 复现

用户粘贴社区文案 (含 mimo coding plan token base64 + "lark_024" 等杂文):
```
... 由 lin2101 发布 ... lark_024 ... dHAtY2QwbW[使劲蹬啊]ZlODI5...
```
智能粘贴识别为 **火山引擎 (doubao)**, 预期 **小米 MiMo (coding plan)**。

## 根因

`matchPlatform` keyword fallback (platformPaste.ts:323) 按 `PLATFORM_PRESETS` 顺序首个命中:
- doubao preset (Platforms.tsx:40) keyword 含 `"ark"`
- mimo preset (Platforms.tsx:44) 在 doubao 之后
- `normalizeForMatch("lark_024")` = `"lark 024"`, `.includes("ark")` = **true** (lark 含 ark 子串)
- doubao 先命中 → 误识别火山

`"ark"` 是 doubao coding plan base_url host 段 (ark.cn-beijing.volces.com), 但 host 匹配 (步骤 1) 已覆盖; keyword fallback 里 "ark" 过短, 误伤 "lark"/"mark"/"dark" 等子串。

## 修复

### 主修: doubao keyword 去泛化
删 `"ark"` (host 匹配已覆盖 base_url; keyword 留 "火山"/"volcengine"/"豆包"/"doubao"/"seed"/"volces"/"agentplan" 足够特异)。

### 可选增强: mimo coding plan 识别
修主 bug 后, 纯 token 文案 "MIMO" 命中 xiaomi_mimo 普通 preset (非 coding plan)。用户实际要 coding plan (token-plan-cn.xiaomimimo.com)。
- mimo coding plan token 前缀 `tp-` (token plan) 可作信号
- 但 base64 编码后 text 里无 "tp-", 需在 parsePlatformPaste 提取 apiKey 后, 若 mimo + apiKey 以 `tp-` 开头 → 升级 coding plan 变体

## 验收

1. 用户文案不再误识别为火山 → 命中 mimo (普通或 coding plan 视增强)。
2. 正常火山 base_url (ark.cn-beijing.volces.com) 粘贴仍识别 doubao (host 匹配)。
3. "lark"/"mark"/"dark" 等含 ark 子串文案不误命中 doubao。
4. platformPaste.test.ts 加回归用例; yarn build 过; cargo 无关。

## 待确认

见 AskUserQuestion (是否做 coding plan 增强识别)。
