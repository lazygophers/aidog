# fs_fallback_remove 大小写不敏感匹配 skill 名

## 根因

`npx skills list --json` 返 name 小写化（如 `skill-anything`），磁盘目录保留原大小写（`~/.claude/skills/SkillAnything`，cc-switch 管）。fs_fallback_remove 用 name 精确 `Path::join(name)` → 大小写不匹配 → "removed 0 path(s)" success=false。

## 修复

`fs_fallback_remove` 改列目录 + case-insensitive 匹配 name（规范存储 / 各 agent skills 目录 / project）。is_safe_skill_name 保留防遍历。

## Acceptance

- [ ] case-insensitive 扫描。
- [ ] cargo clippy 0 项目 warning；cargo test 全过。
- [ ] 实测卸载 skill-anything 成功。
