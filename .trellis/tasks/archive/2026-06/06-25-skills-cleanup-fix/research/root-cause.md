# Research: aidog 何时清理 npx skills 安装的 skills

- **Query**: 定位「aidog 何时会清理掉 npx skills 安装的 skills」的全部触发路径与根因
- **Scope**: internal (只读源码分析)
- **Date**: 2026-06-25

## TL;DR 根因结论

**没有任何「启动时」或「后台自动」路径会物理删除 skills**。所有「物理删除」触发点都需用户显式操作（Skills 页点 disable/uninstall/uninstall_all/align、或 ImportExport 页点「应用」勾选 skills）。用户报告「skills 消失」最可能根因有二：

1. **最可能 (缓存写空假象)**：Skills 页 mount / 获焦触发 `list_refresh`，npx 失败/超时/HOME env 错时 `list_installed` 返回空 → `write_cache(scope, [])` **覆盖磁盘缓存为空**（cache.rs:118-122）。前端显示空列表，但物理 skill 文件（`~/.agents/skills/`、`~/.claude/skills/` symlink）**仍在磁盘**。下次 npx 成功即恢复。属「假清理」。
2. **次可能 (导入 .aidogx 触发真删)**：用户导入过含 skills scope 的 `.aidogx` 文件（含旧机已卸载 skill 的 entry，`enabled=false`）→ `apply/mod.rs:247` → `skills_sync::import_skills` 对每个 `enabled=false` 的 agent 跑 `npx skills remove -s <name> -a <slug> -g -y`（skills_sync.rs:107-111）→ **物理删规范存储目录 + 各 agent symlink**。前端默认勾选 skills 条目（ImportExport.tsx:136 `setSelectedItems(new Set(prev.items.map(...)))`），用户若没逐项取消勾选即点应用，会触发删除。

---

## 全部清理触发点总表

| # | 路径 | 触发方式 | 物理删 or 缓存写空 | 可恢复 | file:line |
|---|------|---------|-------------------|--------|-----------|
| 1 | `skills_disable` 命令 → `ops::disable` → `npx skills remove -s <name> -a <slug> [-g] -y` | 用户点 Skills 页 agent 图标 toggle（已启用→disable） | **半物理**：删该 agent 的 symlink/锁条目，**不删规范存储目录**（其他 agent 仍可用）；属「关闭启用」非「卸载」 | 重新 toggle 即恢复 | commands/skills.rs:136-149; ops.rs:133-149; Skills.tsx:230-231 |
| 2 | `skills_uninstall` 命令 → `ops::uninstall` → `npx skills remove -s <name> [-g] -y`（不带 `-a`） | 用户点 Skills 页单条「卸载」按钮（二次确认 modal） | **真物理删**：删规范存储目录 + 所有 agent symlink/锁条目 | 不可恢复（需重 `npx skills add`） | commands/skills.rs:180-201; ops.rs:171-217; Skills.tsx:295 |
| 3 | `ops::fs_fallback_remove`（uninstall 的兜底分支） | #2 中 npx 返回 "No matching skills found"（第三方/手动 symlink skill，如 understand-*）时触发 | **真物理删**：扫 `~/.agents/skills/<name>` + `~/.<dotdir>/skills/<name>` 全删 | 不可恢复 | ops.rs:196-216, 254-307 |
| 4 | `skills_uninstall_all` 命令 → `ops::uninstall_all` → `npx skills remove --all [-g]` | 用户点 Skills 页「一键卸载」（二次确认 modal） | **真物理删**：scope 下所有 skill 的规范存储 + 所有 agent symlink | 不可恢复 | commands/skills.rs:167-175; ops.rs:161-165; Skills.tsx:277 |
| 5 | `skills_align_agents` → `bulk::align_agents` → 逐 skill `ops::disable` | 用户点 Skills 页「对齐配置」按钮 | 同 #1（半物理，关多余启用） | 重新 enable 即恢复 | commands/skills.rs:206-219; bulk.rs:64; Skills.tsx:313 |
| 6 | `import_skills`（`.aidogx` 导入 apply 阶段）→ 对 `enabled=false` 的 agent 跑 `npx skills remove` | **用户在 ImportExport 页选 .aidogx → 默认全选条目 → 点「应用」** | **半物理**：同 #1（npx remove -s -a，删该 agent 启用，不删规范存储）；但若 entry 所有 agent 都 enabled=false，等价逐 agent 清完 | 重新 enable 即恢复 | apply/mod.rs:247; skills_sync.rs:107-111 |
| 7 | `list_refresh` 命令 → `cache::list_refresh` → npx 失败/空 → `write_cache(scope, [])` | **自动**：Skills 页 mount / scope 切换 / 窗口获焦（10s 节流） | **纯缓存写空**：磁盘缓存 `~/.aidog/skills-cache.json` 对应 scope 写空；**物理 skill 文件不删** | 下次 npx 成功即恢复 | cache.rs:114-122; list.rs:15-25; Skills.tsx:91-107, 145-165 |

---

## 调研问题逐条回答

### Q1: 全部 npx remove / uninstall / disable 调用点

见上方总表。补充说明：

- **触发方式分布**：
  - **用户主动**（点按钮）：#1, #2, #3（#2 的兜底）, #4, #5
  - **导入自动**（apply 阶段）：#6
  - **页面交互自动**（mount/获焦）：#7（仅缓存写空，不物理删）
- **物理删等级**：
  - 真物理删规范存储目录：#2, #3, #4（`npx skills remove -s <name>` 不带 `-a` = 全删）
  - 半物理（删 agent symlink/锁条目，规范存储保留）：#1, #5, #6（`npx skills remove -s <name> -a <slug>` 带 `-a` = 单 agent）
  - 纯缓存：#7
- **可恢复性**：#2/#3/#4 不可恢复；#1/#5/#6 重新 enable 即恢复；#7 下次 npx 成功即恢复

证据：
- `commands/skills.rs:136` `skills_disable` / `:180` `skills_uninstall` / `:167` `skills_uninstall_all` / `:206` `skills_align_agents`
- `ops.rs:133` `disable` / `:185` `uninstall` / `:161` `uninstall_all` / `:254` `fs_fallback_remove`
- `skills_sync.rs:137` `build_remove_args` / `:109` import_skills 内 remove 分支

### Q2: import_skills 完整分支逻辑

读 `skills_sync.rs:80-114` 全文。对单条 `SkillExportEntry` 的每个 `AgentState`：

```
for entry in entries:
    for agent in entry.agents:
        agent_enum = parse_agent(agent.display)?   # 未知 display → push error, continue
        if agent.enabled:
            args = build_add_args(name, source, agent_enum, scope)  # npx skills add <source> -s <name> -a <slug> [-g] -y
            res = run_npx(args, scope)
            success → bump(report, "skills")
            fail    → push error
        else:  # enabled=false
            args = build_remove_args(name, agent_enum, scope)  # npx skills remove -s <name> -a <slug> [-g] -y
            let _ = run_npx(args, scope)   # 失败静默忽略（测试 import_skills_disabled_agent_runs_remove_gracefully 验证）
```

**关键点**：
- `enable=false` 时**无条件**跑 `npx skills remove -s <name> -a <slug>`（skills_sync.rs:107-111）。
- remove 命令带 `-a <slug>`（单 agent），**不带** `--all`，故**只删该 agent 的启用配置（symlink/锁条目）**，不删 `~/.agents/skills/<name>/` 规范存储目录（除非该 skill 在所有 agent 都 enabled=false，则等价清完 agent 启用，但规范存储目录仍由 npx 保留——除非用户在 #2/#4 路径主动卸载）。
- **source 为空时**：`build_add_args` 仍会拼出 `add "" -s <name> ...` 命令并跑 npx（npx 会报错）；**不会跳过该 entry**。export 时 `source` 来自 `info.installed_path`（skills_sync.rs:48），为空则 export 跳过（skills_sync.rs:49-51），故导入端不会遇到真空 source（除非 .aidogx 被外部篡改）。
- **entry 缺失 source/path 时**：`SkillExportEntry.source` 是 `String`（非 Option），反序列化缺失字段 → serde 报错（整个 payload parse 失败，apply 提前返回 Err）。不会进入 import_skills。
- **区分「agent 未启用该 skill」vs「skill 整体卸载」**：import_skills 只做前者（per-agent remove with `-a`），**不做整体卸载**（不带 `--all`、不带 `-s` 单删）。整体卸载只在 #2/#4 用户主动路径。

测试佐证：`skills_sync.rs:333` `import_skills_disabled_agent_runs_remove_gracefully` —— 验证 `enabled=false` → 跑 remove，失败静默。

### Q3: 导入 .aidogx 流程触发链

读 `apply/mod.rs:221-251`。

**触发链**：
1. 用户在 ImportExport.tsx 选 `.aidogx` 文件 → `importExportApi.readPreview(p)` → 后端 `backup::import_read_file` → `apply::preview`（解密+校验+冲突扫描，不写）
2. 前端 `loadPreview` 默认全选条目（ImportExport.tsx:136 `setSelectedItems(new Set(prev.items.map((it) => itemKey(it.scope, it.key))))`）+ 默认冲突全 overwrite（:130-134）
3. 用户点「应用」→ `handleApply`（ImportExport.tsx:248）→ `importExportApi.apply(path, ds, selection)` → 后端 `backup::import_apply` → `apply::apply(payload, decisions, selection, db)`
4. apply 内：先写 codex/claude-code 文件，再 db 事务（group/platform/group_platform/setting），最后（apply/mod.rs:239-248）：
   ```rust
   let skills_sel: Vec<_> = payload.skills.iter()
       .filter(|s| is_selected(selection, SCOPE_SKILLS, &s.name))
       .cloned().collect();
   if !skills_sel.is_empty() {
       super::skills_sync::import_skills(&skills_sel, &mut report);
   }
   ```

**默认勾选状态**：
- 前端 scope 列表（ImportExport.tsx:46-53）含 `skills`，**默认初始化 state 不含 skills**（:67 `useState(new Set(["platform", "group", "group_platform", "setting"]))` —— 这是**导出** scope 的默认，**导入**用 `preview.items` 逐项，与 scope state 无关）
- 导入预览返回后，`selectedItems` 默认**全选所有 items 包括 skills**（:136）
- 用户需手动取消勾选 skills 条目，否则会被应用

**误删场景**：用户从 A 机导出 `.aidogx`（A 机某 skill 仅 codex 启用，claude 未启用 → export 记录 `claude.enabled=false`）→ 在 B 机导入（B 机该 skill claude/codex 都启用）→ 应用 → import_skills 对 claude 跑 remove → B 机 claude 失去该 skill 启用。**规范存储目录不删**（因带 `-a`），codex 仍可用，但 claude 这边「消失」。

**用户确认闸门**：
- `loadPreview` 后必须用户点「应用」按钮才会触发（非自动）
- 但**默认全选 + 无 skills 专属二次确认**，用户若不细看条目易误触

证据：apply/mod.rs:64-66（preview 含 skills 计数）、:187-194（build_items skills 条目）、:239-248（apply 调 import_skills）；ImportExport.tsx:136（默认全选）、:248-269（handleApply）。

### Q4: 启动时机

读 `startup.rs`、`app_setup.rs`、`main.rs`、`lib.rs`。

**启动时调用的与 skills 相关的操作**：**零**。
- `app_setup.rs:18-336` setup 闭包：DB init / auto_vacuum 迁移 / stats_agg 重建 / 日志初始化 / try_sync_settings（settings 文件同步，非 skills）/ ensure_default_coding_tools_settings / 中间件引擎 / 备份调度器 / 软删平台清理 / 通知授权 / 托盘 / 冷启动 tray 预估 / 静默启动。**全程无 import_skills / apply / skills_sync / skills_disable / skills 列表 refresh 调用**。
- `startup.rs:1-206` 仅 Builder + invoke_handler 注册（命令实现惰性调用，需前端 invoke）。
- 备份调度器 `backup/scheduler.rs:80-103` 只调 `maybe_backup → run_backup → collect + encrypt + 写文件`（:64-75 `run_backup_inner`），collect 调 `export_skills`（collect.rs:69，**纯读 npx list**），**不调 import_skills**。

**结论**：启动时**无任何 skills 写操作**（不 import、不 apply、不 disable、不 refresh-写空）。用户报告「启动时清理」的感知，若属实，只能由前端 Skills 页被打开后 mount 触发 #7（缓存写空）解释，非后端启动逻辑。

### Q5: list_refresh 写空缓存

读 `cache.rs:114-122`、`list.rs:15-25`。

**行为**：
```rust
pub fn list_refresh(scope: &SkillScope, proxy_url: Option<&str>) -> CachedSkills {
    let items = list_installed(scope, proxy_url);  // npx skills list --json
    write_cache(scope, items.clone());  // 无条件写（空也写）
    CachedSkills { items, stale: false }
}
```
`list_installed`（list.rs:15-25）：npx 失败（`!res.success`）→ 返回空 Vec；JSON 解析失败 → 返回空 Vec。

**物理文件是否还在**：**还在**。`list_installed` 只调 `npx skills list --json` 读列表，**不调任何 remove/uninstall**。磁盘上的 `~/.agents/skills/<name>/`、`~/.claude/skills/<name>`（symlink）、`~/.codex/skills/<name>` 全部保留。

**下次 list_refresh 成功是否恢复**：**是**。下次 npx 成功 → 返回真实列表 → write_cache 覆盖空缓存 → 前端 listInstalled/listRefresh 拉到真实数据。

**HOME env 错时是否漏检已装 → 返回空 → 写空缓存**：
- env.rs:39-58 `resolve_home_env`：优先 `dirs::home_dir()`（getpwuid 解析），fallback `std::env::var("HOME")`。
- env.rs:62-72 `apply_home_env`：`dirs::home_dir()` 返 None（极罕见）时 warn 跳过，**不阻断**。
- 历史 bug [[skills-claude-code-home-env]]：skills CLI 的 `detectInstalled` 仅依赖 HOME env（无 getpwuid 兜底）。打包 GUI launchd 启动时 HOME 可能缺失/异常 → skills CLI `agents[]` 漏报 Claude Code → `list_installed` 解析时 `enabled_agents` 不含 claude（但 skill 本身仍在列表，因规范存储目录扫描不依赖 HOME）。
- **当前状态（已修）**：env.rs 已用 `dirs::home_dir()` 显式注入 HOME 到 npx 子进程（env.rs:62-65），缓解了 launchd env 极简场景。若用户仍遇 HOME 异常（如 CLAUDE_CONFIG_DIR 指向不存在路径），可能出现 `agents[]` 漏 claude，但 **skill 不会从列表整体消失**（仅 enabled_agents 字段空）。

**结论**：#7 是「假清理」（缓存写空 + UI 显示空），**物理文件不删，下次成功即恢复**。需排除而非真问题。唯一例外：若 npx 长期失败（如 node 损坏、网络代理死锁），缓存长期空，用户感知「skills 没了」直到 npx 恢复。

### Q6: 排序与结论

按「最可能是用户报告的清理」到「最不可能」排序：

1. **【最可能】#7 缓存写空假象**（cache.rs:118-122）
   - 触发频率最高（每次进 Skills 页 / 获焦都可能触发）
   - 无需用户主动操作
   - 现象：UI 显示空，但 `ls ~/.agents/skills/` 文件还在
   - 排除方法：让用户在终端跑 `ls ~/.agents/skills/ && ls ~/.claude/skills/`，若有目录则是此问题

2. **【次可能】#6 导入 .aidogx 触发 remove**（apply/mod.rs:247; skills_sync.rs:107-111）
   - 需用户主动导入且未取消 skills 勾选
   - 现象：特定 skill 在特定 agent 失去启用（规范存储目录不删）
   - 排除方法：询问用户近期是否导入过 `.aidogx` 文件

3. **【较低】#1/#5 用户主动 disable/align**（ops.rs:133; bulk.rs:64）
   - 需用户在 Skills 页显式操作
   - 现象：单 agent 失去启用，其他 agent 不受影响
   - 用户一般记得操作过

4. **【最低】#2/#3/#4 用户主动卸载**（ops.rs:185, 161, 254）
   - 需二次确认 modal，用户必记得
   - 现象：规范存储目录真删，所有 agent 失去

**最可能根因**：**#7 缓存写空假象**（若用户报告「重启后/打开后 skills 没了」）或 **#6 导入误触**（若用户报告「导入备份后 skills 没了」）。需向用户确认：
- 「没了」是 UI 显示空，还是 `ls ~/.agents/skills/` 也空？
- 近期是否导入过 `.aidogx`？是否点过 Skills 页的 disable/卸载/对齐？

---

## 修复方向建议（不实施）

1. **#7 缓存写空**：`cache.rs:118` `list_refresh` npx 失败时**不覆盖**已有缓存（仅 npx 成功才 write_cache）。注释 :116-117 已意识到此问题（"这里仍写空覆盖"），改为「失败保留旧缓存 + 返回 stale=true 让前端显加载失败提示」。
2. **#6 导入误删**：
   - `skills_sync.rs:107` 对 `enabled=false` 的 remove 分支，改为**只 add enabled=true 的 agent**，**不主动 remove**（导入语义应是「增」非「减」）。
   - 或前端 ImportExport.tsx:187-194 skills 条目标记 `conflict=true`，强制用户显式决策（不默认全选）。
   - 或 apply 前对 skills scope 加专属二次确认 modal（「将移除以下 agent 的 X 项 skill 启用」）。
3. **诊断增强**：`list_refresh` npx 失败时记 warn 日志含 stderr，便于用户/support 判断是「真无 skill」还是「npx 故障」。
4. **HOME env 防御**：env.rs:67 当前 warn 即跳过，可考虑 `dirs::home_dir()` None 时**拒绝执行 list**（返回明确错误而非空列表），避免假空缓存。

---

## 引用证据汇总

- `src-tauri/src/commands/skills.rs:136` skills_disable / `:167` skills_uninstall_all / `:180` skills_uninstall / `:206` skills_align_agents
- `src-tauri/src/gateway/skills/ops.rs:133` disable / `:161` uninstall_all / `:185` uninstall / `:196` fs_fallback 触发点 / `:254` fs_fallback_remove
- `src-tauri/src/gateway/skills/cache.rs:114-122` list_refresh 写空 / `:125-142` write_cache
- `src-tauri/src/gateway/skills/list.rs:15-25` list_installed npx 失败返空
- `src-tauri/src/gateway/skills/env.rs:39-72` resolve_home_env / apply_home_env
- `src-tauri/src/gateway/skills/npx.rs:40-74` run_npx_in_scope
- `src-tauri/src/gateway/skills/bulk.rs:64` align_agents disable 调用
- `src-tauri/src/gateway/import_export/skills_sync.rs:80-114` import_skills 主循环 / `:107-111` enabled=false remove 分支 / `:137-150` build_remove_args
- `src-tauri/src/gateway/import_export/apply/mod.rs:239-248` apply 调 import_skills / `:64-66, 187-194` preview/build_items skills
- `src-tauri/src/gateway/import_export/collect.rs:68-70` export_skills 调用（导出，纯读）
- `src-tauri/src/gateway/backup/scheduler.rs:64-75` run_backup_inner（不调 import_skills）/ `:80-103` spawn_scheduler
- `src-tauri/src/app_setup.rs:18-336` setup（无 skills 写操作）
- `src-tauri/src/startup.rs:1-206` Builder invoke_handler 注册
- `src/pages/Skills.tsx:91-107` refreshInstalled / `:114-147` loadInstalled mount / `:145-165` 获焦 revalidate / `:230-231` disable 调用 / `:277, 295, 313` 卸载/对齐
- `src/components/settings/ImportExport.tsx:136` 默认全选 / `:187-194`（注：实际在 build_items 后端） / `:248-269` handleApply

## Caveats / Not Found

- 未实际运行 aidog 复现；结论基于静态代码分析 + 测试用例反推。
- #7 的「缓存写空」是否真触发，取决于用户环境的 npx 稳定性（node 版本/网络代理/HOME 配置）。需用户实测 `ls ~/.agents/skills/` 验证物理文件是否还在。
- #6 的实际触发率取决于用户是否使用 `.aidogx` 导入功能（不用则不会触发）。
- 未深挖 `npx skills remove -s <name> -a <slug>` 在 skills CLI 内部是否真「只删 agent symlink 不删规范存储」——此为 npx skills CLI（第三方包）行为，源码不在本仓。ops.rs:167-176 注释「不带 -a = 删该 skill 在所有 agent 的启用配置 + 规范存储」暗示**带 `-a` = 不删规范存储**，但需 npx skills CLI 源码或实测确认。
