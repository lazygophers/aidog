// ─── Middleware Rule Engine UI ──────────────────────────────
// 中间件规则引擎前端 UI。复用于 AppSettings 中间件 tab（scope=global）
// 与 group / platform 编辑页内嵌（scope=group / platform）。
// 消费 C1 冻结的 services/api.ts 契约，只读不改。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  middlewareApi,
  type MiddlewareRule,
  type MiddlewareSettings,
  type CreateMiddlewareRule,
  type RuleType,
  type RuleScope,
  type MatchType,
  type RuleAction,
} from "../../services/api";
import { F, S } from "./editors";
import { IconClose, IconEdit } from "../icons";

// ── 静态枚举常量（与 api.ts 契约对齐，禁裸 string）──

const RULE_TYPES: RuleType[] = [
  "request_filter",
  "sensitive_word",
  "redaction",
  "content_filter",
  "dynamic_injection",
  "response_override",
  "rectifier",
  "error_rule",
];

const MATCH_TYPES: MatchType[] = ["contains", "regex", "exact"];

const RULE_ACTIONS: RuleAction[] = [
  "mask",
  "block",
  "warn",
  "inject",
  "override",
  "classify",
];

/** 每个 rule_type 默认动作（建表/默认 action 参照 design.md）。 */
const DEFAULT_ACTION: Record<RuleType, RuleAction> = {
  request_filter: "block",
  sensitive_word: "block",
  redaction: "mask",
  content_filter: "warn",
  dynamic_injection: "inject",
  response_override: "override",
  rectifier: "override",
  error_rule: "classify",
};

// ── 标签翻译 helper（key 缺失时回退默认文案）──

function ruleTypeLabel(t: TFunction, rt: RuleType): string {
  const map: Record<RuleType, string> = {
    request_filter: t("middleware.type.request_filter", "请求过滤"),
    sensitive_word: t("middleware.type.sensitive_word", "敏感词"),
    redaction: t("middleware.type.redaction", "脱敏"),
    content_filter: t("middleware.type.content_filter", "内容过滤"),
    dynamic_injection: t("middleware.type.dynamic_injection", "动态注入"),
    response_override: t("middleware.type.response_override", "响应改写"),
    rectifier: t("middleware.type.rectifier", "纠正器"),
    error_rule: t("middleware.type.error_rule", "错误规则"),
  };
  return map[rt];
}

function matchTypeLabel(t: TFunction, mt: MatchType): string {
  const map: Record<MatchType, string> = {
    contains: t("middleware.match.contains", "包含"),
    regex: t("middleware.match.regex", "正则"),
    exact: t("middleware.match.exact", "精确"),
  };
  return map[mt];
}

function actionLabel(t: TFunction, a: RuleAction): string {
  const map: Record<RuleAction, string> = {
    mask: t("middleware.action.mask", "脱敏"),
    block: t("middleware.action.block", "拦截"),
    warn: t("middleware.action.warn", "告警"),
    inject: t("middleware.action.inject", "注入"),
    override: t("middleware.action.override", "改写"),
    classify: t("middleware.action.classify", "分类"),
  };
  return map[a];
}

// ── 规则编辑表单 ──

interface RuleFormProps {
  /** 已有规则 → 编辑模式；undefined → 新建模式 */
  rule?: MiddlewareRule;
  /** 固定作用域（group / platform 内嵌时锁定 scope + scope_ref） */
  fixedScope?: RuleScope;
  fixedScopeRef?: string;
  onSave: (draft: CreateMiddlewareRule) => Promise<void>;
  onCancel: () => void;
}

function RuleForm({ rule, fixedScope, fixedScopeRef, onSave, onCancel }: RuleFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(rule?.name ?? "");
  const [description, setDescription] = useState(rule?.description ?? "");
  const [ruleType, setRuleType] = useState<RuleType>(rule?.rule_type ?? "sensitive_word");
  const [matchType, setMatchType] = useState<MatchType>(rule?.match_type ?? "contains");
  const [pattern, setPattern] = useState(rule?.pattern ?? "");
  const [action, setAction] = useState<RuleAction>(rule?.action ?? DEFAULT_ACTION["sensitive_word"]);
  const [config, setConfig] = useState(rule?.config ?? "{}");
  const [priority, setPriority] = useState(rule?.priority ?? 0);
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [configError, setConfigError] = useState("");
  const [saving, setSaving] = useState(false);

  // 切换 rule_type 时（新建模式）联动默认 action
  const handleRuleTypeChange = (rt: RuleType) => {
    setRuleType(rt);
    if (!rule) setAction(DEFAULT_ACTION[rt]);
  };

  const validateConfig = (raw: string): boolean => {
    if (!raw.trim()) return true;
    try {
      JSON.parse(raw);
      setConfigError("");
      return true;
    } catch (e) {
      setConfigError(String(e));
      return false;
    }
  };

  const handleSave = async () => {
    const cfg = config.trim() || "{}";
    if (!validateConfig(cfg)) return;
    setSaving(true);
    try {
      const draft: CreateMiddlewareRule = {
        name,
        description,
        rule_type: ruleType,
        scope: fixedScope ?? "global",
        scope_ref: fixedScope ? fixedScopeRef ?? "" : "",
        match_type: matchType,
        pattern,
        action,
        config: cfg,
        priority,
        enabled,
      };
      await onSave(draft);
    } catch (e) {
      console.error("save middleware rule failed", e);
      setConfigError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const configHint = configHintFor(t, ruleType);

  return (
    <div
      className="glass-surface animate-fade-in"
      style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}
    >
      <div style={{ fontSize: F.label, fontWeight: 600 }}>
        {rule
          ? t("middleware.editRule", "编辑规则")
          : t("middleware.addRule", "新增规则")}
      </div>

      {/* 名称 + 描述 */}
      <input
        className="input"
        style={{ fontSize: F.body }}
        placeholder={t("middleware.name", "规则名称")}
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input
        className="input"
        style={{ fontSize: F.hint }}
        placeholder={t("middleware.description", "描述（可选）")}
        value={description}
        onChange={(e) => setDescription(e.target.value)}
      />

      {/* rule_type / match_type / action */}
      <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
        <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 160px" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
            {t("middleware.ruleType", "规则类型")}
          </span>
          <select
            className="input"
            style={{ fontSize: F.hint }}
            value={ruleType}
            onChange={(e) => handleRuleTypeChange(e.target.value as RuleType)}
          >
            {RULE_TYPES.map((rt) => (
              <option key={rt} value={rt}>
                {ruleTypeLabel(t, rt)}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 120px" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
            {t("middleware.matchType", "匹配方式")}
          </span>
          <select
            className="input"
            style={{ fontSize: F.hint }}
            value={matchType}
            onChange={(e) => setMatchType(e.target.value as MatchType)}
          >
            {MATCH_TYPES.map((mt) => (
              <option key={mt} value={mt}>
                {matchTypeLabel(t, mt)}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 120px" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
            {t("middleware.action", "动作")}
          </span>
          <select
            className="input"
            style={{ fontSize: F.hint }}
            value={action}
            onChange={(e) => setAction(e.target.value as RuleAction)}
          >
            {RULE_ACTIONS.map((a) => (
              <option key={a} value={a}>
                {actionLabel(t, a)}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: "flex", flexDirection: "column", gap: 4, width: 90 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
            {t("middleware.priority", "优先级")}
          </span>
          <input
            className="input"
            type="number"
            style={{ fontSize: F.hint }}
            value={priority}
            onChange={(e) => setPriority(Number(e.target.value) || 0)}
          />
        </label>
      </div>

      {/* pattern */}
      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.pattern", "匹配模式 / 目标")}
        </span>
        <input
          className="input"
          style={{ fontSize: F.hint, fontFamily: '"SF Mono", "Fira Code", monospace' }}
          placeholder={t("middleware.patternHint", "匹配模式 / 目标 path / header 名")}
          value={pattern}
          onChange={(e) => setPattern(e.target.value)}
        />
      </label>

      {/* config JSON */}
      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.config", "配置 (JSON)")}
        </span>
        <textarea
          className="input"
          style={{
            fontFamily: '"SF Mono", "Fira Code", monospace',
            fontSize: 12,
            lineHeight: 1.6,
            minHeight: 90,
            resize: "vertical",
            whiteSpace: "pre",
          }}
          value={config}
          onChange={(e) => {
            setConfig(e.target.value);
            validateConfig(e.target.value);
          }}
          spellCheck={false}
        />
        {configHint && (
          <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {configHint}
          </div>
        )}
        {configError && (
          <div style={{ fontSize: 11, color: "var(--color-danger)", wordBreak: "break-all" }}>
            {configError}
          </div>
        )}
      </label>

      {/* enabled toggle */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.enabled", "启用")}
        </span>
        <div
          className={`toggle ${enabled ? "active" : ""}`}
          onClick={() => setEnabled(!enabled)}
          role="switch"
          aria-checked={enabled}
          tabIndex={0}
        />
      </div>

      {/* actions */}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button className="btn" style={{ fontSize: F.hint }} onClick={onCancel} disabled={saving}>
          {t("action.cancel", "取消")}
        </button>
        <button
          className="btn btn-primary"
          style={{ fontSize: F.hint }}
          onClick={handleSave}
          disabled={saving || !name || !!configError}
        >
          {t("action.save", "保存")}
        </button>
      </div>
    </div>
  );
}

/** 按 rule_type 给 config JSON 形状提示（参照 design.md）。 */
function configHintFor(t: TFunction, rt: RuleType): string {
  switch (rt) {
    case "redaction":
    case "content_filter":
    case "response_override":
      return t(
        "middleware.configHint.redaction",
        '示例: { "replacement": "****", "fields": ["messages","system"] }',
      );
    case "dynamic_injection":
      return t(
        "middleware.configHint.dynamic_injection",
        '示例: { "inject_mode": "system_append", "target": "", "value": "..." }',
      );
    case "error_rule":
      return t(
        "middleware.configHint.error_rule",
        '示例: { "category": "prompt_limit", "override_status": 400, "retryable": false }',
      );
    case "rectifier":
      return t(
        "middleware.configHint.rectifier",
        '示例: { "fix": "sse", "target": "", "default": null }',
      );
    case "request_filter":
      return t(
        "middleware.configHint.request_filter",
        '示例: { "field": "model", "op": "reject", "value": "..." }',
      );
    case "sensitive_word":
      return t(
        "middleware.configHint.sensitive_word",
        "敏感词规则 config 可留空 {}，pattern 即词。",
      );
    default:
      return "";
  }
}

// ── 单条规则行 ──

interface RuleRowProps {
  rule: MiddlewareRule;
  onEdit: (rule: MiddlewareRule) => void;
  onToggle: (rule: MiddlewareRule) => void;
  onDelete: (id: number) => void;
}

function RuleRow({ rule, onEdit, onToggle, onDelete }: RuleRowProps) {
  const { t } = useTranslation();
  return (
    <div
      style={{
        display: "flex",
        gap: 10,
        alignItems: "center",
        padding: "10px 14px",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-glass)",
        border: "1px solid var(--border)",
        opacity: rule.enabled ? 1 : 0.55,
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontSize: F.hint, fontWeight: 600 }}>{rule.name}</span>
          {rule.is_builtin && (
            <span className="badge badge-accent" style={{ fontSize: 10 }}>
              {t("middleware.builtin", "内置")}
            </span>
          )}
          <span className="badge" style={{ fontSize: 10 }}>
            {ruleTypeLabel(t, rule.rule_type)}
          </span>
          <span className="badge" style={{ fontSize: 10 }}>
            {actionLabel(t, rule.action)}
          </span>
        </div>
        {rule.pattern && (
          <div
            style={{
              fontSize: 11,
              color: "var(--text-tertiary)",
              marginTop: 3,
              fontFamily: '"SF Mono", "Fira Code", monospace',
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {matchTypeLabel(t, rule.match_type)}: {rule.pattern}
          </div>
        )}
      </div>

      {/* enable toggle */}
      <div
        className={`toggle ${rule.enabled ? "active" : ""}`}
        onClick={() => onToggle(rule)}
        role="switch"
        aria-checked={rule.enabled}
        tabIndex={0}
        title={t("middleware.enabled", "启用")}
      />

      {/* 内置规则禁删，仅可禁用（与 C4 约定）；非内置可编辑+删除 */}
      <button
        className="btn btn-ghost btn-icon"
        onClick={() => onEdit(rule)}
        title={t("action.edit", "编辑")}
      >
        <IconEdit size={14} />
      </button>
      {!rule.is_builtin && (
        <button
          className="btn btn-ghost btn-icon"
          onClick={() => onDelete(rule.id)}
          title={t("action.delete", "删除")}
          style={{ color: "var(--text-tertiary)" }}
        >
          <IconClose size={14} />
        </button>
      )}
    </div>
  );
}

// ── 作用域规则面板（可复用：global / group / platform）──

export interface MiddlewareRulesPanelProps {
  /** 作用域：global（中间件 tab）/ group / platform（内嵌编辑页） */
  scope: RuleScope;
  /** group_key 或 platform_id 字符串；global 时空 */
  scopeRef?: string;
  /** 内嵌（group/platform）时隐藏总开关相关说明，仅展示该作用域规则 */
  embedded?: boolean;
}

export function MiddlewareRulesPanel({ scope, scopeRef = "", embedded = false }: MiddlewareRulesPanelProps) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<MiddlewareRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [editingRule, setEditingRule] = useState<MiddlewareRule | undefined>(undefined);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [importing, setImporting] = useState(false);

  const matchesScope = useCallback(
    (r: MiddlewareRule) => r.scope === scope && (scope === "global" || r.scope_ref === scopeRef),
    [scope, scopeRef],
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const all = await middlewareApi.listRules();
      setRules((all || []).filter(matchesScope));
    } catch (e) {
      console.error("list middleware rules failed", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [matchesScope]);

  useEffect(() => {
    load();
  }, [load]);

  const handleSave = async (draft: CreateMiddlewareRule) => {
    if (editingRule) {
      await middlewareApi.updateRule({ ...draft, id: editingRule.id });
    } else {
      await middlewareApi.createRule(draft);
    }
    setShowForm(false);
    setEditingRule(undefined);
    await load();
  };

  const handleToggle = async (rule: MiddlewareRule) => {
    try {
      await middlewareApi.updateRule({
        id: rule.id,
        name: rule.name,
        description: rule.description,
        rule_type: rule.rule_type,
        scope: rule.scope,
        scope_ref: rule.scope_ref,
        match_type: rule.match_type,
        pattern: rule.pattern,
        action: rule.action,
        config: rule.config,
        priority: rule.priority,
        enabled: !rule.enabled,
        is_builtin: rule.is_builtin,
      });
      await load();
    } catch (e) {
      console.error("toggle middleware rule failed", e);
      setError(String(e));
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await middlewareApi.deleteRule(id);
      await load();
    } catch (e) {
      console.error("delete middleware rule failed", e);
      setError(String(e));
    }
  };

  const openEdit = (rule: MiddlewareRule) => {
    setEditingRule(rule);
    setShowForm(true);
  };

  const openCreate = () => {
    setEditingRule(undefined);
    setShowForm(true);
  };

  // 一键导入默认（内置）中间件规则——复用 mitm importDefaults 模式。
  // 仅 global scope 显示（内置规则 scope=global）；INSERT 仅补缺失项，幂等可重复点。
  const handleImportDefaults = async () => {
    setImporting(true); setError(""); setMessage("");
    try {
      const { imported, skipped } = await middlewareApi.importDefaults();
      setMessage(t("middleware.importDefaultsDone", "已导入 {{imported}} 条默认规则（{{skipped}} 条已存在跳过）", { imported, skipped }));
      await load();
    } catch (e) {
      console.error("import default middleware rules failed", e);
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {!embedded && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
          {t("middleware.globalRulesHint", "全局规则对所有分组 / 平台生效，可被分组 / 平台级规则就近覆盖")}
        </div>
      )}

      {loading ? (
        <div className="text-secondary" style={{ fontSize: F.hint, padding: 8 }}>
          {t("status.loading", "加载中…")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {rules.length === 0 && !showForm && (
            <div className="text-tertiary" style={{ fontSize: F.hint, padding: 8 }}>
              {t("middleware.noRules", "暂无规则")}
            </div>
          )}
          {rules.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              onEdit={openEdit}
              onToggle={handleToggle}
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}

      {showForm ? (
        <RuleForm
          rule={editingRule}
          fixedScope={scope === "global" ? undefined : scope}
          fixedScopeRef={scope === "global" ? undefined : scopeRef}
          onSave={handleSave}
          onCancel={() => {
            setShowForm(false);
            setEditingRule(undefined);
          }}
        />
      ) : (
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <button
            className="btn btn-ghost"
            style={{ fontSize: F.hint }}
            onClick={openCreate}
          >
            + {t("middleware.addRule", "新增规则")}
          </button>
          {scope === "global" && (
            <button
              className="btn btn-ghost"
              style={{ fontSize: F.hint, opacity: importing ? 0.6 : 1 }}
              onClick={handleImportDefaults}
              disabled={importing}
            >
              {t("middleware.importDefaults", "导入默认规则")}
            </button>
          )}
        </div>
      )}

      {message && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all", color: "var(--color-success, #22c55e)" }}>
          {message}
        </div>
      )}
      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
          {error}
        </div>
      )}
    </div>
  );
}

// ── 中间件设置 tab（总开关 + rule_type 子开关 + 全局规则 CRUD）──

export function MiddlewareSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<MiddlewareSettings>({ enabled: true, type_toggles: {} });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    (async () => {
      try {
        const s = await middlewareApi.getSettings();
        setSettings(s);
      } catch (e) {
        console.error("get middleware settings failed", e);
        // 读失败时按默认 enabled=true 处理
        setSettings({ enabled: true, type_toggles: {} });
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const persist = async (next: MiddlewareSettings) => {
    setSettings(next);
    try {
      await middlewareApi.setSettings(next);
    } catch (e) {
      console.error("set middleware settings failed", e);
      setError(String(e));
    }
  };

  const toggleMaster = () => persist({ ...settings, enabled: !settings.enabled });

  // 子开关缺省键视为 true
  const typeEnabled = (rt: RuleType) => settings.type_toggles[rt] !== false;
  const toggleType = (rt: RuleType) =>
    persist({
      ...settings,
      type_toggles: { ...settings.type_toggles, [rt]: !typeEnabled(rt) },
    });

  if (loading) {
    return (
      <div className="text-secondary" style={{ padding: 20 }}>
        {t("status.loading", "加载中…")}
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 总开关（默认 ON） */}
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.masterToggle", "中间件总开关")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("middleware.masterToggleDesc", "关闭后全部规则旁路，请求 / 响应不经过中间件处理")}
          </div>
        </div>
        <div
          className={`toggle ${settings.enabled ? "active" : ""}`}
          onClick={toggleMaster}
          role="switch"
          aria-checked={settings.enabled}
          tabIndex={0}
        />
      </div>

      {/* rule_type 子开关 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.typeToggles", "规则类型开关")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: -4 }}>
          {t("middleware.typeTogglesDesc", "按规则类型单独启用 / 禁用")}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, paddingTop: 4 }}>
          {RULE_TYPES.map((rt) => (
            <div key={rt} style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontSize: 12 }}>{ruleTypeLabel(t, rt)}</span>
              <div
                className={`toggle ${typeEnabled(rt) ? "active" : ""}`}
                onClick={() => toggleType(rt)}
                role="switch"
                aria-checked={typeEnabled(rt)}
                tabIndex={0}
              />
            </div>
          ))}
        </div>
      </div>

      {/* 全局规则 CRUD */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.globalRules", "全局规则")}</div>
        <MiddlewareRulesPanel scope="global" />
      </div>

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
          {error}
        </div>
      )}
    </div>
  );
}
