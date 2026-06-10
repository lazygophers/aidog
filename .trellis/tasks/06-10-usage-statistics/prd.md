# usage-statistics

## Goal

新增「使用统计」独立页面，按平台 / 分组 / 模型维度展示请求量、Token 消耗、延迟等聚合指标，支持按天 / 小时粒度与时间范围筛选。

## What I already know

### 现状

- 数据源：`proxy_logs` 表（`src-tauri/migrations/001_init.sql:59`）
- 已有索引：`idx_proxy_logs_group(group_name)`, `idx_proxy_logs_created(created_at)`
- 前端路由：`src/App.tsx` 基于 activeNav 切换
- i18n：7 语言 `src/locales/*.json`
- DB 层：rusqlite，`Db` struct 封装 `Mutex<Connection>`

### 调研结论

- `proxy_logs` 缺 `platform_id`，用 `target_protocol` 聚合作为"平台类型"维度

## Assumptions (temporary)

- 用户关注"协议类型"维度统计，非单个平台实例
- SQLite 聚合性能足够（单机场景）
- 前端图表用纯 SVG，不引入第三方库

## Open Questions

无 (范围已明确)

## Deliverable 矩阵

| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1 | Rust 后端统计 API | diff | cargo check 通过；前端调用返回 JSON | P0 |
| D2 | 前端统计页面 | UI | 页面渲染正常，筛选生效 | P0 |
| D3 | 导航入口 + i18n | diff | 侧栏出现统计入口；7 语言正确 | P0 |

## Requirements

### R1 (D1) — 后端聚合 API

- R1.1 新增 Tauri command `stats_query`，接受时间范围、粒度、分组维度、筛选条件
- R1.2 返回 overview + 时间桶 + 维度排行
- R1.3 支持按 group_name / model / target_protocol 筛选
- R1.4 SQL 使用已有索引

### R2 (D2) — 前端统计页面

- R2.1 独立页面 `src/pages/Stats.tsx`
- R2.2 筛选区：时间范围、分组、模型、粒度
- R2.3 指标卡片：总请求、成功率、Token、延迟
- R2.4 趋势图：SVG bar chart
- R2.5 分布表格：维度排行榜
- R2.6 Liquid Glass 风格

### R3 (D3) — 导航与国际化

- R3.1 侧栏新增统计入口
- R3.2 7 语言翻译

## Subtask 拆分

| ID | Subtask | 所属 D | 边界 | 说明 | 详情 |
| --- | --- | --- | --- | --- | --- |
| S1 | API 结构体 + command | D1 | models.rs, lib.rs | 定义结构体 + 注册 command | 内联 |
| S2 | DB 聚合查询 | D1 | db.rs, lib.rs | SQL 聚合实现 | 内联 |
| S3 | 前端页面 + 导航 + i18n | D2,D3 | Stats.tsx, App.tsx, api.ts, locales | 完整页面 | 内联 |

### Subtask 调度图

```mermaid
flowchart LR
    S1[S1 · API 结构体] --> S2[S2 · DB 聚合]
    S2 --> S3[S3 · 前端页面]
    S3 --> G1{{G1 · 全量验收}}
    classDef serial fill:#fff3e0,stroke:#e65100
    class S1,S2,S3,G1 serial
```

## Acceptance Criteria

- [ ] cargo check 通过
- [ ] 页面渲染无错，筛选交互正常
- [ ] 侧栏统计入口可见，7 语言正确

## Definition of Done

- Requirements 实现 + AC 勾选
- commit 完成
- worktree 合并 + 移除
- 非平凡发现落 cortex

## Out of Scope

- 实时推送 / WebSocket
- 数据导出 CSV/Excel
- 第三方图表库
- 单平台实例聚合
- 统计缓存/预计算

## Technical Notes

### 文件位置

- DB schema: `src-tauri/migrations/001_init.sql`
- DB 查询: `src-tauri/src/gateway/db.rs`
- 模型: `src-tauri/src/gateway/models.rs`
- Commands: `src-tauri/src/lib.rs`
- 页面: `src/pages/`
- 路由: `src/App.tsx`
- API: `src/services/api.ts`
- i18n: `src/locales/*.json`

### 验证命令

```bash
cd src-tauri && cargo check
cd .. && npx tsc --noEmit
```
