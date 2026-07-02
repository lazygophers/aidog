# PRD — CLI 语言设置 select 样式修正

## 现象（用户 2026-07-02）
CLI 集成 tab（CodingToolsSettings.tsx）语言设置项：
- label（title）字号偏小
- select 宽度过大（minWidth:180 撑宽），不够紧凑

## 决策锁（2026-07-02 AskUserQuestion 用户裁定）
| # | 决策 | 锁定 |
|---|---|---|
| 1 | select 宽度 | **够用即可**（去 minWidth:180 撑宽，按内容自适应），**不要特别大** |
| 2 | select 对齐 | **右对齐**（flex space-between 已天然贴右；select 内选项左对齐不动，容器层右贴） |
| 3 | label 字号 | title 13→14（用户称"太小"，提到与其他卡片标题协调；desc 12/路径 11 不动） |
| 4 | 归属 | 另开独立 task（非并入 body-date；用户明确选） |

## 目标
语言设置项视觉收紧：label 字号提一档可读，select 宽度按内容贴合不撑宽，整体更紧凑。

## 交付
单文件微调 `src/components/settings/CodingToolsSettings.tsx` L194/L202-204 段：
1. L194 label title `fontSize: 13` → `fontSize: 14`
2. L204 select `style={{ fontSize: 13, minWidth: 180 }}` → 去掉 `minWidth: 180`，改按内容自适应（`width: "auto"` 或直接删 minWidth 让 input className 自然撑开），保留 `fontSize: 13`
3. select 容器右对齐：父 flex 已 `justifyContent: "space-between"` + `alignItems: "center"`，select 天然贴右；若 select 改 auto 后有偏移，确认仍贴右（无需额外样式，验视觉）

## 验收
1. label title 字号视觉大于 desc（14 > 12）
2. select 宽度收紧到「刚好容下当前语言选项」，不再 minWidth:180 撑宽
3. select 仍贴右（space-between 布局不变）
4. `yarn build` 绿（tsc）
5. 不影响其他段（applyPlugin/skipOnboarding 开关不动）

## 非目标
- 不改语言设置逻辑（handleLanguageChange / LANGUAGE_OPTIONS 不动）
- 不改其他卡片样式
- 不改 i18n key

## 风险
- select 去 minWidth 后若 option 文本短（如 "en"），select 过窄不美观 → 验视觉，过窄可加 `minWidth: "fit-content"` 兜底
- 与 body-date task 同文件（CodingToolsSettings.tsx）→ **必须等 body-date finish 释放 slot 后 start**（用户选另开非并入，文件冲突串行）

## 阶段
1. planning（本步，grill 轻校对 — 单文件 ≤20 行微调，决策已用户裁定）
2. 等 body-date finish（slot + 文件冲突）
3. exec（单 subagent，轻量模式）
4. check（yarn build）
5. finish
