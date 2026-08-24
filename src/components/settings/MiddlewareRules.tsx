// ─── Middleware Rule Engine UI（统一引擎，ADR 0003）──────────────
// 一条规则 = 条件树 + 动作链 + Applies To。本文件为票 02 级 UI：
// 列表（内置徽标 / Failed 徽标 / 启停 / 删除）+ JSON 级编辑表单。
// 票 04/05（递归树卡片编辑器 + DSL 源码模式）在此文件上迭代。

import { useState, useEffect, useCallback } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  middlewareApi,
  type MiddlewareRule,
  type MiddlewareSettings,
  type CreateMiddlewareRule,
  type ConditionNode,
  type ActionStep,
  type AppliesTo,
  type ActionKind,
} from "../../services/api";
import { F, S } from "./editors";
import { IconClose, IconEdit } from "../icons";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { useReveal, makeRipple } from "../../components/shared";

// ponytail: 单卡片包装 — 每实例独立 useReveal (React 规则禁 map 内条件 hook),
// hover-lift + reveal 萤火虫入场 (stagger idx*60)。
function MwSectionCard({
  staggerMs,
  style,
  children,
}: {
  staggerMs: number;
  style?: React.CSSProperties;
  children: ReactNode;
}) {
  const { ref, shown } = useReveal<HTMLDivElement>(staggerMs);
  return (
    <div
      ref={ref}
      className={`glass-surface glass-highlight hover-lift reveal${shown ? " in" : ""}`}
      style={style}
    >
      {children}
    </div>
  );
}

// ── 摘要渲染（列表行用：条件树 / 动作链一眼可读）──

/** 条件树摘要：递归渲染为 `a AND (b OR c)` 形式。 */
function conditionsSummary(node: ConditionNode): string {
  if (node.kind === "leaf") {
    const tgt = { request_body: "req.body", request_headers: "req.headers", response_body: "resp.body", response_headers: "resp.headers", status: "status", model: "model" }[node.target] ?? node.target;
    const field = node.field ? `.${node.field}` : "";
    return `${tgt}${field} ${node.match_type} /${node.pattern}/`;
  }
  const joined = node.children.map(conditionsSummary).join(node.kind === "all" ? " AND " : " OR ");
  return node.children.length > 1 ? `(${joined})` : joined || "∅";
}

function actionLabel(t: TFunction, a: ActionKind): string {
  const map: Record<ActionKind, string> = {
    mask: t("middleware.action.mask", "脱敏"),
    block: t("middleware.action.block", "拦截"),
    warn: t("middleware.action.warn", "告警"),
    inject: t("middleware.action.inject", "注入"),
    override: t("middleware.action.override", "改写"),
    classify: t("middleware.action.classify", "分类"),
  };
  return map[a];
}

function actionsSummary(t: TFunction, steps: ActionStep[]): string {
  return steps.map((s) => actionLabel(t, s.kind)).join(" → ") || "∅";
}

function appliesSummary(at: AppliesTo): string {
  const parts: string[] = [];
  if (at.platforms.length) parts.push(`p:${at.platforms.join(",")}`);
  if (at.groups.length) parts.push(`g:${at.groups.join(",")}`);
  if (at.models.length) parts.push(`m:${at.models.join(",")}`);
  return parts.join(" ");
}

// ── 规则编辑表单（JSON 级：conditions/actions/applies_to 三块 JSON + 基础字段）──

interface RuleFormProps {
  rule?: MiddlewareRule;
  /** 新建时预置的 applies_to（group / platform 内嵌面板） */
  presetApplies?: AppliesTo;
  onSave: (draft: CreateMiddlewareRule) => Promise<void>;
  onCancel: () => void;
}

const DEFAULT_CONDITIONS: ConditionNode = {
  kind: "leaf",
  target: "request_body",
  field: "",
  match_type: "contains",
  pattern: "",
};
const DEFAULT_ACTIONS: ActionStep[] = [{ kind: "mask", params: { replacement: "****" } as ActionStep["params"] }];

function JsonField({
  label,
  value,
  onChange,
  error,
  hint,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  error: string;
  hint: string;
}) {
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{label}</span>
      <Textarea
        style={{
          fontFamily: '"SF Mono", "Fira Code", monospace',
          fontSize: 12,
          lineHeight: 1.6,
          minHeight: 80,
          resize: "vertical",
          whiteSpace: "pre",
        }}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={false}
      />
      <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>{hint}</div>
      {error && (
        <div style={{ fontSize: 11, color: "var(--color-danger)", wordBreak: "break-all" }}>{error}</div>
      )}
    </label>
  );
}

function RuleForm({ rule, presetApplies, onSave, onCancel }: RuleFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(rule?.name ?? "");
  const [description, setDescription] = useState(rule?.description ?? "");
  const [priority, setPriority] = useState(rule?.priority ?? 0);
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [conditionsText, setConditionsText] = useState(
    JSON.stringify(rule?.conditions ?? presetApplies ?? DEFAULT_CONDITIONS, null, 2),
  );
  const [actionsText, setActionsText] = useState(
    JSON.stringify(rule?.actions ?? DEFAULT_ACTIONS, null, 2),
  );
  const [appliesText, setAppliesText] = useState(
    JSON.stringify(rule?.applies_to ?? presetApplies ?? {}, null, 2),
  );
  const [jsonErrors, setJsonErrors] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  const parseJson = (key: string, raw: string): unknown | undefined => {
    try {
      const v = JSON.parse(raw);
      setJsonErrors((e) => ({ ...e, [key]: "" }));
      return v;
    } catch (e) {
      setJsonErrors((errs) => ({ ...errs, [key]: String(e) }));
      return undefined;
    }
  };

  const handleSave = async () => {
    const conditions = parseJson("conditions", conditionsText);
    const actions = parseJson("actions", actionsText);
    const applies_to = parseJson("applies", appliesText);
    if (conditions === undefined || actions === undefined || applies_to === undefined) return;
    setSaving(true);
    try {
      await onSave({
        name,
        description,
        conditions: conditions as ConditionNode,
        actions: actions as ActionStep[],
        applies_to: applies_to as AppliesTo,
        priority,
        is_builtin: false,
        enabled,
      });
    } catch (e) {
      console.error("save middleware rule failed", e);
      setJsonErrors((errs) => ({ ...errs, save: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  const hasError = Object.values(jsonErrors).some(Boolean);

  return (
    <MwSectionCard
      staggerMs={0}
      style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}
    >
      <div style={{ fontSize: F.label, fontWeight: 600 }}>
        {rule ? t("middleware.editRule", "编辑规则") : t("middleware.addRule", "新增规则")}
      </div>

      <Input
        style={{ fontSize: F.body }}
        placeholder={t("middleware.name", "规则名称")}
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <Input
        style={{ fontSize: F.hint }}
        placeholder={t("middleware.description", "描述（可选）")}
        value={description}
        onChange={(e) => setDescription(e.target.value)}
      />

      <JsonField
        label={t("middleware.conditions", "条件树 (JSON)")}
        value={conditionsText}
        onChange={(v) => setConditionsText(v)}
        error={jsonErrors.conditions ?? ""}
        hint={t(
          "middleware.conditionsHint",
          '叶子: {"kind":"leaf","target":"request_body|request_headers|response_body|response_headers|status|model","field":"","match_type":"regex|contains|exact","pattern":"..."}；组: {"kind":"all"|"any","children":[...]}',
        )}
      />
      <JsonField
        label={t("middleware.actions", "动作链 (JSON，有序)")}
        value={actionsText}
        onChange={(v) => setActionsText(v)}
        error={jsonErrors.actions ?? ""}
        hint={t(
          "middleware.actionsHint",
          '[{"kind":"mask|block|warn|inject|override|classify","params":{...}}]；block/classify 终止后续',
        )}
      />
      <JsonField
        label={t("middleware.appliesTo", "应用范围 (JSON，空 = 全部)")}
        value={appliesText}
        onChange={(v) => setAppliesText(v)}
        error={jsonErrors.applies ?? ""}
        hint={t(
          "middleware.appliesHint",
          '{"platforms":[1,2],"groups":["gk"],"models":["m"]}；三维各自空 = 不限，多值命中任一',
        )}
      />

      <label style={{ display: "flex", flexDirection: "column", gap: 4, width: 120 }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.priority", "优先级")}
        </span>
        <Input
          type="number"
          style={{ fontSize: F.hint }}
          value={priority}
          onChange={(e) => setPriority(Number(e.target.value) || 0)}
        />
      </label>

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.enabled", "启用")}
        </span>
        <Switch checked={enabled} onCheckedChange={setEnabled} />
      </div>

      {jsonErrors.save && (
        <div style={{ fontSize: 11, color: "var(--color-danger)", wordBreak: "break-all" }}>
          {jsonErrors.save}
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <Button variant="outline" style={{ fontSize: F.hint }} onClick={onCancel} disabled={saving}>
          {t("action.cancel", "取消")}
        </Button>
        <Button
          variant="default"
          className="ripple"
          style={{ fontSize: F.hint }}
          onClick={(e) => { makeRipple(e); handleSave(); }}
          disabled={saving || !name || hasError}
        >
          {t("action.save", "保存")}
        </Button>
      </div>
    </MwSectionCard>
  );
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
        border: rule.failed ? "1px solid var(--color-danger)" : "1px solid var(--border)",
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
          {rule.failed && (
            <span className="badge" style={{ fontSize: 10, color: "var(--color-danger)" }}>
              {t("middleware.failed", "失效")}
            </span>
          )}
          <span className="badge" style={{ fontSize: 10 }}>
            {actionsSummary(t, rule.actions)}
          </span>
          {!!appliesSummary(rule.applies_to) && (
            <span className="badge" style={{ fontSize: 10 }}>
              {appliesSummary(rule.applies_to)}
            </span>
          )}
        </div>
        {!rule.failed && (
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
            {conditionsSummary(rule.conditions)}
          </div>
        )}
      </div>

      <Switch
        checked={rule.enabled}
        onCheckedChange={() => onToggle(rule)}
        title={t("middleware.enabled", "启用")}
      />

      {/* 内置规则禁删禁编辑（只允许启停）；Failed 规则只可删除 */}
      {!rule.is_builtin && !rule.failed && (
        <Button variant="ghost" onClick={() => onEdit(rule)} title={t("action.edit", "编辑")}>
          <IconEdit size={14} />
        </Button>
      )}
      {!rule.is_builtin && (
        <Button
          variant="ghost"
          onClick={() => onDelete(rule.id)}
          title={t("action.delete", "删除")}
          style={{ color: "var(--text-tertiary)" }}
        >
          <IconClose size={14} />
        </Button>
      )}
    </div>
  );
}

// ── 规则面板（可复用：global / group / platform 内嵌按 applies_to 过滤）──

export interface MiddlewareRulesPanelProps {
  /** group 内嵌：只看 applies_to 含该 group（或未限定 group）的规则 */
  groupKey?: string;
  /** platform 内嵌：只看 applies_to 含该 platform（或未限定 platform）的规则 */
  platformId?: number;
  embedded?: boolean;
}

export function MiddlewareRulesPanel({ groupKey, platformId, embedded = false }: MiddlewareRulesPanelProps) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<MiddlewareRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [editingRule, setEditingRule] = useState<MiddlewareRule | undefined>(undefined);
  const [error, setError] = useState("");

  const matchesScope = useCallback(
    (r: MiddlewareRule) => {
      const g = r.applies_to.groups;
      const p = r.applies_to.platforms;
      if (groupKey) return g.length === 0 || g.includes(groupKey);
      if (platformId !== undefined) return p.length === 0 || p.includes(platformId);
      // global 视图：展示全部（含限定范围的规则，供总览）
      return true;
    },
    [groupKey, platformId],
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
        conditions: rule.conditions,
        actions: rule.actions,
        applies_to: rule.applies_to,
        priority: rule.priority,
        enabled: !rule.enabled,
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

  const presetApplies: AppliesTo | undefined = groupKey
    ? { platforms: [], groups: [groupKey], models: [] }
    : platformId !== undefined
      ? { platforms: [platformId], groups: [], models: [] }
      : undefined;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {!embedded && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
          {t("middleware.globalRulesHint", "规则按优先级堆叠执行；应用范围为空时对所有分组 / 平台生效")}
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
          presetApplies={presetApplies}
          onSave={handleSave}
          onCancel={() => {
            setShowForm(false);
            setEditingRule(undefined);
          }}
        />
      ) : (
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <Button variant="ghost" style={{ fontSize: F.hint }} onClick={openCreate}>
            + {t("middleware.addRule", "新增规则")}
          </Button>
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

// ── 中间件设置 tab（总开关 + 全局规则列表）──

export function MiddlewareSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<MiddlewareSettings>({ enabled: true });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    (async () => {
      try {
        const s = await middlewareApi.getSettings();
        setSettings(s);
      } catch (e) {
        console.error("get middleware settings failed", e);
        setSettings({ enabled: true });
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
      <MwSectionCard
        staggerMs={0}
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.masterToggle", "中间件总开关")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("middleware.masterToggleDesc", "关闭后全部规则旁路，请求 / 响应不经过中间件处理")}
          </div>
        </div>
        <Switch checked={settings.enabled} onCheckedChange={toggleMaster} />
      </MwSectionCard>

      {/* 规则列表 */}
      <MwSectionCard
        staggerMs={60}
        style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}
      >
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.globalRules", "规则列表")}</div>
        <MiddlewareRulesPanel />
      </MwSectionCard>

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
          {error}
        </div>
      )}
    </div>
  );
}
