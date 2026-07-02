# PRD — 粘贴 base64 分享文本识别 MiMo 裸 key

## 背景
论坛分享帖常把整段配置(base_url + apikey)base64 编码后粘贴。aidog 智能识别(`src/utils/platformPaste.ts`)已支持 base64 解码 + 标签复合串解析,但小米 MiMo(	token-plan-cn.xiaomimimo.com)这类文案解码后 **apikey 裸在末尾、无「令牌/密钥/key」标签前缀**,当前 `parseCompoundLabeled` 按标签锚定切分时把裸 key 归入「接口」段被 URL 正则忽略,致 `apiKeys=[]` —— platform 识别对但 key 丢。

## 复现
粘贴(整段 base64):
```
5YW85a65IE9wZW5BSSDmjqXlj6PljY/orq7vvJoKaHR0cHM6Ly90b2tlbi1wbGFuLWNuLnhpYW9taW1pbW8uY29tL3YxCuWFvOWuuSBBbnRocm9waWMg5o6l5Y+j5Y2P6K6u77yaCmh0dHBzOi8vdG9rZW4tcGxhbi1jbi54aWFvbWltaW1vLmNvbS9hbnRocm9waWMKdHAtY3R6Ymg2ODF1NmRnYzVheHJ6czdycm5mYWpjaDkydzA2cTgweXI2ODA3NXdoNjQ3
```
解码得:`兼容 OpenAI 接口协议：\nhttps://token-plan-cn.xiaomimimo.com/v1\n兼容 Anthropic 接口协议：\nhttps://token-plan-cn.xiaomimimo.com/anthropic\ntp-ctzbh681u6dgc5axrzs7rrnfajch92w06q80yr68075wh647`

当前结果:`platform=MiMo coding` ✓ / `apiKeys=[]` ✗ / `baseUrls=[v1]`

## 决策(根因驱动,无需用户拍板)
| 维度 | 决策 |
|---|---|
| 根因 | `parseCompoundLabeled`(platformPaste.ts:160)按 `COMPOUND_LABEL_RE`(:145)标签锚定切分,裸 key(无令牌/密钥/key 标签)漏提 |
| 修复点 | `extractCompoundFromBase64`(:367)对 `tryBase64DecodeUtf8` 解码后的明文,补跑标准前缀裸 key 扫描(`PREFIX_TOKEN_RE` :65,覆盖 tp-/sk-/ark- 等),填入 `parts.apiKey` |
| 不改 | `parseCompoundLabeled` 标签模型(服务「标签紧贴值」变体,不破坏);不改 preset;不加新依赖 |

## 交付
1. **`extractCompoundFromBase64`**(`platformPaste.ts:367`)—— 解码成功后,对 `decoded` 明文跑 `PREFIX_TOKEN_RE.matchAll`,命中的标准前缀 key(`tp-...`/`sk-...`/`ark-...`,长度 ≥16,stripCjk)填入该 parts 的 `apiKey`(若 `parseCompoundLabeled` 未已填)。
   - 复用现有 `PREFIX_TOKEN_RE` + `stripCjk` + `pushUnique` 语义,禁新正则
   - 守卫:仅当 `parts.apiKey` 为空时补(避免覆盖显式标签提取的 key);补的 key 须 `hasKnownPrefix` 或 `DECODED_KEY_SHAPE`
2. **测试 `platformPaste.test.ts`** —— 加用例:本文案(整段 base64)→ 断言 `apiKeys` 含 `tp-ctzbh...`、`platform` 命中 MiMo coding、`baseUrls` 含 `/v1`。复用现有 MiMo 用例结构。

## 验收
- 粘贴该 base64 → `parsePlatformPaste` 返回 `apiKeys=[tp-ctzbh...]` + `platform=MiMo coding` + `baseUrls` 含 token-plan-cn.xiaomimimo.com/v1
- 现有 platformPaste.test.ts 全绿(无回归,标签锚定 / 防爬噪声 / 多 key / coding 升级等既有用例)
- `yarn build` 绿

## 非目标(YAGNI)
- 双端点(/v1 + /anthropic)同时取 —— 取首个(/v1,coding plan 默认 OpenAI 兼容)即可,用户切协议手改
- 非 base64 的同类裸 key 文案(裸 key 场景由 extractApiKeys 主路径 PREFIX_TOKEN_RE 已覆盖;本 task 仅修整段 base64 解码后的裸 key 漏提)
- base64 整段解码后的 model 提取(裸 model 无标签同样漏,但用户未报,本 task 不扩)

## 调度
独立 task,write-files = `src/utils/platformPaste.ts` + `src/utils/platformPaste.test.ts`,与 active task(marker 改 sync_settings/editors/api、deeplink 改 Platforms/ShareModal)文件不相交 → active 腾槽后可 start。

## 风险
- `PREFIX_TOKEN_RE` 带 `u` flag + 命中范围(含 CJK 穿插场景),补跑时须 `stripCjk` 清洗再长度守卫
- decoded 明文若含 URL 片段误命中前缀正则(如 `tp-` 出现在 URL path)—— `tp-` 前缀 + `[A-Za-z0-9_\-]{N}` 长串在 URL 里概率极低,且 `DECODED_KEY_SHAPE` 守卫兜底;implement 时 grep 确认无 URL 误报
