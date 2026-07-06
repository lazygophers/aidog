# defaults.ts JSON 抽取 + async 化

**Parent**: `07-06-platform-defaults-json-sync` (Phase 1, 先于 sync-jsdelivr)

## Goal
把 `src/domains/platforms/defaults.ts` 4 函数硬编码默认配置抽到 `resources/defaults.json` (手维护), 函数改 async 从 Tauri command 读 JSON。

## 改动
1. **新建** `resources/defaults.json` — 手维护, 从 defaults.ts 现有 4 函数抽数据 (62 protocol × endpoints/models/model_list/client_type), schema 见 parent PRD, `last_updated` 用 Unix 秒时间戳 (i64)
2. **新建** Tauri command `get_defaults_json()` — 读 `~/.aidog/defaults.json` → 缺失回退 bundled `resources/defaults.json` (Tauri resources 机制), 返 JSON 字符串给前端
3. **改** `src/domains/platforms/defaults.ts` — 4 函数 (`defaultClientForProtocol` / `getDefaultEndpoints` / `getDefaultModels` / `getDefaultModelList`) 改 async, 从 `get_defaults_json()` 读 JSON 后查 protocol
4. **改** 26 调用点 (6 文件) 加 await:
   - `src/utils/ccswitchMatch.ts`
   - `src/utils/sub2apiMatch.ts`
   - `src/components/platforms/PlatformCard.tsx`
   - `src/pages/platforms/platformPasteApply.ts`
   - `src/pages/platforms/usePlatformForm.ts`
   - `src/pages/platforms/formSectionsEndpoints.tsx`
5. PROTOCOLS 常量 (`src/domains/platforms/constants.ts`) 不动, STATIC_MODEL_IDS (后端) 不动
6. `src/services/api.ts` 加 `getDefaultsJson()` invoke 封装

## Acceptance
- [ ] resources/defaults.json 含 62 protocol 完整数据 (零丢失, 对照 defaults.ts 旧值)
- [ ] get_defaults_json command: app data 优先 → bundled fallback
- [ ] 4 函数 async 化, 26 调用点 await (yarn build 通过)
- [ ] 新建平台流程: 默认值从 JSON 读, 与旧硬编码值一致
- [ ] cargo test + yarn build 全绿

## Out of Scope
- 同步机制 (jsDelivr/raw) → child 2
- price_sync URL → child 3
- STATIC_MODEL_IDS / PROTOCOLS 常量 JSON 化

## 依赖
- 无 (Phase 1 基础, 先于 child 2)
