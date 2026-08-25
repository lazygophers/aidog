# ADR 0004: Registry——以平台为基准的真值源结构取代单文件 models.json / platform-presets.json

日期: 2026-08-26
状态: accepted

## 背景

价格与平台预设目前是两个手维护单文件：`src-tauri/defaults/models.json`（价格同步源）与
`src-tauri/defaults/platform-presets.json`（75 协议预设，仅 bundled + `~/.aidog/` 本地覆盖，无远程同步）。
新需求把「价格同步」升级为「模型信息」中枢：模型清单、能力、版本链、价格（默认/分时/缓存）、
官方标记、Claude Code 内置工具支持、上下文限制。单文件无法承载该规模，且官方标记天然是
per-(模型, 平台) 属性。

## 决策

1. **新真值源** `src-tauri/defaults/registry/`，**以平台为基准拆分**：
   - `registry/index.json`：列出全部平台（名称、code、platform 文件与模型文件位置）
   - `registry/platforms/<code>/platform.json`：该平台（协议）的 endpoints / models / model_list /
     peak_hours（由 platform-presets.json 拆分收编，一协议一文件）
   - `registry/platforms/<code>/models/<model>.json`：该平台视角的模型条目
2. **同一模型每平台独立一条 Model Entry**（定价/能力/入参可不同），跨平台关联靠双 id：
   `model_id`（平台真实请求名）+ Canonical Model（内部统一 id，转换映射与聚合用）。
3. 模型条目字段：`model_id` / `canonical_model` / `family` / `version` / `predecessor` /
   `capabilities[]`（text/vision/image_gen/tool_use/reasoning/audio/video/embedding，**取代 modality**）/
   `builtin_tools_excluded[]`（黑名单，缺省=全支持）/
   `max_input_tokens` / `max_output_tokens` / `context_window` /
   价格（input/output/cache_read 绝对价 + 可选 `peak` 分时绝对价 + `official` 官方标记）。
4. 版本链（family/version/predecessor）每平台文件各带一份，自包含，不提升到 index 层。
5. 旧 `models.json`、`platform-presets.json` 废弃移除。
6. 同步：jsDelivr 主 + raw 兜底，**index 驱动**（先拉 index.json，按清单逐文件拉），best-effort
   （单文件失败保留 DB 旧数据 + 警告，不阻塞整轮）。presets 与模型同一套机制获得远程同步能力。
7. bundled（编译期 include）仅作 DB 空且同步失败时的兜底。

## 后果

- 平台增删 = 改 index + 对应文件夹；无 index 的文件不会被同步。
- 同一模型多平台信息重复维护，漂移风险由维护者自负（用户明确接受，换取 per-platform 独立性）。
- `resolve_price` / est_cost / 前端 defaults 读取链路全部改查 registry 结构。

## 备选方案

- 模型归厂商文件夹一份、其他平台价格写进 pricing（第 5 轮否决）：用户要求不同平台定价、
  功能、入参独立接入，不能共享条目。
- 目录枚举逐文件拉（jsDelivr data API）：请求多、部分失败面大，index 驱动确定性更强。
- catalog / knowledge 命名：registry 为用户选定。
