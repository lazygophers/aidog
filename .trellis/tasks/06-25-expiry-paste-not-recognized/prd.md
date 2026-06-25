# 粘贴识别「M.D 到期」格式失败

## 需求 (用户)

「没有识别到过期时间」+ 测试样本 (社区分享帖):
```
分享一个MIMO key，PRO套餐，6.27到期
...
base url
兼容 OpenAI 接口协议：https://token-plan-cn.xiaomimimo.com/v1
兼容 Anthropic 接口协议：https://token-plan-cn.xiaomimimo.com/anthropic

key
tp-caxzn1uh0ck6f46btdxr35o6hyb9a5aa25uk93yh4u1n628k
```

预期: 识别「6.27到期」→ 过期时间 2026-06-27 23:59:59 (今天 2026-06-25, 当年未过, dateOnly → end-of-day, 按 [[platform-expiry-toggle]] + [[extractexpiryat-false-positive-fallback]] 约定)。

## 根因 (已定位)

`src/utils/platformPaste.ts:369-370` `DATETIME_RE`:
```js
/(?:(\d{4})[-\/](\d{1,2})[-\/](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2}))?)|(?:(\d{1,2})[-\/月](\d{1,2})(?:[日号 T](\d{1,2}):(\d{1,2}))?)/gu
```

MM-DD 分支分隔符字符类 `[-\/月]` **缺 `.`** → `6.27` (月.日格式) 不匹配 → 无候选 → return null。

其他链路均正常:
- 语义词 `EXPIRY_KEYWORDS` (line 373) 已含「到期」✓
- 60 字符距离门槛 (line 471) — 「6.27」紧贴「到期」距离 0 ✓
- dateOnly → end-of-day (line 416-420) ✓
- 当年补全 + 已过推次年 (line 406-410) ✓

## 方案

DATETIME_RE MM-DD 分支分隔符字符类加 `.`:
```js
// 原: [-\/月]
// 改: [-\/月.]
```

字符类内 `.` 字面量无需转义。

`6.27` → m[6]=6 m[7]=27 → mo=6 d=27 → 当年 2026-06-27, dateOnly → 23:59:59.999 ✓

## 风险与防护

`.` 分隔易与版本号 (如 `Claude 4.5`) / 比例 / IP 混淆。现有防护已足够:
1. **语义词硬门** (line 433-440): 文案无过期语义词直接 return null → 大部分版本号语境被挡
2. **60 字符距离门槛** (line 471): 候选须紧邻语义词
3. **mo 1-12 / d 1-31 校验** (line 412-413): parseCandidate 内
4. **URL 已被 URL_RE 吞走** (line 367 注释) — 域名 N.N 不入此正则

残留风险: 「Claude 4.5 到期」类文案 → 4.5 误识别为 4月5日。但:
- 社区分享帖罕见此格式
- 用户场景优先识别「6.27到期」
- 误识别为非破坏性 (toggle 默认 OFF, 用户可改/关)

可接受。若需更紧, 可加约束: `.` 分隔候选要求紧邻语义词 (距离 ≤ 10 字符), 但架构改动大, 非 MVP。

## 验收

1. 本样本识别 → 2026-06-27 23:59:59.999
2. vitest 加 case 覆盖:
   - `6.27到期` → 当年 6-27 23:59:59
   - `6.27` 无语义词 → null (收紧防护不破)
   - `12.31到期` → 当年 12-31 23:59:59 (跨年边界若已过推次年)
   - `过期 6.27` (语义词在前) → 同 #1
3. 现有 case 全绿 (不回归)
4. `yarn build` + `cargo test` + `cargo clippy -- -D warnings` + `check-i18n.mjs` 全绿, 无新 warning
5. 手动验: 贴样本进添加平台智能粘贴 → 表单 expiresAt 填 2026-06-27 23:59:59 (toggle OFF)

## 不改

- 业务逻辑 (extractExpiryAt 收紧架构不变)
- 其他分隔符分支 (`-` / `/` / `月`)
- 后端
