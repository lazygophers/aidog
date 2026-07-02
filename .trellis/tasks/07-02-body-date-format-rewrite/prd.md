# PRD — body 日期格式改写防检测（CLI 集成开关 → middleware 规则）

## 现象（用户 2026-07-02）
- Claude Code system prompt 注入当前日期 `Today's date is 2026/07/02`（斜杠格式）
- 斜杠 `YYYY/MM/DD` 是中文区惯用格式，易被上游针对性检测识别为中文用户 → 封禁风险
- DB 实证：28732 条 body 含斜杠日期（全含日期+时间），URL/path 含日期 0 条（无 URL 误伤）

## 决策锁（2026-07-02 grill + 用户裁定）
| # | 决策 | 锁定 |
|---|---|---|
| 1 | 默认值 | **默认开**（enabled=1，种子幂等尊重用户后续禁用） |
| 2 | 应用范围 | 所有出站 body（middleware global scope，入站挂载转发前） |
| 3 | UI 开关归属 | **CLI 集成 tab**（CodingToolsSettings.tsx 加第三开关） |
| 4 | 实现架构 | **A' — 复用 middleware 引擎**：内置 redaction 规则 + CLI 开关切 enabled |

> 用户明确：开关在 CLI 集成 tab，但实际作用于 middleware 配置（非 forward.rs 独立改写）。

## 目标
内置一条 middleware redaction 规则：regex `(\d{4})/(\d{1,2})/(\d{1,2})` → replacement `$1-$2-$3`，将 body 中斜杠日期改 ISO 横杠。CLI 集成 tab 加开关镜像该规则 enabled。

## 交付
1. **后端** `src-tauri/src/gateway/db/schema.rs`：`builtin_rule_specs()` 加一条 spec
   - name: `"内置·日期格式改写防检测"`
   - rule_type: `redaction`, match_type: `regex`, action: `mask`
   - pattern: `r"(\d{4})/(\d{1,2})/(\d{1,2})"`
   - config: `r#"{"replacement":"$1-$2-$3","fields":["messages","system"]}"#`
   - priority: 20
   - 种子 enabled=1（默认开），幂等（name+is_builtin=1）
2. **后端便捷 command**（按 name 读/切内置规则 enabled）或前端直接 middlewareApi（list filter name + update enabled）— exec subagent 定
3. **前端** `src/components/settings/CodingToolsSettings.tsx`：加第三开关「日期格式改写防检测」，onChange 切规则 enabled
4. **i18n** 8 locale：新 key（开关 label + description）

## 改写机制（复用现有引擎，零新改写逻辑）
- middleware 引擎 inbound（handler.rs:293 group + forward.rs:140 platform 层）转发前跑
- redaction action=mask + regex → `re.replace_all(s, "$1-$2-$3")`（mod.rs:265，Rust regex capture 支持）
- fields messages+system 覆盖 Claude Code system prompt 日期

## 验收
1. 开关开时，DB `upstream_request_body` 实证 `2026/07/02` → `2026-07-02`（时间冒号不动）
2. 开关关时，原样透传
3. middleware tab 可见该内置规则（is_builtin=1，可禁用不可硬删）
4. 8 locale 新 key 全覆盖（check:i18n 绿）
5. cargo test + cargo clippy + yarn build 全绿

## 非目标
- 不改响应 body（仅请求出站）
- 不动 forward.rs:244 改写段（走 middleware，非独立改写）
- 不改 Rectifier 枚举（未实现，用 redaction）

## 风险
- regex capture `$1-$2-$3` 在 Rust regex crate 已验证支持
- 内置规则与用户自定义 redaction 规则共存（priority 区分，不冲突）
- slot 满（arch + recurring in_progress），start 需排队等 slot 空

## 阶段
1. planning（本步，grill 校对）
2. exec（subagent：后端 spec + 前端开关 + i18n）
3. check（cargo test/clippy + yarn build + check:i18n）
4. finish
