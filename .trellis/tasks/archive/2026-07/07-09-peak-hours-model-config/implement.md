# Implement Plan — 按时段切换模型配置

## subtask 拆分 (顺序，跨 Rust↔前端，单 subagent 保字段名自洽)

### ST1: Rust 数据模型 + parse + 路由
- 文件:
  - src-tauri/src/gateway/models/platform.rs — 加 TimeModelRule struct
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct TimeModelRule {
        pub windows: Vec<PeakWindow>,       // 复用 PeakWindow（仅窗口，multiplier 字段忽略）
        pub models: PlatformModels,          // 5 档（default/opus/sonnet/haiku/gpt）
    }
    ```
    （PeakWindow 若未在 models/platform.rs，从 peak_hours.rs 引或 re-export）
  - src-tauri/src/gateway/peak_hours.rs 或新 time_models.rs — parse_platform_time_models(extra: &str) -> Vec<TimeModelRule>
    - 模式参考 parse_platform_peak_hours (peak_hours.rs:100+)：extra JSON blob → 解析 time_models 字段 → 失败返空 Vec
  - src-tauri/src/gateway/router/selection.rs:61 — resolve_model 前插时段匹配
    ```rust
    let effective_models = resolve_time_models(&platform.extra, &platform.models, now);
    let target_model = if mapping.is_none() {
        resolve_model(&effective_models, source_model)
    } else { target_model };
    ```
    - resolve_time_models(extra, default_models, now): 按当前 hour+weekday first-match time_models → 命中 rule.models → 否则 default_models（复用 peak_hours::hit 逻辑）
    - now 来源：selection.rs 现有上下文（或传 Utc::now）
- 验收: cargo build + cargo clippy 绿；cargo test（router 现有 test + 加 time_models hit/miss test）

### ST2: 前端类型 + state
- 文件:
  - src/services/api/types/part1.ts（或 platforms types）— 加 TimeModelRule TS 类型（windows: PeakWindow[], models: Partial<Record<ModelSlot,string>>）
  - src/pages/platforms/usePlatformForm.ts — state: time_models: TimeModelRule[] + setTimeModels（从 platform.extra 解析 + 写回）
  - extra 读写模式参考 peak_hours（platform.extra JSON blob 解析/序列化）
- 验收: TS 编译过；extra.time_models 读写 round-trip

### ST3: 前端 UI TimeModelsSection
- 文件: src/pages/platforms/formSections.tsx（或新 formSectionsTimeModels.tsx 若体积大）
- 改动:
  - 新 TimeModelsSection（props: rules/setRules/protocol/tzMode/t）
  - 列表式：每项 = PeakWindow editor（复用 PeakHoursSection 的窗口输入 UI）+ 5 档 models 输入（复用 ModelsSection 档位 input：default/opus/sonnet/haiku/gpt）+ 删除按钮
  - 「添加时段规则」按钮（空 rule：windows=[], models={}）
  - 「从 peak_hours 快捷导入」按钮：读 platform.extra.peak_hours windows → 复制为新 rule.windows（独立，不联动）→ 若已有时确认 modal
  - 拖拽排序（first-match 优先级）— 简单上下移按钮即可
  - 挂载于 PlatformEditForm.tsx（ModelsSection 旁，editing && !isPassthrough）
- 验收: UI 增删改 rule + 排序 + 档位编辑；快捷导入复制 peak_hours windows

### ST4: 确认 modal（删 rule / 快捷导入覆盖）
- 删 rule：直接删（小操作，无 modal）或确认 modal（防误删）
- 快捷导入覆盖：若已有 rule，确认 modal（createPortal）「快捷导入将追加 N 条规则（基于当前 peak_hours），是否继续？」
- modal-window-center-rule：createPortal(document.body)

### ST5: i18n 8 语言
- 8 locale json 加 key（platform. 命名空间）:
  - time_models_section: "时段模型配置" / "Time-based Model Config"
  - time_models_desc: "按时段切换主力模型档；命中窗口用该档，未中用默认" / ...
  - time_models_add: "添加时段规则" / "Add Time Rule"
  - time_models_import_shortcut: "从高峰时段导入" / "Import from Peak Hours"
  - time_models_empty: "未配置 → 全时段用默认模型档" / ...
  - time_models_move_up / move_down / delete
  - time_models_import_confirm_body: "将基于当前高峰时段追加 {{count}} 条规则，是否继续？" / ...
- 验收: check-i18n 0 缺失

## 验证
- cd src-tauri && cargo build && cargo clippy && cargo test（router + 新 time_models test）
- yarn build 绿
- node scripts/check-i18n.mjs 0 缺失
- 手动: 配 time_models rule（peak 时段 sonnet 档某模型）→ 时段内请求验证上游收 sonnet 档模型；时段外收 default

## 失败处理
- PeakWindow 类型跨 Rust↔TS 字段名 drift → cross-layer-rules.md guide；resolve_time_models 复用 peak_hours::hit 不重写
- extra JSON blob 解析失败 → 返空 Vec（退化 default models，禁 panic）
- cargo test router 现有 test 因 effective_models 改动失败 → time_models 空时 effective_models = platform.models（行为不变），保 round-trip
- 字段名: TS windows/models 对齐 Rust windows/models（serde 默认 snake_case，TS 同）

## 资源
- spec: cross-layer-rules.md / code-reuse-rules.md / frontend/conventions.md / backend/db-conventions.md / platform-lifecycle.md
- memory: modal-window-center-rule
- prd.md 决策（复制独立 / first-match / 5档）
