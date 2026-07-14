# platform-detect-minimax — 调研收敛

## 根因(单条, file:line 实证)
`src/utils/platformPaste.ts:349-360` matchPlatform keyword fallback「presets 列表顺序首个命中即 return」。anthropic 在 `platform-presets.json:5`(idx0), keywords `["claude","克劳德","官方"]`(`:18`)含跨平台通用词。后续平台(minimax idx9 `:157` 等)分享帖含「兼容 Claude Code」/「官方 API」→ `hay.includes("claude")` 抢先命中 → return anthropic, minimax keyword 不检查。

presets 列表由 `src/domains/platforms/defaults.ts:275-301 buildProtocolsFromPresets` 按 `Object.keys(doc.protocols)` 顺序派生, 顺序 = JSON 文件顺序。

## 复现实证(脱敏, 脚本已删)
- 场景1 "Minimax Token Plan 分享 ... 兼容 Claude Code" → anthropic (hit 'claude') ❌
- 场景2 "minimax 海螺 PRO 套餐 ... 支持 Claude Code" → anthropic (hit 'claude') ❌
- 场景3 "Minimax 官方 API 分享" → anthropic (hit '官方') ❌
- 场景4 "Minimax 分享 https://api.minimaxi.com/v1" → minimax ✅ (走 host 优先级1)

结论: 无 base_url + 文案含通用兼容词 → 必走 keyword fallback → idx0 anthropic 抢匹配。

## 证伪假设
- **sk-cp- 硬编码映射**: grep 全仓 `sk-cp` src/+presets 零出现(仅 `tp-` xiaomi_mimo 声明)。证伪。
- **机制 B 升级救场**: `platformPaste.ts:547-556` 要求初始 `platform.value === cpPreset.value`, 初始误判 anthropic ≠ minimax, 升级不触发。证伪。

## 同族影响(idx0/idx1 通用词抢匹配)
- anthropic `claude`/`官方`: minimax / minimax_en / kimi / doubao / glm / bailian / qianfan / xiaomi_mimo / 阿里百炼通义
- openai `gpt`/`chatgpt`/`官方`: 后续凡含 "gpt"/"官方" 的平台

## 现有测试盲区
`platformPaste.test.ts:12-57` fixture **缺 anthropic preset**, 故 anthropic 抢匹配回归从未被覆盖。fixture 须补 anthropic 真实 keywords 才能复现+锁回归。

## 关键证据文件
- `src/utils/platformPaste.ts:321-360`(matchPlatform) / `:522-565`(parsePlatformPaste)
- `src/domains/platforms/defaults.ts:275-301`(buildProtocolsFromPresets)
- `src-tauri/defaults/platform-presets.json:5,18,157`(anthropic idx0 + keywords + minimax)
- `src/utils/platformPaste.test.ts:12-57`(fixture 缺 anthropic)

## recall 命中
- arch/trellis-05: keywords 字段须与 JSON 真值对齐, 否则误匹配 base_url 子串(本 bug 是 keyword 通用词抢匹配, 同源)
- frontend/trellis-18: PROTOCOLS[].label 硬编码 fallback, platformPaste/ccswitchMatch 用
- CLAUDE.md: platform-presets.json 真值源 + getDefaultEndpoints async(已覆盖)
