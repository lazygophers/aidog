# platform-detect-minimax — minimax 帖子误识别为 claude

## 目标
- [x] 复现 + 定位根因(已完成, findings.md)
- [ ] 修复 matchPlatform keyword fallback 抢匹配, minimax(及同族)正确识别
- [ ] 补回归测试覆盖(anthropic 真实 keywords + 同族竞争场景)

## 根因(实证)
`src/utils/platformPaste.ts:349-360` keyword fallback「presets 列表顺序首个命中即 return」。anthropic(idx0, `platform-presets.json:5`) keywords `["claude","克劳德","官方"]`(`:18`)含跨平台通用词。minimax 等平台 token plan 分享帖常含「兼容 Claude Code」/「官方 API」→ `hay.includes("claude")` 先命中返 anthropic, minimax 自身 keyword 不检查。

同族影响: minimax / minimax_en / kimi / doubao / glm / bailian / qianfan / xiaomi_mimo 等凡文案含 "claude code"/"官方" 的全被 idx0 anthropic 抢匹配。openai(idx1, `gpt`/`chatgpt`/`官方`) 同型。

证据: 复现脚本 3 场景全返 anthropic(场景1 hit 'claude' / 场景3 hit '官方'); 现有 fixture `platformPaste.test.ts:12-57` 缺 anthropic preset, 回归从未覆盖。sk-cp- 前缀假设证伪(全仓零出现, 非硬编码映射)。

## 用户价值
- minimax/kimi/doubao 等平台分享帖正确识别, 不误配 claude
- 智能粘贴准确率提升(同族根治)

## 边界
- [x] 改 `matchPlatform` keyword fallback 打分逻辑
- [x] 补测试 fixture(含 anthropic 真实 keywords + 同族竞争)
- [x] 可选: minimax preset 补 `codingKeyPrefixes: ["sk-cp-"]` + parsePlatformPaste 前置 key 前缀判定
- [x] 不改 platform-presets.json 已有 keywords(向后兼容); 可新增字段
- [x] 不改 base_url URL 子串匹配(优先级 1, 信号最强, 无 bug)

## 非目标
- 不重构 matchPlatform 整体架构
- 不改 base_url 匹配路径
- 不逐一验证全 75 协议(抽样同族回归)

## 验收标准
- [ ] `yarn test`(platformPaste.test.ts) 通过, 新增 fixture 覆盖 anthropic 抢匹配场景
- [ ] minimax 帖子(含 "claude code"/"官方" + minimax keyword)识别为 minimax
- [ ] 同族(kimi/doubao/glm)典型帖不再误判 anthropic(抽样验证)
- [ ] anthropic 真实分享帖(无更强平台信号)仍识别 anthropic(不回归)
- [ ] `yarn build` 通过

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 调度: task.json(`skein.py subtask list platform-detect-minimax`)
