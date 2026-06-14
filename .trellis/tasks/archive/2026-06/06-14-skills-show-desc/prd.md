# PRD: Skills 展示每条 desc 字段

## 背景
前端 `Skills.tsx:301` 已条件渲染 `skill.description`, 但实测页面无 desc 显示。

根因: `npx skills list --json` 输出 `[{name, path, scope, agents}]` **无 description 字段** (见 [[npx-skills-cli]] 记忆)。后端 `skills.rs:190` `item.get("description")` 恒得 None → 前端 `{skill.description && ...}` 不渲染。

desc 实际在 skill 规范存储 `<path>/SKILL.md` 的 YAML frontmatter (`description: ...`), path 已由 list 输出。

## 目标 (单交付, main worktree 内直接写)
后端 `parse_list_json` 为每条 skill 从 `<path>/SKILL.md` frontmatter 补全 description。

## 方案
- `skills.rs` 新增 `parse_skill_description_from_frontmatter(content: &str) -> Option<String>` (纯解析, 单测验)
- `skills.rs` 新增 `read_skill_description(skill_path: &str) -> Option<String>` (读 `<path>/SKILL.md` 调纯解析)
- `parse_list_json` description: item 无 description 字段时 fallback 调 `read_skill_description(path)`
- 前端无改动 (已渲染)

## frontmatter 解析规则
- 首行 `---` 起, 到下一个 `---` 止
- 单行 `description: <value>`, 去首尾引号 (单/双)
- 多行折叠 (`>-`) 不支持 (SKILL.md desc 实测均单行, YAGNI)

## 验证
- `cargo test --lib gateway::skills` 绿 (含新单测: frontmatter 解析 / 无 frontmatter / 无 desc / 带引号)
- `cargo clippy` 无 warning
- `yarn build` exit 0
- 实跑: list_installed 返回 description 非空 (brandkit 等已知有 desc)

## 不做
- 不加 serde_yaml dep (手写行扫描, frontmatter 简单)
- 不支持多行 description
- 不改前端
