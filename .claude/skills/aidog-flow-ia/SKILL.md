---
name: aidog-flow-ia
description: aidog 用户流程与信息架构（IA）优化——梳理任务流、减少步骤与跳转、对齐 create/update 等成对操作的一致性、保存/同步/反馈时序、tab 与侧栏的信息层级。聚焦"用户完成一件事要点几下、会不会卡住、状态有没有反馈"，区别于纯视觉的 aidog-frontend-experience。触发词：流程、步骤太多、太繁琐、几下、卡住、一致性、create 和 update 不一样、保存没反应、状态不同步、信息架构、IA、导航结构、tab 太乱、找不到。
when_to_use: 某个操作流程繁琐/易卡住要精简；成对操作（新增vs编辑、导入vs导出）行为不一致要对齐；保存/同步状态无反馈或时序混乱；侧栏/tab 信息层级混乱要重组时
---

# aidog 流程与信息架构优化

针对「用户完成一件事的路径」而非单个像素。aidog 是代理网关管理器，核心流程围绕：平台(Platforms) → 分组(Groups) → 日志(Logs) → 设置/统计。本 skill 优化这些流程的步骤数、一致性、状态反馈、信息层级。

## 何时用

- 某流程「步骤太多 / 太绕 / 中途会卡」。
- 成对操作不一致：新增 vs 编辑、导入 vs 导出、启用 vs 停用 行为/UI 不对称。
- 保存/同步「点了没反应」「状态不更新」。
- 侧栏/tab 信息层级乱，用户「找不到」。

## aidog 流程现状锚点

| 流程 | 入口 | 关键约定 |
|---|---|---|
| 导航结构 | `App.tsx` 侧栏（页级）+ `AppSettings.tsx` tab（设置内） | 无 react-router，纯本地 state |
| 离页保护 | `utils/navGuard.ts` 注册表 | 有未保存改动时拦截切页 |
| 设置保存 | 字段保存时**确定性物化**，禁靠 debounce effect | 改 group 配置后须 `syncGroupSettings` |
| Group 统计 | 前端逐 group 调 `groupUsageApi.stats` | 后端按 `proxy_log.group_name` 聚合 |
| 平台重试 | 多平台失败 failover + 三态 status | 后端 `router.rs` + `scheduling.rs` |
| 导入导出 | 7 scope 逐项冲突处理 | `gateway/import_export/` |

## 执行流程

### Step 1：画现状流程图（禁凭感觉改）

1. 把目标流程拆成步骤序列：用户从哪进入 → 每步点什么 → 每步看到什么反馈 → 在哪结束。
2. 标注痛点类型：①步骤冗余 ②无反馈 ③易误触 ④成对操作不一致 ⑤状态不同步。
3. grep 涉及的 page/command，确认真实交互（如保存走哪个 invoke、同步触发点）。

### Step 2：对照「成对操作一致性」

aidog 大量成对操作。优化一个流程时检查它的「对偶」是否需同步改：

| 操作 | 对偶 | 一致性检查点 |
|---|---|---|
| 新增平台 | 编辑平台 | 字段、校验、保存反馈是否对称 |
| 导入 | 导出 | scope 范围、冲突提示是否对齐 |
| 启用 skill | 停用 skill | 乐观更新 + 失败回滚是否都做了 |
| 创建 group | 修改 group | 改完是否都触发 syncGroupSettings |

🔴 CHECKPOINT：改成对操作之一前，确认是否要同步改对偶。只改一半 = 引入新的不一致。

### Step 3：状态反馈与时序

每个会产生副作用的操作必须有：发起态（loading/busy）→ 成功态 → 失败态（可回滚）。
- 保存/同步：禁靠 debounce effect 隐式触发，须显式确定性物化（项目踩坑：statusLine 字段曾因 debounce 丢保存）。
- 改 group 配置后必须 `syncGroupSettings`，否则磁盘配置与 UI 不一致。
- 长操作（安装 skill、同步价格）做乐观更新，禁全页 spinner 阻塞（Skills 页已是范本）。

### Step 4：信息架构（侧栏/tab 层级）

- 一个页放太多 → 拆 tab（参考 `AppSettings.tsx` 模式）。
- 高频操作前置，低频/危险操作收起。
- 改导航结构同步检查 navGuard 注册是否仍覆盖新增页。

### Step 5：验证

```bash
yarn build          # 类型 + 构建
yarn check:i18n     # 新增流程文案的 key
```

实际走一遍改后流程，数点击次数 / 确认每步有反馈。

## 失败模式编码（if-then）

| 触发 | 一线修复 | 仍失败兜底 |
|---|---|---|
| 改完保存「点了没反应」 | 确认是显式物化而非 debounce effect | 检查 invoke 是否真触发 + 加成功 toast |
| group 改完 UI 对但实际没生效 | 确认调了 `syncGroupSettings` | 查 `do_sync_group_settings`（会 strip `_aidog_statusline` 等） |
| 精简步骤后丢了离页保护 | navGuard 注册随新流程更新 | 检查注册/注销时机 |
| 成对操作改一半导致不一致 | 回 Step 2 同步改对偶 | 列对偶清单逐项核对 |

## 反例黑名单（不要做）

1. ❌ 只改成对操作的一半（如只改"新增"不改"编辑"）。
2. ❌ 用 debounce effect 做关键保存 —— 必显式确定性物化。
3. ❌ 改 group 配置不调 syncGroupSettings。
4. ❌ 长操作用全页阻塞 spinner —— 用乐观更新。
5. ❌ 重组导航后忘了更新 navGuard 覆盖。
6. ❌ 凭感觉精简步骤而不先画现状流程图。

## 相关

- 视觉/布局：`aidog-frontend-experience` skill
- 请求链路：`aidog-request-inspect` skill
- 性能：`aidog-perf-audit` agent
