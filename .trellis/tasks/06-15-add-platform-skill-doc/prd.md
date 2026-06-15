# PRD — child B: 编写 aidog-add-platform 领域 skill

> parent: `06-15-aidog-add-platform-skill`

## 目标

产出项目级领域 skill `.claude/skills/aidog-add-platform/`，让 AI/新人加或改一个 aidog 平台时，不必重读源码就知道改哪几处、什么顺序、怎么验证，并避开调研发现的反直觉陷阱。

## 落点 + 结构

```
.claude/skills/aidog-add-platform/
├── SKILL.md                    # 主文件：触发词 + 两路径主流程 + 决策树 + 验证门禁
└── references/
    ├── touchpoints-map.md      # 全文件触点地图（file:line，加平台/改平台/加协议）
    ├── quota-coding-plan.md    # 余额 + coding plan 查询子流程模板（照着改）
    └── default-model.md        # 默认模型预设节（依赖 child A 落地后定稿）
```

## SKILL.md 必含内容（对照 3 份 research）

### frontmatter
- `name: aidog-add-platform`
- `description`: 前置触发词「加平台/新增平台/改平台默认配置/平台 base_url/获取余额/coding plan 配额/默认模型/Protocol 枚举」+ 场景描述
- `paths: [src/pages/Platforms.tsx, src-tauri/src/gateway/**]`

### 核心认知纠偏（开篇必写，research 最大价值）
1. **平台预设住前端** `Platforms.tsx`，db.rs 无 seed（research/01 行 9-10、202）
2. **Protocol 枚举 Rust↔TS 必须逐字双写**，无容错，失配整体解析失败（research/03 第 3 点、research/01 行 18、34-36）
3. **glm/kimi/minimax/bailian/codex adapter 是死代码**，别去改（research/01 行 141-144、203）
4. quota coding plan 按 **base_url 子串** dispatch，非 Protocol（research/01 行 204、research/02）

### 路径 1：纯 OpenAI/Anthropic 兼容平台（90% 场景，6 处）
对照 research/01 §5 表格 + research/03 第 4 点：
1. models.rs Protocol 加 `#[serde(rename)] 变体`
2. api.ts Protocol 联合加 `| "xxx"`
3. Platforms.tsx PROTOCOLS 加选项
4. Platforms.tsx getDefaultEndpoints 加端点（base_url 含版本前缀）
5. Platforms.tsx 显示名 map
6. Platforms.tsx 颜色 map
+ 默认模型预设（child A 新增的 getDefaultModels）— **default-model.md 节**

### 路径 2：加新 wire 协议（重活）
对照 research/01 行 157-160、186-192 + research/03 「加新协议」：
- 新建 adapter/xxx.rs + converter.rs convert_request/parse_sse/parse_incoming_request match + adapter/mod.rs 注册
- 特殊鉴权头（proxy.rs:2302-2320）
- coding_plan 注入（proxy.rs:2553-2579）

### 子流程模板（references）
- **余额查询**：照 `query_deepseek_balance`（quota.rs:138）骨架 → references/quota-coding-plan.md（research/02 第 1 点）
- **coding plan 配额**：照 `query_kimi_coding_plan`（quota.rs:243），tier name 须 ∈ cycle_ms_for_tier 集合（research/02 第 2 点 caveat）
- **价格**：resolve_price 回退链，无需改代码只填 model_price.price_data（research/02 第 3 点）
- **默认模型**：child A 的 getDefaultModels 用法（research/01 §默认模型 + child A 实现）

### URL 构造铁律（强调）
base_url 含版本前缀 + provider_api_path 只返 /chat/completions，禁额外拼接（CLAUDE.md 既有约束 + research/01 行 30）

### client_type 陷阱
协议≠身份；coding plan 上游有身份白名单（如 Kimi coding 只接 claude_code 拒 codex）（research/03 第 2 点）

### 验证门禁
- `yarn build` / `yarn check:i18n`（若动 i18n）
- `cargo build` / `cargo clippy`（零 warning）/ `cargo test`（若动后端）
- model_test 与 proxy parity（research/03 第 1 点：inject 两处调用须同步）

## 验收标准

- skill 自包含（复制目录可独立用，references 不依赖外部）
- 两路径触点与 3 份 research 的 file:line 一致（抽查 ≥5 处）
- 默认模型节与 child A 实际实现一致（A 完成后核对字段/函数名）
- frontmatter description 触发词前置（参照 CLAUDE.md skill 可发现性规则）
- 不引导改死代码 adapter；不写「预设在 db.rs」等已证伪的内容

## 依赖 / 资源

- research/01、02、03（parent 目录，jsonl 引用 `../06-15-aidog-add-platform-skill/research/*`）
- **依赖 child A**：默认模型节（default-model.md + SKILL 内引用）须等 child A 落地后按真实 `getDefaultModels` 字段/函数名定稿
- 参照现有 `.claude/skills/aidog-*` skill 风格保持一致
