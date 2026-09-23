# Registry 数据持续对齐

## Goal

`src-tauri/defaults/registry/` 模型条目数据无缺失：分层口径（2026-09-23 拍板）——
`family` / `version` / `capabilities` / `context_window` 四字段凡官方有公布即必补清零；
`thinking_supported` / `thinking_toggleable` / `predecessor` / `display_name` 尽力补。

## Trigger

连续执行：用户 2026-09-23 令「现在开始跑，一直跑到所有东西都没问题」。
按平台分批推进，每轮 1-2 个平台（约 50-200 条），轮间无间隔直到门禁达标。

## Loop body（每轮）

1. 选平台：优先用户报过的，其余按必补字段缺失量降序。
2. research agent 查官方一手来源（平台定价页 / 模型卡 / docs，jina reader 破地区封锁），
   产出「字段值 + 来源 URL」事实表。
3. 依据事实表文本级编辑 registry JSON（禁 JSON round-trip 全文件重写，禁从 canonical
   推断 family/version，数值字面保真）。
4. `node scripts/bump-registry-last-updated.mjs` 盖戳。
5. `yarn check:registry` + `cargo test --manifest-path src-tauri/Cargo.toml -p aidog_db` 全绿。
6. commit（路径限定 platforms/<code>/ 与 index.json）。

## Checkpoint

无人工 checkpoint（用户 2026-09-23 授权 agent 自动写入，事后审计）。
写入质量铁律不因自动写入放宽：只写官方核验值，缺省=未知。

## Gate（验收判据）

覆盖率硬阈值：四必补字段各自覆盖率 ≥90%（起线，随轮次单调收紧）。判定命令：

```bash
AIDOG_REGISTRY_COVERAGE_MIN=90 node scripts/check-registry.mjs
```

默认不带 env 只报数不拦截；thinking_*/predecessor 只统计不设门。
门禁全绿 = 循环终止。

## State

- 2026-09-23 基线（4570 条，check-registry ⑩ 实测）：family 15.8% / version 5.3% /
  capabilities 96.6% / context_window 72.5%；thinking_supported 20.9% /
  thinking_toggleable 18.1% / predecessor 2.8% / display_name 9.5%。
- 平台缺失排名头部：bailian 1016 / bailian_coding 1014 / aihubmix 906 /
  openrouter 744 / bailian_en 732 / therouter 534。
- 2026-09-23 门禁基建提交 `011c60b`（check-registry ⑩ 覆盖率统计 + env 门禁）。
- DB 侧独立线：过期行清理（prune_model_entries）已实现待验证提交。
