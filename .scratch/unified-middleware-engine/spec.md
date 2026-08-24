# Spec — 统一中间件规则引擎（Condition Tree + Action Chain）

**Triage label:** `ready-for-agent`

**Decisions of record:** `docs/adr/0003-unified-middleware-rule-engine.md`. Vocabulary:
`CONTEXT.md` (Middleware 词条: Middleware Rule / Condition Tree / Action Chain / Applies To /
Builtin Rule / Failed Rule). 5 轮盘问共识（2026-08-24，Ask UI session ask-ui-20260824072306-ac0e）。

## Problem Statement

用户面对 8 类中间件规则（request_filter / sensitive_word / redaction / content_filter /
dynamic_injection / response_override / rectifier / error_rule），每类一套 type-specific config、
一个子开关、固定的执行顺序和隐藏兜底行为（content_filter 空 pattern 偷偷用内置检测器、
error_rule 空 pattern 偷偷匹配任意非 2xx）。想表达「正则匹配某字段，命中后 block/改写/脱敏/
注入/错误分类的组合」必须跨多条不同类型的规则拼，且三级就近覆盖让 platform 规则会静默吞掉
group/global 规则。同时密钥/邮箱检测器等逻辑写死在引擎里，用户看不到也关不掉；流式响应的
proxy_log 只存一个 `[stream]` 占位，排查断流问题时什么都看不到。

## Solution

一条 Middleware Rule = 嵌套 Condition Tree（ALL/ANY 递归布尔组合，叶子 target + field +
match_type + pattern）+ 有序 Action Chain（mask / block / warn / inject / override / classify，
block 与 classify 终止本链及后续所有规则）+ Applies To 过滤器（platforms / groups / models
数组，空 = 全部，规则按 priority 累加执行）。中间件只影响发往上游的请求和回给下游的 body，
不影响日志记录。内置检测器（AI token / 邮箱 / 手机号 / DB-Redis 密钥）迁为 Builtin Rule——
只可启停、不可编辑删除、升级按 name 强制覆盖内容并保留用户停用态。「一键导入默认」前后端
入口移除。流式响应聚合完整 SSE 文本落 proxy_log，废 `[stream]` 占位，断流也落已聚合部分，
终态判定改显式 done 标记列。

## User Stories

1. As a proxy user, I want one rule to match any target (request/response body/headers、status、
   model) with contains/regex/exact, so that I don't need to pick a rule type before I can express
   the match.
2. As a proxy user, I want nested ALL/ANY condition groups, so that I can express boolean
   combinations like "(body contains X AND model is Y) OR header has Z".
3. As a proxy user, I want an ordered action chain per rule, so that one hit can mask and then warn
   without maintaining two coordinated rules.
4. As a proxy user, I want block and classify to stop everything (chain + later rules), so that
   post-block rewrites never happen.
5. As a proxy user, I want applies_to filters on platforms / groups / models with empty = all and
   any-of matching, so that rule scope is explicit and rules stack predictably by priority.
6. As a proxy user, I want every condition explicit (no empty-pattern fallback), so that a rule is
   self-documenting with no hidden behavior.
7. As a proxy user, I want validation to reject rules mixing request-side and response-side leaves,
   so that I can't build a rule whose semantics span phases.
8. As a proxy user, I want error handling expressed as a classify action in the chain
   (category / retryable / override_status / override_body), so that error classification uses the
   same rule model as everything else.
9. As a proxy user, I want a recursive group-card editor as the primary way to build the tree, so
   that I can compose conditions visually.
10. As a power user, I want a DSL source mode alongside the group cards, so that I can type complex
    trees fast; the tree JSON stays the single source of truth and unparseable DSL blocks
    save/switch-back.
11. As a proxy user, I want a single master switch plus per-rule enabled, so that the old 8 type
    sub-switches' redundancy is gone.
12. As a proxy user, I want builtin secret detection (AI tokens, emails, phone numbers, DB/Redis
    credentials) shipped as Builtin Rules I can toggle, so that hardcoded detectors become visible
    and controllable.
13. As a proxy user, I want Builtin Rules to be non-editable and non-deletable, so that upgrades
    can't desync from what the engine expects and I can't accidentally break them.
14. As a proxy user, I want upgrades to overwrite Builtin Rule content by name while keeping my
    disabled state, so that pattern updates arrive without resurrecting rules I turned off.
15. As a proxy user upgrading from the old model, I want untranslatable old rules shown as Failed
    Rules with guidance to delete them, so that migration needs no silent lossy conversion.
16. As a proxy user, I want builtin credential patterns to match only unambiguous forms (mainland +
    explicit international phone formats; connection-string URIs and explicit key=value secrets),
    so that risky high-false-positive patterns are never shipped.
17. As a proxy user, I want the import-defaults entry point gone from both UI and backend, so that
    there is exactly one way builtin rules come to exist (seed).
18. As a proxy user debugging a streaming request, I want the full aggregated SSE text stored in
    proxy_log, so that I can see exactly what the upstream sent.
19. As a proxy user debugging an interrupted stream, I want the already-aggregated partial text
    stored too, so that I can see where the stream broke.
20. As a proxy user, I want stream completion tracked by an explicit done flag, so that retention
    and terminal-state logic no longer depends on a magic `[stream]` placeholder.
21. As a proxy user, I want streaming SSE behavior well-defined per action: mask/override applied
    per chunk, block effective only before the first forwarded chunk, error/inject/warn disabled
    with a log line, so that no action has undefined streaming semantics.
22. As a proxy user, I want middleware to never affect what gets logged, so that log records reflect
    the actual traffic regardless of any rule.

## Implementation Decisions

- **Rule model** (replaces RuleType / RuleScope / MatchType+pattern / single action / config):
  - `conditions`: nested tree JSON — group nodes `{connector: "all"|"any", children: [...]}`,
    leaf nodes `{target, field, match_type, pattern}`. Targets: request_body / request_headers /
    response_body / response_headers / status / model. Field is a JSON path for body targets, a
    header name for header targets.
  - `actions`: ordered array JSON; each action `{kind, params}` from mask / block / warn / inject /
    override / classify. classify carries `{category, retryable, override_status, override_body}`;
    retryable=false feeds the existing retry orchestration (immediate return, no candidate switch).
  - `applies_to`: `{platforms: [], groups: [], models: []}` filter JSON; empty = all, multi-value
    any-of; rules stack by priority ascending. The three-level cascade (platform overrides group
    overrides global, non-additive) is abolished.
  - Phase (inbound vs outbound execution point) is derived from targets; one rule's leaves must all
    be same-phase, validated at save.
- **DB**: rebuild `middleware_rule` table (no compat migration). Old rows that can't be translated
  are marked as Failed Rule (visible, deletable, not executed). Old columns rule_type / scope /
  scope_ref / match_type / pattern / action / config removed; conditions / actions / applies_to JSON
  columns added; is_builtin / priority / enabled / created_at / updated_at retained.
- **Builtin rules**: seeded on startup by name upsert — content force-overwritten, user's disabled
  state preserved; CRUD API rejects edit/delete for builtin, toggle only. Seed set: AI token
  (sk-/ghp_/AKIA/AIza/xox…), email, phone (mainland + explicit international forms),
  DB/Redis credential (connection-string URI + explicit key=value forms); each only unambiguous
  patterns. The hardcoded detectors in the engine (`BUILTIN_SECRET_PATTERN` / email) are removed.
- **Settings**: `MiddlewareSettings` drops the 8 type_enabled sub-switches; keep master `enabled`
  only.
- **Engine** (aidog_middleware): single evaluation path — resolve applicable rules by applies_to,
  sort by priority, evaluate tree per phase, execute action chains; block/classify terminate
  everything. `classify_error` independent path folded into classify action. Empty-pattern fallback
  semantics removed. Streaming per-action degradation table: mask/override per chunk (cross-chunk
  boundary miss remains a known limitation, sliding window later), block only before first forwarded
  chunk, error/inject/warn disabled with log. ReDoS guards (size/dfa limits, fail-open on compile
  failure) retained.
- **Error contract**: ts-rs generated types regenerated for the new rule shape; the 8 RuleType and
  RuleScope/MatchType/RuleAction type files are replaced. `classify_error`'s ErrorClassification
  remains the handoff type to retry orchestration.
- **UI**: MiddlewareRules tab rebuilt — recursive condition-group cards (primary) + DSL source mode
  (view over tree JSON, parse errors block save/switch); action-chain editor (ordered, drag or
  up/down); applies_to multi-select for platforms/groups/models; builtin rows show toggle-only;
  Failed Rules shown with delete guidance; import-defaults button and its backend command removed.
- **Streaming log** (in scope): aggregate full SSE text via existing StreamAggregator and write back
  to proxy_log response body at stream end; interrupted streams store the partial aggregate; `[stream]`
  placeholder and its retention/terminal-state special-case removed; explicit done flag column set
  at write-back, used by retention/strip logic.
- **Middleware never touches log content** — rules apply to upstream request and downstream body only.

## Testing Decisions

Good tests assert external behavior through existing seams (confirmed with user, 2026-08-24):

- **Engine layer (primary seam)**: `MiddlewareEngine` public functions fed rules + ChatRequest /
  body / chunk, asserting rewritten output, blocked outcome, classification result. Prior art:
  aidog_middleware test_mod / test_inbound / test_outbound. Cover: tree evaluation (nesting, ALL/ANY,
  phase rejection), action-chain order and terminal semantics, applies_to stacking by priority,
  streaming degradation table, ReDoS fail-open.
- **DB/seed layer**: prior art — aidog_db tests. Cover: table rebuild, seed upsert preserving
  disabled state, builtin edit/delete rejection, Failed Rule marking.
- **UI layer**: prior art — 6 existing component tests. Cover: tree card editor render/submit, DSL
  parse-error blocking, toggle-only builtin rows, Failed Rule presentation.
- **Streaming log layer**: prior art — proxy stream tests. Cover: full aggregation write-back,
  partial write on interruption, done flag set, retention using done flag.

Tests only assert observable outcomes (body text, status, blocked flags, DB rows), never internal
bucket structure.

## Out of Scope

- Sliding-window cross-chunk matching for streaming rewrites (known limitation, later work).
- Old-rule automatic translation beyond Failed Rule marking (user deletes manually).
- proxy_log `[REDACTED]` header sanitization (stays a log-layer security baseline, not a rule).
- strip_redacted_thinking_blocks (forward-layer behavior, not middleware).
- rectifier's SSE format fixing as a dedicated concept (expressible via override actions if needed).
- Circuit-breaker behavior (owned by group scheduling, untouched).

## Further Notes

- Full interview record: 5 rounds, 25 questions, Ask UI session `ask-ui-20260824072306-ac0e`
  (2026-08-24), summarized in ADR 0003.
- The DSL is a frontend-only view; no DSL parser exists in Rust — the engine consumes tree JSON via
  serde.
- Builtin credential patterns deliberately exclude ambiguous forms (bare `password=` without URI
  context, loose international phone ranges) per user instruction "只有明确的才可以处理，有风险的不要匹配".
