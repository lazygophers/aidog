# PeakWindow starts_at/expires_at → start_at/end_at 重命名

## Goal

PeakWindow（高峰时段窗口）的生效期字段 `starts_at`/`expires_at` 重命名为 `start_at`/`end_at`。跨 Rust ↔ TS ↔ JSON ↔ i18n 全栈对称重命名，硬切不兼容（无 serde alias）。

**为什么**：用户要求字段名统一为 `start_at`/`end_at`（与 `start_hour`/`start_minute` 命名一致，去 `s`/`expires` 不规则形态）。

## 范围边界（关键，禁误伤）

### ✅ 改（PeakWindow 上的字段）
判定规则：`.starts_at` **全改**（仅 PeakWindow 有此字段）；`.expires_at` **看 host 类型**——PeakWindow 上的改，Platform 上的**不动**。

- **Rust**：`src-tauri/src/gateway/peak_hours.rs`（struct 字段 + 解析逻辑 + helper fn 签名 + 全部测试 fixture + 注释）、`src-tauri/src/gateway/time_models.rs:96`
- **TS 类型**：`src/domains/platforms/defaults.ts`（PeakWindow type 字段 + 注释里的 Rust 引用）
- **TS 消费**：`src/utils/peakHours.ts:31-32`、`src/pages/platforms/formSections.tsx:677,690`（`w.starts_at`/`w.expires_at`，w = PeakWindow）
- **JSON**：`src-tauri/defaults/platform-presets.json:383`（GLM peak_hours starts_at key）
- **i18n key**：8 语言文件 `platform.peak_hours_starts_at` → `platform.peak_hours_start_at`、`platform.peak_hours_expires_at` → `platform.peak_hours_end_at`；`src/pages/platforms/formSections.tsx:672,685` t() 调用 key 同步

### ⛔ 不动（Platform 过期时间，DB 列 + 业务字段，名字巧合相同）
- `src/components/platforms/PlatformCard.tsx`：`p.expires_at`（p = Platform，过期时间）
- `src/pages/platforms/usePlatformForm.ts`：`p.expires_at`（p = Platform）
- Rust DB schema_late.rs / platform.rs / platform_lifecycle.rs 等 Platform 的 `expires_at` 列
- import_export / proxy 测试里 Platform.expires_at

**区分技巧**：变量是 PeakWindow（`w`/`window`/`peakWindow`/helper）→ 改；变量是 Platform（`p`/`platform`）→ 不动。

## Requirements

### R1 Rust 重命名
- R1.1 `PeakWindow` struct（peak_hours.rs:40,45）：`pub starts_at: Option<i64>` → `pub start_at: Option<i64>`；`pub expires_at: Option<i64>` → `pub end_at: Option<i64>`。
- R1.2 serde 默认序列化 → JSON key 自动变 `start_at`/`end_at`（无 rename，无 alias，硬切）。
- R1.3 全部 `w.starts_at`/`w.expires_at` 字段访问改新名（peak_hours.rs 解析 + 测试断言）。
- R1.4 helper fn `make_window(...)` 签名参数 `starts_at`/`expires_at` → `start_at`/`end_at`（peak_hours.rs:341 附近）。
- R1.5 `time_models.rs:96` 的 `starts_at: None, expires_at: None` → `start_at: None, end_at: None`。
- R1.6 注释 / doc comment / 测试函数名里 `starts_at`/`expires_at` 文本全改新名（如 `peak_window_starts_at_expires_at_parse` → `peak_window_start_at_end_at_parse`）。注释里对 TS `PeakWindow.starts_at` 的引用也改。

### R2 TS 重命名
- R2.1 `src/domains/platforms/defaults.ts`：PeakWindow type `starts_at?: number` → `start_at?: number`；`expires_at?: number` → `end_at?: number`。注释里 Rust 引用同步。
- R2.2 `src/utils/peakHours.ts:31-32`：`w.starts_at`/`w.expires_at` → `w.start_at`/`w.end_at`。
- R2.3 `src/pages/platforms/formSections.tsx:677,690`：`w.starts_at`/`w.expires_at` → `w.start_at`/`w.end_at`（w 是 PeakWindow，**不是** Platform）。

### R3 JSON
- R3.1 `src-tauri/defaults/platform-presets.json:383`：`"starts_at":` → `"start_at":`（GLM peak_hours 唯一处）。`last_updated` 不动。

### R4 i18n key 重命名
- R4.1 8 语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）：`platform.peak_hours_starts_at` → `platform.peak_hours_start_at`；`platform.peak_hours_expires_at` → `platform.peak_hours_end_at`（值/翻译文案不变，仅 key 名改）。
- R4.2 `src/pages/platforms/formSections.tsx:672,685` t() 调用 key 同步。

### R5 验证
- R5.1 `cargo test`（重点 peak_hours tests 全过，测试 fixture 已改新名）。
- R5.2 `cargo clippy` 无新 warning。
- R5.3 `npx tsc --noEmit` 无错。
- R5.4 `yarn check:i18n`（若存在）或 `node scripts/check-i18n.mjs` 无错（i18n key 跨 8 语言对齐）。
- R5.5 grep 残留：`grep -rn "starts_at" src-tauri/src src` 应仅剩 Platform 的（无，因 Platform 无 starts_at）→ 应为 0；`grep -rn "expires_at"` 剩 Platform 的（PlatformCard/usePlatformForm/DB schema）。

## Acceptance Criteria

- [ ] Rust struct + 全消费点 + 测试 fixture 改 `start_at`/`end_at`
- [ ] TS type + 消费点改 `start_at`/`end_at`
- [ ] JSON key 改 `start_at`
- [ ] i18n key 8 语言 + t() 调用改 `peak_hours_start_at`/`peak_hours_end_at`
- [ ] Platform 的 `expires_at` 零改动（grep 验证 PlatformCard/usePlatformForm/DB 不变）
- [ ] `cargo test` exit 0；`cargo clippy` 无新 warning；`tsc --noEmit` exit 0；check-i18n 无错
- [ ] grep `starts_at` 全仓 = 0；`peak_hours_starts_at`/`peak_hours_expires_at` 全仓 = 0

## Definition of Done

- 全栈对称重命名（Rust serde ↔ TS ↔ JSON ↔ i18n key）
- Platform.expires_at 零误伤（边界守住）
- 全部门禁绿（cargo test/clippy + tsc + check-i18n）
- journal 记录边界判定技巧（PeakWindow vs Platform 同名字段区分）

## Technical Approach

机械重命名，跨层 guide 驱动：
1. Rust struct 改字段名 → serde 默认序列化自动跟（无手动 rename）→ cargo build 驱动找出所有字段访问编译错误 → 逐一改
2. 测试 fixture + 断言改新名（build/test 会报）→ 注释文本改（grep `starts_at\|expires_at` 在 peak_hours.rs 逐行）
3. TS type 改 → tsc 驱动找出 w.starts_at 访问错误 → 逐一改（注意 w vs p 区分）
4. JSON key 单点改
5. i18n key 全 8 语言（脚本辅助 sed 或逐文件 Edit）+ t() 调用点

## Decision (ADR-lite)

**Context**：用户要求字段名统一。
**Decision**：
1. 硬切不兼容（无 serde alias）—— starts_at/expires_at 是 07-09 新功能，影响面小。
2. i18n key 一起改 —— 跨层一致性（cross-layer-rules）。
3. 边界判定看 host 类型（PeakWindow vs Platform），同名 expires_at 不混淆。
**Consequences**：
- 旧 app data 若手存了 starts_at/expires_at 的 platform.extra.peak_hours → 硬切后丢字段（窗口失效/立即启用语义丢失）。可接受（新功能，用户量小）。
- bundled JSON 改 start_at → 旧版二进制读新版 JSON 会丢字段（向前不兼容，但发版同步）。

## Out of Scope

- Platform.expires_at（DB 列 + 业务字段，不动）
- 其他协议的 peak_hours 数据补齐
- 数据迁移脚本（硬切，不迁旧数据）

## Technical Notes

- 跨层 guide：`.trellis/spec/guides/cross-layer-rules.md`（Rust serde ↔ TS 字段对称）
- i18n guide：`.trellis/spec/frontend/locale-tag-cross-layer.md`（8 语言 key 对齐）
- 边界判定：`.starts_at` 全改（仅 PeakWindow 有）；`.expires_at` 看 host（w=PeakWindow 改，p=Platform 不动）
- Platform.expires_at 出现点（**参考禁改**）：PlatformCard.tsx:340-370、usePlatformForm.ts:331-403、Rust db/schema_late.rs/platform.rs/platform_lifecycle.rs
