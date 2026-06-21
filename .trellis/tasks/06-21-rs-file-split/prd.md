# PRD: Rust 文件拆分（所有 .rs ≤500 行，目标 ≤300）

## 目标
把 src-tauri/src 下 18 个超 500 行的 .rs 拆成内聚子模块，每文件 ≤500 行（硬限），尽量 ≤300（允许少数天生长的 match/连续结构体停在 300-500）。**行为保持**：零功能变更，468 测试全程不回归。

## 拆分方法论（统一）
- `foo.rs` → `foo/mod.rs` + `foo/<concern>.rs`。**父模块声明 `mod foo;` 不变**（变目录模块）→ 各大文件拆分**互不碰父 mod.rs / lib.rs 声明**，文件级隔离。
- `mod.rs` 只留：子模块声明 + 必要 re-export（`pub use` 保持对外路径 `gateway::db::X` 不变）+ 跨子模块共享的小私有 helper/常量。
- 按领域/职责切（如 db → platform/group/proxy_log/stats/settings/migration/cache...）。
- **命名硬规**：① 禁 `part` 风格（不要 `x_part1.rs`/`split_a.rs`），用语义化领域名，再大按子领域细分（stats→stats_today/stats_query/stats_agg）。② **测试文件 = `test_<源文件名>.rs`，1:1 对应**（源 `stats.rs` 的测试只能在 `test_stats.rs`；同目录；源文件内 `#[cfg(test)] mod test_stats;`）。禁聚合多源到一个测试文件、禁 `tests_*.rs`/`_test.rs` 后缀。
- 可见性：跨子模块用 `pub(crate)` / `pub(super)`，不放大对外可见性。
- 每文件拆完 gate：`cargo build` + `cargo clippy -- -D warnings`（block 第三方除外）+ `cargo test` 全绿 + 前端不受影响（如动 api 契约才 yarn build）。

## 分期（parent + child，逐个 plan→exec→check→finish→合）
- **Phase 1 · 4 巨石**（>2000 行，收益最大）：db.rs(7884) / proxy.rs(5430) / lib.rs(4617) / models.rs(2672)
- **Phase 2 · 中量**（1000-2300）：skills.rs(2287) / middleware.rs(1345) / mcp.rs(1161) / notification.rs(1040) / import_export/apply.rs(1009)
- **Phase 3 · 小量**（500-900）：estimate.rs(857) / router.rs(821) / hooks.rs(742) / quota.rs(704) / ccswitch.rs(683) / openai.rs(661) / mock.rs(646) / backup.rs(577) / converter.rs(513)

## 执行约束
- **优先 worktree 并行**（不在 master 直接改）：不同大文件拆分文件级互斥（各建自己目录模块，父 mod 声明不变），每个 exec agent 在**独立手动 git worktree**（`git worktree add .worktrees/<name> -b split/<name> HEAD`）里作业 + 在 worktree 内过 gate + 提交到自己分支；main 串行 merge 各分支回 master（互斥文件 → 干净合并）。Agent isolation:worktree hook 损坏，故用手动 worktree。
- worktree 内命令必须用绝对路径 cd（worktree-cwd 相对路径默认落主仓陷阱）。
- 每文件一个 child task / 一个 exec subagent。lib.rs 特殊（crate 根，不能变目录模块）→ 内容下沉到新建子模块（如 src/commands/*.rs），lib.rs 留薄 glue + invoke_handler。

## 验收（每 child）
- 目标文件及其拆出的所有新文件 ≤500 行（尽量 ≤300）。
- cargo build / clippy -D warnings / cargo test 全绿，零行为变更。
- 对外 API 路径（use 路径）不变（靠 mod.rs re-export）。

## 不做
- 不改业务逻辑、不改 SQL、不改公开 API 签名、不顺手"优化"。纯结构搬移。
