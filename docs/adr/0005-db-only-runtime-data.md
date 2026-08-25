# ADR 0005: 运行时数据统一入库，~/.aidog/ 不保留 JSON 文件缓存

日期: 2026-08-26
状态: accepted

## 背景

现状两处 JSON 文件缓存/覆盖：`~/.aidog/platform-presets.json`（用户可手改的 preset 本地覆盖层，
`defaults.rs::get_defaults_json` 读它回退 bundled）与新 registry 远程同步结果的落点。文件层与
DB 层双真值导致状态不可见、易漂移。

## 决策

1. **运行时一律 DB**：远程同步（模型条目、平台预设）直接 upsert 入库；读取链路 DB 优先，
   bundled（编译期）仅作 DB 空时的兜底。
2. **取消 `~/.aidog/` 下全部 JSON 缓存/覆盖文件**。
3. **老 `~/.aidog/platform-presets.json` 不迁移**，升级后直接忽略，回落 bundled/DB（用户明确选择
   忽略，接受手改配置失效）。
4. DB 存储 per-(platform, model) 键（新表），替代 `model_price` 的全局 model_name 单键——
   同一 model_id 在多平台各有条目，单键必然相撞。

## 后果

- 手改过本地 preset JSON 的用户升级后改动丢失（已接受）。
- `get_defaults_json` 的本地文件读取路径删除；`model_price` 表被新表取代，需数据重建。
- preset 从「编译期真值 + 本地文件」变为「bundled 兜底 + DB 可远程更新」，新 App 版本字段兼容
  由「忽略未知字段」语义兜住。

## 备选方案

- 一次性导入老本地 JSON 再忽略：需写迁移逻辑，用户明确不需要。
- 保留文件缓存作离线兜底：双真值漂移问题依旧，弃。
