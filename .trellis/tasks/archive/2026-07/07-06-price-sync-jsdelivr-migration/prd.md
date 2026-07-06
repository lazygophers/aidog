# price_sync.rs models.json URL 迁移 jsDelivr

**Parent**: `07-06-platform-defaults-json-sync` (Phase 1, 与 defaults-json-extract 并行)

## Goal
`src-tauri/src/gateway/price_sync.rs` 当前硬编码 raw.githubusercontent.com URL, 改 jsDelivr 主 + raw fallback, 统一同步源策略。

## 改动
1. `price_sync.rs:10-12` 常量改:
   - 主源: `https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/data/models.json`
   - fallback: `https://raw.githubusercontent.com/lazygophers/aidog/master/data/models.json`
2. `fetch_models_json()` 改双源尝试: jsDelivr 失败/非 200 → raw fallback
3. 注释 :3 同步更新 (数据源 = jsDelivr master)
4. 逻辑不变 (fetch + parse + upsert model_price)

## Acceptance
- [ ] price_sync 双源 (jsDelivr 主 + raw fallback), 任一成功即用
- [ ] 同步结果不变 (upsert model_price 表数据一致)
- [ ] cargo test (price_sync 现有用例) 全绿
- [ ] log 清晰标识从哪个源拉取

## Out of Scope
- defaults.json 同步 (child 1/2)
- models.json schema 变更

## 依赖
- 无 (独立, 与 child 1 并行)
