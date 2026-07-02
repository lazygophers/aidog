# PRD — settings.json 不写 `_aidog_managed` marker

## 背景
aidog 默认分组同步时,`write_default_claude_settings`(`src-tauri/src/commands/sync_settings.rs:110`)把托管字段 dot-path 数组以 `_aidog_managed` key 写入用户的 `~/.claude/settings.json`。这是污染用户配置文件——CC 虽忽略未知 key,但:
1. 用户文件里塞了 aidog 内部元数据,不干净
2. 前端「从 Claude Code 导入」diff 依赖该 marker 排除托管字段,marker 与 settings 同文件,用户手改/外部工具写 settings 易不同步

决策:搬 marker 到 aidog 内部 DB(复用现有 `setting` KV 表),settings.json 连旧值一起清。

## 决策(用户已锁)
| 维度 | 决策 |
|---|---|
| marker 存储 | aidog 内部 DB `setting` 表 KV(复用 `get_setting`/`set_setting`,scope=`claude_default_group`,key=`managed_paths`),不加新表/新列 |
| settings.json | **不再写** `_aidog_managed` key;sync 时**移除**已存在的旧 `_aidog_managed` key(连旧值清,迁老用户) |
| 前端读取 | 改 invoke 新 command `get_managed_paths` 读 DB,不再从 incoming settings `_aidog_managed` 读 |
| diff 行为 | 零变化——仍是「排除托管叶子集,只列用户新增/改动」,只是数据源 settings→DB |

## 交付
1. **Rust `write_default_claude_settings`**(`sync_settings.rs:110`)——
   - 签名加 `db: &Db` 参数(调用点 `:335` `do_sync_group_settings` 已有 db,透传)
   - 删 `obj.insert(MARKER_MANAGED, ...)` 写 settings 逻辑
   - 改 `set_setting(db, SetSettingInput { scope: "claude_default_group", key: "managed_paths", value: managed.to_json() })` 写 DB
   - merge 后**显式 remove** base 里已存在的 `_aidog_managed` key(`base.as_object_mut().remove(MARKER_MANAGED)`),清旧值
2. **Rust 新 command `get_managed_paths`**(`lib.rs` 注册)——`get_setting(db, "claude_default_group", "managed_paths")` → `Vec<String>`(空/缺省→空数组)。前端 invoke 读。
3. **前端 `readManagedPaths`**(`editors.tsx:2535`)——签名从 `(incoming)` 改为读 DB:调用方先 `await invoke('get_managed_paths')` 拿 Set,再传入;或改 async。`collectValueLeafPaths`(`:2616`)默认参数 `managed = readManagedPaths(incoming)` 调用链同步调整(导入 diff 入口先取一次 managed Set 再下传,避免逐节点 invoke)。
4. **api.ts** 加 `getManagedPaths(): Promise<string[]>` 封装。
5. **测试 `test_sync_settings.rs`**——
   - `write_default_claude_settings_records_managed_paths`(`:142`)改:断言 settings.json **无** `_aidog_managed` key + DB `setting` 表有 `managed_paths`;保留叶子快照语义断言(改读 DB)
   - `write_default_claude_settings_merges_and_idempotent`(`:62`)补断言 settings 无 marker
   - 加用例:旧 settings.json 含 `_aidog_managed` → sync 后被移除(连旧值清)
   - write_default 签名加 db,所有测试调用点(`:74/:84/:159`)补 db fixture

## 验收
- sync 后 `~/.claude/settings.json` **无** `_aidog_managed` key(grep 实证)
- 老用户 settings.json 残留 `_aidog_managed` → 再次 sync 后被清
- aidog DB `setting` 表 scope=`claude_default_group`/key=`managed_paths` 存叶子快照
- 前端「从 Claude Code 导入」diff 行为零回归(托管字段仍被排除,只列用户新增)
- `cargo test test_sync_settings` + `cargo clippy` + `yarn build` 全绿

## 非目标(YAGNI)
- marker schema 变更(仍是叶子 dot-path 数组,只换存储)
- 多分组 marker(仅默认分组写 settings,scope 固定 `claude_default_group`)
- 历史 marker 迁移(旧 settings 值直接清,不读入 DB——DB 本次 sync 重算快照覆盖)

## 风险
- `write_default_claude_settings` 加 db 参数波及测试 fixture(3 处调用),需 HomeGuard + Db 双隔离
- `readManagedPaths` 改 async 可能波及 `collectValueLeafPaths` 同步调用链——若链路深,可顶层取一次 managed Set 下传(invoke 一次/导入),禁逐节点 invoke
- `set_setting`/`get_setting` scope 命名需与现有 scope 体系一致(grep 现有 scope 值确认无碰撞)
