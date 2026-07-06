# data/models.json → src-tauri/defaults/models.json 迁移

## Goal
`data/models.json` (349KB, 11625 行, Python pricing scraper 聚合产出的定价/max_tokens 唯一信源, app 经 GitHub raw 定时拉取) 从仓库根 `data/` 移到 `src-tauri/defaults/`, 与 `platform-presets.json` / `settings.json` 同目录, 集中托管「Tauri 打包内嵌 + 外部同步」类静态数据资源。文件名保持 `models.json` 不变 (纯路径迁移)。

## 改动范围

### 文件 move (git mv 保留 blame)
- `data/models.json` → `src-tauri/defaults/models.json`
- 若 `data/` 迁空 (无其它文件) → 删空目录

### Rust URL 路径 (price_sync.rs)
- `src-tauri/src/gateway/price_sync.rs`:
  - L12 jsDelivr URL: `.../aidog@master/data/models.json` → `.../aidog@master/src-tauri/defaults/models.json`
  - L16 raw URL: `.../aidog/master/data/models.json` → `.../aidog/master/src-tauri/defaults/models.json`
  - L1 / L18 docstring `data/models.json` → `src-tauri/defaults/models.json`

### Python scraper 输出路径
- `scripts/pricing/aggregate.py`: 原子写目标路径 `data/models.json` → `src-tauri/defaults/models.json` (常量 / Makefile / 命令行参数, 实现依现状)
- `scripts/pricing/README.md`: 流程描述路径
- `scripts/pricing/pyproject.toml`: description 字符串
- `scripts/pricing/schema.py`: docstring
- 若有 Makefile / 配置文件指向 `data/models.json` → 同步

### 文档
- `.wiki/modules/pricing.md`: L17 / L28 路径引用
- `CLAUDE.md` (项目): 若提及 `data/models.json` (grep 验证)
- 其它 grep 命中 (排除 .trellis/tasks/archive/** 历史归档不动)

### 不动 (重要边界)
- **`.trellis/tasks/archive/**`** 中历史 PRD/design/implement 提及旧路径 —— 归档历史, 禁追溯改
- **文件内容 schema**: 仅路径迁移, JSON 结构 / 字段 / generated_at 不变
- **price_sync 业务逻辑**: 仅 URL 字符串改, parse/upsert 流程不动
- **Tauri resources / build**: models.json 是运行时 GitHub raw 同步 (非 include_str! 编译内嵌), 无 tauri.conf.json 改动 (与 platform-presets.json 不同, 后者 include_str!)

## Acceptance
- [ ] `git mv data/models.json src-tauri/defaults/models.json` (保留 blame)
- [ ] `data/` 目录处理: 若空则删, 否则保留
- [ ] `grep -rn "data/models\.json\|data/models" --include="*.rs" --include="*.py" --include="*.toml" --include="*.md" --include="*.mjs" --include="*.json" . 2>/dev/null | grep -v node_modules | grep -v .git/ | grep -v .worktrees/ | grep -v ".trellis/tasks/archive/"` 仅剩 archive 历史引用 (允许)
- [ ] `cargo build` + `cargo test --lib` + `cargo clippy --lib` (0 新警告) 全绿
- [ ] `cd scripts/pricing && python -m aggregate --help` (或现有 dry-run/校验入口) 不炸 (路径常量同步)
- [ ] price_sync URL 改后, jsDelivr + raw 两条新 URL HTTP 200 (master 分支合并后验, 24h CDN 缓存可接受延迟)
- [ ] CLAUDE.md / .wiki/modules/pricing.md 路径更新

## Out of Scope
- 文件内容 schema 改动
- price_sync 业务逻辑 (parse/upsert) 改动
- jsDelivr CDN 缓存清理 (24h 自然过期)
- 历史 archive PRD 路径回溯
- Tauri resources 配置 (运行时同步, 非内嵌)
- models.json 改名 (用户已确认保持原名)

## Technical Notes
- models.json = Python scraper 输出 + app 端 GitHub raw 拉取的「外部同步」资源, 与 `include_str!` 编译内嵌的 `platform-presets.json` / `settings.json` 性质不同 (后者是 Tauri 二进制内嵌), 但同属「静态数据资源」目录, 集中托管合理
- 改 URL 后 app 同步会立即拉新路径 (raw URL 无 CDN 缓存), jsDelivr 24h 内仍可能返旧 (但旧路径仍可访问因 git mv 保留 blame 不会删 git 历史, raw URL 会重定向? 实际 raw 不重定向 → 旧 URL 在 commit 推送后 404)。**风险**: 用户未升级 app + master 已推送 → 旧 app 拉旧 URL 404, 同步失败 (但不影响已缓存数据)。可接受 (生成产物定期刷新, 失败重试即可)
- 与 `platform-presets-rename` task 文件集相交 (CLAUDE.md + src-tauri/defaults/ 目录 + price_sync.rs 邻近), **必须串行**: 等 rename merge 后再 start, 避免双 worktree 合并冲突

## 依赖
- 阻塞: `07-06-platform-presets-rename` (in_progress, 文件集相交 CLAUDE.md / defaults/ / price_sync.rs 邻区) → 完成合并后 start
- 阻塞: 任务级并发 (active 已 3, in_progress 1, 上限 2) → rename finish 后腾位
