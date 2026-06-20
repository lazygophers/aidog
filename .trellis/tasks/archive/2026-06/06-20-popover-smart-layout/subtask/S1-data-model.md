---
id: S1
slug: data-model
deliverable: D1
parent-task: 06-20-popover-smart-layout
status: ready
execution-layer: sub-agent
isolation: worktree
depends-on: []
blocks: [S2, S3, S4]
estimated-tokens: 25000
---

# S1 · 扩展浮窗数据模型 + 旧配置兼容

## 目标

给 `PopoverItem`/`PopoverConfig` 加 `row/size/color/rows` 字段(Rust + TS 双写)，全带 serde default，旧配置(无新字段)反序列化兜底，加 Rust test 验证。

## 产出

- `src-tauri/src/gateway/models.rs`：`RowMeta` struct + `PopoverItem` 加 row/size/color + `PopoverConfig` 加 rows + `TrayColor` impl Default(若缺)
- `src/services/api.ts`：同步 `RowMeta`/`PopoverItem`/`PopoverConfig`(新字段 optional)
- models.rs 新增 test：旧 JSON 兜底 + 新字段往返

## 验证

```bash
cd /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout/src-tauri && cargo test && cargo clippy 2>&1 | grep -i warning
```

期望：cargo test 退出码 0(含新旧配置 test)；clippy grep 无输出(零 warning)。

## 资源

- 独占文件：`src-tauri/src/gateway/models.rs` `src/services/api.ts`(db.rs 仅在需迁移时碰，按 design 采前端 fallback 则不改 db.rs)
- 审批槽位：否

## 依赖

无上游。

## 执行细节

按 design.md「数据契约」节：
- `RowMeta{cols:i32}` default_cols=1
- `PopoverItem` 加 `row:i32`(serde default 0)、`size:String`(default "m")、`color:TrayColor`(serde default)
- `PopoverConfig` 加 `rows:Vec<RowMeta>`(serde default 空)
- `TrayColor` 必须 `impl Default`(mode="follow")——查现有定义，缺则加 `#[derive(Default)]` 或手写 Default
- **不做 DB 迁移脚本**(采 design 决定：渲染层前端 fallback `row??order`)，故 db.rs:1560-1586 的 get/set 可不动(serde 自动容错新字段)
- TS 侧 api.ts:566-618 加 optional 字段

### Dispatch Prompt

```
Active task: .trellis/tasks/06-20-popover-smart-layout
# isolation: worktree

## 目标
给 PopoverItem/PopoverConfig 加 row/size/color/rows 字段(Rust models.rs + TS api.ts 双写)，serde default 兜底旧配置，加 Rust test。

## 已知
- 现状字段与行号：models.rs:871-933(PopoverItem/PopoverConfig)、api.ts:566-618、db.rs:1560-1586
- TrayColor{mode,value} 已存在(popover.tsx resolveColor 解析)，需确保 impl Default(mode="follow")
- 不做 DB 迁移脚本：渲染层用 row??order fallback，db.rs get/set 不改
- 读 .trellis/tasks/06-20-popover-smart-layout/design.md「数据契约」节为准

## 工作目录与范围
- cwd: /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-20-popover-smart-layout
- 可改: src-tauri/src/gateway/models.rs, src/services/api.ts
- 禁改: db.rs(除非 TrayColor Default 必须在此), .trellis/**, **/dist/**

## 输出格式
diff，含新 Rust test。

## 验收标准
cd src-tauri && cargo test 退出码 0(新旧配置 test 都过) + cargo clippy 零 warning。

## 失败处理
- 工具瞬时错误→重试1次
- TrayColor 定义找不到/Default 冲突→输出 `需要: <问题>` 停
- 业务阻塞→报 Blocked

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn trellis-implement/trellis-check。
```

## 回滚

- 触发：cargo test 红或字段破坏旧配置
- 步骤：`git -C .worktrees/06-20-popover-smart-layout reset --hard HEAD`

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| TrayColor 无 Default 致编译错 | S1 阻塞 | 先补 impl Default |
| 旧配置缺字段反序列化失败 | 老用户配置丢 | serde default + test 双保险 |

## 历史

- 2026-06-20: created
