# PRD: 修复 make lint 错误

## Bug
`make lint` (= `cargo clippy -- -D warnings`) 失败：
```
error[E0609]: no field `source` on type `gateway::skills::SkillInfo`
  src/gateway/import_export/skills_sync.rs:45
```

## 根因
import-export task worktree 陈旧基线（[[worktree-stale-base-merge-conflict]] 同类）：
- fix-skills-enable(3525117) 移除 SkillInfo.source → 改用 installed_path 作 npx add package
- import-export(6b8d233) 在 fix-skills-enable 合并前分支，skills_sync.rs 仍引用 info.source
- 两者合并后 master 上 skills_sync.rs 编译错

## 修复
skills_sync.rs：
- `info.source?` → `info.installed_path?`
- SkillExportEntry.source 字段语义改为「npx add package（本地 installed_path）」
- import 时 `npx skills add <installed_path>`（同机备份/恢复场景有效；跨机因 path 本地化受限，注释说明）

## 验收
- `make lint` exit 0
- `cargo test --lib gateway::import_export` 全过
