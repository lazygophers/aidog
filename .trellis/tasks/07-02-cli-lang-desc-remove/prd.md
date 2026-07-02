# PRD — CLI 语言设置区两项微调（删冗余描述 + select 紧凑）

## 现象（用户 2026-07-02）
CLI 集成 tab 语言设置项两问题：
1. 描述块 `~/.claude/settings.json · language`（技术路径泄露 + 视觉冗余）不需要
2. select 仍太长（上轮 cli-lang-select-style 去 minWidth:180 后用户仍嫌长），要更紧凑

## 决策锁（用户 2026-07-02 连续裁定）
| # | 决策 | 锁定 |
|---|---|---|
| 1 | 描述块 | **删除** L286-288 `~/.claude/settings.json · language` 整块（text-tertiary monospace div） |
| 2 | select 尺寸 | **移除长度限制**（去 className="input" 的宽度约束 / 任何 minWidth / width），用**默认中等大小或 small**（紧凑一档） |
| 3 | select 对齐 | 右对齐保持（space-between 不动） |

## 目标
语言设置区视觉收紧：去冗余技术路径描述，select 紧凑不撑宽。

## 交付（单文件 `src/components/settings/CodingToolsSettings.tsx`）
1. **删描述块**：L286-288 三行（`<div className="text-tertiary" ...>~/.claude/settings.json · language</div>` 整块删除）
2. **select 紧凑**：
   - 探索 `className="input"` 的 CSS 定义（`src/themes/` 或全局 CSS），若 .input 定义了 width/min-width/padding → select 改用更紧凑的 class 或内联样式覆盖
   - 用户要"默认中等大小或 small"：考虑①换 size variants（若有 .input-sm/.input-small）②内联 `padding`/`fontSize` 收一档 ③去 className 用原生默认 select 外观（最紧凑）
   - exec subagent 读 .input CSS + 主题 variants 定具体实现，目标：select 视觉宽度明显比当前窄（刚容下选项 + 紧凑 padding），整体 medium/small 档

## 验收
1. `~/.claude/settings.json · language` 描述块消失（label title + desc 两行保留）
2. select 视觉宽度明显收紧（不再占当前宽度），medium/small 紧凑档
3. select 仍贴右（space-between 不变）
4. `yarn build` 绿
5. 不影响其他段（applyPlugin/skipOnboarding/dateRewrite 开关不动）

## 非目标
- 不改语言设置逻辑（handleLanguageChange / LANGUAGE_OPTIONS）
- 不改 i18n key（描述块删除不需删 key，codingTools.language.desc 保留——desc 是另一行，删的是 monospace 路径行）
- 不动 label title fontSize:14（上轮已定）

## 风险
- .input class 若被多处用，改 class 定义会波及 → **只改 select 这一处**（内联覆盖或换 class），禁改 .input 全局定义
- select 过窄导致 option 文本截断 → 验视觉，按内容自适应 + 紧凑 padding 平衡

## 阶段
1. planning（本步，grill 轻校对 — 单文件 ≤10 行改，决策已用户裁定）
2. exec（单 subagent 轻量模式）
3. check（yarn build）
4. finish
