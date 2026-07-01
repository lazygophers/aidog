# PRD — 添加平台 / 智能识别支持多 apikey 批量创建

> 用户请求 (/trellisx-flow): 添加平台、智能识别都应支持多 apikey 批量创建。并入现有 task `batch-add-platform` (原 planning 空壳)。

## 目标

解析出/输入的 N 个 apikey → 批量创建 N 个平台实例 (同 base_url + platform preset, 各挂自己的 key), 而非当前只用首个 key 创建单平台。

## 现状 (main 调研)

- **解析器已支持多 key**: `src/utils/platformPaste.ts` `ParsedPaste.apiKeys: string[]` 去重候选, 已剔除 CJK/已 base64 解码。**无需改解析器**。
- **智能识别弹窗只用首 key**: `src/components/platforms/SmartPasteModal.tsx:119` `setSelKey(parsed.apiKeys[0])` 单选 radio → `onApply` 只传 1 个 `apiKey` → 1 平台。
- **手动添加表单单 key**: Platforms.tsx 添加平台表单 apikey 字段单值, 调 `platform_create` (lib.rs) 单平台创建。
- **平台 name 无 UNIQUE** (memory `platform-name-not-unique-import`), 但 UI 需可区分 → 批量须生成区分名。
- **创建命令**: `platform_create` (lib.rs, services/api.ts `platformApi.create`), 单平台; 无批量命令。
- **分组关联**: 平台可关联多 group (group_platform 表), 现有表单已有分组选择器。

## 决策 (用户 brainstorm 已定)

1. **入口 (multiSelect)**: ① 智能识别弹窗 SmartPasteModal ② 手动添加表单 apikey 字段。两入口都支持批量。
2. **命名**: `{平台名}-{key 后 4 位}` (如 `DeepSeek-a1b2`), 便于肉眼区分对应哪个 key。撞名时 (同尾 4 位) 追加序号 `-2`。
3. **分组**: N 个平台挂同一分组 (复用现有分组选择器, 含「无分组」)。
4. **创建后**: 默认 `enabled=true`, **不自动 model_test** (避免批量烧 quota/慢; 用户手动测)。

## scope / 交付

### D1: 智能识别弹窗批量 (SmartPasteModal.tsx + Platforms.tsx applyPaste)

- SmartPasteModal: 多 key 时 UI 从单选 radio → **多选 checkbox** (默认全选), 单 key 时保持单选不变 (向后兼容)
- `onApply` payload: `apiKey: string` → `apiKeys: string[]` (单 key 时长度 1 数组, 保持 applyPaste 简单)
- Platforms.tsx `applyPaste`/`handleApplyPaste`: 收到 `apiKeys.length > 1` → 走批量分支:
  - 循环 `platformApi.create` N 次, 每次同 base_url/protocol/endpoints, 不同 api_key + name=`{label}-{key.slice(-4)}`
  - 撞名追号: name 已存在则 `-{key.slice(-4)}-2` (查现有 platforms 列表)
  - 分组关联: 全挂 applyPaste 时选中的分组 (1 个)
  - enabled=true, 不调 model_test
  - 进度反馈: 批量创建中 toast/inline 进度 (N 个串行 invoke, 成功/失败计数)
- 失败处理: 单 key 创建失败 (如 name 冲突未解/DB 错) 不中断整批, 收集失败项, 末尾汇总 toast「成功 X / 失败 Y」+ 失败 key 列表

### D2: 手动添加表单批量 (Platforms.tsx 添加平台表单)

- apikey 字段: 支持**多行 / 逗号 / 空白分隔**粘入多 key → 解析为 `string[]` (复用 platformPaste.ts 的 key 抽取逻辑, 抽成共享 util `extractApiKeys(rawText)`)
- 检测到多 key (>1) → 走批量创建分支 (同 D1 逻辑, 复用)
- 单 key 保持现有单平台创建路径

### D3 (可选, 若 D1/D2 复用充分则不单列): 后端批量命令

- 评估: 前端循环 N 次 `platform_create` vs 新增 `platform_create_batch`。**默认前端循环** (复用现有命令 + 验证逻辑, N 通常 <20, 串行可接受); 若性能/原子性需求再加后端批量。

## 验收

1. 智能识别粘贴含 N key 文案 → 弹窗多选 checkbox → 确认 → 创建 N 个平台, name=`{平台}-{尾4位}`, 同分组, 全启用, 不自动测
2. 手动添加表单 apikey 字段粘入多 key (多行/逗号) → 提交 → 批量创建 N 平台
3. 撞名 (同尾 4 位) 自动追号, 不报错中断
4. 单 key 场景行为不变 (单选/单创建, 向后兼容)
5. 批量失败项汇总展示, 不静默吞
6. `yarn build` + `check-i18n` 全绿; 新增 i18n key 全 7 语言补齐
7. 平台添加走 `aidog-add-platform` skill 约定 (memory `aidog-add-platform-skill`)

## 非目标

- 不改 platformPaste.ts 解析器 (已支持多 key)
- 不改 platform 数据模型 (name 仍无 UNIQUE)
- 不实现批量 model_test (用户手动)
- 不改导入 (.aidogx) / 分享 / deeplink 导入路径 (那些是 D2/D3 of deeplink-share)

## 风险

- 批量 N 次 invoke 串行慢 (N>20 时); 缓解: 进度反馈 + 接受串行 (YAGNI 并发)
- 撞名追号逻辑需查现有 platforms, 若并发创建可能竞态; 缓解: 串行创建内即时更新本地列表
- SmartPasteModal onApply payload 字段变更 (apiKey→apiKeys) 需同步所有调用方 (Platforms/Home/Groups/TrayConfig 若共用)

## 调度

- 槽位: 当前 2/2 (deeplink-share parent + glm-1210 in_progress), 本 task **planning 完成, start 排队** — 等 glm-1210 finish 腾槽
- D1 + D2 可同 task 内顺序执行 (D1 先, D2 复用 D1 批量逻辑); 单 worktree, 单 implement agent 串行 (动态调度退化为轻量模式)
- 复杂度中等 (2 入口 + 共享批量 util + 撞号), subagent 编排足够, 不升级 workflow
