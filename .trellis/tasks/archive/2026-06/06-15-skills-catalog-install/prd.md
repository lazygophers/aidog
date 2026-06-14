# PRD: skills-catalog-install

## 背景
Skills 页当前仅管理**已装** skills（统一列表 + per-item agent toggle）。catalog 搜索/安装 UI 曾存在后被移除（用户当时未需求）。现以**独立子视图**形态加回。

## 决策（已与 main 确认）
1. **agent 多选**: 一条 catalog 条目可一次装到 claude+codex 多 agent（复选），贴合现有 per-item 多 agent toggle 模式。
2. **页面形态 = Skills 页内子视图**: Skills.tsx 本地 state 切到"搜索安装"视图（仿 AppSettings tab），有返回按钮。**不改顶层 nav**。

## 现状（复用，禁重造）
- 后端 `gateway/skills.rs` **已保留** `browse_catalog()` / `search(kw)` / `CatalogEntry{id,name,description,repo_url}` / `fetch_catalog_http` / `npx_find`。
- lib.rs **已注册** `skills_browse_catalog` / `skills_search` command。
- 前端 `api.ts` **已保留** `skillsApi.browseCatalog()` / `skillsApi.search(keyword)` + `CatalogEntry` 类型。
- 现有 `skills_enable(name, path, agent, scope)` 用 **installed_path**（仅适用已装 skill 启用到 agent）。catalog **远程新装** 需不同路径：`npx skills add <owner/repo> [-s <skill>] -a <slug> [-g] -y`。

## 改动清单

### 1. 后端：新增 catalog-install command
**文件**: `src-tauri/src/gateway/skills.rs` + `src-tauri/src/lib.rs`

`skills.rs`:
```rust
/// 从 catalog 安装 skill 到指定 agents（npx skills add <id> [-s <skill>] -a <slug> -y）。
/// id = CatalogEntry.id（owner/repo）。逐 agent 调用，合并结果。
pub fn install(
    id: &str,
    skill_name: &str,   // CatalogEntry.name，作 -s 候选（单 skill repo 可省）
    agents: &[SkillAgent],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult
```
逻辑：对每个 agent slug，构造 `add <id> -s <skill_name> -a <slug> [-g] -y`，逐个 `run_npx`，合并 stdout/stderr，全成功才 success=true。成功后 `invalidate(scope)`。

⚠️ **实施期实测**（check 阶段）: `-s` 是否必需 / id 格式。先跑 `npx skills add <已知catalog id> -a claude-code -g -y`（不带 -s）看 exit code；失败则加 `-s <name>`。实测确认命令形态后再固化。**禁凭 help 推断**（见 npx-skills-cli memory 教训）。

`lib.rs`:
```rust
#[tauri::command]
async fn skills_install(
    db: State<'_, Db>,
    id: String,
    skill_name: String,
    agents: Vec<SkillAgent>,
    scope: SkillScope,
) -> Result<SkillsOpResult, String>
```
注册到 invoke_handler。

### 2. 前端：api 层
**文件**: `src/services/api.ts`
```ts
install: (id, skillName, agents, scope) =>
  invoke<SkillsOpResult>("skills_install", { id, skillName: skillName, agents, scope }),
```

### 3. 前端：Skills 页子视图
**文件**: `src/pages/Skills.tsx` + 新组件 `src/components/skills/SkillInstallView.tsx`

Skills.tsx：
- 新增 local state `view: "list" | "install"`（默认 list）
- list 视图顶部加按钮「添加 Skills」（glass 按钮 + plus 图标）→ `setView("install")`
- install 视图 = `<SkillInstallView onBack={() => setView("list")} scope={scope} onInstalled={refresh} />`

SkillInstallView.tsx：
- 顶部：搜索框（debounce 300ms）+ scope 感知（沿用 Skills 页 scope）+ 返回按钮
- 主体：catalog 列表（browse 全量 / search 过滤）。每条卡片：name + description + repo_url 链接 + agent 复选组（claude/codex SVG 图标，仿现有 per-item toggle 样式）+ 「安装」按钮
- agent 复选默认全选（claude+codex）
- 安装：调 `skillsApi.install(id, name, selectedAgents, scope)`，busy 禁并发，结果经 applyResult setMessage 弹出
- 已装标记：交叉比对 `skillsApi.listInstalled(scope)` 的 items，已装的标「已装」并禁用重复安装（或显示已启用 agent）
- 空态 / 加载态 / 错误态（catalog 抓取失败提示）

### 4. i18n
**文件**: `src/locales/{zh,en,ar,fr,de,ru,ja}.json` + Rust backend i18n（如需）

新增 key（`skills.install.*`）:
- `skills.install.title` 添加 Skills / Add Skills
- `skills.install.search` 搜索 skills / Search skills
- `skills.install.searchPlaceholder` 搜索占位
- `skills.install.install` 安装 / Install
- `skills.install.installing` 安装中… / Installing…
- `skills.install.installed` 已装 / Installed
- `skills.install.noResults` 无结果 / No results
- `skills.install.selectAgent` 选择 agent / Select agents
- `skills.install.loadFailed` catalog 加载失败 / Failed to load catalog
- `skills.install.installSuccess` 安装成功 / Installed successfully
- `skills.install.installFailed` 安装失败 / Installation failed
- `skills.install.addBtn`（list 视图按钮）添加 / Add

7 语言全覆盖（项目硬规，check-i18n.mjs 兜底）。

## 验证
- `cd src-tauri && cargo build` 无错
- `cd src-tauri && cargo clippy` 无 warning
- `cd src-tauri && cargo test`（skills.rs 现有 17 单测不破 + 新增 install args 构造单测）
- `yarn build`（tsc + vite）无错
- `node scripts/check-i18n.mjs` 全覆盖
- 手动：Skills 页点「添加」→ 子视图 → 搜索 → 选 agent → 安装 → 返回 list 刷新见新 skill
- npx install 命令实测 exit code + stdout（-s 必要性确认）

## 非目标
- 不改顶层 nav
- 不改现有已装列表 / per-item toggle 逻辑
- 不做 catalog 分类/标签过滤（仅关键字搜索）
- 不做安装进度条（npx 无流式输出，busy 态足够）

## 风险
- npx `add` 命令形态未实测（-s 必要性）→ check 阶段实测固化
- skills.sh catalog 网络可达性 → 已有 fetch_catalog_http + npx_find 双路径回退
- 跨 agent 部分成功（claude 成功 codex 失败）→ SkillsOpResult 合并 stderr，前端展示完整信息，list 刷新反映实际状态
