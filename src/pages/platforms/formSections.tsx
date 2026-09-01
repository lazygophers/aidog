// formSections — PlatformEditForm 的表单分区子组件（纯 props 驱动，无自身 state）。
// ponytail: 从 PlatformEditForm.tsx 抽出以控制文件行数（每分区 ~50-150 行，合计 ~740 行）。
//   所有分区组件经 props 收 state/setters，不持有自己的 state；跨组件无共享 useState（符合
//   design.md 决策锁「区块拆保持单组件内 state」）。
//
// 共用 helpers（FormSection / ApiKeyField / toDatetimeLocal）也在此导出，供 PlatformEditForm 主组件消费。
//   EndpointsSection 已移到 formSectionsEndpoints.tsx（体积大），通过末尾 re-export 暴露。
import React, { useState, useEffect } from "react";
import type { TFunction } from "i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  type Platform, type Protocol, type PlatformEndpoint,
  type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit,
  type DevinConfig, type SchedulingBreakerSettings, type GroupDetail,
} from "../../services/api";
import { newManualBudget, type TimeWindow, getDefaultPeak, getDefaultModelList, type QuotaScriptVariant, quotaVariantLabel } from "../../domains/platforms";
import { isCurrentlyPeak, utcToDisplay, displayToUtc, WINDOW_TIMEZONES, type TzMode } from "../../utils/timeWindow";
import { formatDateTime, pad } from "../../utils/formatters";
import type { ThemeMode } from "../../themes/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

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
          <div style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)" }}>{title}</div>
          {desc && (
            <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4, marginTop: 2 }}>{desc}</div>
          )}
        </div>
        {action && <div style={{ flexShrink: 0 }}>{action}</div>}
      </div>
      {children}
    </div>
  );
}

/** Token 显隐 + 复制按钮（透传配置区 / 认证区共用）。
 *  multiline=true（创建态认证区）：textarea 多行输入，每行一个 token，多行触发批量创建预览
 *  （拆分/批量链路见 splitApiKeys + runBatchCreateFromPaste）；无密码遮蔽（textarea 不支持 password）。 */
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
          style={{ flex: 1, resize: "vertical", minHeight: 58, fontFamily: "var(--font-mono, monospace)", fontSize: 12, lineHeight: 1.6 }}
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

/** 自定义伪变体下拉哨兵值（radix Select 禁空串 value；选中时表单切到 customScript 编辑）。 */
export const QUOTA_CUSTOM_VARIANT = "__custom__";

export function QuotaScriptSection({
  protocol, variants, variantId, onVariantChange,
  customScript, onCustomScriptChange,
  requires, onRequiresChange,
  locale, t,
}: {
  protocol: Protocol;
  variants: QuotaScriptVariant[];
  variantId: string;
  onVariantChange: (v: string) => void;
  customScript: string;
  onCustomScriptChange: (v: string) => void;
  requires: Record<string, string>;
  onRequiresChange: (key: string, value: string) => void;
  locale?: string;
  t: TFunction;
}) {
  // 无变体且无自定义脚本的平台不渲染本控件（quota-scripts spec user story 5 反面）。
  if (variants.length === 0 && !customScript.trim()) return null;
  const hasCustom = !!customScript.trim();
  const idValid = variants.some(v => v.id === variantId);
  // 选中值解析（与后端 resolve_quota_script 对齐：custom 优先 → 显式 id → 回落首条）。
  const selection = hasCustom || variantId === QUOTA_CUSTOM_VARIANT
    ? QUOTA_CUSTOM_VARIANT
    : idValid ? variantId : (variants[0]?.id ?? "");
  // 存量 id 失效（远程改名/删条）→ 已回落首条，提示用户重选（spec「UI 显示已回落」）。
  const fellBack = !hasCustom && variantId !== QUOTA_CUSTOM_VARIANT && variantId !== "" && !idValid;
  const selVariant = selection === QUOTA_CUSTOM_VARIANT ? null : variants.find(v => v.id === selection) ?? null;
  return (
    <FormSection
      title={t("platform.quotaScript.title", "配额查询脚本")}
      desc={t("platform.quotaScript.desc", "选择匹配你部署的查询脚本变体；脚本要求的参数会自动出现")}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.quotaScript.variant", "脚本变体")}
        </div>
        <Select value={selection || QUOTA_CUSTOM_VARIANT} onValueChange={onVariantChange}>
          <SelectTrigger className="input">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {variants.map(v => (
              <SelectItem key={v.id} value={v.id}>{quotaVariantLabel(v.name, v.id, locale)}</SelectItem>
            ))}
            <SelectItem value={QUOTA_CUSTOM_VARIANT}>{t("platform.quotaScript.custom", "自定义脚本")}</SelectItem>
          </SelectContent>
        </Select>
        {fellBack && (
          <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {t("platform.quotaScript.fellBack", "已选变体已失效，已回落到首个变体")}
          </div>
        )}
      </div>
      {selection === QUOTA_CUSTOM_VARIANT ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("platform.quotaScript.customLabel", "自定义 JS 脚本")}
          </div>
          <textarea
            className="input"
            placeholder={t("platform.quotaScript.customPlaceholder", "// 返回 { success, balance, tiers, newapi_user_id }")}
            value={customScript}
            onChange={(e) => onCustomScriptChange(e.target.value)}
            spellCheck={false}
            rows={10}
            style={{ resize: "vertical", minHeight: 120, fontFamily: "var(--font-mono, monospace)", fontSize: 12, lineHeight: 1.6 }}
          />
          <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {t("platform.quotaScript.customHint", "ctx 提供 baseUrl / apiKey / extra（requires 参数在 extra 顶层）；可用 http.get / http.post 出站（走系统代理、记录日志）。")}
          </div>
        </div>
      ) : (selVariant?.requires ?? []).map(r => (
        <div key={r.key} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {quotaVariantLabel(r.label, r.key, locale)}
          </div>
          <Input
            className="input"
            type="text"
            placeholder={r.key}
            value={requires[r.key] ?? ""}
            onChange={(e) => onRequiresChange(r.key, e.target.value)}
          />
        </div>
      ))}
      {/* New API 用户 ID：非 requires 字段，保留旧表单入口（查询结果 newapi_user_id 的回填目标，
          回填链现状见 notes/01；序列化写 extra.newapi.user_id）。 */}
      {protocol === "newapi" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("platform.newapiUserId", "用户 ID")}
          </div>
          <Input
            className="input"
            type="text"
            placeholder={t("platform.newapiUserIdPlaceholder", "数字 ID（可选）")}
            value={requires.user_id ?? ""}
            onChange={(e) => onRequiresChange("user_id", e.target.value)}
          />
        </div>
      )}
    </FormSection>
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
      title={t("platform.devinConfig", "Devin 配置")}
      desc={t("platform.devinConfigHint", "API Key 填 cog_ 前缀凭证；组织 ID 在「配额查询脚本」区配置")}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.devinTimeout", "Session 超时（秒，可选）")}
        </div>
        <Input
          className="input"
          type="number"
          min={0}
          placeholder={t("platform.devinTimeoutPlaceholder", "默认 300")}
          value={config.devin_timeout}
          onChange={(e) => onChange(prev => ({ ...prev, devin_timeout: e.target.value }))}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.devinMode", "默认模式（可选）")}
        </div>
        {/* radix Select 禁 value="" → __none__ 哨兵映射回空串 */}
        <Select
          value={config.devin_mode || "__none__"}
          onValueChange={(v) => onChange(prev => ({ ...prev, devin_mode: v === "__none__" ? "" : v }))}
        >
          <SelectTrigger className="input">
            <SelectValue placeholder={t("platform.devinModeAuto", "按模型自动映射")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">{t("platform.devinModeAuto", "按模型自动映射")}</SelectItem>
            {devinModes.map(m => (
              <SelectItem key={m} value={m}>{m}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </FormSection>
  );
}

export function PassthroughConfigSection({ endpoints, setEndpoints, apiKey, setApiKey, showKey, setShowKey, t }: {
  endpoints: PlatformEndpoint[];
  setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  apiKey: string; setApiKey: React.Dispatch<React.SetStateAction<string>>;
  showKey: boolean; setShowKey: React.Dispatch<React.SetStateAction<boolean>>;
  t: TFunction;
}) {
  return (
    <FormSection title={t("platform.sectionPassthrough", "透传配置")}>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("platform.passthroughBaseUrl", "上游地址（Base URL）")}
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
        <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("platform.passthroughBaseUrlHint", "填 host 根（如 https://api.anthropic.com）。纯透传会拼接客户端原始 path/query 直接转发，请勿带版本前缀。")}
        </div>
      </div>
      <ApiKeyField
        value={apiKey} onChange={setApiKey} show={showKey} onToggleShow={() => setShowKey(!showKey)}
        placeholder={t("platform.apiKeyOptional", "Token（可选，透传可留空）")}
      />
      <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
        {t("platform.passthroughNote", "纯透传：客户端请求的 header（含订阅 OAuth 认证）与 body 原样转发，aidog 不做任何转换或认证注入。上方 Token 可留空。")}
      </div>
    </FormSection>
  );
}

export function ManualBudgetsSection({ budgets, setBudgets, t }: {
  budgets: ManualBudget[];
  setBudgets: React.Dispatch<React.SetStateAction<ManualBudget[]>>;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.manualBudgetTitle", "手动预算")}
      desc={t("platform.manualBudgetDesc", "该平台无上游额度自动查询，可手动设置一个或多个预算限额，按用量预估扣减；任一耗尽时停止转发（返回 402），窗口/次日恢复后自动放行。")}
      action={(
        <Button
          variant="ghost"
          size="sm"
          style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
          onClick={() => setBudgets([...budgets, newManualBudget()])}
        >
          {t("platform.manualBudgetAdd", "添加限额")}
        </Button>
      )}
    >
      {budgets.length === 0 && (
        <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "2px 0" }}>
          {t("platform.manualBudgetEmpty", "暂无限额，点击「添加限额」开始配置。")}
        </div>
      )}
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
          <div key={b.id} style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
            <Select
              value={b.kind}
              onValueChange={(v) => onKindChange(v as ManualBudgetKind)}
            >
              <SelectTrigger className="input" style={{ width: 110, flexShrink: 0 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="total">{t("platform.manualBudgetKindTotal", "总额")}</SelectItem>
                <SelectItem value="rolling">{t("platform.manualBudgetKindRolling", "滑动窗口")}</SelectItem>
                <SelectItem value="fixed">{t("platform.manualBudgetKindFixed", "固定窗口")}</SelectItem>
                <SelectItem value="daily">{t("platform.manualBudgetKindDaily", "每日")}</SelectItem>
              </SelectContent>
            </Select>
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
              </SelectContent>
            </Select>
            <Input
              className="input"
              type="number"
              min={0}
              step="any"
              style={{ width: 100, flexShrink: 0 }}
              placeholder={t("platform.manualBudgetAmount", "额度")}
              value={b.amount || ""}
              onChange={e => update({ amount: parseFloat(e.target.value) || 0 })}
            />
            {needsWindow && (
              <>
                <Input
                  className="input"
                  type="number"
                  min={0}
                  step="any"
                  style={{ width: 80, flexShrink: 0 }}
                  placeholder={t("platform.manualBudgetWindow", "窗口")}
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
                    <SelectItem value="minute">{t("platform.windowUnitMinute", "分钟")}</SelectItem>
                    <SelectItem value="hour">{t("platform.windowUnitHour", "小时")}</SelectItem>
                    <SelectItem value="day">{t("platform.windowUnitDay", "天")}</SelectItem>
                    <SelectItem value="week">{t("platform.windowUnitWeek", "周")}</SelectItem>
                    <SelectItem value="month">{t("platform.windowUnitMonth", "月")}</SelectItem>
                  </SelectContent>
                </Select>
              </>
            )}
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--text-secondary)", cursor: "pointer" }}>
              <Checkbox
                checked={b.enabled}
                onCheckedChange={v => update({ enabled: v === true })}
              />
              {t("platform.manualBudgetEnabled", "启用")}
            </label>
            <Button
              variant="ghost"
              size="icon"
              className="btn-danger"
              style={{ flexShrink: 0, color: "var(--color-danger)" }}
              title={t("action.delete", "删除")}
              onClick={() => setBudgets(budgets.filter((_, i) => i !== idx))}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
              </svg>
            </Button>
          </div>
        );
      })}
    </FormSection>
  );
}

export function BreakerSection({ defaults, failure, setFailure, openSecs, setOpenSecs, halfOpenMax, setHalfOpenMax, t }: {
  defaults: SchedulingBreakerSettings | null;
  failure: string; setFailure: React.Dispatch<React.SetStateAction<string>>;
  openSecs: string; setOpenSecs: React.Dispatch<React.SetStateAction<string>>;
  halfOpenMax: string; setHalfOpenMax: React.Dispatch<React.SetStateAction<string>>;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.breakerTitle", "熔断阈值")}
      desc={t("platform.breakerDesc", "连续失败达阈值后临时摘除该平台，冷却后半开探测恢复。留空 = 继承系统设置的全局默认值。")}
    >
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: "10px 12px" }}>
        <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerFailureThreshold", "失败阈值")}</span>
        <Input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_failure_threshold)) : t("platform.breakerInheritGeneric", "继承默认")}
          value={failure}
          onChange={e => setFailure(e.target.value)}
        />
        <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerOpenSecs", "熔断时长(秒)")}</span>
        <Input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_open_secs)) : t("platform.breakerInheritGeneric", "继承默认")}
          value={openSecs}
          onChange={e => setOpenSecs(e.target.value)}
        />
        <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "半开探测数")}</span>
        <Input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_half_open_max)) : t("platform.breakerInheritGeneric", "继承默认")}
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
const WEEKDAY_LABELS = ["S", "M", "T", "W", "T", "F", "S"] as const;

/** number input 值裁到 [min,max]；非法输入回落 min。 */
function clampInt(raw: string, min: number, max: number): number {
  const v = Math.round(Number(raw));
  if (!Number.isFinite(v)) return min;
  return Math.min(max, Math.max(min, v));
}

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
    ? t("platform.timezone_local", "本地")
    : t("platform.timezone_utc", "UTC+0"));

  // 跨天判定：end_hour < start_hour（原始 UTC 值，不在 display 层比较）
  const isNextDay = w.end_hour < w.start_hour;
  const nextDayLabel = isNextDay
    ? t("platform.peak_next_day", "次日")
    : "";

  return `${startStr} - ${endStr}（${tzLabel}）${nextDayLabel ? `（${nextDayLabel}）` : ""}`;
}

export function PeakSection({ windows, setWindows, tzMode, setTzMode, disableDuringPeak, setDisableDuringPeak, protocol, themeMode, t }: {
  windows: TimeWindow[];
  setWindows: React.Dispatch<React.SetStateAction<TimeWindow[]>>;
  tzMode: TzMode;
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>;
  disableDuringPeak: boolean;
  setDisableDuringPeak: React.Dispatch<React.SetStateAction<boolean>>;
  protocol: Protocol;
  themeMode: ThemeMode;
  t: TFunction;
}) {
  const [defaultCache, setDefaultCache] = useState<TimeWindow[] | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  // model scope 候选列表：从 preset model_list 拉取，chip 编辑 + datalist 自动补全用。
  const [modelList, setModelList] = useState<string[]>([]);
  // 每窗口 model 输入框临时态（idx → 输入串），Enter/逗号/失定提交为 chip。
  const [modelInput, setModelInput] = useState<Record<number, string>>({});

  useEffect(() => {
    getDefaultPeak(protocol).then(setDefaultCache);
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
    }));
    setWindows(copied);
    setModalOpen(false);
  };
  const update = (idx: number, patch: Partial<TimeWindow>) => {
    setWindows(prev => prev.map((w, i) => i === idx ? { ...w, ...patch } : w));
  };
  const remove = (idx: number) => {
    setWindows(prev => prev.filter((_, i) => i !== idx));
  };
  const add = () => {
    setWindows(prev => [...prev, { start_hour: 0, end_hour: 24, multiplier: 1.0 }]);
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
  const nowPeak = isCurrentlyPeak(windows, Date.now());

  return (
    <FormSection
      title={t("platform.peak", "高峰时段倍率")}
      desc={t("platform.peak_desc", "设置时段倍率（>1 加价 / <1 折扣）。时段按各窗口指定时区的本地时间判定；时区默认 UTC。多窗口按顺序 first-match；不命中 = 1.0。")}
      action={
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <div style={{ display: "flex", gap: 4, padding: 2, background: "var(--bg-glass)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
            {(["local", "utc"] as const).map(m => (
              <Button
                key={m}
                variant="ghost"
                size="sm"
                className={tzMode === m ? "btn-primary" : ""}
                style={{ padding: "2px 8px", fontSize: 11, height: "auto" }}
                onClick={() => setTzMode(m)}
              >
                {m === "local" ? t("platform.timezone_local", "本地") : t("platform.timezone_utc", "UTC+0")}
              </Button>
            ))}
          </div>
          <Button
            variant="ghost"
            size="sm"
            style={{ padding: "2px 8px", fontSize: 11, whiteSpace: "nowrap", height: "auto" }}
            disabled={!defaultCache || defaultCache.length === 0}
            title={!defaultCache || defaultCache.length === 0 ? t("platform.peak_no_default", "该平台无默认高峰配置") : ""}
            onClick={() => setModalOpen(true)}
          >
            {t("platform.peak_import_default", "导入默认配置")}
          </Button>
        </div>
      }
    >
      {/* 高峰禁用开关：启用后该平台在 peak window 命中时从路由候选排除（不改 status，临时闸门）。 */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: 8, borderRadius: "var(--radius-sm)", background: "var(--bg-glass)", border: "1px solid var(--border)" }}>
        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--text-secondary)", cursor: "pointer" }}>
          <Checkbox
            checked={disableDuringPeak}
            onCheckedChange={v => setDisableDuringPeak(v === true)}
          />
          <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{t("platform.disable_during_peak", "高峰期禁用")}</span>
        </label>
        {/* 实时态预览：仅当开关 on 才显（off 时无意义） */}
        {disableDuringPeak && (
          <span
            style={{
              fontSize: 11, fontWeight: 600, padding: "2px 8px", borderRadius: 5, whiteSpace: "nowrap",
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
              ? t("platform.currently_peak", "当前：高峰期")
              : t("platform.currently_off_peak", "当前：非高峰期")}
          </span>
        )}
        <span style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4, marginLeft: 4 }}>
          {t("platform.disable_during_peak_desc", "启用后该平台在高峰时段从路由候选排除（不改 status，临时闸门）。")}
        </span>
      </div>
      {windows.length === 0 && (
        <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
          {t("platform.peak_empty", "未配置 → 按预设默认或 1.0（无调整）")}
        </div>
      )}
      {windows.map((w, idx) => (
        <div key={idx} style={{ display: "flex", flexDirection: "column", gap: 6, padding: 8, borderRadius: "var(--radius-sm)", background: "var(--bg-glass)", border: "1px solid var(--border)" }}>
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
                  <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                    {t("platform.start_hour", "起")}
                    <Input
                      className="input" type="number" min={0} max={23} style={{ width: 64 }}
                      value={startDisp.hour}
                      onChange={e => {
                        const utc = fromDisp(clampInt(e.target.value, 0, 23), startDisp.minute);
                        update(idx, { start_hour: utc.hour, start_minute: utc.minute });
                      }}
                    />
                    :
                    <Input
                      className="input" type="number" min={0} max={59} style={{ width: 64 }}
                      value={startDisp.minute}
                      onChange={e => {
                        const utc = fromDisp(startDisp.hour, clampInt(e.target.value, 0, 59));
                        update(idx, { start_hour: utc.hour, start_minute: utc.minute });
                      }}
                    />
                  </label>
                  <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                    {t("platform.end_hour", "止")}
                    <Input
                      className="input" type="number" min={0} max={24} style={{ width: 64 }}
                      value={endDisp.hour}
                      onChange={e => {
                        const utc = fromDisp(clampInt(e.target.value, 0, 24), endDisp.minute);
                        update(idx, { end_hour: utc.hour, end_minute: utc.minute });
                      }}
                    />
                    :
                    <Input
                      className="input" type="number" min={0} max={59} style={{ width: 64 }}
                      value={endDisp.minute}
                      onChange={e => {
                        const utc = fromDisp(endDisp.hour, clampInt(e.target.value, 0, 59));
                        update(idx, { end_hour: utc.hour, end_minute: utc.minute });
                      }}
                    />
                  </label>
                </>
              );
            })()}
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
              {t("platform.multiplier", "倍率")}
              <Input
                className="input" type="number" step={0.1} min={0} style={{ width: 84 }}
                value={w.multiplier}
                onChange={e => update(idx, { multiplier: Number(e.target.value) || 1 })}
              />
            </label>
            {/* 窗口时区：__utc__ 哨兵 = 无 timezone 字段（= UTC，向后兼容）；
                切换时区只改解释基准，存值数字不动（用户自定时段含义）。 */}
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
              {t("platform.window_timezone", "时区")}
              <Select
                value={w.timezone ?? "__utc__"}
                onValueChange={(v) => update(idx, { timezone: v === "__utc__" ? undefined : v })}
              >
                <SelectTrigger className="input" style={{ width: 150, fontSize: 11, height: 28 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {WINDOW_TIMEZONES.map((tz) => (
                    <SelectItem key={tz} value={tz}>
                      {tz === "__utc__" ? t("platform.window_timezone_utc_default", "UTC（默认）") : tz}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <div style={{ display: "flex", gap: 2 }}>
              {WEEKDAY_LABELS.map((lbl, di) => {
                const active = (w.days_of_week ?? []).includes(di);
                return (
                  <Button
                    key={di}
                    variant="ghost"
                    size="icon"
                    title={t("platform.days_of_week", "星期（0=周日…6=周六，缺省=每天）")}
                    onClick={() => toggleDay(idx, di)}
                    style={{
                      width: 22, height: 22, padding: 0, fontSize: 11, lineHeight: 1,
                      borderRadius: "var(--radius-sm)", cursor: "pointer",
                      background: active ? "var(--primary)" : "transparent",
                      color: active ? "var(--primary-foreground)" : "var(--text-secondary)",
                      border: "1px solid var(--border)",
                    }}
                  >
                    {lbl}
                  </Button>
                );
              })}
            </div>
            <Button
              variant="ghost"
              size="icon"
              title={t("platform.remove_window", "删除窗口")}
              onClick={() => remove(idx)}
              style={{ marginLeft: "auto" }}
            >
              ✕
            </Button>
          </div>
          {/* 窗口预览行：可读时段（半开区间 [start, end) → end-1:59:59） */}
          <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4 }}>
            {formatWindowPreview(w, tzMode, t)}
          </div>
          {/* model scope（受影响模型）：chip 多选 + 自由输入（支持 `prefix*` 通配）。
              absent / 空 = 全平台（标「全部模型」），UI 暴露 model 维度过滤可见性。 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
              {t("platform.peak_model_scope", "受影响模型")}
              {(!w.models || w.models.length === 0) && (
                <span style={{ color: "var(--text-tertiary)", marginLeft: 4 }}>
                  · {t("platform.peak_model_scope_all", "全部模型")}
                </span>
              )}
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
              {(w.models ?? []).map((m, mi) => (
                <span
                  key={mi}
                  className="badge badge-muted"
                  style={{ fontSize: 10, padding: "2px 6px", display: "inline-flex", alignItems: "center", gap: 4 }}
                >
                  {m}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => removeModel(idx, mi)}
                    style={{
                      width: "auto", height: "auto", minWidth: "auto",
                      border: "none", background: "transparent", color: "var(--text-tertiary)",
                      cursor: "pointer", padding: 0, lineHeight: 1, fontSize: 12,
                    }}
                    title={t("platform.remove_window", "删除窗口")}
                  >
                    ✕
                  </Button>
                </span>
              ))}
              <Input
                className="input"
                style={{ width: 160, fontSize: 11, padding: "2px 6px" }}
                list={`peak-models-${idx}`}
                placeholder={t("platform.peak_model_placeholder", "模型名或 前缀*")}
                value={modelInput[idx] ?? ""}
                onChange={e => setModelInput(s => ({ ...s, [idx]: e.target.value }))}
                onKeyDown={e => {
                  if (e.key === "Enter" || e.key === ",") {
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
                {modelList.map(m => <option key={m} value={m} />)}
              </datalist>
            </div>
          </div>
          {/* 生效期：start_at / end_at（Unix 秒 ↔ datetime-local）。
              空 = 立即可用 / 永久；福利期自动切换用（design §1.3）。 */}
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11, color: "var(--text-secondary)" }}>
              {t("platform.peak_start_at", "生效起始")}
              <Input
                className="input"
                type="datetime-local"
                style={{ width: 180, fontSize: 11, padding: "2px 6px", colorScheme: themeMode }}
                value={secToLocalInput(w.start_at)}
                onChange={e => {
                  const sec = localInputToSec(e.target.value);
                  update(idx, sec == null ? { start_at: undefined } : { start_at: sec });
                }}
              />
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11, color: "var(--text-secondary)" }}>
              {t("platform.peak_end_at", "生效截止")}
              <Input
                className="input"
                type="datetime-local"
                style={{ width: 180, fontSize: 11, padding: "2px 6px", colorScheme: themeMode }}
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
      <Button variant="ghost" size="sm" onClick={add}>
        + {t("platform.add_window", "添加窗口")}
      </Button>
      <Dialog open={modalOpen} onOpenChange={(v) => { if (!v) setModalOpen(false); }}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 400 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 14 }}>
              {t("platform.peak_overwrite_confirm_title", "覆盖高峰配置？")}
            </DialogTitle>
            <DialogDescription style={{ fontSize: 12 }}>
              {t("platform.peak_overwrite_confirm_body", "当前高峰配置将被默认值替换（{{count}} 个窗口），此操作不可撤销。").replace("{{count}}", String(defaultCache?.length ?? 0))}
            </DialogDescription>
          </DialogHeader>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button
              variant="ghost"
              onClick={() => setModalOpen(false)}
            >
              {t("platform.peak_overwrite_cancel_button", "取消")}
            </Button>
            <Button
              onClick={handleImportDefault}
            >
              {t("platform.peak_overwrite_confirm_button", "确认")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </FormSection>
  );
}

export function GroupAssignSection({ editing, lockedGroupId, groupDetails, autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds, t }: {
  editing: Platform | null;
  lockedGroupId: number | null;
  groupDetails: GroupDetail[];
  autoGroup: boolean; setAutoGroup: React.Dispatch<React.SetStateAction<boolean>>;
  joinGroupIds: number[]; setJoinGroupIds: React.Dispatch<React.SetStateAction<number[]>>;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.groupAssignTitle", "分组归属")}
      desc={t("platform.groupAssignDesc", "可同时创建默认分组并加入其他已有分组；都不选则该平台不在任何分组。")}
    >
      {lockedGroupId != null ? (
        <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
          <span className="badge badge-muted" style={{ padding: "0 6px" }}>
            {groupDetails.find(g => g.group.id === lockedGroupId)?.group.name ?? `#${lockedGroupId}`}
          </span>
          {t("platform.groupLocked", "已锁定到此分组")}
        </div>
      ) : !editing ? (
        // 创建默认分组是「创建时一次性判断」，仅创建表单显示；编辑表单不再判断建组。
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <span style={{ fontSize: 13 }}>{t("platform.groupAssignAuto", "创建默认分组")}</span>
          <Switch checked={autoGroup} onCheckedChange={setAutoGroup} />
        </div>
      ) : null}
      {lockedGroupId == null && groupDetails.length > 0 && (
        <>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", margin: "10px 0 6px" }}>
            {t("platform.groupAssignJoin", "加入已有分组")}
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
                      padding: "4px 12px", borderRadius: 999, fontSize: 12, fontWeight: 500,
                      cursor: "pointer",
                      border: `1px solid ${checked ? "var(--primary)" : "var(--border)"}`,
                      background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                      color: checked ? "var(--primary)" : "var(--text-secondary)",
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

export function ExpirySection({ expiresAt, setExpiresAt, expiryEnabled, setExpiryEnabled, themeMode, t }: {
  expiresAt: number; setExpiresAt: React.Dispatch<React.SetStateAction<number>>;
  expiryEnabled: boolean; setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  themeMode: "light" | "dark";
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.expiresAt", "过期时间")}
      desc={t("platform.expiresAtHint", "可选。到期后该平台自动从路由候选排除（等效禁用），改值或清空即恢复。")}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: expiryEnabled ? 8 : 0 }}>
        <span style={{ fontSize: 13 }}>{t("platform.expiresAtEnable", "启用过期")}</span>
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
              style={{ fontSize: 12, padding: "4px 10px" }}
              onClick={() => setExpiresAt(0)}
            >
              {t("platform.expiresAtClear", "清空")}
            </Button>
          )}
          {expiresAt > 0 && (
            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
              {(() => {
                const nowMs = Date.now();
                if (nowMs >= expiresAt) {
                  return t("platform.expired", "已过期");
                }
                const inDay = expiresAt - nowMs < 86_400_000;
                const txt = formatDateTime(expiresAt) || "-";
                return inDay
                  ? t("platform.expiresAtSoon", "临近过期：{{time}}", { time: txt })
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
