# skills_list_installed 缓存命中后 enrich source 向后兼容

## 根因

source-grouping task 后，`SkillInfo.source` 字段新增，但 `~/.aidog/skills-cache.json` 旧缓存 items **无 source 字段**（task 前写入）。`list_cached` 命中缓存直接返旧 items → `skills_list_installed` command 返无 source 数据 → 前端全归「其他」组。

锁文件正常（`~/.agents/.skill-lock.json` 52 skills 有 source）。`list_refresh`（npx + enrich）跑后会写新缓存，但 SWR 开页先渲染缓存，refresh 未完成时显示旧数据。

## 修复

`gateway/skills.rs::list_cached`：命中缓存返回前，对 items 跑 `enrich_with_sources(&mut items, scope)`（读锁文件补 source，0 npx，cheap）。

- 旧缓存 None + 锁文件有 source → 补上 ✓
- 新缓存已有 source + 锁文件有 → 重赋同值（幂等）✓
- 第三方 symlink None + 锁文件无条目 → 保持 None（归「其他」）✓

## Acceptance

- [ ] list_cached 命中缓存 enrich 后返回
- [ ] `cargo clippy --all-targets` 0 项目 warning；`cargo test` 全过
- [ ] 实测：重启 dev 后开 skills 页（缓存命中）skills 按 source 分组，非全「其他」

## Out of Scope

- 不改 list_refresh / write_cache（fresh 路径已 enrich）
- 不清旧缓存（enrich 兼容即可）
