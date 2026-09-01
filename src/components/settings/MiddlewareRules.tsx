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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useReveal, makeRipple } from "../../components/shared";
import { treeToDsl, parseDsl } from "../../utils/mwDsl";
import { platformApi } from "../../services/api/platforms";
import { groupApi } from "../../services/api/groups";

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

// ── 编辑器常量 ──

const TARGETS = ["request_body", "request_headers", "response_body", "response_headers", "status", "model"] as const;
const MATCH_TYPES = ["contains", "regex", "exact"] as const;
const ACTION_KINDS = ["mask", "block", "warn", "inject", "override", "classify"] as const;

/** target 是否响应侧（与 Rust Target::is_response_side 对称）。 */
function isResponseTarget(t: string): boolean {
  return t === "response_body" || t === "response_headers" || t === "status";
}

/** 混阶段检查：树内所有叶子必须同侧（与 Rust validate_rule_phases 对称，前端提前提示）。 */
function mixedPhase(node: ConditionNode): string | null {
  let phase: boolean | null = null;
  const walk = (n: ConditionNode) => {
    if (n.kind === "leaf") {
      const p = isResponseTarget(n.target);
      if (phase !== null && phase !== p) {
        throw new Error(`混阶段条件被拒：'${n.target}' 与请求侧条件不能同树`);
      }
      phase = p;
    } else {
      n.children.forEach(walk);
    }
  };
  try {
    walk(node);
    return null;
  } catch (e) {
    return String((e as Error).message);
  }
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

// ── 条件树编辑器（递归组卡片，票 04）──

interface NodeEditorProps {
  node: ConditionNode;
  onChange: (n: ConditionNode) => void;
  /** 删除本节点（顶层不可删） */
  onRemove?: () => void;
  depth: number;
}

function ConditionLeafEditor({ node, onChange, onRemove }: Omit<NodeEditorProps, "depth"> & { node: Extract<ConditionNode, { kind: "leaf" }> }) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
      <Select value={node.target} onValueChange={(v) => onChange({ ...node, target: v as typeof node.target, field: "" })}>
        <SelectTrigger style={{ fontSize: F.hint, width: 140 }}><SelectValue /></SelectTrigger>
        <SelectContent>
          {TARGETS.map((x) => (
            <SelectItem key={x} value={x}>{conditionsTargetLabel(t, x)}</SelectItem>
          ))}
        </SelectContent>
      </Select>
      {(node.target === "request_body" || node.target === "response_body" || node.target === "request_headers" || node.target === "response_headers") && (
        <Input
          style={{ fontSize: F.hint, flex: "1 1 100px", fontFamily: '"SF Mono", "Fira Code", monospace' }}
          placeholder={t("middleware.fieldHint", "字段（空=整体 / JSON path / header 名）")}
          value={node.field}
          onChange={(e) => onChange({ ...node, field: e.target.value })}
        />
      )}
      <Select value={node.match_type} onValueChange={(v) => onChange({ ...node, match_type: v as typeof node.match_type })}>
        <SelectTrigger style={{ fontSize: F.hint, width: 100 }}><SelectValue /></SelectTrigger>
        <SelectContent>
          {MATCH_TYPES.map((x) => (
            <SelectItem key={x} value={x}>{x}</SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Input
        style={{ fontSize: F.hint, flex: "2 1 160px", fontFamily: '"SF Mono", "Fira Code", monospace' }}
        placeholder={t("middleware.pattern", "匹配模式")}
        value={node.pattern}
        onChange={(e) => onChange({ ...node, pattern: e.target.value })}
      />
      {onRemove && (
        <Button variant="ghost" onClick={onRemove} title={t("action.delete", "删除")} style={{ color: "var(--text-tertiary)" }}>
          <IconClose size={12} />
        </Button>
      )}
    </div>
  );
}

function conditionsTargetLabel(t: TFunction, x: string): string {
  const map: Record<string, string> = {
    request_body: t("middleware.target.request_body", "请求 body"),
    request_headers: t("middleware.target.request_headers", "请求 header"),
    response_body: t("middleware.target.response_body", "响应 body"),
    response_headers: t("middleware.target.response_headers", "响应 header"),
    status: t("middleware.target.status", "状态码"),
    model: t("middleware.target.model", "模型"),
  };
  return map[x] ?? x;
}

function ConditionNodeEditor({ node, onChange, onRemove, depth }: NodeEditorProps) {
  const { t } = useTranslation();
  if (node.kind === "leaf") {
    return <ConditionLeafEditor node={node} onChange={onChange} onRemove={onRemove} />;
  }
  const setChild = (i: number, c: ConditionNode) => onChange({ ...node, children: node.children.map((x, j) => (j === i ? c : x)) });
  const removeChild = (i: number) => onChange({ ...node, children: node.children.filter((_, j) => j !== i) });
  const addChild = (leaf: boolean) =>
    onChange({
      ...node,
      children: [
        ...node.children,
        leaf
          ? { kind: "leaf", target: "request_body", field: "", match_type: "contains", pattern: "" }
          : { kind: "any", children: [{ kind: "leaf", target: "request_body", field: "", match_type: "contains", pattern: "" }] },
      ],
    });
  return (
    <div
      style={{
        border: "1px solid var(--border)",
        borderRadius: "var(--radius-sm)",
        padding: 8,
        display: "flex",
        flexDirection: "column",
        gap: 6,
        marginLeft: depth * 12,
        background: "var(--bg-glass)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <Select value={node.kind} onValueChange={(v) => onChange({ ...node, kind: v as "all" | "any" })}>
          <SelectTrigger style={{ fontSize: F.hint, width: 92 }}><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">AND (全部满足)</SelectItem>
            <SelectItem value="any">OR (任一满足)</SelectItem>
          </SelectContent>
        </Select>
        <div style={{ flex: 1 }} />
        <Button variant="ghost" style={{ fontSize: 11 }} onClick={() => addChild(true)}>
          + {t("middleware.addLeaf", "条件")}
        </Button>
        <Button variant="ghost" style={{ fontSize: 11 }} onClick={() => addChild(false)}>
          + {t("middleware.addGroup", "子组")}
        </Button>
        {onRemove && (
          <Button variant="ghost" onClick={onRemove} title={t("action.delete", "删除")} style={{ color: "var(--text-tertiary)" }}>
            <IconClose size={12} />
          </Button>
        )}
      </div>
      {node.children.map((c, i) => (
        <ConditionNodeEditor key={i} node={c} onChange={(n) => setChild(i, n)} onRemove={() => removeChild(i)} depth={depth + 1} />
      ))}
    </div>
  );
}

// ── 动作链编辑器（有序，票 04）──

/** ActionParams 前端默认值（与 Rust serde default 对齐）。 */
function defaultParams(): ActionStep["params"] {
  return {
    replacement: "****",
    fields: [],
    inject_mode: "",
    target: "",
    value: "",
    category: "",
    retryable: true,
    override_status: null,
    override_body: null,
  };
}

function ActionChainEditor({ steps, onChange }: { steps: ActionStep[]; onChange: (s: ActionStep[]) => void }) {
  const { t } = useTranslation();
  const setStep = (i: number, st: ActionStep) => onChange(steps.map((x, j) => (j === i ? st : x)));
  const remove = (i: number) => onChange(steps.filter((_, j) => j !== i));
  const move = (i: number, d: -1 | 1) => {
    const j = i + d;
    if (j < 0 || j >= steps.length) return;
    const next = [...steps];
    [next[i], next[j]] = [next[j], next[i]];
    onChange(next);
  };
  const add = () => onChange([...steps, { kind: "warn", params: defaultParams() }]);
  const terminal = (k: ActionKind) => k === "block" || k === "classify";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {steps.map((st, i) => (
        <div key={i} style={{ border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", padding: 8, display: "flex", flexDirection: "column", gap: 6, background: "var(--bg-glass)" }}>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <span style={{ fontSize: 11, color: "var(--text-tertiary)", width: 18 }}>{i + 1}</span>
            <Select value={st.kind} onValueChange={(v) => setStep(i, { ...st, kind: v as ActionKind })}>
              <SelectTrigger style={{ fontSize: F.hint, width: 110 }}><SelectValue /></SelectTrigger>
              <SelectContent>
                {ACTION_KINDS.map((k) => (
                  <SelectItem key={k} value={k}>{actionLabel(t, k)}{terminal(k) ? " ⏹" : ""}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <div style={{ flex: 1 }} />
            <Button variant="ghost" style={{ fontSize: 11 }} onClick={() => move(i, -1)} disabled={i === 0}>↑</Button>
            <Button variant="ghost" style={{ fontSize: 11 }} onClick={() => move(i, 1)} disabled={i === steps.length - 1}>↓</Button>
            <Button variant="ghost" onClick={() => remove(i)} title={t("action.delete", "删除")} style={{ color: "var(--text-tertiary)" }}>
              <IconClose size={12} />
            </Button>
          </div>
          {/* 参数区（按 kind 显示相关字段） */}
          {(st.kind === "mask" || st.kind === "override") && (
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
              <Input
                style={{ fontSize: F.hint, flex: "1 1 120px", fontFamily: '"SF Mono", "Fira Code", monospace' }}
                placeholder='replacement（默认 ****，regex 支持 $1）'
                value={st.params.replacement}
                onChange={(e) => setStep(i, { ...st, params: { ...st.params, replacement: e.target.value } })}
              />
              {st.kind === "mask" && (
                <Input
                  style={{ fontSize: F.hint, flex: "1 1 140px" }}
                  placeholder='fields 逗号分隔（messages,system；空=全部）'
                  value={st.params.fields.join(",")}
                  onChange={(e) => setStep(i, { ...st, params: { ...st.params, fields: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) } })}
                />
              )}
            </div>
          )}
          {st.kind === "inject" && (
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              <Select value={st.params.inject_mode || "system_append"} onValueChange={(v) => setStep(i, { ...st, params: { ...st.params, inject_mode: v } })}>
                <SelectTrigger style={{ fontSize: F.hint, width: 150 }}><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="system_append">system_append</SelectItem>
                  <SelectItem value="body_set">body_set</SelectItem>
                  <SelectItem value="header_set">header_set</SelectItem>
                </SelectContent>
              </Select>
              {st.params.inject_mode === "body_set" && (
                <Input
                  style={{ fontSize: F.hint, flex: "1 1 100px", fontFamily: '"SF Mono", "Fira Code", monospace' }}
                  placeholder="target JSON key"
                  value={st.params.target}
                  onChange={(e) => setStep(i, { ...st, params: { ...st.params, target: e.target.value } })}
                />
              )}
              <Input
                style={{ fontSize: F.hint, flex: "2 1 160px" }}
                placeholder="value"
                value={st.params.value}
                onChange={(e) => setStep(i, { ...st, params: { ...st.params, value: e.target.value } })}
              />
            </div>
          )}
          {st.kind === "classify" && (
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
              <Input
                style={{ fontSize: F.hint, flex: "1 1 100px" }}
                placeholder="category"
                value={st.params.category}
                onChange={(e) => setStep(i, { ...st, params: { ...st.params, category: e.target.value } })}
              />
              <label style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11 }}>
                <Switch checked={st.params.retryable} onCheckedChange={(v) => setStep(i, { ...st, params: { ...st.params, retryable: v } })} />
                retryable
              </label>
              <Input
                style={{ fontSize: F.hint, width: 90 }}
                type="number"
                placeholder="override status"
                value={st.params.override_status ?? ""}
                onChange={(e) => setStep(i, { ...st, params: { ...st.params, override_status: e.target.value ? Number(e.target.value) : null } })}
              />
              <Input
                style={{ fontSize: F.hint, flex: "1 1 120px" }}
                placeholder="override body"
                value={st.params.override_body ?? ""}
                onChange={(e) => setStep(i, { ...st, params: { ...st.params, override_body: e.target.value || null } })}
              />
            </div>
          )}
        </div>
      ))}
      <Button variant="ghost" style={{ fontSize: F.hint, alignSelf: "flex-start" }} onClick={add}>
        + {t("middleware.addAction", "动作")}
      </Button>
    </div>
  );
}

// ── Applies To 编辑器（三维多选，票 04）──

function AppliesToEditor({ value, onChange }: { value: AppliesTo; onChange: (a: AppliesTo) => void }) {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<{ id: number; name: string }[]>([]);
  const [groups, setGroups] = useState<{ id: number; name: string; group_key: string }[]>([]);
  useEffect(() => {
    platformApi
      .list()
      .then((ps: { id: number; name: string }[]) => setPlatforms(ps.map((x) => ({ id: x.id, name: x.name }))))
      .catch(() => {});
    groupApi.list().then((gs: { id: number; name: string; group_key: string }[]) => setGroups(gs)).catch(() => {});
  }, []);
  const toggle = (dim: "platforms" | "groups", v: number | string, on: boolean) => {
    const cur = value[dim] as (number | string)[];
    onChange({
      ...value,
      [dim]: on ? [...cur, v] : cur.filter((x) => x !== v),
    } as AppliesTo);
  };
  const check = (dim: "platforms" | "groups", v: number | string) => (value[dim] as (number | string)[]).includes(v);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div>
        <div style={{ fontSize: F.hint, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("middleware.appliesPlatforms", "平台（空 = 全部）")}
        </div>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {platforms.map((p) => (
            <label key={p.id} style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11 }}>
              <input type="checkbox" checked={check("platforms", p.id)} onChange={(e) => toggle("platforms", p.id, e.target.checked)} />
              {p.name}
            </label>
          ))}
        </div>
      </div>
      <div>
        <div style={{ fontSize: F.hint, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("middleware.appliesGroups", "分组（空 = 全部）")}
        </div>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {groups.map((g) => (
            <label key={g.id} style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11 }}>
              <input type="checkbox" checked={check("groups", g.group_key)} onChange={(e) => toggle("groups", g.group_key, e.target.checked)} />
              {g.name}
            </label>
          ))}
        </div>
      </div>
      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.appliesModels", "模型（逗号分隔，空 = 全部）")}
        </span>
        <Input
          style={{ fontSize: F.hint, fontFamily: '"SF Mono", "Fira Code", monospace' }}
          value={value.models.join(",")}
          onChange={(e) => onChange({ ...value, models: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })}
        />
      </label>
    </div>
  );
}

// ── 规则编辑表单（卡片树 / DSL 双模式，票 04 + 05）──

interface RuleFormProps {
  rule?: MiddlewareRule;
  /** 只读查看模式（内置规则：可看详情，不可改不可存） */
  readOnly?: boolean;
  onSave: (draft: CreateMiddlewareRule) => Promise<void>;
  onCancel: () => void;
}

export function RuleForm({ rule, readOnly, onSave, onCancel }: RuleFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(rule?.name ?? "");
  const [description, setDescription] = useState(rule?.description ?? "");
  const [priority, setPriority] = useState(rule?.priority ?? 0);
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [conditions, setConditions] = useState<ConditionNode>(
    rule?.conditions ?? { kind: "leaf", target: "request_body", field: "", match_type: "contains", pattern: "" },
  );
  const [actions, setActions] = useState<ActionStep[]>(
    rule?.actions?.length ? rule.actions : [{ kind: "mask", params: { ...defaultParams(), replacement: "****" } }],
  );
  const [applies, setApplies] = useState<AppliesTo>(rule?.applies_to ?? { platforms: [], groups: [], models: [] });
  const [mode, setMode] = useState<"cards" | "dsl">("cards");
  const [dslText, setDslText] = useState<string>(() => treeToDsl(rule?.conditions ?? { kind: "leaf", target: "request_body", field: "", match_type: "contains", pattern: "" }));
  const [dslError, setDslError] = useState("");
  const [saveError, setSaveError] = useState("");
  const [saving, setSaving] = useState(false);

  const phaseError = mixedPhase(conditions);

  const handleSave = async () => {
    if (mode === "dsl") {
      // DSL 模式下保存前必须解析成功（切回卡片时已同步，此处兜底）。
      try {
        setConditions(parseDsl(dslText));
      } catch (e) {
        setDslError(String((e as Error).message));
        return;
      }
    }
    if (mixedPhase(conditions)) return; // 前端已禁用保存按钮，兜底
    setSaving(true);
    try {
      await onSave({
        name,
        description,
        conditions,
        actions,
        applies_to: applies,
        priority,
        is_builtin: false,
        enabled,
      });
    } catch (e) {
      console.error("save middleware rule failed", e);
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const switchToDsl = () => {
    setDslText(treeToDsl(conditions));
    setDslError("");
    setMode("dsl");
  };
  const switchToCards = () => {
    try {
      setConditions(parseDsl(dslText));
      setDslError("");
      setMode("cards");
    } catch (e) {
      setDslError(String((e as Error).message));
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {readOnly && (
        <div style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
          {t("middleware.builtinReadonlyHint", "内置规则只可启停，内容不可修改")}
        </div>
      )}

      {/* 只读模式：pointer-events 阻断全部交互控件（含 Radix Select），文本仍可选中复制 */}
      <div style={readOnly ? { pointerEvents: "none", userSelect: "text", display: "flex", flexDirection: "column", gap: S.gap } : { display: "flex", flexDirection: "column", gap: S.gap }}>
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

      {/* 条件区：卡片树 / DSL 双模式 */}
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
          {t("middleware.conditions", "条件")}
        </span>
        <div style={{ flex: 1 }} />
        {mode === "cards" ? (
          <Button variant="ghost" style={{ fontSize: 11 }} onClick={switchToDsl}>
            {t("middleware.toDsl", "DSL 源码")}
          </Button>
        ) : (
          <Button variant="ghost" style={{ fontSize: 11 }} onClick={switchToCards} disabled={!!dslError}>
            {t("middleware.toCards", "卡片模式")}
          </Button>
        )}
      </div>
      {mode === "cards" ? (
        <ConditionNodeEditor node={conditions} onChange={setConditions} depth={0} />
      ) : (
        <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <Textarea
            style={{
              fontFamily: '"SF Mono", "Fira Code", monospace',
              fontSize: 12,
              lineHeight: 1.6,
              minHeight: 120,
              resize: "vertical",
              whiteSpace: "pre",
            }}
            value={dslText}
            onChange={(e) => {
              setDslText(e.target.value);
              try {
                parseDsl(e.target.value);
                setDslError("");
              } catch (err) {
                setDslError(String((err as Error).message));
              }
            }}
            spellCheck={false}
          />
          <div style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
            {t("middleware.dslHint", '语法: ALL(...)/ANY(...)/叶子，叶子 = target[.field] contains|regex|exact "pattern"')}
          </div>
          {dslError && (
            <div style={{ fontSize: 11, color: "var(--color-danger)", wordBreak: "break-all" }}>{dslError}</div>
          )}
        </label>
      )}
      {phaseError && (
        <div style={{ fontSize: 11, color: "var(--color-danger)" }}>{phaseError}</div>
      )}

      {/* 动作链 */}
      <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
        {t("middleware.actions", "动作链（按顺序执行；block/classify 后停止）")}
      </span>
      <ActionChainEditor steps={actions} onChange={setActions} />

      {/* applies_to */}
      <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
        {t("middleware.appliesTo", "应用范围（各自空 = 不限；多值命中任一）")}
      </span>
      <AppliesToEditor value={applies} onChange={setApplies} />

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
      </div>

      {saveError && (
        <div style={{ fontSize: 11, color: "var(--color-danger)", wordBreak: "break-all" }}>{saveError}</div>
      )}

      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <Button variant="outline" style={{ fontSize: F.hint }} onClick={onCancel} disabled={saving}>
          {readOnly ? t("action.close", "关闭") : t("action.cancel", "取消")}
        </Button>
        {!readOnly && (
          <Button
            variant="default"
            className="ripple"
            style={{ fontSize: F.hint }}
            onClick={(e) => { makeRipple(e); handleSave(); }}
            disabled={saving || !name || !!phaseError || (mode === "dsl" && !!dslError)}
          >
            {t("action.save", "保存")}
          </Button>
        )}
      </div>
    </div>
  );
}

/** 规则编辑弹窗：表单从「页面最底部内嵌」改为独立 modal（Radix Portal → document.body，
 *  满足 modal 居中规则；祖先的 backdrop-filter 不再把 fixed 拉回 page 内）。 */
export function RuleFormDialog({
  open,
  onOpenChange,
  ...formProps
}: RuleFormProps & { open: boolean; onOpenChange: (v: boolean) => void }) {
  const { t } = useTranslation();
  const title = formProps.readOnly
    ? t("middleware.viewRule", "查看规则")
    : formProps.rule
      ? t("middleware.editRule", "编辑规则")
      : t("middleware.addRule", "新增规则");
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="glass-surface"
        style={{
          padding: 20,
          maxWidth: 720,
          width: "min(92vw, 720px)",
          maxHeight: "86vh",
          overflowY: "auto",
          borderRadius: "var(--radius-lg)",
          gap: 12,
        }}
      >
        <DialogHeader>
          <DialogTitle style={{ fontSize: F.label, fontWeight: 600 }}>{title}</DialogTitle>
        </DialogHeader>
        <RuleForm {...formProps} />
      </DialogContent>
    </Dialog>
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

      {/* 内置规则可点开查看详情（表单只读）；Failed 规则（含内置残留）只可删除 */}
      {!rule.failed && (
        <Button variant="ghost" onClick={() => onEdit(rule)} title={t("action.edit", "编辑")}>
          <IconEdit size={14} />
        </Button>
      )}
      {!rule.is_builtin || rule.failed ? (
        <Button
          variant="ghost"
          onClick={() => onDelete(rule.id)}
          title={t("action.delete", "删除")}
          style={{ color: "var(--text-tertiary)" }}
        >
          <IconClose size={14} />
        </Button>
      ) : null}
    </div>
  );
}

// ── 规则面板（全局唯一入口：规则限定平台 / 分组一律经表单里的「应用范围」）──

export function MiddlewareRulesPanel() {
  const { t } = useTranslation();
  const [rules, setRules] = useState<MiddlewareRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [editingRule, setEditingRule] = useState<MiddlewareRule | undefined>(undefined);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const all = await middlewareApi.listRules();
      setRules(all || []);
    } catch (e) {
      console.error("list middleware rules failed", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
        {t("middleware.globalRulesHint", "规则按优先级堆叠执行；应用范围为空时对所有分组 / 平台生效")}
      </div>

      {loading ? (
        <div className="text-secondary" style={{ fontSize: F.hint, padding: 8 }}>
          {t("status.loading", "加载中…")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {rules.length === 0 && (
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

      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <Button variant="ghost" style={{ fontSize: F.hint }} onClick={openCreate}>
          + {t("middleware.addRule", "新增规则")}
        </Button>
      </div>

      {/* 新增 / 编辑 / 查看一律走弹窗（key 强制换实例：切换规则时表单 state 重新初始化） */}
      <RuleFormDialog
        key={editingRule?.id ?? "new"}
        open={showForm}
        onOpenChange={(v) => {
          if (!v) {
            setShowForm(false);
            setEditingRule(undefined);
          }
        }}
        rule={editingRule}
        readOnly={editingRule?.is_builtin}
        onSave={handleSave}
        onCancel={() => {
          setShowForm(false);
          setEditingRule(undefined);
        }}
      />

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
