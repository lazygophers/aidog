# i18n 全覆盖硬化: 修复 settings 等裸 key + 自动检查 + spec 规则

## 背景

用户反复报告"设置子菜单只看到 key 没看到翻译"。根因调查:

**locale 严重不对齐**:
```
zh-CN: 1077 key | en-US: 958 | es-ES: 957 | ar/fr/de/ru/ja: 各 938
```
并集 1079 key,除 zh-CN 外各 locale 缺 121-141。

**两大缺失源**:
1. `env.*` — zh-CN **129**,其他 7 locale 全 **10**(差 119)。env 配置项 label/desc 是动态模板 `t(\`env.${key}\`)` / `t(\`env.${key}.desc\`)` 调用,其他 locale 漏 → **设置子菜单 env 区裸 key 主源**。
2. `logs.*` — zh/en/es 47,ar/fr/de/ru/ja 25(漏 22 × 5 locale)。

另有 16 静态 t() key(stats.*)8 locale 全无。

**根因**: 无自动化 locale 对齐检查。每次加 t() key 只补 zh-CN(+ 偶尔 en),5-7 locale 漏。memory `frontend-i18n-coverage` 已记经验但未进 spec,sub-agent 看不到 → 反复遗漏。

## 目标

1. **零裸 key**: 8 locale key 集合完全对齐(并集),任何 locale 切换不 fallback。
2. **自动检查防线**: `scripts/check-i18n.mjs` 检测 t() 静态 key 覆盖 + locale 间对齐,check 阶段必跑,0 报警才过。
3. **spec 规则**: `frontend/conventions.md` 新增 i18n 章节,sub-agent 强制读。

## 交付物

### 1. 自动检查脚本 `scripts/check-i18n.mjs`(入仓,根治工具)

检查项:
- **A. t() 静态 key 覆盖**: 扫 `src/**/*.{ts,tsx}` 所有 `t("literal")` / `t('literal')` 字面量,每个 key 必须在所有 8 locale 存在。
- **B. locale 间对齐**: 8 locale key 集合必须等于并集(任何 locale 缺并集中的 key → 报警)。
- **C. 动态模板清单**: 输出所有 `t(\`prefix${var}\`)` 模板 + 基准 locale 匹配 key 数,供人工审计(无法全自动展开)。
- 退出码: 有缺失 → 非 0(check 阶段 fail)。
- 输出格式: `key ← missing [locale1,locale2]` 可操作。

### 2. 补齐所有缺失(真实本地化翻译)

补齐策略:
- 翻译用真实多语言本地化(非英文兜底),保证质量。
- env.* 119 key × 7 locale + logs.* 22 × 5 locale + stats.* 16 × 8 locale + 零散对齐。
- 品牌名(AiDog)/协议名(Anthropic/Codex/Claude)/技术术语保留不译(对齐现有约定)。
- 分批写 locale JSON,每批跑脚本验证。

### 3. spec 规则 `frontend/conventions.md` 新增 i18n 章节

```
## i18n (MUST)
- 所有用户可见文案必须 t(),禁硬编码中/英文字面量
- 新增 t() key 必须 8 locale 同步补全(zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES)
- 动态模板 t(`prefix.${var}`) 必须枚举所有变量取值,每个值对应 key 8 locale 全补
- 品牌/协议/技术术语保留不译
- check 前必须跑 `node scripts/check-i18n.mjs`,0 报警才可 finish
- 验证: `node scripts/check-i18n.mjs` exit 0
```

### 4. check.jsonl 注入

check sub-agent 必跑 `node scripts/check-i18n.mjs`。

## 范围

| 项 | 量 | 说明 |
|---|---|---|
| env.* 补齐 | ~119 key × 7 locale | env 配置 label/desc |
| logs.* 补齐 | ~22 key × 5 locale | 日志页文案 |
| stats.* 补齐 | 16 key × 8 locale | 统计页文案(全无) |
| 零散对齐 | ~20 key | locale 间零碎差 |
| 脚本 | 1 文件 | scripts/check-i18n.mjs |
| spec | 1 章节 | frontend/conventions.md |

## 验证

- `node scripts/check-i18n.mjs` → exit 0(零缺失)
- `yarn build`(tsc)→ 0 error
- 抽查: ar-SA / de-DE 等缺最多 locale 的 env 区,翻译值合理非裸 key

## 非目标

- 现有正确翻译的润色/改写
- 动态模板变量取值源的重构(仅审计覆盖,不改调用方式)
- 删除废弃死 key(并集策略,冗余无害,另任务清理)
