# 仓库清理: 空文件夹 / 空文件 / 临时 / 备份

## Goal
扫描并移除仓库中无意义的空目录 / 空文件 / 临时文件 / 备份文件, 保持仓库整洁。**仅删确实无价值项**, 测试 fixture / build cache / agent placeholder 保留。

## 入参
- `--no-worktree`: 直接改主工作区, 不开 worktree (用户显式)

## 改动范围 (已扫描, 待审批)

### A. 删除: 神秘空目录 (无 git 历史, 推测误建)
- `src-tauri/src-tauri/` — 双嵌套空壳 (`migrations/` + `src/` 均空), 推测脚本/手误产生
- `src-tauri/src/context/` `src-tauri/src/locales/` `src-tauri/src/styles/` `src-tauri/src/themes/` — Rust 源码树下 4 个空目录, 无 git 历史, 与前端 `src/` 同名但属错位, Rust 模块树 (`lib.rs` 等) 不引用
- `.trellis/tasks/archive/2026-07/07-03-mitm-whitelist-clash-rules/research/` — 归档 task 空残留

### B. 删除: macOS junk (untracked)
- `.DS_Store` (根)
- `src-tauri/.DS_Store`
- `src-tauri/icons/.DS_Store`
- `src/assets/.DS_Store`
- `.trellis/tasks/.DS_Store`
- 同步 `.gitignore` 增 `.DS_Store` 全局规则 (若已配则跳过)

### C. 删除: 备份文件 (已 staged delete, 收尾确认)
- `.trellis/task.md.bak` — working tree 已删, git status 标 deleted, 本次一并 commit 清掉

### D. 保留 (不动, 重要边界)
- **`scripts/statusline-golden/golden/*.golden` 空文件** — 测试 fixture (空输入用例), golden regression 必需
- **`.agents/skills/` `.codex/skills/` `.claude/worktrees/`** — agent 工具链 placeholder 目录 (含 `skills/` 子目录), 工具同步用, 非空壳
- **`src-tauri/target/**`** — Rust build cache, gitignored, 内部空文件 (`.lock` / `.rmeta` / `incremental/**`) 是 cargo 增量构建产物, `cargo clean` 用户自决, 本 task 不动
- **`node_modules/` `docs/node_modules/` `scripts/pricing/.venv/`** — 依赖目录, gitignored, 内部空文件是包管理器产物, 不动
- **`.trellis/spec/backend/index.md` 等已 modified 文件** — 其它 task 改动, 不属本 cleanup 范围

### E. 扫描结论 (无命中)
- `.bak` / `.backup` / `.orig` / `.old` 文件: 仅 `.trellis/task.md.bak` (已 C 列)
- `.tmp` / `.swp` / `~` / `Thumbs.db`: 0 命中

## Acceptance
- [ ] A/B/C 列所有目标删除
- [ ] `find src-tauri/src-tauri -maxdepth 0` 无此目录
- [ ] `find src-tauri/src -type d -empty` 返空 (4 个空目录已删)
- [ ] `find . -name ".DS_Store" -not -path "./.git/*" -not -path "./node_modules/*" -not -path "./.worktrees/*" -not -path "./target/*"` 返空
- [ ] `.gitignore` 含 `.DS_Store` 规则 (grep 验证)
- [ ] `git status` 不含 `.trellis/task.md.bak` 残留
- [ ] **保留项全部不动** (D 列): golden fixture / agent placeholder / target / node_modules / .venv
- [ ] `cargo build` 不炸 (删的目录无 Rust 模块引用)
- [ ] `yarn build` 不炸

## Out of Scope
- `cargo clean` / `rm -rf node_modules` (用户自决, 非仓库治理)
- 归档 task PRD 路径回溯
- 修改 `.trellis/spec/backend/index.md` 等其它 task 改动
- `.git` 内部对象

## Technical Notes
- 删除前已验证 A 列目录无 git 历史 (`git log --all --oneline -- <path>` 返空) + 无文件 (`find -type f` 返空)
- `src-tauri/src-tauri/` 双嵌套是反模式信号, 推测来自某次 `cargo new` 或脚本误执行, 现在清理掉防混淆
- 操作 `--no-worktree`: 直接主工作区改, finish 时无 worktree 合并, 直 commit 主工作区改动 + archive

## 依赖
- 阻塞: 任务级并发 (active 已 4 含本, in_progress 1) → 等任一 finish 腾位
- 文件集与其它 active task 不相交 (仅删空目录/junk, 不动源码/.trellis/spec/.trellis/tasks/active): 可与 rename/locale/models-json 串行无忧
- 不影响 a30a7a816f7da32ed (其跑在 rename worktree, 本 task 改主工作区, 无重叠)
