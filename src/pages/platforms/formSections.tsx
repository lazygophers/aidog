// formSections — PlatformEditForm 的表单分区子组件（纯 props 驱动，无自身 state）。
// ponytail: 从 PlatformEditForm.tsx 抽出以控制文件行数（每分区 ~50-150 行，合计 ~740 行）。
//   所有分区组件经 props 收 state/setters，不持有自己的 state；跨组件无共享 useState（符合
//   design.md 决策锁「区块拆保持单组件内 state」）。
//
// 共用 helpers（FormSection / ApiKeyField / toDatetimeLocal）也在此导出，供 PlatformEditForm 主组件消费。
//   EndpointsSection 已移到 formSectionsEndpoints.tsx（体积大），通过末尾 re-export 暴露。
import React, { useState, useEffect, useRef } from "react";
import { RateLimitPolicyLibrary } from "./RateLimitPolicyLibrary";
import { RateLimitRulesList, ruleToDraft, draftToRule, type RateLimitRuleDraft } from "./RateLimitRules";
import type { TFunction } from "i18next";
import { writeText } from "../../services/platform";
import {
  type Platform, type Protocol,
  type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit,
  type DevinConfig, type SchedulingBreakerSettings, type GroupDetail,
  QUOTA_SCRIPT_SAMPLE, isUserQuotaScript, QUOTA_CUSTOM_VARIANT, resolveQuotaSelection,
  quotaApi, type PlatformQuota,
} from "../../services/api";
import { rateLimitsSummary, clientCategoryState, toggleClientCategory, toggleClientVariant, validateClientUaRule, type ClientUaRule, type ClientUaRuleKind, type RateLimitsBundle, type RateLimitsConfig, type QuotaWindowConfig, type ClientVocabCategory } from "../../services/api";
import { Checkbox } from "@/components/ui/checkbox";
import { newManualBudget, type TimeWindow, getDefaultPeak, getDefaultModelList, getDefaultPlanQuotas, type PlanQuotaTier, type QuotaScriptVariant, quotaVariantLabel } from "../../domains/platforms";
import { isCurrentlyPeak, currentPeakMultiplier, utcToDisplay, displayToUtc, WINDOW_TIMEZONES, type TzMode } from "../../utils/timeWindow";
import { useProxyConfigured } from "../../domains/platforms/proxyConfig";
import { formatAmount, formatDateTime, formatPercent, pad } from "../../utils/formatters";
import type { ThemeMode } from "../../themes/types";
import { ModelCombobox, useModelCandidates } from "./ModelCombobox";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { TimePicker } from "./WindowsEditModal";
import { HolidayCalendarSelect } from "./HolidayCalendarSelect";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import type { PlatformDraft } from "../../domains/platforms/platformDraft";

/** draft 字段 → SetStateAction setter 适配（票 14：控件对草稿只经 draft + patch 一个口；
 *  section 内部沿用局部 setter 名，函数式 updater 当场以 draft 当前值解析）。
 *  等价 usePlatformForm 已删的兼容 setter 层——收进各 section 一行，不回 hook。 */
export function draftSetter<K extends keyof PlatformDraft>(
  draft: PlatformDraft, patch: (p: Partial<PlatformDraft>) => void, k: K,
): React.Dispatch<React.SetStateAction<PlatformDraft[K]>> {
  return (v) => patch({ [k]: typeof v === "function" ? (v as (prev: PlatformDraft[K]) => PlatformDraft[K])(draft[k]) : v });
}

/** 毫秒时间戳 → datetime-local input 值 "YYYY-MM-DDTHH:MM"（本地时区，无秒）。
 *  datetime-local 不解析 ISO Z 后缀，须手动拼本地时间分量。 */
export function toDatetimeLocal(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Unix 秒 → datetime-local 字符串（本地时区）；0/undefined → 空串。
 *  PRD 07-09 D2：peak start_at/end_at 用 Unix 秒存储。 */
function secToLocalInput(sec: number | undefined): string {
  if (!sec || sec <= 0) return "";
  return toDatetimeLocal(sec * 1000);
}

/** datetime-local 字符串 → Unix 秒；空串/非法 → null（caller 据此清 undefined）。 */
function localInputToSec(v: string): number | null {
  if (!v) return null;
  const ms = new Date(v).getTime();
  if (!Number.isFinite(ms) || ms <= 0) return null;
  return Math.floor(ms / 1000);
}

/** 编辑页分区卡片：glass-surface 容器 + 标题 + 可选描述 + 内容区，统一视觉层次。 */
export function FormSection({ title, desc, action, children }: { title: string; desc?: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div
      className="glass-surface"
      style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, borderRadius: "var(--radius-md)" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>{title}</div>
          {desc && (
            <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.4, marginTop: 2 }}>{desc}</div>
          )}
        </div>
        {action && <div style={{ flexShrink: 0 }}>{action}</div>}
      </div>
      {children}
    </div>
  );
}

/** 使用代理开关（issue #64 T03）：写平台 extra.proxy_enabled；全局代理地址为空时提示「暂按直连」。 */
export function ProxyEnabledSection({ enabled, onChange, t }: { enabled: boolean; onChange: (v: boolean) => void; t: TFunction }) {
  // 全局代理地址是否已配置（设置页「代理配置」host 非空）：为空时开关可照常操作，实际直连。
  const proxyConfigured = useProxyConfigured();
  return (
    <FormSection title={t("platform.useProxy", "Use proxy")}>
      <div data-testid="platform-use-proxy-row" style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", padding: "9px 12px", borderRadius: "var(--radius-sm)", background: "var(--card)", border: "1px solid var(--border)" }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, cursor: "pointer" }}>
          <Switch data-testid="platform-use-proxy" checked={enabled} onCheckedChange={v => onChange(v === true)} />
          <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{t("platform.useProxy", "Use proxy")}</span>
        </label>
        {!proxyConfigured && (
          <span data-testid="proxy-unconfigured-hint" style={{
            fontSize: 14, fontWeight: 600, padding: "2px 8px", borderRadius: 5, whiteSpace: "nowrap",
            color: "var(--color-warning)",
            background: "color-mix(in srgb, var(--color-warning) 12%, transparent)",
            border: "1px solid color-mix(in srgb, var(--color-warning) 30%, transparent)",
          }}>
            {t("platform.proxyUnconfiguredHint", "No proxy configured; direct connection for now")}
          </span>
        )}
        <span style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.4 }}>
          {t("platform.useProxyDesc", "Outbound requests of this platform go through the proxy configured in Settings → Proxy.")}
        </span>
      </div>
    </FormSection>
  );
}

/** Token 显隐 + 复制按钮（透传配置区 / 认证区共用）。
 *  multiline=true（创建态认证区）：textarea 多行输入，每行一个 token，多行触发批量创建预览
 *  （拆分/批量链路见 splitApiKeys + runBatchCreateFromPaste）；无密码遮蔽（textarea 不支持 password）。 */
/** ── C 方向稿共享视觉件（2026-10-22 预算治理三区重设计：去抽屉版）──
 *  三区（高峰倍率 / 限频 / 手动预算）统一的实时态条与行卡样式；
 *  分区仍各自独立常展开（三区分居不同 tab，无折叠收纳，用户拍板原话见 ask d1c9fc964eab）。 */
const C = {
  /** 分区头图标瓦片 */
  tile: (color: string): React.CSSProperties => ({
    width: 30, height: 30, borderRadius: 9, flex: "none", fontSize: 14, fontWeight: 600,
    display: "flex", alignItems: "center", justifyContent: "center",
    color, background: `color-mix(in srgb, ${color} 12%, transparent)`,
    border: `1px solid color-mix(in srgb, ${color} 32%, transparent)`,
  }),
  /** 实时态 / 摘要 pill（稿 2/3/4 .pill：10px；neutral = 透明底 + line 边框） */
  pill: (tone: "neutral" | "hot" | "ok"): React.CSSProperties => {
    const c = tone === "hot" ? "var(--color-warning)" : tone === "ok" ? "var(--color-success)" : "var(--text-tertiary)";
    return {
      fontSize: 14, fontWeight: 600, padding: "3px 9px", borderRadius: 6, whiteSpace: "nowrap",
      color: c,
      ...(tone === "neutral"
        ? { border: "1px solid var(--border)", background: "transparent" }
        : {
            border: `1px solid color-mix(in srgb, ${c} 35%, transparent)`,
            background: `color-mix(in srgb, ${c} 8%, transparent)`,
          }),
    };
  },
  /** 编辑行卡 */
  row: {
    display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap",
    padding: "9px 12px", borderRadius: "var(--radius-sm)",
    background: "var(--card)", border: "1px solid var(--border)",
  } as React.CSSProperties,
  /** 虚线添加按钮类名（含 hover，样式在 globals.css .aidog-addbtn） */
  addBtnClass: "aidog-addbtn",
  /** 分区实时态条容器 */
  strip: {
    display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginBottom: 10,
  } as React.CSSProperties,
  /** 行卡 hover 类名（边框过渡在 globals.css .aidog-rowcard） */
  rowClass: "aidog-rowcard",
};

/** 24h spark 条：命中时段点亮（跨天窗口首尾两段都亮）。纯展示，宽 ~96px。 */
export function PeakSpark({ w }: { w: TimeWindow }) {
  const on = (h: number) => {
    const s = w.start_hour, e = w.end_hour;
    return e < s ? h >= s || h < e : h >= s && h < e; // e===s 视为空窗
  };
  return (
    <div style={{ display: "flex", gap: 1, width: 96, height: 12, flex: "none" }}>
      {Array.from({ length: 24 }, (_, h) => (
        <i key={h} style={{
          flex: 1, borderRadius: 1,
          background: on(h) ? "color-mix(in srgb, var(--color-warning) 85%, transparent)" : "var(--bg-glass)",
          boxShadow: on(h) ? "0 0 6px color-mix(in srgb, var(--color-warning) 35%, transparent)" : undefined,
        }} />
      ))}
    </div>
  );
}

export function ApiKeyField({ value, onChange, show, onToggleShow, editing, placeholder = "Token", multiline = false }: {
  value: string;
  onChange: (v: string) => void;
  show: boolean;
  onToggleShow: () => void;
  editing?: boolean;
  placeholder?: string;
  multiline?: boolean;
}) {
  return (
    <div style={{ display: "flex", gap: 6, alignItems: multiline ? "flex-start" : "center" }}>
      {multiline ? (
        <textarea
          className="input"
          placeholder={placeholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          spellCheck={false}
          rows={Math.min(6, Math.max(2, value.split("\n").length))}
          style={{ flex: 1, resize: "vertical", minHeight: 58, fontFamily: "var(--font-mono, monospace)", fontSize: 14, lineHeight: 1.6 }}
        />
      ) : (
      <Input
        className="input"
        type={show ? "text" : "password"}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{ flex: 1 }}
      />
      )}
      {!multiline && (
      <Button
        variant="ghost"
        size="icon"
        title={show ? "Hide token" : "Show token"}
        onClick={onToggleShow}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          {show ? (
            <>
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
              <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
              <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </>
          ) : (
            <>
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </>
          )}
        </svg>
      </Button>
      )}
      {editing && value && (
        <Button
          variant="ghost"
          size="icon"
          title="Copy token"
          onClick={() => void writeText(value)}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
        </Button>
      )}
    </div>
  );
}

// ─── 配额查询脚本（quota-scripts T6）────────────────────────────────────
// 变体下拉（registry quota_scripts + 自定义伪变体）+ requires 动态表单。
// 数据流：usePlatformForm 持 state（变体列表经 getDefaultQuotaScripts 异步拉取），
//   本组件纯 props 渲染；requires 值 / 变体 id / 自定义正文的 extra 序列化见
//   services/api/platforms.ts::serializeQuotaScriptConfig。

export { QUOTA_CUSTOM_VARIANT };


/** QuotaScriptSection 的裸内容（合区后由 QuotaSection 的 Tab 内渲染；本组件保留旧签名供测试）。 */
export function QuotaScriptFields({
  protocol, variants, onVariantChange, customScript, onCustomScriptChange, requires, onRequiresChange, locale, t,
  customOnly, selection, fellBack, selVariant, baseUrl, apiKey,
}: {
  protocol: Protocol;
  variants: QuotaScriptVariant[];
  onVariantChange: (v: string) => void;
  customScript: string;
  onCustomScriptChange: (v: string) => void;
  requires: Record<string, string>;
  onRequiresChange: (key: string, value: string) => void;
  locale?: string;
  t: TFunction;
  customOnly: boolean;
  selection: string;
  fellBack: boolean;
  selVariant: QuotaScriptVariant | null;
  baseUrl?: string;
  apiKey?: string;
}) {
  const [magpieNames, setMagpieNames] = useState<string[]>([]);
  const [magpieNamesLoading, setMagpieNamesLoading] = useState(false);
  const [magpieNamesError, setMagpieNamesError] = useState<string | null>(null);
  const loadMagpieNames = async () => {
    setMagpieNamesLoading(true);
    setMagpieNamesError(null);
    try { setMagpieNames(await quotaApi.magpieNames(baseUrl ?? "", apiKey ?? "")); }
    catch (e) { setMagpieNamesError(e instanceof Error ? e.message : String(e)); }
    finally { setMagpieNamesLoading(false); }
  };
  return (
    <>
      {customOnly ? (
        <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("platform.quotaScript.noBuiltin", "This platform has no built-in lookup script. Write your own below; once saved, a refresh button appears on the platform card.")}
        </div>
      ) : (
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
          {t("platform.quotaScript.variant", "Script variant")}
        </div>
        <Select value={selection || QUOTA_CUSTOM_VARIANT} onValueChange={onVariantChange}>
          <SelectTrigger className="input" data-testid="quota-variant-select">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {variants.map(v => (
              <SelectItem key={v.id} value={v.id}>{quotaVariantLabel(v.name, v.id, locale)}</SelectItem>
            ))}
            <SelectItem value={QUOTA_CUSTOM_VARIANT}>{t("platform.quotaScript.custom", "Custom script")}</SelectItem>
          </SelectContent>
        </Select>
        {fellBack && (
          <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {t("platform.quotaScript.fellBack", "Selected variant no longer exists; fell back to the first one")}
          </div>
        )}
      </div>
      )}
      {selection === QUOTA_CUSTOM_VARIANT ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
            {t("platform.quotaScript.customLabel", "Custom JS script")}
          </div>
          <textarea
            className="input"
            placeholder={t("platform.quotaScript.customPlaceholder", "// return { success, balance, tiers, newapi_user_id }")}
            data-testid="quota-custom-script"
            value={customScript}
            onChange={(e) => onCustomScriptChange(e.target.value)}
            spellCheck={false}
            rows={10}
            style={{ resize: "vertical", minHeight: 120, fontFamily: "var(--font-mono, monospace)", fontSize: 14, lineHeight: 1.6 }}
          />
          <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {t("platform.quotaScript.customHint", "ctx provides baseUrl / apiKey / extra (requires params at the top level of extra); http.get / http.post available for outbound calls (system proxy, logged).")}
          </div>
        </div>
      ) : (selVariant?.requires ?? []).map(r => (
        <div key={r.key} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
            {quotaVariantLabel(r.label, r.key, locale)}
          </div>
          {r.key === "magpie_name" ? (
            <>
              <div style={{ display: "flex", gap: 8 }}>
                <Select value={requires.magpie_name || "__none__"} onValueChange={v => onRequiresChange("magpie_name", v === "__none__" ? "" : v)}>
                  <SelectTrigger className="input" data-testid="magpie-account-select"><SelectValue placeholder={t("platform.quotaScript.magpiePlaceholder", "Select Magpie account")} /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">{t("platform.quotaScript.magpieSelectAccount", "Select account")}</SelectItem>
                    {magpieNames.map(name => <SelectItem key={name} value={name}>{name}</SelectItem>)}
                  </SelectContent>
                </Select>
                <Button type="button" variant="outline" disabled={magpieNamesLoading} onClick={() => void loadMagpieNames()}>
                  {magpieNamesLoading ? t("platform.quotaScript.magpieLoadingNames", "Loading…") : t("platform.quotaScript.magpieLoadNames", "Load names")}
                </Button>
              </div>
              {magpieNamesError && <div role="alert">{magpieNamesError}</div>}
            </>
          ) : (
            <Input
              className="input"
              type="text"
              placeholder={r.key}
              value={requires[r.key] ?? ""}
              onChange={(e) => onRequiresChange(r.key, e.target.value)}
            />
          )}
        </div>
      ))}
      {/* New API 用户 ID：非 requires 字段，保留旧表单入口（查询结果 newapi_user_id 的回填目标，
          回填链现状见 notes/01；序列化写 extra.newapi.user_id）。 */}
      {protocol === "newapi" && selection !== QUOTA_CUSTOM_VARIANT && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
            {t("platform.newapiUserId", "User ID")}
          </div>
          <Input
            data-testid="quota-newapi-user-id"
            className="input"
            type="text"
            placeholder={t("platform.newapiUserIdPlaceholder", "Numeric ID (optional)")}
            value={requires.user_id ?? ""}
            onChange={(e) => onRequiresChange("user_id", e.target.value)}
          />
        </div>
      )}
    </>
  );
}

export function DevinConfigSection({ config, onChange, t }: {
  config: DevinConfig;
  onChange: React.Dispatch<React.SetStateAction<DevinConfig>>;
  t: TFunction;
}) {
  const devinModes = ["normal", "fast", "lite", "ultra", "fusion"];
  return (
    <FormSection
      title={t("platform.devinConfig", "Devin Configuration")}
      desc={t("platform.devinConfigHint", "API Key uses cog_ prefixed credential; Organization ID is configured in the \"Quota Query Script\" section")}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
          {t("platform.devinTimeout", "Session timeout (seconds, optional)")}
        </div>
        <Input
          className="input"
          type="number"
          min={0}
          placeholder={t("platform.devinTimeoutPlaceholder", "Default 300")}
          value={config.devin_timeout}
          onChange={(e) => onChange(prev => ({ ...prev, devin_timeout: e.target.value }))}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
          {t("platform.devinMode", "Default mode (optional)")}
        </div>
        {/* radix Select 禁 value="" → __none__ 哨兵映射回空串 */}
        <Select
          value={config.devin_mode || "__none__"}
          onValueChange={(v) => onChange(prev => ({ ...prev, devin_mode: v === "__none__" ? "" : v }))}
        >
          <SelectTrigger className="input">
            <SelectValue placeholder={t("platform.devinModeAuto", "Auto-map by model")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">{t("platform.devinModeAuto", "Auto-map by model")}</SelectItem>
            {devinModes.map(m => (
              <SelectItem key={m} value={m}>{m}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </FormSection>
  );
}

export function PassthroughConfigSection({ draft, patch, showKey, setShowKey, t }: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  showKey: boolean; setShowKey: React.Dispatch<React.SetStateAction<boolean>>;
  t: TFunction;
}) {
  const endpoints = draft.endpoints;
  const setEndpoints = draftSetter(draft, patch, "endpoints");
  const apiKey = draft.apiKey;
  const setApiKey = draftSetter(draft, patch, "apiKey");
  return (
    <FormSection title={t("platform.sectionPassthrough", "Passthrough Config")}>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("platform.passthroughBaseUrl", "Upstream Base URL")}
        </div>
        <Input
          className="input"
          placeholder="https://api.anthropic.com"
          value={endpoints[0]?.base_url ?? ""}
          onChange={(e) => {
            const next = [...endpoints];
            if (next.length === 0) {
              next.push({ protocol: "anthropic" as Protocol, base_url: e.target.value, client_type: "default", coding_plan: false });
            } else {
              next[0] = { ...next[0], base_url: e.target.value };
            }
            setEndpoints(next);
          }}
        />
        <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("platform.passthroughBaseUrlHint", "Enter host root (e.g. https://api.anthropic.com). Passthrough forwards original path/query directly, do not include version prefix.")}
        </div>
      </div>
      <ApiKeyField
        value={apiKey} onChange={setApiKey} show={showKey} onToggleShow={() => setShowKey(!showKey)}
        placeholder={t("platform.apiKeyOptional", "Token (optional, leave empty for passthrough)")}
      />
      <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
        {t("platform.passthroughNote", "Passthrough: client headers (including OAuth) and body are forwarded as-is. aidog performs no conversion or auth injection. Token can be empty.")}
      </div>
    </FormSection>
  );
}

// ─── 自定义透传（passthrough 协议）专属配置 ─────────────────────────────

/** header 预设（共识稿 2026-10-08 §2）：Authorization Bearer 默认 / 三家裸 key header / psfix-03 补 4 种授权前缀形态（Key / token / APIKey / API-KEY）/ 自定义。 */
export const PASS_THROUGH_HEADER_PRESETS: { id: string; header: string; template: string }[] = [
  { id: "bearer", header: "Authorization", template: "Bearer {key}" },
  { id: "x-api-key", header: "x-api-key", template: "{key}" },
  { id: "api-key", header: "api-key", template: "{key}" },
  { id: "x-goog-api-key", header: "x-goog-api-key", template: "{key}" },
  { id: "key", header: "Authorization", template: "Key {key}" },
  { id: "token", header: "Authorization", template: "token {key}" },
  { id: "apikey", header: "Authorization", template: "APIKey {key}" },
  { id: "api-key-dash", header: "Authorization", template: "API-KEY {key}" },
];
const PASS_THROUGH_CUSTOM = "__custom__";

export function PassThroughSection({
  draft, patch, showKey, setShowKey, t,
}: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  showKey: boolean; setShowKey: React.Dispatch<React.SetStateAction<boolean>>;
  t: TFunction;
}) {
  const endpoints = draft.endpoints;
  const setEndpoints = draftSetter(draft, patch, "endpoints");
  const apiKey = draft.apiKey;
  const setApiKey = draftSetter(draft, patch, "apiKey");
  const auth = draft.passAuth;
  const setAuth = draftSetter(draft, patch, "passAuth");
  // 预设选中值派生：header+template 精确命中某预设 → 该预设 id；否则自定义。
  const matchedPreset = PASS_THROUGH_HEADER_PRESETS.find(
    p => p.header === auth.auth_header.trim() && p.template === auth.auth_template.trim(),
  );
  const selection = matchedPreset ? matchedPreset.id : PASS_THROUGH_CUSTOM;
  return (
    <FormSection
      title={t("platform.passthrough.sectionTitle", "Passthrough Settings")}
      desc={t("platform.passthrough.note", "Auth only: client auth headers are stripped and re-injected with the apikey header above; all other headers and the body are forwarded as-is, with the original path/query appended to the base URL.")}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("platform.passthroughBaseUrl", "Upstream Base URL")}
        </div>
        <Input
          className="input"
          placeholder="https://api.example.com"
          value={endpoints[0]?.base_url ?? ""}
          onChange={(e) => {
            const next = [...endpoints];
            if (next.length === 0) {
              next.push({ protocol: "passthrough" as Protocol, base_url: e.target.value, client_type: "default", coding_plan: false });
            } else {
              next[0] = { ...next[0], base_url: e.target.value };
            }
            setEndpoints(next);
          }}
        />
        <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("platform.passthroughBaseUrlHint", "Enter host root (e.g. https://api.anthropic.com). Passthrough forwards original path/query directly, do not include version prefix.")}
        </div>
      </div>
      <ApiKeyField
        value={apiKey} onChange={setApiKey} show={showKey} onToggleShow={() => setShowKey(!showKey)}
        placeholder={t("platform.tokenPlaceholderEdit", "Token")}
      />
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
          {t("platform.passthrough.headerPreset", "Auth Header Preset")}
        </div>
        <Select
          value={selection}
          onValueChange={(v) => {
            if (v === PASS_THROUGH_CUSTOM) {
              setAuth(prev => ({ ...prev, auth_header: "", auth_template: "" }));
            } else {
              const p = PASS_THROUGH_HEADER_PRESETS.find(x => x.id === v);
              if (p) setAuth({ auth_header: p.header, auth_template: p.template });
            }
          }}
        >
          <SelectTrigger className="input">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PASS_THROUGH_HEADER_PRESETS.map(p => (
              <SelectItem key={p.id} value={p.id}>{`${p.header}: ${p.template}`}</SelectItem>
            ))}
            <SelectItem value={PASS_THROUGH_CUSTOM}>{t("platform.passthrough.headerCustom", "Custom")}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      {selection === PASS_THROUGH_CUSTOM && (
        <>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.passthrough.headerName", "Header Name")}
            </div>
            <Input
              className="input"
              placeholder="X-My-Auth"
              value={auth.auth_header}
              onChange={(e) => setAuth(prev => ({ ...prev, auth_header: e.target.value }))}
            />
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.passthrough.headerTemplate", "Value Template")}
            </div>
            <Input
              className="input"
              placeholder="Bearer {key}"
              value={auth.auth_template}
              onChange={(e) => setAuth(prev => ({ ...prev, auth_template: e.target.value }))}
            />
            <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
              {t("platform.passthrough.headerTemplateHint", "The {key} placeholder is replaced with the apikey above.")}
            </div>
          </div>
        </>
      )}
    </FormSection>
  );
}

/** registry 内置档位 → 一组新 ManualBudget（各条独立 id，consumed 从 0 起算）。 */
function tierToBudgets(tier: PlanQuotaTier): ManualBudget[] {
  return tier.budgets.map(b => ({
    ...newManualBudget(),
    kind: b.kind,
    unit: b.unit,
    amount: b.amount,
    window_hours: b.window_hours ?? null,
    window_unit: b.window_unit ?? "hour",
  }));
}


/** ManualBudgetsSection 的裸内容（合区后由 QuotaSection 的 Tab 内渲染；本组件保留旧签名供测试）。
 *  档位填入行原在 FormSection action 槽，合区后作为内容首行（与 03 票原型一致）。 */
export function ManualBudgetsFields({ budgets, setBudgets, tiers, setTierId, selectedTier, t }: {
  budgets: ManualBudget[];
  setBudgets: React.Dispatch<React.SetStateAction<ManualBudget[]>>;
  tiers: PlanQuotaTier[];
  setTierId: (id: string) => void;
  selectedTier: PlanQuotaTier | undefined;
  t: TFunction;
}) {
  // 键入态（按行 id）：保留用户键入的原始字符串（失焦回落到解析后的持久值），
  // 避免 `value={b.amount || ""}` 把 "0" 回写成 "" 导致无法键入 0.x（frontend-refactor-tests 渲染期回落禁令）
  const [amountDraft, setAmountDraft] = useState<Record<string, string>>({});
  return (
    <>
      {/* 实时态条：限额计数（用量扣减数据未回传表单，先展示静态配置态） */}
      <div className="aidog-anim-rise" style={C.strip}>
        <div style={C.tile("var(--color-success)")}>{t("platform.tile.quota", "Q")}</div>
        <span style={C.pill("neutral")}>
          {t("platform.manualBudgetCount", "{{n}} budgets", { n: budgets.length })}
          {budgets.length > 0 && ` · ${budgets.filter(b => b.enabled).length}/${budgets.length}`}
        </span>
        {budgets.length === 0 && (
          <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
            {t("platform.manualBudgetEmpty", "No limits yet. Click \"Add limit\" to configure.")}
          </span>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap", marginBottom: 10 }}>
          {tiers.length > 0 && (
            <>
              {tiers.length > 1 && (
                <Select value={selectedTier?.id ?? ""} onValueChange={setTierId}>
                  <SelectTrigger className="input" style={{ width: 120, flexShrink: 0, height: 28, fontSize: 14 }}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {tiers.map(x => <SelectItem key={x.id} value={x.id}>{x.name}</SelectItem>)}
                  </SelectContent>
                </Select>
              )}
              <Button
                variant="ghost"
                size="sm"
                style={{ fontSize: 14, gap: 4, padding: "4px 10px", color: "var(--primary)" }} // 稿 .fillbtn = primary
                title={selectedTier?.source_url}
                onClick={() => selectedTier && setBudgets(tierToBudgets(selectedTier))}
              >
                {t("platform.manualBudgetUseTier", "Fill built-in quota ({{tier}})").replace("{{tier}}", selectedTier?.name ?? "")}
              </Button>
            </>
          )}
      </div>
      {budgets.map((b, idx) => {
        const update = (patch: Partial<ManualBudget>) =>
          setBudgets(budgets.map((x, i) => i === idx ? { ...x, ...patch } : x));
        const needsWindow = b.kind === "rolling" || b.kind === "fixed";
        const onKindChange = (kind: ManualBudgetKind) => {
          const willNeedWindow = kind === "rolling" || kind === "fixed";
          // 切到 rolling/fixed 且尚无窗口配置 → 给合理默认（7 天）
          if (willNeedWindow && (b.window_hours == null || b.window_hours <= 0)) {
            update({ kind, window_hours: 7, window_unit: "day" });
          } else {
            update({ kind });
          }
        };
        return (
          <div key={b.id} className={C.rowClass} style={{ ...C.row, opacity: b.enabled ? undefined : 0.65 }}>
            <Select
              value={b.kind}
              onValueChange={(v) => onKindChange(v as ManualBudgetKind)}
            >
              <SelectTrigger className="input" style={{ width: 110, flexShrink: 0 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="total">{t("platform.manualBudgetKindTotal", "Total")}</SelectItem>
                <SelectItem value="rolling">{t("platform.manualBudgetKindRolling", "Rolling")}</SelectItem>
                <SelectItem value="fixed">{t("platform.manualBudgetKindFixed", "Fixed")}</SelectItem>
                <SelectItem value="daily">{t("platform.manualBudgetKindDaily", "Daily")}</SelectItem>
              </SelectContent>
            </Select>
            {/* 金额：C 方向稿大数字底边线输入（4-手动预算区.html .amt） */}
            <Input
              className="aidog-bignum"
              type="number"
              min={0}
              step="any"
              style={{ width: 100, flexShrink: 0, height: "auto", fontSize: 15, fontWeight: 300, border: 0, borderBottom: "1px solid var(--border)", borderRadius: 0, background: "transparent" }}
              placeholder={t("platform.manualBudgetAmount", "Amount")}
              data-testid="manual-budget-amount"
              value={amountDraft[b.id] ?? (b.amount || "")}
              onChange={e => { setAmountDraft(m => ({ ...m, [b.id]: e.target.value })); update({ amount: parseFloat(e.target.value) || 0 }); }}
              onBlur={() => setAmountDraft(m => { const { [b.id]: _drop, ...rest } = m; return rest; })}
            />
            <Select
              value={b.unit}
              onValueChange={(v) => update({ unit: v as ManualBudgetUnit })}
            >
              <SelectTrigger className="input" style={{ width: 90, flexShrink: 0 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="usd">$ USD</SelectItem>
                <SelectItem value="token">{t("platform.manualBudgetUnitToken", "Token")}</SelectItem>
                <SelectItem value="count">{t("platform.manualBudgetUnitCount", "Requests")}</SelectItem>
              </SelectContent>
            </Select>
            {needsWindow ? (
              <>
                <Input
                  className="input"
                  type="number"
                  min={0}
                  step="any"
                  style={{ width: 80, flexShrink: 0, textAlign: "center", fontVariantNumeric: "tabular-nums" }}
                  placeholder={t("platform.manualBudgetWindow", "Window")}
                  value={b.window_hours ?? ""}
                  onChange={e => update({ window_hours: e.target.value === "" ? null : (parseFloat(e.target.value) || 0) })}
                />
                <Select
                  value={b.window_unit ?? "hour"}
                  onValueChange={(v) => update({ window_unit: v as WindowUnit })}
                >
                  <SelectTrigger className="input" style={{ width: 90, flexShrink: 0 }}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="minute">{t("platform.windowUnitMinute", "Minutes")}</SelectItem>
                    <SelectItem value="hour">{t("platform.windowUnitHour", "Hours")}</SelectItem>
                    <SelectItem value="day">{t("platform.windowUnitDay", "Days")}</SelectItem>
                    <SelectItem value="week">{t("platform.windowUnitWeek", "Weeks")}</SelectItem>
                    <SelectItem value="month">{t("platform.windowUnitMonth", "Months")}</SelectItem>
                  </SelectContent>
                </Select>
              </>
            ) : (
              /* 无窗口类型（总额/每日）：破折号占位保持行内对齐（设计稿 .dash） */
              <span style={{ color: "var(--text-tertiary)" }}>—</span>
            )}
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 14, color: "var(--text-secondary)", cursor: "pointer" }}>
              <Switch
                checked={b.enabled}
                onCheckedChange={v => update({ enabled: v === true })}
              />
              {t("platform.manualBudgetEnabled", "Enabled")}
            </label>
            <Button
              variant="ghost"
              size="icon"
              className="btn-danger"
              style={{ flexShrink: 0, color: "var(--color-danger)", marginLeft: "auto" }}
              title={t("action.delete", "Delete")}
              onClick={() => setBudgets(budgets.filter((_, i) => i !== idx))}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
              </svg>
            </Button>
          </div>
        );
      })}
      <button
        type="button"
        className={C.addBtnClass}
        onClick={() => setBudgets([...budgets, newManualBudget()])}
      >
        + {t("platform.manualBudgetAdd", "Add limit")}
      </button>
    </>
  );
}

export function BreakerSection({ defaults, draft, patch, t }: {
  defaults: SchedulingBreakerSettings | null;
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  t: TFunction;
}) {
  const failure = draft.breakerFailureThreshold;
  const setFailure = draftSetter(draft, patch, "breakerFailureThreshold");
  const halfOpenMax = draft.breakerHalfOpenMax;
  const setHalfOpenMax = draftSetter(draft, patch, "breakerHalfOpenMax");
  return (
    <FormSection
      title={t("platform.breakerTitle", "Circuit breaker")}
      desc={t("platform.breakerDesc", "Temporarily remove this platform after consecutive failures reach the threshold; recover via half-open probes after cooldown. Leave empty to inherit the global defaults from System Settings.")}
    >
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: "10px 12px" }}>
        <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("platform.breakerFailureThreshold", "Failure threshold")}</span>
        <Input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "Inherit default {{n}}").replace("{{n}}", String(defaults.breaker_failure_threshold)) : t("platform.breakerInheritGeneric", "Inherit default")}
          value={failure}
          onChange={e => setFailure(e.target.value)}
        />
        <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "Half-open probes")}</span>
        <Input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "Inherit default {{n}}").replace("{{n}}", String(defaults.breaker_half_open_max)) : t("platform.breakerInheritGeneric", "Inherit default")}
          value={halfOpenMax}
          onChange={e => setHalfOpenMax(e.target.value)}
        />
      </div>
    </FormSection>
  );
}

// ─── Peak Hours（高峰/低峰时段倍率）─────────────────────
// 时区切换：仅前端运行时态（默认本地，可切 UTC+0），不持久化。
// 存储恒 UTC+0；展示 / 输入按选中时区换算，换算内核见 src/utils/peak.ts（shiftClock，按绝对分钟，半时区精确）。



/** 窗口预览可读时段：半开区间 [start, end) → 显示 end-1:59:59。
 *  格式：HH:MM:SS - HH:MM:SS（<tz 标签>）+ (次日) 如跨天。
 *  全天特例：end==24 或 start==end 退化 → 00:00:00 - 23:59:59。
 *  时区标签：窗口带 timezone → 存值即该时区本地值，直接显示 tz 名；
 *  无 timezone（= UTC 存储）→ 按 tzMode 换算显示。 */
function formatWindowPreview(w: TimeWindow, tzMode: TzMode, t: TFunction): string {
  // start：hour+minute 一起换算（带 timezone 窗口存值即本地，不换算）
  const startDisplay = w.timezone
    ? { hour: w.start_hour, minute: w.start_minute ?? 0 }
    : utcToDisplay(w.start_hour, w.start_minute ?? 0, tzMode);

  // end：先算绝对分钟 -1（半开区间），再拆 hour:minute，再对 hour+minute 一起换算
  const endTotalMin = w.end_hour * 60 + (w.end_minute ?? 0);
  const endMinusOneMin = endTotalMin - 1;
  // 负数 → 23:59（跨天边界，如 0:00 -1 = -1 → 前一天 23:59）
  const endHourRaw = endMinusOneMin < 0 ? 23 : Math.floor(endMinusOneMin / 60);
  const endMinRaw = endMinusOneMin < 0 ? 59 : endMinusOneMin % 60;
  const endDisplay = w.timezone
    ? { hour: endHourRaw, minute: endMinRaw }
    : utcToDisplay(endHourRaw, endMinRaw, tzMode);

  const startStr = `${pad(startDisplay.hour)}:${pad(startDisplay.minute)}:00`;
  const endStr = `${pad(endDisplay.hour)}:${pad(endDisplay.minute)}:59`;

  const tzLabel = w.timezone ?? (tzMode === "local"
    ? t("platform.timezone_local", "Local")
    : t("platform.timezone_utc", "UTC+0"));

  // 跨天判定：end_hour < start_hour（原始 UTC 值，不在 display 层比较）
  const isNextDay = w.end_hour < w.start_hour;
  const nextDayLabel = isNextDay
    ? t("platform.peak_next_day", "Next day")
    : "";

  return `${startStr} - ${endStr} (${tzLabel})${nextDayLabel ? ` (${nextDayLabel})` : ""}`;
}

export function PeakSection({ draft, patch, tzMode, setTzMode, protocol, platformCalendar, themeMode, t }: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  /** tzMode/setTzMode：展示态（windowsTz），不进 draft。 */
  tzMode: TzMode;
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>;
  protocol: Protocol;
  platformCalendar?: string;
  themeMode: ThemeMode;
  t: TFunction;
}) {
  const windows = draft.peak;
  const setWindows = draftSetter(draft, patch, "peak");
  const disableDuringPeak = draft.disableDuringPeak;
  const setDisableDuringPeak = draftSetter(draft, patch, "disableDuringPeak");
  const [defaultCache, setDefaultCache] = useState<TimeWindow[] | null>(null);
  const windowsRef = useRef(windows);
  const autoImportAttempted = useRef(false);
  const [modalOpen, setModalOpen] = useState(false);
  // model scope 候选列表：从 preset model_list 拉取，chip 编辑 + datalist 自动补全用。
  const [modelList, setModelList] = useState<string[]>([]);
  // 每窗口 model 输入框临时态（idx → 输入串），Enter/逗号/失定提交为 chip。
  const [modelInput, setModelInput] = useState<Record<number, string>>({});
  const localTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";

  useEffect(() => {
    // 竞态守卫：快速切协议时旧协议的默认 peak 后到会覆盖新协议的缓存，
    // 「导入默认配置」按钮按错误协议导入；对齐下方 getDefaultModelList 的 cancelled 写法
    let cancelled = false;
    getDefaultPeak(protocol)
      .then((w) => { if (!cancelled) setDefaultCache(w); })
      .catch(() => { if (!cancelled) setDefaultCache(null); });
    return () => { cancelled = true; };
  }, [protocol]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const list = await getDefaultModelList(protocol);
      if (!cancelled) setModelList(list);
    })();
    return () => { cancelled = true; };
  }, [protocol]);

  const handleImportDefault = () => {
    if (!defaultCache || defaultCache.length === 0) return;
    const copied = defaultCache.map(w => ({
      ...w,
      days_of_week: w.days_of_week ? [...w.days_of_week] : undefined,
      models: w.models ? [...w.models] : undefined,
      timezone: w.timezone ?? localTimezone,
    }));
    setWindows(copied);
    setModalOpen(false);
  };
  useEffect(() => { windowsRef.current = windows; }, [windows]);
  useEffect(() => {
    if (defaultCache === null || autoImportAttempted.current) return;
    autoImportAttempted.current = true;
    if (windowsRef.current.length === 0 && defaultCache.length > 0) handleImportDefault();
    // 初次编辑水合后只尝试一次，避免用户清空窗口时被重新导入。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [defaultCache]);
  const update = (idx: number, patch: Partial<TimeWindow>) => {
    setWindows(prev => prev.map((w, i) => i === idx
      ? { ...w, ...patch, timezone: "timezone" in patch ? patch.timezone : w.timezone ?? localTimezone }
      : w));
  };
  const remove = (idx: number) => {
    setWindows(prev => prev.filter((_, i) => i !== idx));
  };
  const add = () => {
    setWindows(prev => [...prev, { start_hour: 0, end_hour: 24, multiplier: 1.0, timezone: localTimezone }]);
  };
  const toggleDay = (idx: number, day: number) => {
    setWindows(prev => prev.map((w, i) => {
      if (i !== idx) return w;
      const cur = w.days_of_week ?? [];
      const next = cur.includes(day) ? cur.filter(d => d !== day) : [...cur, day].sort();
      return { ...w, days_of_week: next.length === 0 ? undefined : next };
    }));
  };
  /** model chip 添加（去重 + trim；空串忽略）。 */
  const addModel = (idx: number, raw: string) => {
    const v = raw.trim();
    if (!v) return;
    setWindows(prev => prev.map((w, i) => {
      if (i !== idx) return w;
      const cur = w.models ?? [];
      if (cur.includes(v)) return w;
      return { ...w, models: [...cur, v] };
    }));
  };
  /** model chip 删除；删空 → models 字段置 undefined（= 全平台，向后兼容）。 */
  const removeModel = (idx: number, mIdx: number) => {
    setWindows(prev => prev.map((w, i) => {
      if (i !== idx) return w;
      const cur = w.models ?? [];
      const next = cur.filter((_, j) => j !== mIdx);
      return { ...w, models: next.length === 0 ? undefined : next };
    }));
  };

  // 高峰禁用预览：实时算（基于当前 windows + now）。无窗口 → 视为非高峰（不会触发排除）。
  const nowPeak = isCurrentlyPeak(windows, Date.now(), "", platformCalendar);
  const nowMult = currentPeakMultiplier(windows, Date.now(), "", platformCalendar);

  return (
    <FormSection
      title={t("platform.peak", "Peak hours multiplier")}
      desc={t("platform.peak_desc", "Set time-window multipliers (>1 surcharge / <1 discount). Windows are evaluated in their own timezone's local time; timezone defaults to UTC. Multi-window first-match in order; no match = 1.0.")}
      action={
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <div style={{ display: "flex", gap: 4, padding: 2, background: "var(--bg-glass)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
            {(["local", "utc"] as const).map(m => (
              <Button
                key={m}
                variant="ghost"
                size="sm"
                style={tzMode === m
                  ? { padding: "2px 8px", fontSize: 14, height: "auto", fontWeight: 400, background: "var(--foreground)", color: "var(--background)", borderColor: "transparent" }
                  : { padding: "2px 8px", fontSize: 14, height: "auto", fontWeight: 400 }}
                onClick={() => setTzMode(m)}
              >
                {m === "local" ? t("platform.timezone_local", "Local") : t("platform.timezone_utc", "UTC+0")}
              </Button>
            ))}
          </div>
          <Button
            variant="ghost"
            size="sm"
            style={{ padding: "2px 8px", fontSize: 14, whiteSpace: "nowrap", height: "auto" }}
            disabled={!defaultCache || defaultCache.length === 0}
            title={!defaultCache || defaultCache.length === 0 ? t("platform.peak_no_default", "No default peak config for this platform") : ""}
            onClick={() => setModalOpen(true)}
          >
            {t("platform.peak_import_default", "Import default")}
          </Button>
        </div>
      }
    >
      {/* 实时态条：当前生效倍率（first-match 实时判定，与路由同 hit 逻辑） */}
      <div className="aidog-anim-rise" style={C.strip}>
        <div style={C.tile("var(--color-warning)")}>{t("platform.tile.multiplier", "×")}</div>
        <span style={C.pill(nowPeak ? "hot" : "neutral")}>
          {nowPeak
            ? t("platform.peakPillPeak", "Peak ×{{n}}").replace("{{n}}", String(nowMult))
            : t("platform.peakPillOff", "Off-peak ×1.0")}
        </span>
        {windows.length > 0 && (
          <span style={C.pill("neutral")}>
            {t("platform.rateLimits.windowsCount", "{{n}} windows", { n: windows.length })}
          </span>
        )}
      </div>

      {/* 高峰禁用开关：启用后该平台在 peak window 命中时从路由候选排除（不改 status，临时闸门）。 */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "9px 12px", borderRadius: "var(--radius-sm)", background: "var(--card)", border: "1px solid var(--border)" }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, color: "var(--text-secondary)", cursor: "pointer" }}>
          <Switch
            checked={disableDuringPeak}
            onCheckedChange={v => setDisableDuringPeak(v === true)}
          />
          <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{t("platform.disable_during_peak", "Disable during peak")}</span>
        </label>
        {/* 实时态徽标恒显（稿 2 badge-peak；2026-10-27 拍板 q6）：高峰编辑直接相关，off 态也报当前状态 */}
        <span
            style={{
              fontSize: 14, fontWeight: 600, padding: "2px 8px", borderRadius: 5, whiteSpace: "nowrap",
              color: nowPeak ? "var(--color-danger)" : "var(--text-tertiary)",
              background: nowPeak
                ? "color-mix(in srgb, var(--color-danger) 12%, transparent)"
                : "transparent",
              border: nowPeak
                ? "1px solid color-mix(in srgb, var(--color-danger) 30%, transparent)"
                : "1px solid var(--border)",
            }}
          >
            {nowPeak
              ? t("platform.currently_peak", "Current: peak")
              : t("platform.currently_off_peak", "Current: off-peak")}
        </span>
        <span style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.4 }}>
          {t("platform.disable_during_peak_desc", "When on, this platform is excluded from routing candidates during peak hours (any peak window hit). Does not change status — temporary gate; recovers when toggled off or outside the window.")}
        </span>
      </div>
      {windows.length === 0 && (
        <div style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
          {t("platform.peak_empty", "Not configured → use preset default or 1.0 (no adjustment)")}
        </div>
      )}
      {windows.map((w, idx) => (
        <div key={idx} className="aidog-anim-rise" style={{ display: "flex", flexDirection: "column", gap: 6, padding: 8, borderRadius: "var(--radius-sm)", background: "var(--card)", border: "1px solid var(--border)" }}>
          {/* 顶行：时段 + 倍率 + 周日 + 删除 */}
          <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
            {(() => {
              // 起/止 hour:minute —— 无 timezone（= UTC 存储）按 tzMode 双向换算；
              // 带 timezone：存值即该时区本地值，直接读写不换算（半时区精确到分钟）。
              const toDisp = (h: number, m: number) =>
                w.timezone ? { hour: h, minute: m } : utcToDisplay(h, m, tzMode);
              const fromDisp = (h: number, m: number) =>
                w.timezone ? { hour: h, minute: m } : displayToUtc(h, m, tzMode);
              const startDisp = toDisp(w.start_hour, w.start_minute ?? 0);
              const endDisp = toDisp(w.end_hour, w.end_minute ?? 0);
              return (
                <>
                  <TimePicker
                    suffix={t("platform.start_hour", "Start")} hour={startDisp.hour} minute={startDisp.minute} maxHour={23}
                    t={t}
                    onSet={(h, m) => {
                      const utc = fromDisp(h, m);
                      update(idx, { start_hour: utc.hour, start_minute: utc.minute });
                    }}
                  />
                  <TimePicker
                    suffix={t("platform.end_hour", "End")} hour={endDisp.hour} minute={endDisp.minute} maxHour={24}
                    t={t}
                    onSet={(h, m) => {
                      const utc = fromDisp(h, m);
                      update(idx, { end_hour: utc.hour, end_minute: utc.minute });
                    }}
                  />
                </>
              );
            })()}
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.multiplier", "Mult")}
              <Input
                className="input" type="number" step={0.1} min={0} style={{ width: 84 }}
                value={w.multiplier}
                onChange={e => update(idx, { multiplier: Number(e.target.value) || 1 })}
              />
            </label>
            {/* 窗口时区：__utc__ 哨兵 = 无 timezone 字段（= UTC，向后兼容）；
                切换时区只改解释基准，存值数字不动（用户自定时段含义）。 */}
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.window_timezone", "Timezone")}
              <Select
                value={w.timezone ?? "__utc__"}
                onValueChange={(v) => update(idx, { timezone: v === "__utc__" ? undefined : v })}
              >
                <SelectTrigger className="input" style={{ width: 150, fontSize: 14, height: 28 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {WINDOW_TIMEZONES.map((tz) => (
                    <SelectItem key={tz} value={tz}>
                      {tz === "__utc__" ? t("platform.window_timezone_utc_default", "UTC (default)") : tz}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <Select
              value={w.day_type ?? "__week__"}
              onValueChange={value => update(idx, { day_type: value === "__week__" ? undefined : value as "workday" | "holiday", days_of_week: undefined, holiday_calendar: value === "__week__" ? undefined : w.holiday_calendar })}
            >
              <SelectTrigger className="input" style={{ width: 125, height: 28, fontSize: 14 }} aria-label={t("platform.holidayCalendar.dayType", "Day type")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="__week__">{t("platform.holidayCalendar.weekday", "Weekdays")}</SelectItem>
                <SelectItem value="workday">{t("platform.windows_dim_workday", "Workdays")}</SelectItem>
                <SelectItem value="holiday">{t("platform.windows_dim_holiday", "Holidays")}</SelectItem>
              </SelectContent>
            </Select>
            {w.day_type && <HolidayCalendarSelect value={w.holiday_calendar} inheritedValue={platformCalendar} timezone={w.timezone} onChange={holiday_calendar => update(idx, { holiday_calendar })} t={t} />}
            {!w.day_type && <div style={{ display: "flex", gap: 2 }}>
              {[0, 1, 2, 3, 4, 5, 6].map(di => {
                const active = (w.days_of_week ?? []).includes(di);
                return (
                  <Button
                    key={di}
                    variant="ghost"
                    size="icon"
                    title={t("platform.days_of_week", "Days of week (0=Sun…6=Sat, empty=every day)")}
                    onClick={() => toggleDay(idx, di)}
                    style={{
                      width: 22, height: 22, padding: 0, fontSize: 14, lineHeight: 1,
                      borderRadius: "var(--radius-sm)", cursor: "pointer",
                      background: active ? "var(--primary)" : "transparent",
                      color: active ? "var(--primary-foreground)" : "var(--text-secondary)",
                      border: "1px solid var(--border)",
                    }}
                  >
                    {t(`platform.weekday_short.${di}`)}
                  </Button>
                );
              })}
            </div>}
            <Button
              variant="ghost"
              size="icon"
              title={t("platform.remove_window", "Remove window")}
              onClick={() => remove(idx)}
              style={{ marginLeft: "auto" }}
            >
              ✕
            </Button>
          </div>
          {/* 窗口预览行：24h spark（命中时段点亮）+ 可读时段（半开区间 [start, end) → end-1:59:59） */}
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <PeakSpark w={w} />
            <div style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.4, minWidth: 0 }}>
              {formatWindowPreview(w, tzMode, t)}
            </div>
          </div>
          {/* model scope（受影响模型）：chip 多选 + 自由输入（支持 `prefix*` 通配）。
              absent / 空 = 全平台（标「全部模型」），UI 暴露 model 维度过滤可见性。 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.peak_model_scope", "Affected models")}
              {(!w.models || w.models.length === 0) && (
                <span style={{ color: "var(--text-tertiary)", marginLeft: 4 }}>
                  · {t("platform.peak_model_scope_all", "All models")}
                </span>
              )}
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
              {(w.models ?? []).map((m, mi) => (
                <span
                  key={mi}
                  className="badge badge-muted"
                  style={{ fontSize: 14, padding: "2px 6px", display: "inline-flex", alignItems: "center", gap: 4 }}
                >
                  {m}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => removeModel(idx, mi)}
                    style={{
                      width: "auto", height: "auto", minWidth: "auto",
                      border: "none", background: "transparent", color: "var(--text-tertiary)",
                      cursor: "pointer", padding: 0, lineHeight: 1, fontSize: 14,
                    }}
                    title={t("platform.remove_window", "Remove window")}
                  >
                    ✕
                  </Button>
                </span>
              ))}
              <Input
                className="input"
                style={{ width: 160, fontSize: 14, padding: "2px 6px" }}
                list={`peak-models-${idx}`}
                placeholder={t("platform.peak_model_placeholder", "Model name or prefix*")}
                value={modelInput[idx] ?? ""}
                onChange={e => setModelInput(s => ({ ...s, [idx]: e.target.value }))}
                onKeyDown={e => {
                  if ((e.key === "Enter" && !isImeComposing(e)) || e.key === ",") {
                    e.preventDefault();
                    addModel(idx, modelInput[idx] ?? "");
                    setModelInput(s => ({ ...s, [idx]: "" }));
                  }
                }}
                onBlur={() => {
                  const v = (modelInput[idx] ?? "").trim();
                  if (v) {
                    addModel(idx, v);
                    setModelInput(s => ({ ...s, [idx]: "" }));
                  }
                }}
              />
              <datalist id={`peak-models-${idx}`}>
                {modelList.filter(m => !(w.models ?? []).includes(m)).map(m => <option key={m} value={m} />)}
              </datalist>
            </div>
          </div>
          {/* 生效期：start_at / end_at（Unix 秒 ↔ datetime-local），平铺恒显（稿 2）。
              非必填：空 = 立即可用 / 永久；福利期自动切换用（design §1.3）。 */}
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", marginTop: 2 }}>
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.peak_start_at", "Effective from")}
              <Input
                id={`peak-start-${idx}`}
                className="input"
                type="datetime-local"
                style={{ width: 180, fontSize: 14, padding: "2px 6px", colorScheme: themeMode }}
                value={secToLocalInput(w.start_at)}
                onChange={e => {
                  const sec = localInputToSec(e.target.value);
                  update(idx, sec == null ? { start_at: undefined } : { start_at: sec });
                }}
              />
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 14, color: "var(--text-secondary)" }}>
              {t("platform.peak_end_at", "Expires at")}
              <Input
                id={`peak-end-${idx}`}
                className="input"
                type="datetime-local"
                style={{ width: 180, fontSize: 14, padding: "2px 6px", colorScheme: themeMode }}
                value={secToLocalInput(w.end_at)}
                onChange={e => {
                  const sec = localInputToSec(e.target.value);
                  update(idx, sec == null ? { end_at: undefined } : { end_at: sec });
                }}
              />
            </label>
          </div>
        </div>
      ))}
      <button type="button" className={C.addBtnClass} style={{ marginTop: 2 }} onClick={add}>
        + {t("platform.add_window", "Add window")}
      </button>
      <Dialog open={modalOpen} onOpenChange={(v) => { if (!v) setModalOpen(false); }}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 400 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 14 }}>
              {t("platform.peak_overwrite_confirm_title", "Overwrite peak config?")}
            </DialogTitle>
            <DialogDescription style={{ fontSize: 14 }}>
              {t("platform.peak_overwrite_confirm_body", "Current peak config will be replaced with default ({{count}} windows). This operation cannot be undone.").replace("{{count}}", String(defaultCache?.length ?? 0))}
            </DialogDescription>
          </DialogHeader>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button
              variant="ghost"
              onClick={() => setModalOpen(false)}
            >
              {t("platform.peak_overwrite_cancel_button", "Cancel")}
            </Button>
            <Button
              onClick={handleImportDefault}
            >
              {t("platform.peak_overwrite_confirm_button", "Confirm")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </FormSection>
  );
}

export function GroupAssignSection({ editing, groupDetails, draft, patch, t }: {
  editing: Platform | null;
  groupDetails: GroupDetail[];
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  t: TFunction;
}) {
  const lockedGroupId = draft.lockedGroupId;
  const autoGroup = draft.autoGroup;
  const setAutoGroup = draftSetter(draft, patch, "autoGroup");
  const joinGroupIds = draft.joinGroupIds;
  const setJoinGroupIds = draftSetter(draft, patch, "joinGroupIds");
  return (
    <FormSection
      title={t("platform.groupAssignTitle", "Group Assignment")}
      desc={t("platform.groupAssignDesc", "Create a default group and join other existing groups at the same time; selecting none means the platform belongs to no group.")}
    >
      {lockedGroupId != null ? (
        <div style={{ fontSize: 14, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
          <span className="badge badge-muted" style={{ padding: "0 6px" }}>
            {groupDetails.find(g => g.group.id === lockedGroupId)?.group.name ?? `#${lockedGroupId}`}
          </span>
          {t("platform.groupLocked", "Locked to this group")}
        </div>
      ) : !editing ? (
        // 创建默认分组是「创建时一次性判断」，仅创建表单显示；编辑表单不再判断建组。
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <span style={{ fontSize: 14 }}>{t("platform.groupAssignAuto", "Create default group")}</span>
          <Switch checked={autoGroup} onCheckedChange={setAutoGroup} />
        </div>
      ) : null}
      {lockedGroupId == null && groupDetails.length > 0 && (
        <>
          <div style={{ fontSize: 14, color: "var(--text-secondary)", margin: "10px 0 6px" }}>
            {t("platform.groupAssignJoin", "Join existing groups")}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {groupDetails
              // 编辑态隐藏该平台自己的 auto 分组（由上方 toggle 管理）。
              .filter(gd => !editing || gd.group.auto_from_platform !== String(editing.id))
              .map(gd => {
                const checked = joinGroupIds.includes(gd.group.id);
                return (
                  <Button
                    key={gd.group.id}
                    variant="ghost"
                    size="sm"
                    onClick={() => setJoinGroupIds(prev => checked
                      ? prev.filter(id => id !== gd.group.id)
                      : [...prev, gd.group.id])}
                    style={{
                      display: "inline-flex", alignItems: "center", height: "auto",
                      padding: "4px 12px", borderRadius: 999, fontSize: 14, fontWeight: 500,
                      cursor: "pointer",
                      border: `1px solid ${checked ? "var(--accent-edge)" : "var(--border)"}`,
                      background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                      color: checked ? "var(--accent)" : "var(--text-secondary)",
                      transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
                    }}
                  >
                    {gd.group.name}
                  </Button>
                );
              })}
          </div>
        </>
      )}
    </FormSection>
  );
}

export function ExpirySection({ draft, patch, expiryEnabled, setExpiryEnabled, themeMode, t }: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  expiryEnabled: boolean; setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  themeMode: "light" | "dark";
  t: TFunction;
}) {
  const expiresAt = draft.expiresAt;
  const setExpiresAt = draftSetter(draft, patch, "expiresAt");
  return (
    <FormSection
      title={t("platform.expiresAt", "Expires at")}
      desc={t("platform.expiresAtHint", "Optional. After expiration the platform is automatically excluded from routing candidates (equivalent to disabled); changing or clearing the value restores it.")}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: expiryEnabled ? 8 : 0 }}>
        <span style={{ fontSize: 14 }}>{t("platform.expiresAtEnable", "Enable expiration")}</span>
        <Switch
          checked={expiryEnabled}
          onCheckedChange={(v) => {
            setExpiryEnabled(v);
            // ON→OFF：清零 expiresAt（不生效）；OFF→ON：保留 expiresAt 若有粘贴识别值（预填）。
            if (!v) setExpiresAt(0);
          }}
        />
      </div>
      {expiryEnabled && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <Input
            className="input"
            type="datetime-local"
            // colorScheme 控 WKWebView 原生日历弹出层明暗；input 本体 color/bg/border 走 .input 的 CSS 变量。
            style={{ flex: 1, minWidth: 200, colorScheme: themeMode }}
            // datetime-local 值为 "YYYY-MM-DDTHH:MM" 本地时间；expiresAt=0 → 空串（未设）。
            value={expiresAt > 0 ? toDatetimeLocal(expiresAt) : ""}
            onChange={(e) => {
              const v = e.target.value;
              if (!v) { setExpiresAt(0); return; }
              // 本地时间 "YYYY-MM-DDTHH:MM" → 毫秒时间戳（new Date 按本地时区解析）。
              const ms = new Date(v).getTime();
              if (Number.isFinite(ms) && ms > 0) setExpiresAt(ms);
            }}
          />
          {expiresAt > 0 && (
            <Button
              variant="ghost"
              size="sm"
              style={{ fontSize: 14, padding: "4px 10px" }}
              onClick={() => setExpiresAt(0)}
            >
              {t("platform.expiresAtClear", "Clear")}
            </Button>
          )}
          {expiresAt > 0 && (
            <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
              {(() => {
                const nowMs = Date.now();
                if (nowMs >= expiresAt) {
                  return t("platform.expired", "Expired");
                }
                const inDay = expiresAt - nowMs < 86_400_000;
                const txt = formatDateTime(expiresAt) || "-";
                return inDay
                  ? t("platform.expiresAtSoon", "Expiring soon: {{time}}", { time: txt })
                  : txt;
              })()}
            </span>
          )}
        </div>
      )}
    </FormSection>
  );
}

// ponytail: EndpointsSection 因体积大移到 formSectionsEndpoints.tsx。
//   在此 re-export 保持 PlatformEditForm 单一 import 入口不变（barrel 模式）。
//   ModelsSection 已被 ModelsMatrixSection 取代（PRD 07-09 合并矩阵 card）。
export { EndpointsSection } from "./formSectionsEndpoints";

// ── 配额查询合区（quota-ia 票 03：Tab 页签「自动脚本 / 手动预算」，互斥，保存时清库）──

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { isImeComposing } from "../../utils/ime";

/** 合区后的「配额查询」section：查询方式 Tab 二选一（票 01 互斥 + 票 03 保存时清库）。
 *  表单内切换只弹确认 + 记忆目标 tab（可反悔，取消编辑无损）；对侧数据不动，
 *  清空发生在点保存（后端按 quota_source 变化清，见 db/platform.rs）。
 *  位置：模型矩阵正下方（PlatformEditForm 编排）。 */
export function QuotaSection({
  onSourceChange,
  protocol, variants, onVariantChange, locale,
  baseUrl, apiKey, platformId,
  draft, patch, editing, t,
}: {
  onSourceChange: (v: "auto" | "manual") => void;
  protocol: Protocol;
  variants: QuotaScriptVariant[];
  onVariantChange: (v: string) => void;
  locale?: string;
  /** 测试按钮用：当前表单主端点 base_url / key 字段 / 编辑中的平台 id（新建 = null）。 */
  baseUrl?: string;
  apiKey?: string;
  platformId?: number | null;
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  editing: boolean;
  t: TFunction;
}) {
  const [confirmTarget, setConfirmTarget] = useState<"auto" | "manual" | null>(null);
  // 草稿字段视图 + patch 适配（票 14）：原逐字段 props 收进 draft + patch。
  const quotaSource = draft.quotaSource;
  const variantId = draft.quotaVariantId;
  const customScript = draft.quotaCustomScript;
  const requires = draft.quotaRequires;
  const budgets = draft.manualBudgets;
  const setBudgets = draftSetter(draft, patch, "manualBudgets");
  const onCustomScriptChange = (v: string) => patch({ quotaCustomScript: v });
  const onRequiresChange = (key: string, value: string) => patch({ quotaRequires: { ...requires, [key]: value } });

  // 测试按钮：就地执行一次配额查询并内联展示结果（后端 platform_query_quota 成功时自行 persist）。
  const [testRunning, setTestRunning] = useState(false);
  const [testResult, setTestResult] = useState<PlatformQuota | null>(null);
  const runQuotaTest = async () => {
    if (testRunning) return;
    setTestRunning(true);
    try {
      setTestResult(await quotaApi.query(baseUrl ?? "", apiKey ?? "", platformId ?? undefined, requires));
    } catch (e) {
      setTestResult({
        success: false,
        error: e instanceof Error ? e.message : String(e),
        queried_at: Date.now(),
        balance: null,
        coding_plan: null,
      });
    } finally {
      setTestRunning(false);
    }
  };

  const { selection, customOnly, fellBack } = resolveQuotaSelection(customScript, variantId, variants);
  const selVariant = selection === QUOTA_CUSTOM_VARIANT ? null : variants.find(v => v.id === selection) ?? null;

  // 手动预算侧：档位列表 + 创建态自动填首档（原 ManualBudgetsSection 的状态逻辑原样搬入）。
  const [tiers, setTiers] = useState<PlanQuotaTier[]>([]);
  const [tierId, setTierId] = useState("");
  const autofilledFor = React.useRef<string | null>(null);
  useEffect(() => {
    let alive = true;
    getDefaultPlanQuotas(protocol).then(list => {
      if (!alive) return;
      setTiers(list);
      setTierId(list[0]?.id ?? "");
      if (!editing && list.length > 0 && autofilledFor.current !== protocol) {
        autofilledFor.current = protocol;
        setBudgets(prev => (prev.length === 0 ? tierToBudgets(list[0]) : prev));
      }
    });
    return () => { alive = false; };
  }, [protocol, editing, setBudgets]);
  const selectedTier = tiers.find(x => x.id === tierId) ?? tiers[0];

  // 进入自定义模式且脚本为空 → 把样例预填进表单态一次（保存时 isUserQuotaScript 会丢掉未改样例）；
  // 只在 selection 切换时触发，用户之后清空不会被回填。
  const prevSelection = React.useRef<string | null>(null);
  useEffect(() => {
    if (quotaSource === "auto" && selection === QUOTA_CUSTOM_VARIANT && prevSelection.current !== QUOTA_CUSTOM_VARIANT && customScript === "") {
      onCustomScriptChange(QUOTA_SCRIPT_SAMPLE);
    }
    prevSelection.current = selection;
  }, [quotaSource, selection, customScript, onCustomScriptChange]);

  // 切 tab：对侧有用户数据才确认（票 03：无配置静默切换，有配置弹窗列出将清什么）。
  const scriptConfigured = quotaCustomScriptConfigured(customScript, variantId, variants);
  const scriptAvailable = isUserQuotaScript(customScript) || Boolean(selVariant?.script?.trim());
  const budgetConfigured = budgets.length > 0;
  const trySwitch = (target: "auto" | "manual") => {
    if (target === quotaSource) return;
    const oppositeHasData = target === "manual" ? scriptConfigured : budgetConfigured;
    if (oppositeHasData) setConfirmTarget(target);
    else onSourceChange(target);
  };

  return (
    <FormSection
      title={t("platform.quotaSection.title", "Quota Query")}
      desc={t("platform.quotaSection.desc", "Choose how balance/quota is obtained: auto scripts query the upstream for live balance; manual budgets set your own limits, deducted by usage, returning 402 when exhausted. The two are mutually exclusive; switching and saving clears the other side's config.")}
    >
      <Tabs value={quotaSource} onValueChange={(v) => trySwitch(v as "auto" | "manual")}>
        <TabsList style={{ width: "100%" }}>
          <TabsTrigger value="auto" style={{ flex: 1 }}>{t("platform.quotaSection.tabAuto", "Auto Script")}</TabsTrigger>
          <TabsTrigger value="manual" style={{ flex: 1 }}>{t("platform.quotaSection.tabManual", "Manual Budget")}</TabsTrigger>
        </TabsList>
        <TabsContent value="auto">
          <QuotaScriptFields
            protocol={protocol} variants={variants} onVariantChange={onVariantChange}
            customScript={customScript} onCustomScriptChange={onCustomScriptChange}
            requires={requires} onRequiresChange={onRequiresChange}
            locale={locale} t={t} customOnly={customOnly} selection={selection}
            fellBack={fellBack} selVariant={selVariant} baseUrl={baseUrl} apiKey={apiKey}
          />
          <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 10 }}>
            <Button
              data-testid="quota-test-button"
              variant="outline"
              size="sm"
              disabled={!scriptAvailable || testRunning}
              onClick={runQuotaTest}
            >
              {testRunning ? t("platform.quotaSection.testing", "Querying…") : t("platform.quotaSection.test", "Test")}
            </Button>
            {!scriptAvailable && (
              <div data-testid="quota-test-hint" style={{ fontSize: 14, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("platform.quotaSection.testNoScript", "Configure a query script (pick a variant or write a custom script) before testing.")}
              </div>
            )}
            {testResult && (
              <div data-testid="quota-test-result" style={{ fontSize: 14, lineHeight: 1.6 }}>
                {testResult.success ? (
                  <>
                    {testResult.balance && (
                      <div data-testid="quota-test-balance">
                        {t("platform.quotaSection.testBalance", "Balance")}
                        {": "}
                        {formatAmount(testResult.balance.remaining, testResult.balance.currency)}
                        {testResult.balance.total != null && ` / ${formatAmount(testResult.balance.total, testResult.balance.currency)}`}
                        {testResult.balance.used != null && ` (${t("platform.quotaSection.testUsed", "used")} ${formatAmount(testResult.balance.used, testResult.balance.currency)})`}
                      </div>
                    )}
                    {testResult.coding_plan?.tiers.map(tier => (
                      <div key={tier.name}>
                        {tier.name}: {formatPercent(tier.utilization)}
                      </div>
                    ))}
                    <div>
                      {t("platform.quotaSection.testQueriedAt", "Queried at")}
                      {": "}
                      {formatDateTime(testResult.queried_at) ?? "-"}
                    </div>
                    {testResult.error && (
                      <div data-testid="quota-test-note" style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
                        {t("platform.quotaSection.testNote", "Note")}
                        {": "}
                        {testResult.error}
                      </div>
                    )}
                  </>
                ) : (
                  <div data-testid="quota-test-error" style={{ color: "var(--destructive)" }}>
                    {testResult.error ?? t("platform.quotaSection.testFail", "Query failed")}
                  </div>
                )}
              </div>
            )}
          </div>
        </TabsContent>
        <TabsContent value="manual">
          <ManualBudgetsFields
            budgets={budgets} setBudgets={setBudgets}
            tiers={tiers} setTierId={setTierId} selectedTier={selectedTier} t={t}
          />
        </TabsContent>
      </Tabs>

      {/* 切换确认（票 03：表单内只记忆目标，数据不动，清空在保存时由后端执行） */}
      <AlertDialog open={confirmTarget !== null} onOpenChange={(next) => { if (!next) setConfirmTarget(null); }}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 440, padding: "20px 22px" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmTarget === "manual"
                ? t("platform.quotaSection.switchToManual", "Switch to manual budget?")
                : t("platform.quotaSection.switchToAuto", "Switch to auto script?")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirmTarget === "manual"
                ? t("platform.quotaSection.clearScript", "Saving will clear the script config: variant choice, custom script, query params. Previously fetched balance stays displayed.")
                : t("platform.quotaSection.clearBudgets", "Saving will clear the manual budget limit list.")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => setConfirmTarget(null)}>{t("action.cancel", "Cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={() => { if (confirmTarget) onSourceChange(confirmTarget); setConfirmTarget(null); }}>
              {t("platform.quotaSection.switch", "Switch")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </FormSection>
  );
}

/** 脚本侧是否有用户显式配置（切 manual 时是否值得弹确认）：
 *  自定义脚本正文 / 显式选过变体 id。回落首条（零配置）不算——清了也无感。 */
function quotaCustomScriptConfigured(customScript: string, variantId: string, variants: QuotaScriptVariant[]): boolean {
  if (isUserQuotaScript(customScript)) return true;
  return variantId !== "" && variantId !== QUOTA_CUSTOM_VARIANT && variants.some(v => v.id === variantId);
}

// ─── 限频配置（rate-limit-aware 票 04；2026-10-21 弹窗平铺进 tab 正文）───
// extra.rate_limits（平台级 RPM/TPM + per-model 覆盖）+ extra.quota_windows（额度窗口族）
// 的编辑：tab 正文一屏直编，无弹窗无按钮（2026-10-21 表单拍板）。编辑态就地转换，
// 未按平台「保存」前只进平台表单态、不落库。原 RateLimitsEditModal 的 draft 助手原样搬入。

/** 编辑态：数字一律字符串持有（空串 = 未配），commit 时经 draftToBundle 净化。
 *  窗口时长 = 自由数字 + 单位（分钟/小时/天，2026-10-23 A2 定稿）；预算三选一
 *  token | 金额 $ | 次数（budget_points 已废，见 QuotaWindowConfig 注）。 */
export type QuotaBudgetMode = "tokens" | "usd" | "count";
export interface RateLimitsDraft {
  concurrency?: string;
  rpm: string;
  rps?: string;
  tpm: string;
  rules: RateLimitRuleDraft[];
  windows: { num: string; unit: string; mode: QuotaBudgetMode; amount: string; enabled: boolean; resetMode: string }[];
  /** 策略库导入来源：编辑时原样带回，供「内置策略已更新」判定。 */
  source?: { id?: string; at?: number };
}

/** bundle → 编辑态。window 秒数拆成 数字+单位（优先小时，<1h 用分钟，≥1d 用天）。 */
export function bundleToDraft(b: RateLimitsBundle): RateLimitsDraft {
  const str = (n?: number) => (n ? String(n) : "");
  return {
    concurrency: str(b.rate_limits?.concurrency),
    rpm: str(b.rate_limits?.rpm),
    rps: str(b.rate_limits?.rps),
    tpm: str(b.rate_limits?.tpm),
    rules: (b.rate_limits?.rules ?? []).map(ruleToDraft),
    source: b.rate_limits?.source_policy_id ? { id: b.rate_limits.source_policy_id, at: b.rate_limits.source_last_updated } : undefined,
    windows: (b.quota_windows ?? []).map(w => {
      const sec = w.window;
      // 整除优先：天 → 小时 → 分钟（45s 这类非整分钟向上归 1 分钟）
      const [num, unit] = sec >= 86400 && sec % 86400 === 0 ? [String(sec / 86400), "86400"]
        : sec >= 3600 && sec % 3600 === 0 ? [String(sec / 3600), "3600"]
        : [String(Math.round(sec / 60)), "60"];
      const mode: QuotaBudgetMode = w.budget_usd != null ? "usd" : w.budget_count != null ? "count" : "tokens";
      const amount = mode === "usd" ? w.budget_usd : mode === "count" ? w.budget_count : w.budget_tokens;
      return { num, unit, mode, amount: amount != null ? String(amount) : "", enabled: w.enabled ?? true, resetMode: w.reset_mode ?? "sliding" };
    }),
  };
}

/** 编辑态 → bundle（净化：非法数字丢弃；空即 undefined，serialize 侧删键）。
 *  budget_points 不再写出（存量行编辑保存后清除，2026-10-23 拍板）。
 *  enabled=false / reset_mode 非 sliding 显式写出（与后端 serde 缺省一致，缺省值不写键）。 */
export function draftToBundle(d: RateLimitsDraft): RateLimitsBundle {
  const num = (s: string): number | undefined => {
    const v = Math.floor(Number(s));
    return s.trim() !== "" && Number.isFinite(v) && v > 0 ? v : undefined;
  };
  const rps = Number(d.rps);
  const rules = d.rules.map(draftToRule);
  const rate_limits: RateLimitsConfig = {
    concurrency: num(d.concurrency ?? ""),
    rpm: num(d.rpm),
    rps: Number.isSafeInteger(rps) && rps > 0 ? rps : undefined,
    tpm: num(d.tpm),
    rules: rules.length > 0 ? rules : undefined,
    source_policy_id: d.source?.id,
    source_last_updated: d.source?.at,
  };
  const windows: QuotaWindowConfig[] = [];
  for (const w of d.windows) {
    const sec = num(w.num) !== undefined ? Math.floor(Number(w.num) * Number(w.unit)) : undefined;
    if (sec === undefined || sec <= 0) continue;
    const q: QuotaWindowConfig = { window: sec };
    if (!w.enabled) q.enabled = false;
    if (w.resetMode !== "sliding") q.reset_mode = w.resetMode as QuotaWindowConfig["reset_mode"];
    if (w.mode === "usd") {
      const v = parseFloat(w.amount);
      if (w.amount.trim() !== "" && Number.isFinite(v) && v > 0) q.budget_usd = v;
    } else if (w.mode === "count") {
      q.budget_count = num(w.amount);
    } else {
      q.budget_tokens = num(w.amount);
    }
    if (q.budget_tokens !== undefined || q.budget_usd !== undefined || q.budget_count !== undefined) windows.push(q);
  }
  return {
    rate_limits: rate_limits.concurrency || rate_limits.rpm || rate_limits.rps || rate_limits.tpm || rate_limits.rules ? rate_limits : undefined,
    quota_windows: windows.length > 0 ? windows : undefined,
  };
}

export function RateLimitsSection({ draft, patch, protocol, modelOptions, platformCalendar, t }: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  protocol: Protocol;
  modelOptions: string[];
  platformCalendar?: string;
  t: TFunction;
}) {
  const bundle = draft.rateLimits;
  const setBundle = draftSetter(draft, patch, "rateLimits");
  const [rl, setRl] = useState<RateLimitsDraft>(() => bundleToDraft(bundle));
  // 自身 commit 的 bundle 回声不回灌编辑态；外部重置（载入平台 / resetForm / 粘贴导入）才重取。
  const lastEmitted = React.useRef<string>("");
  useEffect(() => {
    if (JSON.stringify(draftToBundle(bundleToDraft(bundle))) !== lastEmitted.current) {
      setRl(bundleToDraft(bundle));
    }
  }, [bundle]);
  const commit = (d: RateLimitsDraft) => {
    const b = draftToBundle(d);
    lastEmitted.current = JSON.stringify(b);
    setRl(d);
    setBundle(b);
  };
  const upd = <K extends keyof RateLimitsDraft>(k: K, v: RateLimitsDraft[K]) => commit({ ...rl, [k]: v });
  // 导入 = 覆盖 rate_limits（quota_windows 不动）；不经 lastEmitted，让编辑态随新 bundle 重取。
  const importPolicy = (cfg: RateLimitsConfig) => setBundle({ ...bundle, rate_limits: cfg });

  const summary = rateLimitsSummary(bundle, n => t("platform.rateLimits.windowsCount", "{{n}} windows", { n }));
  const ruleCount = bundle.rate_limits?.rules?.length ?? 0;
  const inputStyle = { fontSize: 14, height: 28, minWidth: 70 };
  const numStyle = { ...inputStyle, textAlign: "center", fontVariantNumeric: "tabular-nums" } as React.CSSProperties;
  const labelStyle = { fontSize: 14, opacity: 0.7, color: "var(--text-tertiary)" }; // 稿 .lb/.lb2 = fg-3
  return (
    <FormSection
      title={t("platform.rateLimits.title", "Rate Limits")}
      desc={t("platform.rateLimits.desc", "Configure rate limits and quota windows per plan; scheduling excludes the platform and blocks requests when exceeded.")}
      action={<RateLimitPolicyLibrary protocol={protocol} source={bundle.rate_limits?.source_policy_id ? { id: bundle.rate_limits.source_policy_id, at: bundle.rate_limits.source_last_updated } : undefined} onImport={importPolicy} t={t} headerOnly />}
    >
      {/* 实时态条：配置摘要（调度数据未回传，先展示静态配置态） */}
      <div className="aidog-anim-rise" style={C.strip}>
        <div style={C.tile("var(--data-primary)")}>{t("platform.tile.rateLimit", "R")}</div>
        {summary === "" ? (
          <span style={C.pill("neutral")}>
            {t("platform.rateLimits.none", "Not configured → no rate limiting (429 fallback only)")}
          </span>
        ) : (
          <>
            <span style={C.pill("ok")}>{summary}</span>
            {ruleCount > 0 && (
              <span style={C.pill("neutral")}>
                {t("platform.rateLimits.ruleCount", "{{n}} rules", { n: ruleCount })}
              </span>
            )}
          </>
        )}
      </div>

      {/* 平台级限频：C 方向稿大数字底边线输入 */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 10, marginBottom: 10 }}>
        <label className="aidog-cell" style={{ display: "flex", alignItems: "baseline", gap: 8, ...C.row }}>
          <span style={labelStyle}>{t("platform.rateLimits.concurrency", "Concurrency (in-flight)")}</span>
          <Input id="rl-concurrency" type="number" min={0} value={rl.concurrency ?? ""} onChange={e => upd("concurrency", e.target.value)}
            className="aidog-bignum"
            style={{ ...inputStyle, flex: 1, minWidth: 60, height: "auto", fontSize: 17, fontWeight: 300, border: 0, borderBottom: "1px solid var(--border)", borderRadius: 0, background: "transparent" }}
            placeholder="—" />
        </label>
        <label className="aidog-cell" style={{ display: "flex", alignItems: "baseline", gap: 8, ...C.row }}>
          <span style={labelStyle}>{t("platform.rateLimits.rpm", "RPM (req/min)")}</span>
          <Input id="rl-rpm" type="number" min={0} value={rl.rpm} onChange={e => upd("rpm", e.target.value)}
            className="aidog-bignum"
            style={{ ...inputStyle, flex: 1, minWidth: 60, height: "auto", fontSize: 17, fontWeight: 300, border: 0, borderBottom: "1px solid var(--border)", borderRadius: 0, background: "transparent" }}
            placeholder="—" />
        </label>
        <label className="aidog-cell" style={{ display: "flex", alignItems: "baseline", gap: 8, ...C.row }}>
          <span style={labelStyle}>{t("platform.rateLimits.rps", "Calls per second (RPS)")}</span>
          <Input id="rl-rps" type="number" min={1} step={1} value={rl.rps ?? ""} onChange={e => upd("rps", e.target.value)}
            className="aidog-bignum"
            style={{ ...inputStyle, flex: 1, minWidth: 60, height: "auto", fontSize: 17, fontWeight: 300, border: 0, borderBottom: "1px solid var(--border)", borderRadius: 0, background: "transparent" }}
            placeholder="—" />
        </label>
        <label className="aidog-cell" style={{ display: "flex", alignItems: "baseline", gap: 8, ...C.row }}>
          <span style={labelStyle}>{t("platform.rateLimits.tpm", "TPM (tokens/min)")}</span>
          <Input id="rl-tpm" type="number" min={0} value={rl.tpm} onChange={e => upd("tpm", e.target.value)}
            className="aidog-bignum"
            style={{ ...inputStyle, flex: 1, minWidth: 60, height: "auto", fontSize: 17, fontWeight: 300, border: 0, borderBottom: "1px solid var(--border)", borderRadius: 0, background: "transparent" }}
            placeholder="—" />
        </label>
      </div>

      {/* 限频规则：模型 / 时段条件 + 上限；重叠按列表顺序（可拖动） */}
      <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
        <span style={labelStyle}>{t("platform.rateLimits.rules", "Rate-limit rules")}</span>
        <RateLimitRulesList rules={rl.rules} onChange={rules => upd("rules", rules)} modelOptions={modelOptions} protocol={protocol} platformCalendar={platformCalendar} t={t} />
      </div>

      {/* 额度窗口（2026-10-23 A2 定稿：时长自由数字+单位，预算三选一 token|金额|次数，预设只是快捷填充；
          2026-11-16 执行上线：每窗口启用开关 + 重置方式） */}
      <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
        <span style={labelStyle}>{t("platform.rateLimits.quotaWindows", "Quota windows (5h=18000 / 7d=604800 sec)")}</span>
        {rl.windows.map((w, i) => (
          <div key={i} className={C.rowClass} style={{ ...C.row, opacity: w.enabled ? undefined : 0.65 }}>
            <Input
              type="number" min={1} step={1}
              value={w.num}
              aria-label={t("platform.rateLimits.windowLength", "Window length")}
              placeholder="5"
              onChange={e => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, num: e.target.value } : x))}
              style={{ ...numStyle, width: 64, minWidth: 64, flexShrink: 0 }}
            />
            <Select
              value={w.unit}
              onValueChange={(v) => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, unit: v } : x))}
            >
              <SelectTrigger className="input" style={{ width: 76, flexShrink: 0, height: 28, fontSize: 14 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="60">{t("platform.windowUnitMinute", "Minutes")}</SelectItem>
                <SelectItem value="3600">{t("platform.windowUnitHour", "Hours")}</SelectItem>
                <SelectItem value="86400">{t("platform.windowUnitDay", "Days")}</SelectItem>
              </SelectContent>
            </Select>
            <Select
              value={w.mode}
              onValueChange={(v) => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, mode: v as QuotaBudgetMode, amount: "" } : x))}
            >
              <SelectTrigger className="input" style={{ width: 96, flexShrink: 0, height: 28, fontSize: 14 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tokens">{t("platform.rateLimits.budgetTokens", "token budget")}</SelectItem>
                <SelectItem value="usd">{t("platform.rateLimits.budgetUsd", "Budget ($)")}</SelectItem>
                <SelectItem value="count">{t("platform.rateLimits.budgetCount", "Request count")}</SelectItem>
              </SelectContent>
            </Select>
            <Input
              type="number"
              min={0}
              step={w.mode === "usd" ? "any" : 1}
              value={w.amount}
              data-testid={`quota-window-amount-${i}`}
              aria-invalid={w.amount.trim() !== "" && !(Number(w.amount) > 0) ? true : undefined}
              aria-label={t("platform.rateLimits.budgetAmount", "Budget amount")}
              placeholder={t("platform.manualBudgetAmount", "Amount")}
              onChange={e => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, amount: e.target.value } : x))}
              style={{ ...numStyle, width: 96, minWidth: 96, flexShrink: 0 }}
            />
            <span style={{ fontSize: 14, color: "var(--text-tertiary)", flexShrink: 0 }}>
              {w.mode === "usd" ? "$" : w.mode === "count" ? t("platform.rateLimits.unitCount", "req") : "tokens"}
            </span>
            {/* 重置方式（额度窗口执行 2026-11-16）：sliding / 自然日 / 自然小时 */}
            <Select
              value={w.resetMode}
              onValueChange={(v) => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, resetMode: v } : x))}
            >
              <SelectTrigger
                data-testid={`quota-window-reset-${i}`}
                className="input"
                style={{ width: 108, flexShrink: 0, height: 28, fontSize: 14 }}
                aria-label={t("platform.rateLimits.resetMode", "Reset mode")}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="sliding">{t("platform.rateLimits.resetModeSliding", "Sliding window")}</SelectItem>
                <SelectItem value="calendar_day">{t("platform.rateLimits.resetModeCalendarDay", "Calendar day")}</SelectItem>
                <SelectItem value="calendar_hour">{t("platform.rateLimits.resetModeCalendarHour", "Calendar hour")}</SelectItem>
              </SelectContent>
            </Select>
            {/* 启用开关（关掉 = 不参与阻断与计数；行内控件阻断冒泡到行容器） */}
            <span onClick={(e) => e.stopPropagation()}>
              <Switch
                data-testid={`quota-window-enabled-${i}`}
                aria-label={t("platform.rateLimits.windowEnabled", "Enabled")}
                checked={w.enabled}
                onCheckedChange={v => upd("windows", rl.windows.map((x, j) => j === i ? { ...x, enabled: v === true } : x))}
              />
            </span>
            <Button variant="ghost" size="icon" className="btn-danger" style={{ height: 28, width: 28, marginLeft: "auto" }} onClick={() => upd("windows", rl.windows.filter((_, j) => j !== i))}>×</Button>
          </div>
        ))}
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {([["5", "3600", t("platform.quickAdd.hours", "+ {{n}} h", { n: 5 })], ["1", "86400", t("platform.quickAdd.days", "+ {{n}} d", { n: 1 })], ["7", "86400", t("platform.quickAdd.days", "+ {{n}} d", { n: 7 })], ["30", "60", t("platform.quickAdd.minutes", "+ {{n}} min", { n: 30 })]] as const).map(([n, u, label]) => (
            <button key={label} type="button" className={C.addBtnClass}
              onClick={() => upd("windows", [...rl.windows, { num: n, unit: u, mode: "tokens", amount: "", enabled: true, resetMode: "sliding" }])}>
              {label}
            </button>
          ))}
        </div>
      </div>
    </FormSection>
  );
}

/** 直通模型（直接模型转发）：平台声明 direct_models，声明即生效不查 registry。
 *  添加走文本输入（聚合站自定义模型名可配），逐条删除；空列表 = 未声明。 */
export function DirectModelsSection({ draft, patch, availableModels, protocol, t }: {
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  availableModels: string[];
  protocol: Protocol;
  t: TFunction;
}) {
  const models = draft.directModels;
  const setModels = draftSetter(draft, patch, "directModels");
  const [input, setInput] = useState("");
  const source = useModelCandidates(availableModels, protocol);
  const add = (raw: string = input) => {
    const v = raw.trim();
    if (!v || models.includes(v)) return;
    setModels(prev => [...prev, v]);
    setInput("");
  };
  return (
    <FormSection
      title={t("platform.direct_models", "Direct models")}
      desc={t("platform.direct_models_desc", "When declared, group requests carrying these model ids are routed only to platforms that declare them, preserving the requested model name (failover follows existing priority). If no platform declares the id, the default chain applies. Declaration takes effect immediately — no registry lookup.")}
    >
      <div className="aidog-anim-rise" style={C.strip}>
        <div style={C.tile("var(--data-primary)")}>{t("platform.tile.direct", "D")}</div>
        <span style={C.pill(models.length > 0 ? "ok" : "neutral")}>
          {t("platform.direct_models_count", "{{n}} models", { n: models.length })}
        </span>
      </div>
      {models.map((m, i) => (
        <div key={m} className={C.rowClass} style={C.row}>
          <Input
            value={m}
            readOnly
            data-testid={`direct-model-${i}`}
            style={{ flex: 1, minWidth: 110, fontFamily: "var(--font-mono, monospace)", fontSize: 14, height: 28 }}
          />
          <Button
            variant="ghost" size="icon" className="btn-danger" style={{ height: 28, width: 28 }}
            data-testid={`direct-model-remove-${i}`}
            aria-label={t("platform.delete", "Delete")}
            onClick={() => setModels(prev => prev.filter((_, j) => j !== i))}
          >×</Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, marginTop: models.length > 0 ? 6 : 0 }}>
        <div style={{ flex: 1, minWidth: 110 }} onKeyDown={e => { if (e.key === "Enter" && !isImeComposing(e)) { e.preventDefault(); add(); } }}>
          <ModelCombobox
            value={input} onChange={setInput} onSelect={add}
            source={source} exclude={models} protocol={protocol}
            placeholder={t("platform.direct_models_placeholder", "Model id, e.g. claude-sonnet-4-5")}
            inputProps={{ "data-testid": "direct-model-input" }}
            inputStyle={{ fontFamily: "var(--font-mono, monospace)", fontSize: 14, height: 28 }}
            t={t}
          />
        </div>
        <button type="button" data-testid="direct-model-add" className={C.addBtnClass} onClick={() => add()}>
          + {t("platform.direct_models_add", "Add")}
        </button>
      </div>
    </FormSection>
  );
}

/** 客户端白名单：extra.allowed_clients。按词表大类分组，勾大类 = 允许其全部变体（子项联动、半选可见），
 *  清空 = 取消白名单（全放行）。路由拦截不在本组件（票 04）。 */
export function ClientWhitelistSection({ vocab, draft, patch, t }: {
  vocab: ClientVocabCategory[];
  draft: PlatformDraft;
  patch: (p: Partial<PlatformDraft>) => void;
  t: TFunction;
}) {
  const selected = draft.allowedClients;
  const setSelected = draftSetter(draft, patch, "allowedClients");
  const uaRules = draft.uaRules;
  const setUaRules = draftSetter(draft, patch, "uaRules");
  const restricted = selected.length + uaRules.length;
  const patchRule = (i: number, patch: Partial<ClientUaRule>) =>
    setUaRules(prev => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  return (
    <FormSection
      title={t("platform.client_whitelist", "Client allowlist")}
      desc={t("platform.client_whitelist_desc", "Only the selected clients may use this platform; selecting a category allows all its variants. No selection and no UA rules = no restriction. Clients the built-in vocabulary cannot identify are judged by the UA rules and rejected if none matches.")}
      action={restricted > 0 ? (
        <Button variant="ghost" size="sm" data-testid="client-wl-clear" onClick={() => { setSelected([]); setUaRules([]); }}>
          {t("platform.client_whitelist_clear", "Clear")}
        </Button>
      ) : undefined}
    >
      <div className="aidog-anim-rise" style={C.strip}>
        <span style={C.pill(restricted > 0 ? "ok" : "neutral")} data-testid="client-wl-count">
          {restricted > 0
            ? t("platform.client_whitelist_count", "{{n}} restricted", { n: restricted })
            : t("platform.client_whitelist_none", "No restriction")}
        </span>
      </div>
      <div style={{ maxHeight: 320, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
        {vocab.map(cat => {
          const st = clientCategoryState(selected, cat);
          const catName = t(cat.nameKey, cat.code);
          return (
            <div key={cat.code} data-testid={`client-cat-${cat.code}`} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, fontWeight: 600, color: "var(--text-primary)", cursor: "pointer" }}>
                <Checkbox
                  data-testid={`client-cat-check-${cat.code}`}
                  checked={st === "all" ? true : st === "some" ? "indeterminate" : false}
                  onCheckedChange={v => setSelected(prev => toggleClientCategory(prev, cat, v === true))}
                />
                {catName}
              </label>
              {cat.variants.length > 0 && (
                <div style={{ display: "flex", flexWrap: "wrap", gap: "4px 16px", paddingInlineStart: 24 }}>
                  {cat.variants.map(v => (
                    <label key={v.code} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 14, color: "var(--text-secondary)", cursor: "pointer" }}>
                      <Checkbox
                        data-testid={`client-var-check-${v.code}`}
                        checked={st === "all" || selected.includes(v.code)}
                        onCheckedChange={on => setSelected(prev => toggleClientVariant(prev, cat, v.code, on === true))}
                      />
                      {t(v.nameKey, v.code)}
                    </label>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 12 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
          {t("platform.client_ua_rules", "UA rules")}
        </div>
        <div style={{ fontSize: 14, color: "var(--text-secondary)" }}>
          {t("platform.client_ua_rules_desc", "Apply only to clients the built-in list cannot identify; any matching rule allows the request. Clients the built-in list identifies are judged by the ticks above only. All matching is case-insensitive.")}
        </div>
        {uaRules.map((r, i) => {
          const err = validateClientUaRule(r);
          return (
            <div key={i} className={C.rowClass} style={C.row} data-testid={`client-ua-rule-${i}`} data-state={err ? "invalid" : "valid"}>
              <select
                className="input" style={{ fontSize: 14, width: 110, height: 28 }}
                data-testid={`client-ua-rule-kind-${i}`}
                value={r.kind}
                onChange={e => patchRule(i, { kind: e.target.value as ClientUaRuleKind })}
              >
                <option value="contains">{t("platform.client_ua_rule_kind_contains", "Contains")}</option>
                <option value="prefix">{t("platform.client_ua_rule_kind_prefix", "Starts with")}</option>
                <option value="exact">{t("platform.client_ua_rule_kind_exact", "Equals")}</option>
                <option value="regex">{t("platform.client_ua_rule_kind_regex", "Regex")}</option>
              </select>
              <Input
                data-testid={`client-ua-rule-pattern-${i}`}
                value={r.pattern}
                placeholder={t("platform.client_ua_rule_placeholder", "UA text, e.g. openviking")}
                aria-invalid={err ? true : undefined}
                onChange={e => patchRule(i, { pattern: e.target.value })}
                style={{ flex: 1, minWidth: 110, fontFamily: "var(--font-mono, monospace)", fontSize: 14, height: 28 }}
              />
              {err && (
                <span style={{ fontSize: 14, color: "var(--danger, #c0392b)" }}>
                  {err === "empty"
                    ? t("platform.client_ua_rule_err_empty", "Must not be empty")
                    : t("platform.client_ua_rule_err_regex", "Invalid regex")}
                </span>
              )}
              <Button
                variant="ghost" size="icon" className="btn-danger" style={{ height: 28, width: 28 }}
                data-testid={`client-ua-rule-remove-${i}`}
                aria-label={t("platform.delete", "Delete")}
                onClick={() => setUaRules(prev => prev.filter((_, j) => j !== i))}
              >×</Button>
            </div>
          );
        })}
        <div>
          <Button
            variant="ghost" size="sm" data-testid="client-ua-rule-add"
            onClick={() => setUaRules(prev => [...prev, { kind: "contains", pattern: "" }])}
          >
            + {t("platform.client_ua_rule_add", "Add rule")}
          </Button>
        </div>
      </div>
    </FormSection>
  );
}

/** qianfan AppBuilder 凭据（独立搜索转发，票 20）：AppBuilder key 独立于千帆模型服务 key，
 *  存 platform.extra 顶层 appbuilder_key（票 19 转发读它发 X-Appbuilder-Authorization）。 */
export function AppbuilderKeySection({ value, onChange, t }: {
  value: string;
  onChange: (v: string) => void;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.appbuilder_key", "AppBuilder credential")}
      desc={t("platform.appbuilder_key_desc", "AppBuilder key for independent search/tool endpoint forwarding, separate from the Qianfan model-service key; leave empty if not configured (search requests still forward, 401 enters cooldown).")}
    >
      <Input
        data-testid="appbuilder-key-input"
        value={value}
        placeholder={t("platform.appbuilder_key_placeholder", "AppBuilder API Key (bce-v3/...)")}
        onChange={e => onChange(e.target.value)}
        style={{ width: "100%", fontFamily: "var(--font-mono, monospace)", fontSize: 14, height: 28 }}
      />
    </FormSection>
  );
}
