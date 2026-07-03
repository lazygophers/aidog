// formSections — PlatformEditForm 的表单分区子组件（纯 props 驱动，无自身 state）。
// ponytail: 从 PlatformEditForm.tsx 抽出以控制文件行数（每分区 ~50-150 行，合计 ~740 行）。
//   所有分区组件经 props 收 state/setters，不持有自己的 state；跨组件无共享 useState（符合
//   design.md 决策锁「区块拆保持单组件内 state」）。
//
// 共用 helpers（FormSection / ApiKeyField / toDatetimeLocal）也在此导出，供 PlatformEditForm 主组件消费。
//   EndpointsSection / ModelsSection 已移到 formSectionsEndpoints.tsx（体积大），通过末尾 re-export 暴露。
import React from "react";
import type { TFunction } from "i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  type Platform, type Protocol, type PlatformEndpoint,
  type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit,
  type NewApiConfig, type SchedulingBreakerSettings, type GroupDetail,
} from "../../services/api";
import { LevelPriorityControl } from "../../components/platforms/PlatformCard";
import { newManualBudget } from "../../domains/platforms";

/** 毫秒时间戳 → datetime-local input 值 "YYYY-MM-DDTHH:MM"（本地时区，无秒）。
 *  datetime-local 不解析 ISO Z 后缀，须手动拼本地时间分量。 */
export function toDatetimeLocal(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
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

/** API Key 显隐 + 复制按钮（透传配置区 / 认证区共用）。 */
export function ApiKeyField({ value, onChange, show, onToggleShow, editing, placeholder = "API Key" }: {
  value: string;
  onChange: (v: string) => void;
  show: boolean;
  onToggleShow: () => void;
  editing?: boolean;
  placeholder?: string;
}) {
  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      <input
        className="input"
        type={show ? "text" : "password"}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{ flex: 1 }}
      />
      <button
        type="button"
        className="btn btn-ghost btn-icon"
        title={show ? "Hide key" : "Show key"}
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
      </button>
      {editing && value && (
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          title="Copy key"
          onClick={() => void writeText(value)}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
        </button>
      )}
    </div>
  );
}

export function NewApiBalanceConfigSection({ config, onChange, t }: {
  config: NewApiConfig;
  onChange: React.Dispatch<React.SetStateAction<NewApiConfig>>;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.newapiBalanceConfig", "余额查询配置")}
      desc={t("platform.newapiBalanceHint", "查询余额需要独立的地址和 Token（从控制台获取），与 API Key 不同")}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.newapiBalanceUrl", "余额查询地址")}
        </div>
        <input
          className="input"
          placeholder={t("platform.newapiBalanceUrlPlaceholder", "https://your-newapi-instance.com")}
          value={config.balance_base_url}
          onChange={(e) => onChange(prev => ({ ...prev, balance_base_url: e.target.value }))}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.newapiBalanceKey", "余额查询 Token")}
        </div>
        <input
          className="input"
          type="text"
          placeholder={t("platform.newapiBalanceKeyPlaceholder", "sess-xxxx 或 access token")}
          value={config.balance_api_key}
          onChange={(e) => onChange(prev => ({ ...prev, balance_api_key: e.target.value }))}
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {t("platform.newapiUserId", "用户 ID")}
        </div>
        <input
          className="input"
          placeholder={t("platform.newapiUserIdPlaceholder", "数字 ID（可选）")}
          value={config.user_id}
          onChange={(e) => onChange(prev => ({ ...prev, user_id: e.target.value }))}
        />
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
        <input
          className="input"
          placeholder="https://api.anthropic.com"
          value={endpoints[0]?.base_url ?? ""}
          onChange={(e) => {
            const next = [...endpoints];
            if (next.length === 0) {
              next.push({ protocol: "anthropic" as Protocol, base_url: e.target.value, client_type: "default" });
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
        placeholder={t("platform.apiKeyOptional", "API Key（可选，透传可留空）")}
      />
      <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
        {t("platform.passthroughNote", "纯透传：客户端请求的 header（含订阅 OAuth 认证）与 body 原样转发，aidog 不做任何转换或认证注入。上方 API Key 可留空。")}
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
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
          onClick={() => setBudgets([...budgets, newManualBudget()])}
        >
          {t("platform.manualBudgetAdd", "添加限额")}
        </button>
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
            <select
              className="input"
              style={{ width: 110, flexShrink: 0 }}
              value={b.kind}
              onChange={e => onKindChange(e.target.value as ManualBudgetKind)}
            >
              <option value="total">{t("platform.manualBudgetKindTotal", "总额")}</option>
              <option value="rolling">{t("platform.manualBudgetKindRolling", "滑动窗口")}</option>
              <option value="fixed">{t("platform.manualBudgetKindFixed", "固定窗口")}</option>
              <option value="daily">{t("platform.manualBudgetKindDaily", "每日")}</option>
            </select>
            <select
              className="input"
              style={{ width: 90, flexShrink: 0 }}
              value={b.unit}
              onChange={e => update({ unit: e.target.value as ManualBudgetUnit })}
            >
              <option value="usd">$ USD</option>
              <option value="token">{t("platform.manualBudgetUnitToken", "Token")}</option>
            </select>
            <input
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
                <input
                  className="input"
                  type="number"
                  min={0}
                  step="any"
                  style={{ width: 80, flexShrink: 0 }}
                  placeholder={t("platform.manualBudgetWindow", "窗口")}
                  value={b.window_hours ?? ""}
                  onChange={e => update({ window_hours: e.target.value === "" ? null : (parseFloat(e.target.value) || 0) })}
                />
                <select
                  className="input"
                  style={{ width: 90, flexShrink: 0 }}
                  value={b.window_unit ?? "hour"}
                  onChange={e => update({ window_unit: e.target.value as WindowUnit })}
                >
                  <option value="minute">{t("platform.windowUnitMinute", "分钟")}</option>
                  <option value="hour">{t("platform.windowUnitHour", "小时")}</option>
                  <option value="day">{t("platform.windowUnitDay", "天")}</option>
                  <option value="week">{t("platform.windowUnitWeek", "周")}</option>
                  <option value="month">{t("platform.windowUnitMonth", "月")}</option>
                </select>
              </>
            )}
            <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
              <input
                type="checkbox"
                checked={b.enabled}
                onChange={e => update({ enabled: e.target.checked })}
              />
              {t("platform.manualBudgetEnabled", "启用")}
            </label>
            <button
              type="button"
              className="btn btn-ghost btn-icon btn-danger"
              style={{ flexShrink: 0 }}
              title={t("action.delete", "删除")}
              onClick={() => setBudgets(budgets.filter((_, i) => i !== idx))}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
              </svg>
            </button>
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
        <input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_failure_threshold)) : t("platform.breakerInheritGeneric", "继承默认")}
          value={failure}
          onChange={e => setFailure(e.target.value)}
        />
        <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerOpenSecs", "熔断时长(秒)")}</span>
        <input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_open_secs)) : t("platform.breakerInheritGeneric", "继承默认")}
          value={openSecs}
          onChange={e => setOpenSecs(e.target.value)}
        />
        <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "半开探测数")}</span>
        <input
          className="input" type="number" min={0} style={{ width: 140 }}
          placeholder={defaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(defaults.breaker_half_open_max)) : t("platform.breakerInheritGeneric", "继承默认")}
          value={halfOpenMax}
          onChange={e => setHalfOpenMax(e.target.value)}
        />
      </div>
    </FormSection>
  );
}

export function GroupAssignSection({ editing, lockedGroupId, groupDetails, autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds, uniqueGroupInfo, levelPriority, setLevelPriority, t }: {
  editing: Platform | null;
  lockedGroupId: number | null;
  groupDetails: GroupDetail[];
  autoGroup: boolean; setAutoGroup: React.Dispatch<React.SetStateAction<boolean>>;
  joinGroupIds: number[]; setJoinGroupIds: React.Dispatch<React.SetStateAction<number[]>>;
  uniqueGroupInfo: { show: boolean; groupId: number | null; isAuto: boolean };
  levelPriority: number; setLevelPriority: React.Dispatch<React.SetStateAction<number>>;
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
          <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
            <input type="checkbox" checked={autoGroup} onChange={e => setAutoGroup(e.target.checked)} style={{ display: "none" }} />
            <span className={`toggle ${autoGroup ? "active" : ""}`} />
          </label>
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
                  <button
                    key={gd.group.id}
                    type="button"
                    onClick={() => setJoinGroupIds(prev => checked
                      ? prev.filter(id => id !== gd.group.id)
                      : [...prev, gd.group.id])}
                    style={{
                      display: "inline-flex", alignItems: "center",
                      padding: "4px 12px", borderRadius: 999, fontSize: 12, fontWeight: 500,
                      cursor: "pointer",
                      border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                      background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                      color: checked ? "var(--accent)" : "var(--text-secondary)",
                      transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
                    }}
                  >
                    {gd.group.name}
                  </button>
                );
              })}
          </div>
        </>
      )}
      {/* 唯一分组时提供 per-group 优先级设置（复用 Groups 页同款控件）。
          多分组/零分组不显示——语义上 level_priority 属 group×platform 关联。 */}
      {uniqueGroupInfo.show && (
        <div style={{ marginTop: 12 }}>
          <LevelPriorityControl value={levelPriority} onChange={setLevelPriority} />
        </div>
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
        <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
          <input
            type="checkbox"
            checked={expiryEnabled}
            onChange={e => {
              const v = e.target.checked;
              setExpiryEnabled(v);
              // ON→OFF：清零 expiresAt（不生效）；OFF→ON：保留 expiresAt 若有粘贴识别值（预填）。
              if (!v) setExpiresAt(0);
            }}
            style={{ display: "none" }}
          />
          <span className={`toggle ${expiryEnabled ? "active" : ""}`} />
        </label>
      </div>
      {expiryEnabled && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <input
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
            <button
              type="button"
              className="btn btn-ghost"
              style={{ fontSize: 12, padding: "4px 10px" }}
              onClick={() => setExpiresAt(0)}
            >
              {t("platform.expiresAtClear", "清空")}
            </button>
          )}
          {expiresAt > 0 && (
            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
              {(() => {
                const nowMs = Date.now();
                if (nowMs >= expiresAt) {
                  return t("platform.expired", "已过期");
                }
                const inDay = expiresAt - nowMs < 86_400_000;
                const txt = new Date(expiresAt).toLocaleString();
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

export function ClaudeConfigSection({ show, setShow, json, setJson, globalConfig, t }: {
  show: boolean; setShow: React.Dispatch<React.SetStateAction<boolean>>;
  json: string; setJson: React.Dispatch<React.SetStateAction<string>>;
  globalConfig: Record<string, any>;
  t: TFunction;
}) {
  return (
    <FormSection title={t("settings.claudeCodeConfig")}>
      <button
        type="button"
        className="btn btn-ghost"
        style={{
          width: "100%",
          justifyContent: "space-between",
          fontSize: 12,
          padding: "6px 4px",
          color: "var(--text-secondary)",
        }}
        onClick={() => setShow(!show)}
      >
        <span style={{ fontWeight: 600 }}>{t("settings.claudeConfigToggle", "Config Override")}</span>
        <span style={{ opacity: 0.5 }}>{show ? "▾" : "▸"}</span>
      </button>
      {show && (
        <div className="animate-fade-in" style={{ marginTop: 6 }}>
          <textarea
            className="input"
            style={{
              fontFamily: '"SF Mono", "Fira Code", monospace',
              fontSize: 12,
              lineHeight: 1.6,
              minHeight: 180,
              resize: "vertical",
              whiteSpace: "pre",
            }}
            value={json}
            onChange={(e) => setJson(e.target.value)}
            spellCheck={false}
          />
          <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, lineHeight: 1.5 }}>
            {t("settings.platformConfigHint")}
          </div>
          {(() => {
            try {
              const merged = JSON.parse(json);
              const overridden = Object.keys(merged).filter(
                k => JSON.stringify(merged[k]) !== JSON.stringify(globalConfig[k]),
              );
              return overridden.length > 0 ? (
                <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                  {overridden.map(k => (
                    <span key={k} className="badge badge-accent" style={{ fontSize: 10 }}>
                      {k}
                    </span>
                  ))}
                </div>
              ) : (
                <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
                  {t("settings.allAligned")}
                </div>
              );
            } catch { return null; }
          })()}
        </div>
      )}
    </FormSection>
  );
}

// ponytail: EndpointsSection / ModelsSection 因体积大移到 formSectionsEndpoints.tsx。
//   在此 re-export 保持 PlatformEditForm 单一 import 入口不变（barrel 模式）。
export { EndpointsSection, ModelsSection } from "./formSectionsEndpoints";
