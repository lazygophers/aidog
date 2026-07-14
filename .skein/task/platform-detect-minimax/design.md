# platform-detect-minimax — 详细设计

## 方案选型

### A(推荐, 根治): keyword fallback 改打分
第一遍扫描全部 presets, 统计每个 preset 命中 keyword 数 + 最长命中 keyword 长度; 取「命中数 > 最长关键字 > 列表顺序」三者排序最高者。

- 场景1: minimax 命中 `minimax`+`minimax code` ≥2, anthropic 命中 `claude` 1 → minimax 胜
- Pro: 数据驱动, 零硬编码, 复用现有 keywords; 同族(doubao/glm/kimi)一并根治
- Con: 改 matchPlatform 公共语义, 需补测试

### B(最小改): 通用词弱信号 whitelist
维护弱关键字集合(`claude`/`官方`/`gpt`), 命中后继续扫, 有更强命中让位。
- Pro: 改动小
- Con: 弱词清单手维护, 新协议带通用词易漏配; A 的特例

### C(补强, 不单独够): key 前缀前置判定
parsePlatformPaste 在 matchPlatform 前先扫 apiKeys, 命中任一 preset codingKeyPrefixes 直接返该 preset, 跳 keyword fallback。+ minimax preset 补 `codingKeyPrefixes: ["sk-cp-"]`。
- Pro: 纯 token 粘贴(无文案无 URL)识别率大升; 同 xiaomi_mimo tp- 模式扩展
- Con: 只修「带 key 的粘贴」, 纯文案仍误判; 须与 A 组合

**推荐 A + C**: A 根治文案竞争, C 让 key 前缀成最高信号(带 key 场景兜底)。

## 修复设计

### s1: matchPlatform keyword fallback 改打分(A)
- `src/utils/platformPaste.ts:349-360` keyword fallback 分支:
  - 第一遍: 遍历全部 presets, 对每个 preset 统计命中 keyword 集合 `{preset, hitCount, longestLen}`
  - 过滤 hitCount=0
  - 排序: hitCount desc → longestLen desc → 列表顺序(presets 索引)asc
  - 取首位; 空则 null
- 复杂度 O(presets × keywords), 不变
- codingPlan 透传: 取胜出 preset 的 codingPlan 字段(同现逻辑)

### s2: 补回归测试 fixture
- `src/utils/platformPaste.test.ts:12-57` fixture 补 anthropic preset(value=anthropic, keywords=`["claude","克劳德","官方"]`)
- 新增用例:
  - minimax 帖子(含 "claude code" + "minimax" keyword)→ minimax(非 anthropic)
  - "官方 API" 多平台竞争场景
  - anthropic 真实帖(无更强信号)→ anthropic(不回归)
  - kimi/doubao 同族抽样

### s3(可选 C): key 前缀前置判定 + minimax codingKeyPrefixes
- `src/utils/platformPaste.ts:522 parsePlatformPaste`: matchPlatform 调用前, 若 apiKeys 非空, 先扫 codingKeyPrefixes 命中(遍历 presets × codingKeyPrefixes × apiKeys), 命中则直接返该 preset(codingPlan: true), 跳过 matchPlatform
- `src-tauri/defaults/platform-presets.json` minimax entry 补 `"codingKeyPrefixes": ["sk-cp-"]`
- 与现有 hosts 子串匹配同优先级层(信号强度: key 前缀 ≈ host, 均 > keyword)

## 取舍
- **打分 vs whitelist**: 打分数据驱动零维护, whitelist 手维护易漏 → 选打分(A)
- **C 是否做**: sk-cp- 是 minimax coding plan key 实际前缀(用户实证), 前置判定提升纯 token 场景; 但依赖 presets.json 手补字段 → 做(用户场景命中)
- **同族全验证 vs 抽样**: 全 75 协议逐一验证成本高, 抽样(minimax/kimi/doubao/glm)+ 打分逻辑普适 → 抽样

## 回归风险点
- 打分改语义: 现有依赖「顺序首命中」的用例可能行为变(但 fixture 缺 anthropic, 实际顺序首命中从未被多 preset 竞争测试, 风险低)
- anthropic 真实帖回归: 打分后若 anthropic 帖含 `claude` 1 命中, 其他平台 0 命中 → anthropic 胜(不回归)
- codingKeyPrefixes 新增: minimax 普通版 vs coding 版分裂? 当前 minimax 无 coding 分支, sk-cp- 直接归 minimax

## 数据流(修复后)
```
粘贴 → extractApiKeys + extractBaseUrls
     → base_url 命中? → host 最长串胜出(优先级1, 不变)
     → apiKeys 命中 codingKeyPrefixes? → 该 preset(优先级2, s3 新增)
     → keyword 打分(命中数>最长>顺序) → 胜出(优先级3, s1 改)
     → null
```
