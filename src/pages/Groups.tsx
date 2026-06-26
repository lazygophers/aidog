import { useState, useEffect, useReducer, useCallback, useRef, useMemo, Fragment, memo } from "react";
import { createPortal } from "react-dom";
import type { ReactNode, CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";
import type { TFunction } from "i18next";
import {
  groupDetailApi, groupApi, groupUsageApi, platformApi, proxyApi, onProxyLogUpdated, modelTestApi,
  type GroupDetail, type GroupPlatformDetail, type Platform, type RoutingMode, type ModelSlot, type PlatformUsageStats,
  type ModelMapping, type PlatformQuota, type LastTestResult,
} from "../services/api";
import { SortableList, type DragHandleProps } from "../components/SortableList";
import { IconClose, IconCheck, IconHome, IconBolt, IconCost } from "../components/icons";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../utils/formatters";
import { CompactCard, StatChip, BalanceBar, successRateLevel, costLevel } from "../components/shared";
import { getPlatformLogo, getFaviconUrl } from "../assets/platforms";
import { MiddlewareRulesPanel } from "../components/settings/MiddlewareRules";
import { ModelTestPanel } from "./ModelTestPanel";
import { PlatformCard, type PlatformCardActions } from "../components/platforms/PlatformCard";
import { usePlatformCards } from "../components/platforms/usePlatformCards";
import { ShareModal } from "../components/platforms/ShareModal";

const MODEL_SLOTS: ModelSlot[] = ["default", "sonnet", "opus", "haiku", "gpt"];

/** 分组一键测试并发上限：同时最多 N 个平台在测，完一个补下一个。 */
const BATCH_TEST_CONCURRENCY = 3;

/** 全部调度策略（与 api.ts RoutingMode 契约对齐，禁裸 string）。 */
const ROUTING_MODES: RoutingMode[] = ["failover", "load_balance", "health_aware", "least_latency", "sticky"];

/** 策略短名（i18n，缺键回退默认中文）。 */
function routingModeLabel(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.failover", "故障转移"),
    load_balance: t("group.loadBalance", "负载均衡"),
    health_aware: t("group.routingMode.health_aware", "健康感知"),
    least_latency: t("group.routingMode.least_latency", "最低延迟"),
    sticky: t("group.routingMode.sticky", "会话粘性"),
  };
  return map[mode] ?? mode;
}

/** 策略说明（下拉旁提示）。 */
function routingModeDesc(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.routingModeDesc.failover", "按优先级升序选平台，失败逐个回退。"),
    load_balance: t("group.routingModeDesc.load_balance", "在可用平台间加权随机分流。"),
    health_aware: t("group.routingModeDesc.health_aware", "摘除熔断平台后，在健康平台间加权随机。"),
    least_latency: t("group.routingModeDesc.least_latency", "按各平台延迟均值升序优先选最快平台。"),
    sticky: t("group.routingModeDesc.sticky", "同会话绑定同一平台，失效/熔断后回退加权随机。"),
  };
  return map[mode] ?? "";
}

/** Group 图标：仅关联 1 个平台时跟随该平台 logo（与 Platforms 页一致），否则回退分组名首字文字框。 */
function GroupIcon({ gps, group }: { gps: GroupDetail["platforms"]; group: GroupDetail["group"] }) {
  const [favFailed, setFavFailed] = useState(false);
  const single = gps.length === 1 ? gps[0].platform : null;
  const logo = single ? getPlatformLogo(single.platform_type) : undefined;
  const favicon = single && !logo && !favFailed ? getFaviconUrl(single) : null;
  const box = {
    width: 32, height: 32, borderRadius: "var(--radius-sm)", flexShrink: 0,
    display: "flex", alignItems: "center", justifyContent: "center",
  } as const;
  if (single && (logo || favicon)) {
    return (
      <div style={{ ...box, background: "transparent" }}>
        <img src={(logo || favicon) as string} alt={single.name}
          onError={() => { if (favicon) setFavFailed(true); }}
          style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
      </div>
    );
  }
  return (
    <div style={{
      ...box,
      background: group.auto_from_platform ? "var(--bg-glass)" : "var(--accent-subtle)",
      color: group.auto_from_platform ? "var(--text-secondary)" : "var(--accent)",
      fontSize: 13, fontWeight: 700,
    }}>
      {group.name.slice(0, 3)}
    </div>
  );
}

/** Row model for the sortable selected-platforms list (stable string id for @dnd-kit). */
interface SortablePlatform {
  id: string;
  platformId: number;
}

// ── Design tokens (shared by edit/create views; mirror of F/S below) ──
const PICKER_F = { label: 15, body: 15, hint: 13, small: 12 } as const;

/**
 * 关联平台选择器：已选平台拖拽重排（顺序=优先级）+ 上下移 + 移除 + 下拉添加。
 * 编辑视图与创建视图共用，确保两处交互/组件一致（创建时分组尚无 id，故纯受控 platformIds）。
 */
function PlatformPicker({ platformIds, options, onChange, t }: {
  platformIds: number[];
  options: Platform[];
  onChange: (ids: number[]) => void;
  t: TFunction;
}) {
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <SortableList<SortablePlatform>
          items={platformIds.map(pid => ({ id: String(pid), platformId: pid }))}
          onReorder={next => onChange(next.map(row => row.platformId))}
          renderItem={(row, handle) => {
            const pid = row.platformId;
            const i = platformIds.indexOf(pid);
            const p = options.find(pp => pp.id === pid);
            if (!p) return null;
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 10,
                padding: "8px 12px", borderRadius: "var(--radius-sm)",
                background: "var(--bg-glass)",
                border: "1px solid var(--border)",
                marginBottom: 4,
                transition: "opacity 0.15s, border-color 0.15s",
              }}>
                <span
                  ref={handle.ref}
                  {...handle.attributes}
                  {...handle.listeners}
                  title={t("group.dragToReorder", "拖动排序")}
                  style={{
                    cursor: "grab", color: "var(--text-tertiary)", fontSize: 14,
                    lineHeight: 1, userSelect: "none", flexShrink: 0, touchAction: "none",
                  }}
                >⠿</span>
                <span style={{ fontSize: PICKER_F.hint, color: "var(--text-tertiary)", width: 20, textAlign: "center" }}>
                  {i + 1}
                </span>
                <span style={{
                  width: 28, height: 28, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: "var(--accent-subtle)", color: "var(--accent)",
                  fontSize: 11, fontWeight: 700, flexShrink: 0,
                }}>
                  {p.platform_type.slice(0, 2).toUpperCase()}
                </span>
                <span style={{ flex: 1, fontSize: PICKER_F.body, fontWeight: 500 }}>{p.name}</span>
                <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                  disabled={i === 0}
                  onClick={() => {
                    const ids = [...platformIds];
                    [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
                    onChange(ids);
                  }}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M5 2v6M2 5l3-3 3 3" />
                  </svg>
                </button>
                <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                  disabled={i === platformIds.length - 1}
                  onClick={() => {
                    const ids = [...platformIds];
                    [ids[i], ids[i + 1]] = [ids[i + 1], ids[i]];
                    onChange(ids);
                  }}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M5 8V2M2 5l3 3 3-3" />
                  </svg>
                </button>
                <button type="button" onClick={() => onChange(platformIds.filter(id => id !== pid))} style={{
                  background: "none", border: "none", cursor: "pointer",
                  color: "var(--text-tertiary)", fontSize: PICKER_F.small, padding: 4, lineHeight: 1,
                }}><IconClose size={12} /></button>
              </div>
            );
          }}
        />
      </div>
      {platformIds.length < options.length && (
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <select className="input" style={{ fontSize: PICKER_F.hint, padding: "6px 10px", flex: 1 }}
            onChange={e => {
              const pid = Number(e.target.value);
              if (e.target.value && !platformIds.includes(pid)) {
                onChange([...platformIds, pid]);
              }
              e.target.value = "";
            }}>
            <option value="">{t("group.addPlatform", "+ 添加平台")}</option>
            {options
              .filter(p => !platformIds.includes(p.id))
              .map(p => <option key={p.id} value={p.id}>{p.name} ({p.platform_type})</option>)}
          </select>
        </div>
      )}
    </>
  );
}

/** Row model for the sortable group list (GroupDetail has no top-level stable id). */
interface GroupRow {
  id: string;
  detail: GroupDetail;
}

/** 分组一键测试：单平台测试行状态（串行执行，面板实时刷新）。 */
type GroupTestStatus = "pending" | "testing" | "ok" | "fail";
interface GroupTestRow {
  platformId: number;
  name: string;
  status: GroupTestStatus;
  durationMs?: number;
  error?: string;
}

/**
 * 分组一键测试结果面板。逐平台串行测试，行状态实时刷新。
 * createPortal 挂 body —— 脱离 transform 祖先（liquidGlass/animate-fade-in）避免 fixed 退化，
 * 参考 toast 修复（commit 0aeff95）与 memory `css-transform-breaks-fixed-modal`。
 */
function GroupTestPanel({ groupName, rows, running, onClose, t }: {
  groupName: string;
  rows: GroupTestRow[];
  running: boolean;
  onClose: () => void;
  t: TFunction;
}) {
  const ok = rows.filter(r => r.status === "ok").length;
  const fail = rows.filter(r => r.status === "fail").length;
  const done = ok + fail;
  const statusStyle = (s: GroupTestStatus): CSSProperties => ({
    fontSize: 12, fontWeight: 600,
    color: s === "ok" ? "var(--success)" : s === "fail" ? "var(--danger)" : "var(--text-tertiary)",
  });
  const statusText = (r: GroupTestRow): string => {
    if (r.status === "testing") return "…";
    if (r.status === "pending") return t("group.testAllPending", "等待");
    if (r.status === "ok") return t("group.testAllOk", "成功") + (r.durationMs != null ? ` ${r.durationMs}ms` : "");
    return t("group.testAllFail", "失败");
  };
  return createPortal(
    <div onClick={onClose} style={{
      position: "fixed", inset: 0, background: "rgba(0,0,0,0.45)", zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center", padding: 20,
    }}>
      <div className="glass-surface" onClick={e => e.stopPropagation()} style={{
        width: "min(560px, 92vw)", maxHeight: "80vh", overflow: "auto",
        display: "flex", flexDirection: "column", gap: 10, padding: 20,
        background: "var(--bg-floating)",
      }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
          <div style={{ fontSize: 15, fontWeight: 700 }}>
            {t("group.testAllTitle", "测试分组平台")}：{groupName}
          </div>
          <button className="btn btn-ghost btn-icon" onClick={onClose} title={t("action.dismiss", "关闭")}>
            <IconClose size={16} />
          </button>
        </div>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {running
            ? t("group.testAllProgress", "测试中… {{done}}/{{total}}", { done, total: rows.length })
            : t("group.testAllSummary", "完成：{{ok}} 成功 / {{fail}} 失败 / 共 {{total}}", { ok, fail, total: rows.length })}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {rows.map(r => (
            <div key={r.platformId} style={{
              display: "flex", flexDirection: "column", gap: 4, padding: "6px 8px",
              borderRadius: "var(--radius-sm)", background: "var(--bg-glass)",
              borderLeft: r.status === "ok"
                ? "3px solid var(--success)"
                : r.status === "fail" ? "3px solid var(--danger)" : "3px solid transparent",
            }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <span style={{
                  fontSize: 13, flex: 1, minWidth: 0,
                  overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                }}>{r.name}</span>
                <span style={statusStyle(r.status)}>{statusText(r)}</span>
              </div>
              {r.status === "fail" && r.error && (
                <div
                  title={r.error}
                  style={{
                    fontSize: 11, color: "var(--danger)",
                    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  }}
                >
                  {r.error}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** 分组编辑表单态（原 8 个 useState 合并为单 reducer，减少分散 setState） */
interface EditState {
  target: GroupDetail | null;
  name: string;
  mode: RoutingMode;
  platformIds: number[];
  mappings: ModelMapping[];
  reqTimeout: number;
  connTimeout: number;
  maxRetries: number;
}

const EMPTY_EDIT: EditState = {
  target: null,
  name: "",
  mode: "failover",
  platformIds: [],
  mappings: [],
  reqTimeout: 0,
  connTimeout: 0,
  maxRetries: 10,
};

type EditAction =
  | { type: "open"; detail: GroupDetail }
  | { type: "reset" }
  | { type: "patch"; patch: Partial<EditState> };

function editReducer(state: EditState, action: EditAction): EditState {
  switch (action.type) {
    case "open":
      return {
        target: action.detail,
        name: action.detail.group.name,
        mode: action.detail.group.routing_mode,
        platformIds: action.detail.platforms.map(gp => gp.platform.id),
        mappings: action.detail.model_mappings.map(m => ({
          source_model: m.source_model,
          target_platform_id: m.target_platform_id,
          target_model: m.target_model,
          request_timeout_secs: m.request_timeout_secs,
          connect_timeout_secs: m.connect_timeout_secs,
        })),
        reqTimeout: action.detail.group.request_timeout_secs,
        connTimeout: action.detail.group.connect_timeout_secs,
        maxRetries: action.detail.group.max_retries,
      };
    case "reset":
      return EMPTY_EDIT;
    case "patch":
      return { ...state, ...action.patch };
  }
}

/** Extract all non-empty model names (deduplicated) */
function allModelValues(models: Platform["models"]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const slot of MODEL_SLOTS) {
    const v = models[slot];
    if (v && !seen.has(v)) {
      seen.add(v);
      result.push(v);
    }
  }
  return result;
}

/** 不可变 upsert：按 id 替换或追加平台（保引用稳定，命中则只换该项）。 */
function upsertPlatformInto(prev: Platform[], plat: Platform): Platform[] {
  const idx = prev.findIndex(p => p.id === plat.id);
  if (idx === -1) return [...prev, plat];
  const next = prev.slice();
  next[idx] = plat;
  return next;
}

/** Build the `claude` CLI invocation for a given group settings file */
function buildClaudeCommand(settingsName: string): string {
  return [
    "claude",
    "--brief",
    "--dangerously-skip-permissions",
    "--settings",
    `~/.aidog/settings.${settingsName}.json`,
  ].join(" ");
}

/** POSIX shell 单引号安全转义（内部单引号闭合/转义/重开），杜绝注入。 */
function shellSquote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Build the `codex` CLI invocation for a given group profile.
 * `AIDOG_KEY=<group>`（auth token=分组名，aidog 据此路由）+ `codex -p <group>`
 * 选 `~/.codex/<group>.config.toml` profile + bypass approvals/sandbox。
 */
function buildCodexCommand(groupKey: string): string {
  const g = shellSquote(groupKey);
  return [
    `AIDOG_KEY=${g}`,
    "codex",
    "-p",
    g,
    "--dangerously-bypass-approvals-and-sandbox",
    "-a",
    "never",
  ].join(" ");
}

// ─── Design tokens ───
const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;
const S = { gap: 18, pad: 28, inputPad: "10px 14px", btnPad: "8px 18px", btnIcon: 34 } as const;

/** Copy text to clipboard with a brief visual feedback */
function CopyButton({ text, title, label, icon, size = 14 }: { text: string; title?: string; label?: string; icon?: ReactNode; size?: number }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    // Tauri writeText 走权限系统（capabilities default.json allow-write-text），
    // WKWebView 无手势激活时 navigator.clipboard 被拒静默失败，Tauri 路径更可靠（参 ShareModal/SmartPasteModal）。
    writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  const hasContent = !!(label || icon);
  return (
    <button
      className={hasContent ? "btn btn-ghost" : "btn btn-ghost btn-icon"}
      onClick={handleCopy}
      title={title || text}
      style={{ position: "relative", flexShrink: 0, gap: hasContent ? 5 : 0, fontSize: hasContent ? 12 : undefined, padding: hasContent ? "4px 10px" : undefined }}
    >
      {icon ? icon : copied ? (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M20 6L9 17l-5-5" />
        </svg>
      ) : (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
      {!icon && label && <span style={{ fontWeight: 500 }}>{label}</span>}
    </button>
  );
}

/**
 * 拉取每个 group 的使用统计 + 余额。
 * - usage stats：按 proxy_log.group_key 聚合（`groupUsageApi.statsAll` 单次批量），只含本分组请求，共享平台不重复计入。
 * - balance：关联 platforms 的 est_balance_remaining 求和（平台级属性，无 per-group 概念，维持现状）。
 * load 与 refreshStats 共用，避免两处求和逻辑重复。
 */
async function fetchGroupStats(
  details: GroupDetail[],
  platforms: Platform[],
): Promise<{ statsMap: Record<string, PlatformUsageStats>; balanceMap: Record<number, number> }> {
  const platById = new Map(platforms.map(pp => [pp.id, pp]));
  const statsMap: Record<string, PlatformUsageStats> = {};
  const balanceMap: Record<number, number> = {};
  // usage stats：单次批量 invoke（后端 GROUP BY group_key），消除逐 group N+1 往返。
  // 返回 map 仅含有日志的 group；total_requests > 0 时纳入。
  try {
    const all = await groupUsageApi.statsAll();
    for (const g of details) {
      const s = all[g.group.group_key];
      if (s && s.total_requests > 0) statsMap[g.group.group_key] = s;
    }
  } catch { /* ignore */ }
  // balance：关联平台余额求和（保持平台级语义，无 HTTP）。
  for (const g of details) {
    let balance = 0;
    for (const gp of g.platforms) {
      const est = platById.get(gp.platform.id)?.est_balance_remaining;
      if (typeof est === "number" && est > 0) balance += est;
    }
    if (balance > 0) balanceMap[g.group.id] = balance;
  }
  return { statsMap, balanceMap };
}

/** usePlatformCards 的卡片展示状态快照（memo 化子组件按需接收） */
interface CardsSnapshot {
  quotaMap: Record<number, PlatformQuota>;
  quotaRealIds: Record<number, boolean>;
  quotaRefreshing: Record<number, boolean>;
  usageMap: Record<number, PlatformUsageStats>;
  expandedIds: Set<number>;
  testResults: Record<number, "ok" | "fail">;
  testingId: number | null;
  faviconFailed: Set<number>;
  lastTestMap: Record<number, LastTestResult>;
}

/** GroupListItem props：每个分组行的全部渲染依赖，显式化为 props 以支持 React.memo 细粒度更新。 */
interface GroupListItemProps {
  // 数据
  detail: GroupDetail;
  index: number;
  usageStat: PlatformUsageStats | undefined;
  balance: number | undefined;
  platforms: Platform[];
  // 折叠 / 拖拽 UI 状态
  isExpanded: boolean;
  isDragOver: boolean;
  dropIndicatorIdx: number | null; // 当前 gid 的 dropIndicator.idx，null = 无
  dropIndicatorTotal: number;      // fullPlats.length，计算末尾指示线用
  // 映射表单（仅当 mappingGroupId === group.id 时展开）
  showMappingForm: boolean;
  mSource: string;
  mTargetPlatform: number | "";
  mTargetModel: string;
  availableModels: string[];
  // 测试状态（一键测试按钮 disabled）
  groupTestRunning: boolean;
  // cards 快照
  cards: CardsSnapshot;
  actions: PlatformCardActions;
  // 稳定回调（父级 useCallback）
  t: TFunction;
  onToggleExpanded: (id: number) => void;
  onSetCollapsed: (updater: (prev: Set<number>) => Set<number>) => void;
  onEdit: (detail: GroupDetail) => void;
  onDelete: (id: number) => void;
  onToggleDefault: (group: GroupDetail["group"]) => void;
  onTestGroup: (group: GroupDetail["group"], gps: GroupPlatformDetail[]) => void;
  onCreatePlatform?: (presetGroupIds?: number[], lockedGroupId?: number) => void;
  onNavigate?: (id: string, context?: { groupId?: string; groupKey?: string; platformId?: number; platformName?: string; duplicate?: boolean }) => void;
  onPlatPointerDown: (e: React.PointerEvent, pid: number, gid: number) => void;
  onDeleteMapping: (groupId: number, index: number) => void;
  onSetMappingGroupId: (id: number | null) => void;
  onSetMSource: (v: string) => void;
  onSetMTargetPlatform: (v: number | "") => void;
  onSetMTargetModel: (v: string) => void;
  onAddMapping: () => void;
  onSetLevelPriority: (gid: number, pid: number, v: number) => void;
  onPurgeDisabled: (gid: number) => void;
  // drag handle（来自 SortableList，每次由父 renderItem 传入，非稳定）
  handle: DragHandleProps;
}

/**
 * 单个分组行组件，React.memo 包裹。
 * 父 renderItem 只传稳定 props（handle 除外），避免无关父 state 变化触发全组列表重渲。
 * handle 来自 dnd-kit 每次 render 重建，是唯一不稳定 prop；接受此代价——handle 变化仅触发单行更新。
 */
const GroupListItem = memo(function GroupListItem({
  detail, index, usageStat: u, balance, platforms,
  isExpanded, isDragOver, dropIndicatorIdx, dropIndicatorTotal,
  showMappingForm, mSource, mTargetPlatform, mTargetModel, availableModels,
  groupTestRunning,
  cards, actions, t,
  onToggleExpanded: _onToggleExpanded, onSetCollapsed, onEdit, onDelete, onToggleDefault,
  onTestGroup, onCreatePlatform, onNavigate,
  onPlatPointerDown, onDeleteMapping, onSetMappingGroupId,
  onSetMSource, onSetMTargetPlatform, onSetMTargetModel, onAddMapping,
  onSetLevelPriority, onPurgeDisabled,
  handle,
}: GroupListItemProps) {
  const { group, platforms: gps, model_mappings } = detail;
  const totalTokens = u ? u.total_input_tokens + u.total_output_tokens : 0;
  const sRate = u ? calcSuccessRate(u.success_count, u.total_requests) : 0;

  const header = (
    <div style={{ display: "flex", flexDirection: "column", gap: 10, minWidth: 0 }}>
      {/* ── 行 1：身份 + 快操作 ── */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
        {/* Drag handle */}
        <span
          ref={handle.ref}
          {...handle.attributes}
          {...handle.listeners}
          className={`drag-handle drag-handle-inline${handle.isDragging ? " is-active" : ""}`}
          title={t("group.dragToReorder", "拖动排序")}
          style={{ touchAction: "none", flexShrink: 0, display: "inline-flex" }}
          onClick={e => e.stopPropagation()}
        >
          <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
        </span>
        {/* Group icon：单平台跟随平台 logo */}
        <GroupIcon gps={gps} group={group} />
        {/* Name + path + routing + platform count */}
        <div
          style={{ flex: 1, minWidth: 0, cursor: "pointer" }}
          onClick={() => { if (!handle.isDragging) onSetCollapsed(prev => {
            const s = new Set(prev); isExpanded ? s.add(group.id) : s.delete(group.id); return s;
          }); }}
        >
          <div style={{ fontWeight: 600, fontSize: 14, display: "flex", alignItems: "center", gap: 6 }}>
            {group.name}
            {group.is_default && (
              <span className="badge badge-accent" style={{ fontSize: 10, padding: "0 5px", fontWeight: 500 }} title={t("group.isDefaultTitle", "默认分组")}>{t("group.isDefault", "默认")}</span>
            )}
            {group.auto_from_platform && (
              <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 5px", fontWeight: 500 }}>auto</span>
            )}
          </div>
          <div className="text-secondary" style={{ fontSize: 12, display: "flex", gap: 8, marginTop: 1, alignItems: "center", flexWrap: "wrap" }}>
            <span className="badge badge-muted" style={{ padding: "0 6px" }}>
              {routingModeLabel(t, group.routing_mode)}
            </span>
            {gps.length > 0 && (
              <span className="text-tertiary">{gps.length} {t("group.platforms", "平台")}</span>
            )}
          </div>
        </div>
        {/* Quick actions */}
        <CopyButton text={group.group_key} title={t("group.copyApiKeyTitle", "复制 API Key")} size={14} />
        <CopyButton text={buildClaudeCommand(group.group_key)} icon={<img src={claudeIcon} width={14} height={14} alt="Claude" />} title={t("group.copyCommand", "复制 Claude Code 启动命令")} size={14} />
        <CopyButton text={buildCodexCommand(group.group_key)} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} size={14} />
        <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onNavigate?.("stats", { groupId: String(group.id), groupKey: group.group_key }); }} title={t("group.viewStats", "查看统计")}>
          <svg width="14" height="14" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 15V8M7 15V5M11 15V10M15 15V3" />
          </svg>
        </button>
        <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onTestGroup(group, gps); }} disabled={gps.filter(gp => gp.platform.status === "enabled").length === 0 || groupTestRunning} title={t("group.testAll", "一键测试本组全部平台")}>
          <IconBolt size={14} />
        </button>
        {onCreatePlatform && (
          <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onCreatePlatform([group.id], group.id); }} title={t("group.addPlatformToGroup", "在此分组添加平台")}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M7 2v10M2 7h10" />
            </svg>
          </button>
        )}
        {/* 清理本分组失效（auto_disabled）平台：独占的永久删，共享的仅移除本分组关联 */}
        <button
          className="btn btn-ghost"
          onClick={async (e) => {
            e.stopPropagation();
            if (!window.confirm(t("group.purgeDisabledConfirm", "将清理本分组失效平台（独占的永久删除，共享的仅从本分组移除），确定？"))) return;
            onPurgeDisabled(group.id);
          }}
          title={t("group.purgeDisabled", "清理失效")}
          style={{ fontSize: 11, gap: 4, padding: "3px 8px", display: "inline-flex", alignItems: "center", whiteSpace: "nowrap" }}
        >
          {t("group.purgeDisabled", "清理失效")}
        </button>
        {/* 设为默认分组（单选） */}
        <button
          className="btn btn-ghost"
          aria-pressed={group.is_default}
          aria-label={group.is_default
            ? t("group.unsetDefault", "取消默认分组")
            : t("group.setAsDefault", "设为默认分组")}
          onClick={e => { e.stopPropagation(); onToggleDefault(group); }}
          title={group.is_default
            ? t("group.isDefaultTitle", "默认分组：config 已 merge 写入 ~/.claude/settings.json + ~/.codex/config.toml")
            : t("group.setAsDefault", "设为默认分组")}
          style={{
            fontSize: 11, gap: 4, padding: "3px 8px",
            display: "inline-flex", alignItems: "center", whiteSpace: "nowrap",
            ...(group.is_default ? {
              color: "var(--accent)",
              background: "color-mix(in srgb, var(--accent) 14%, transparent)",
              border: "1px solid color-mix(in srgb, var(--accent) 35%, transparent)",
              borderRadius: "var(--radius-sm)",
            } : {}),
          }}
        >
          {group.is_default
            ? <IconCheck size={12} />
            : <IconHome size={12} />}
          {group.is_default
            ? t("group.defaultConfigWritten", "默认配置已写入")
            : t("group.setAsDefault", "设为默认")}
        </button>
        <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onEdit({ group, platforms: gps, model_mappings }); }} title={t("action.edit", "编辑")}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
          </svg>
        </button>
        {(!group.auto_from_platform || gps.length === 0) && (
          <button className="btn btn-ghost btn-icon btn-danger" onClick={(e) => { e.stopPropagation(); onDelete(group.id); }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
            </svg>
          </button>
        )}
      </div>
      {/* ── 行 2：统计 + 余额 ── */}
      {(u || balance != null) && (
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", paddingLeft: 26 }}>
          {/* Aggregate stats chips */}
          {u && (
            <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
              <StatChip icon={<IconBolt size={13} />} value={formatNumber(totalTokens)} label="tokens" />
              <StatChip icon={<IconCost size={13} />} value={`$${formatCost(u.total_cost)}`} label="cost" level={costLevel(u.total_cost)} />
              {u.total_requests > 0 && (
                <StatChip icon={<IconCheck size={13} />} value={formatPercent(sRate, 0)} label="ok"
                  level={successRateLevel(sRate, u.total_requests)} />
              )}
            </div>
          )}
          {/* Aggregate balance */}
          {balance != null && (
            <div style={{ minWidth: 90, flexShrink: 0 }}>
              <BalanceBar remaining={balance} showTotal={false} />
            </div>
          )}
        </div>
      )}
    </div>
  );

  const fullPlats = gps
    .map(gp => platforms.find(pp => pp.id === gp.platform.id))
    .filter((pp): pp is Platform => !!pp);

  return (
    <div
      className="animate-fade-in"
      data-group-id={group.id}
      style={{ animationDelay: `${index * 60}ms` }}
    >
      <CompactCard
        header={header}
        expanded={isExpanded}
        onToggle={(next) => onSetCollapsed(prev => {
          const s = new Set(prev); next ? s.delete(group.id) : s.add(group.id); return s;
        })}
        toggleLabel={t("group.toggleDetails", "展开/收起明细")}
        style={handle.isDragging
          ? { opacity: 0.5 }
          : isDragOver
            ? { outline: "2px solid var(--accent)", outlineOffset: 2 }
            : undefined}
      >
        {(
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }} onClick={e => e.stopPropagation()}>
            {/* 关联平台：完整 PlatformCard（同 Platforms 主列表），点卡片就地展开详情 */}
            {fullPlats.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {fullPlats.map((p, idx) => (
                  <Fragment key={p.id}>
                    {dropIndicatorIdx === idx && (
                      <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-3px 0", opacity: 0.7 }} />
                    )}
                    <div style={{ display: "flex", gap: 4, alignItems: "stretch" }}>
                      {/* pointer 拖拽把手：组内排序 + 跨分组移动（WKWebView 下 HTML5 drop 不可靠，改 pointer） */}
                      <span
                        onPointerDown={(e) => onPlatPointerDown(e, p.id, group.id)}
                        className="drag-handle drag-handle-inline"
                        style={{ cursor: "grab", display: "inline-flex", alignItems: "center", flexShrink: 0, alignSelf: "center", touchAction: "none" }}
                        title={t("group.dragPlatform", "拖拽排序 / 移动到其他分组")}
                      >
                        <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
                      </span>
                      <div data-gp-id={p.id} style={{ flex: 1, minWidth: 0 }}>
                        <PlatformCard
                          platform={p}
                          index={idx}
                          isDragging={false}
                          dragActive={false}
                          quotaRaw={cards.quotaMap[p.id]}
                          quotaPreferReal={!!cards.quotaRealIds[p.id]}
                          refreshing={!!cards.quotaRefreshing[p.id]}
                          usage={cards.usageMap[p.id]}
                          expanded={cards.expandedIds.has(p.id)}
                          manualResult={cards.testResults[p.id]}
                          testing={cards.testingId === p.id}
                          faviconFailed={cards.faviconFailed.has(p.id)}
                          actions={actions}
                          draggable={false}
                          lastTest={cards.lastTestMap[p.id]}
                          levelPriority={gps.find(gp => gp.platform.id === p.id)?.level_priority ?? 5}
                          onLevelPriorityChange={v => onSetLevelPriority(group.id, p.id, v)}
                        />
                      </div>
                    </div>
                  </Fragment>
                ))}
                {dropIndicatorIdx === dropIndicatorTotal && (
                  <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-3px 0", opacity: 0.7 }} />
                )}
              </div>
            )}

            {/* Model Mappings */}
            {model_mappings.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {model_mappings.map((m, mi) => (
                  <div key={mi} style={{
                    display: "flex", alignItems: "center", gap: 8, fontSize: 12,
                    padding: "6px 10px", borderRadius: "var(--radius-sm)",
                    background: "var(--bg-glass)", border: "1px solid var(--border)",
                  }}>
                    <span style={{ fontWeight: 600, color: "var(--accent)" }}>{m.source_model}</span>
                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                      <path d="M2 6h8M8 4l2 2-2 2" />
                    </svg>
                    <span style={{ flex: 1 }}>{m.target_model}</span>
                    <button className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                      onClick={(e) => { e.stopPropagation(); onDeleteMapping(group.id, mi); }}>
                      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                        <path d="M2 2l6 6M8 2l-6 6" />
                      </svg>
                    </button>
                  </div>
                ))}
              </div>
            )}

            {/* Quick Add Mapping */}
            <button className="btn btn-ghost" style={{ fontSize: 12, gap: 4, padding: "4px 8px", color: "var(--text-secondary)", alignSelf: "flex-start" }}
              onClick={(e) => { e.stopPropagation(); onSetMappingGroupId(showMappingForm ? null : group.id); }}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <path d="M6 2v8M2 6h8" />
              </svg>
              {t("mapping.add")}
            </button>

            {showMappingForm && (
              <div className="animate-fade-in" style={{
                paddingTop: 10, borderTop: "1px solid var(--border)",
                display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap",
              }} onClick={e => e.stopPropagation()}>
                <input className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                  placeholder={t("mapping.source")} value={mSource}
                  onChange={(e) => onSetMSource(e.target.value)} />
                <select className="input" style={{ fontSize: 12, width: 140 }} value={mTargetPlatform}
                  onChange={(e) => { onSetMTargetPlatform(e.target.value === "" ? "" : Number(e.target.value)); onSetMTargetModel(""); }}>
                  <option value="">{t("mapping.targetPlatform")}</option>
                  {platforms.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                </select>
                {availableModels.length > 0 ? (
                  <select className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }} value={mTargetModel}
                    onChange={(e) => onSetMTargetModel(e.target.value)}>
                    <option value="">{t("mapping.target")}</option>
                    {availableModels.map((m) => <option key={m} value={m}>{m}</option>)}
                  </select>
                ) : (
                  <input className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                    placeholder={t("mapping.target")} value={mTargetModel}
                    onChange={(e) => onSetMTargetModel(e.target.value)} />
                )}
                <button className="btn btn-primary" style={{ fontSize: 12, padding: "6px 12px" }}
                  onClick={onAddMapping}
                  disabled={!mSource || !mTargetPlatform || !mTargetModel}>
                  {t("action.create")}
                </button>
              </div>
            )}
          </div>
        )}
      </CompactCard>
    </div>
  );
});

/** 分组内嵌组件（供 Platforms 页使用） */
export function GroupsEmbedded({ onNavigate, onGroupsChanged, onCreatePlatform, onEditPlatform, onDuplicatePlatform, onToast, onViewModeChange, openCreateGroupRef, reloadRef, onCountChange }: {
  onNavigate?: (id: string, context?: { groupId?: string; groupKey?: string; platformId?: number; platformName?: string; duplicate?: boolean }) => void;
  onGroupsChanged?: () => void;
  /** 打开平台创建表单；提供 lockedGroupId = 从某分组 ➕ 触发，预绑该分组且锁定归属。 */
  onCreatePlatform?: (presetGroupIds?: number[], lockedGroupId?: number) => void;
  /** 编辑分组展开区平台卡片：父级(Platforms)直接打开同页编辑表单，避免经 onNavigate 往返导航
   *  （navContext.platformId 不变 + 一次性消费 ref 不复位 → 第二次编辑无反应）。 */
  onEditPlatform?: (p: Platform) => void;
  /** 复制分组展开区平台卡片：父级(Platforms)直接打开同页新建表单（灌入源平台配置），同 onEditPlatform 走直调避免 nav 往返。 */
  onDuplicatePlatform?: (p: Platform) => void;
  /** 透传父级 toast setter（快速测试/额度刷新结果反馈）；不传则 usePlatformCards 兜底空函数。 */
  onToast?: (toast: { text: string; ok: boolean } | null) => void;
  /** 进入/退出全屏视图态（创建/编辑分组）时通知父级，供 Platforms 页隐藏下方未分组平台列表。 */
  onViewModeChange?: (fullscreen: boolean) => void;
  /** 父级(Platforms)页头「添加分组」按钮经此 ref 触发本组件创建弹窗（按钮已上移到 Platforms 页头）。
   *  结构型 { current: fn | null } 免 import，与 useRef<fn|null> 兼容。 */
  openCreateGroupRef?: { current: (() => void) | null };
  /** 父级(Platforms)跨组件刷新入口（如全局 purge 删平台后），触发本组件 load() 重建分组/平台状态。
   *  本组件 load() 只在 mount 跑一次，父级 groupDetails 更新不会自动同步到内部 details/platforms。 */
  reloadRef?: { current: (() => void) | null };
  /** 渐进加载计数回传：随各组平台逐组流入而递增/校正（{total, active}），供父级页头
   *  「N / M active」徽章增量更新。null = 尚未开始/重置回退父级自身列表。 */
  onCountChange?: (counts: { total: number; active: number } | null) => void;
}) {
  const { t } = useTranslation();
  const [details, setDetails] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groupStats, setGroupStats] = useState<Record<string, PlatformUsageStats>>({});
  // 聚合余额：关联 platforms 的 est_balance_remaining 求和（platformApi.list 已带，无额外 HTTP）。group.id → 余额；缺值不写入。
  const [groupBalance, setGroupBalance] = useState<Record<number, number>>({});
  const [loading, setLoading] = useState(true);
  // 代理端口（proxy_get_settings），构造页面级 base_url；取失败兜底 7890。
  const [proxyPort, setProxyPort] = useState(7890);
  const proxyBaseUrl = `http://127.0.0.1:${proxyPort}/proxy`;

  // Edit mode（8 字段合并为单 reducer）
  const [edit, dispatchEdit] = useReducer(editReducer, EMPTY_EDIT);
  const {
    target: editTarget,
    name: editName,
    mode: editMode,
    platformIds: editPlatformIds,
    mappings: editMappings,
    reqTimeout: editReqTimeout,
    connTimeout: editConnTimeout,
    maxRetries: editMaxRetries,
  } = edit;

  // ── Drag reorder for group list (via shared SortableList @dnd-kit) ──
  const handleReorderGroups = (next: GroupRow[]) => {
    const reordered = next.map(r => r.detail);
    setDetails(reordered);
    loadedDetailsRef.current = reordered;
    groupApi.reorder(reordered.map(d => d.group.id)).catch(console.error);
  };

  // Create mode
  const [showCreate, setShowCreate] = useState(false);
  // 父级页头「添加分组」按钮经此 ref 触发本组件创建弹窗（按钮上移到 Platforms 页）。
  useEffect(() => {
    if (!openCreateGroupRef) return;
    openCreateGroupRef.current = () => setShowCreate(true);
    return () => { openCreateGroupRef.current = null; };
  }, [openCreateGroupRef]);
  const [cName, setCName] = useState("");
  const [cGroupKey, setCGroupKey] = useState("");
  const [cMode, setCMode] = useState<RoutingMode>("health_aware");
  const [cPlatformIds, setCPlatformIds] = useState<number[]>([]);

  // Mapping form (for quick add in list view)
  const [mappingGroupId, setMappingGroupId] = useState<number | null>(null);
  const [mSource, setMSource] = useState("");
  const [mTargetPlatform, setMTargetPlatform] = useState<number | "">("");
  const [mTargetModel, setMTargetModel] = useState("");

  // 全屏视图态（创建/编辑分组）：通知父级隐藏下方未分组平台列表，避免与全屏视图并列。
  const fullscreenView = editTarget !== null || showCreate;
  useEffect(() => {
    onViewModeChange?.(fullscreenView);
  }, [fullscreenView, onViewModeChange]);

  // 渐进加载序号守卫：每次 load() 自增，异步阶段回调前比对，丢弃陈旧轮次（reload/StrictMode 双跑）的迟到 setState。
  const loadSeqRef = useRef(0);

  // ── 触底加载（J3：反转 H6 单 JOIN 全量批量，改前端分页无限滚动）──
  // 每页拉一批组 detail（后端 group_detail_list_paged 无 JOIN）；滚到底部 sentinel 触发下一页。
  const PAGE_SIZE = 12;
  // 已拉的页数 = 下一页 offset / PAGE_SIZE。loadedPagesRef 与 details 长度解耦（details 可被增量改）。
  const nextOffsetRef = useRef(0);
  // 全量平台快照（platform_list 一次拉全，跨页统计/未分组区/余额复用，无需逐页 invoke）。
  const allPlatformsRef = useRef<Platform[]>([]);
  // 已拉全部组 detail 累积（跨页累加，供统计聚合复用，避免 detailsRef 受分帧/单组刷新干扰）。
  const loadedDetailsRef = useRef<GroupDetail[]>([]);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const loadingMoreRef = useRef(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  /**
   * 加载（invoke 往返 O(1)，流式视觉靠前端分帧而非串行网络往返）：
   *   ① 一次批量 `group_detail_list`（后端单 invoke 拉全部组+平台，已缓存）+ `platform_list` 并行。
   *      —— 旧版主路径逐组串行 `group_detail`（N 组 = N 次 invoke RTT）+ 每平台 `await Promise.resolve()`
   *         （59 次 microtask 让出）= 真正的「分组加载慢」根因，此处改批量消除。
   *   ② 先渲分组骨架（容器/头，平台区空），再用 **rAF 分帧** 逐组 commit 平台卡（保留流式上屏视觉），
   *      节奏由前端 requestAnimationFrame 控制，不再靠网络往返制造。某帧渲一组的卡。
   *   ③ 未分组平台一次性补齐（已含在 platform_list）+ 余额/统计聚合。
   * 计数随每帧 commit 经 onCountChange 增量回传父级页头徽章。
   */
  /** 计数回传（跨组/未分组去重）：基于「全量平台快照」算 total/active，与分页进度解耦。 */
  const reportCount = () => {
    const all = allPlatformsRef.current;
    let active = 0;
    for (const p of all) if (p.enabled) active++;
    onCountChange?.({ total: all.length, active });
  };

  /** 已拉全部组 detail + 全量平台 → 重算统计/余额（增量加载后复用，无额外组 invoke 往返）。 */
  const recomputeStats = async (seqAlive: () => boolean) => {
    const { statsMap, balanceMap } = await fetchGroupStats(loadedDetailsRef.current, allPlatformsRef.current);
    if (!seqAlive()) return;
    setGroupStats(statsMap);
    setGroupBalance(balanceMap);
  };

  /**
   * 触底加载下一页组 detail（后端 group_detail_list_paged，无 JOIN）。
   *   ① 单表分页拉 PAGE_SIZE 组（含其平台关联，后端内存补 platform）；空页 → hasMore=false 停止。
   *   ② append 到 details + 把本页平台 upsert 进 platforms（保留已渲组卡引用稳定）。
   *   ③ 统计/余额按累积 details 重算。
   * seq 守卫：load() 自增 loadSeqRef，分页迟到回调比对丢弃陈旧轮次。
   */
  const loadMore = async () => {
    if (loadingMoreRef.current) return;
    const seq = loadSeqRef.current;
    const alive = () => seq === loadSeqRef.current;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    try {
      const offset = nextOffsetRef.current;
      const page = (await groupDetailApi.listPaged(offset, PAGE_SIZE)) || [];
      if (!alive()) return;
      nextOffsetRef.current = offset + PAGE_SIZE;
      if (page.length < PAGE_SIZE) setHasMore(false);
      if (page.length === 0) return;

      const filled: GroupDetail[] = page.map(d => ({
        group: d.group,
        platforms: d.platforms || [],
        model_mappings: d.model_mappings || d.group.model_mappings || [],
      }));
      loadedDetailsRef.current = [...loadedDetailsRef.current, ...filled];
      setDetails(prev => [...prev, ...filled]);
      setPlatforms(prev => {
        let next = prev;
        for (const d of filled) for (const gp of d.platforms) next = upsertPlatformInto(next, gp.platform);
        return next;
      });
      reportCount();
      await recomputeStats(alive);
    } catch (e) {
      console.error(e);
    } finally {
      loadingMoreRef.current = false;
      if (alive()) setLoadingMore(false);
    }
  };

  /**
   * 全量重载（mount / 结构变化）：重置分页游标 + 全量平台快照，拉第一页组 detail（触底加载首屏）。
   * 后续页由 sentinel IntersectionObserver 经 loadMore 拉取。组列表去 JOIN → 分页（J3 反转 H6）。
   */
  const load = async () => {
    const seq = ++loadSeqRef.current;
    const alive = () => seq === loadSeqRef.current;
    setLoading(true);
    onCountChange?.({ total: 0, active: 0 });
    // 重置分页态 + 累积缓存（reload 时清旧）。
    nextOffsetRef.current = 0;
    loadedDetailsRef.current = [];
    loadingMoreRef.current = false;
    setHasMore(true);
    setDetails([]);
    setPlatforms([]);
    try {
      // 全量平台一次拉（统计/未分组区/余额复用，非分页对象）。
      const allPlatforms: Platform[] = (await platformApi.list()) || [];
      if (!alive()) return;
      allPlatformsRef.current = allPlatforms;
      // 未分组平台一次性补齐（platform_list 已含全量；分组平台由 loadMore 各页 upsert）。
      setPlatforms(prev => {
        let next = prev;
        for (const plat of allPlatforms) next = upsertPlatformInto(next, plat);
        return next;
      });
      reportCount();
      setLoading(false); // 撤内部加载态；首页组卡经 loadMore 填充

      // 首页组 detail（触底加载第一页）。
      await loadMore();
    } catch (e) {
      console.error(e);
      if (alive()) setLoading(false);
    }
  };

  /**
   * 单组就地刷新：只重拉该组 detail（group_detail，O(1) 一次往返），原地替换对应 GroupDetail
   * + 把该组平台 upsert 进 platforms（保留其余组卡引用稳定，避免 load() 全量 setDetails([])/setPlatforms([])
   * 触发整列表骨架重渲 + 多帧重填的「保存卡顿/闪烁」）。组结构已变（增删组）时回退全量 load()。
   */
  const refreshSingleGroup = async (gid: number) => {
    try {
      const d = await groupDetailApi.get(gid);
      if (!d) { load(); return; } // 该组已不存在（被删）→ 全量回退
      const filled: GroupDetail = {
        group: d.group,
        platforms: d.platforms || [],
        model_mappings: d.model_mappings || d.group.model_mappings || [],
      };
      // 组不在当前列表（新建组结构变化）→ 全量 load 以正确插入排序位
      let found = false;
      setDetails(prev => {
        const next = prev.map(x => {
          if (x.group.id !== gid) return x;
          found = true;
          return filled;
        });
        return found ? next : prev;
      });
      if (!found) { load(); return; }
      // 累积缓存同步替换该组（统计聚合复用 loadedDetailsRef，避免陈旧）。
      loadedDetailsRef.current = loadedDetailsRef.current.map(x => x.group.id === gid ? filled : x);
      setPlatforms(prev => {
        let next = prev;
        for (const gp of filled.platforms) next = upsertPlatformInto(next, gp.platform);
        return next;
      });
      // 该组聚合统计/余额轻量补刷（复用现有批量入口，单组开销可忽略）。
      refreshStats();
    } catch (e) {
      console.error(e);
      load(); // 失败兜底全量
    }
  };

  /** 轻量刷新：刷新全量平台快照（含 est_balance_remaining）+ 按已加载组重算 usage stats / 余额聚合，
   *  不拉 quota HTTP、不重拉组（统计基于已触底加载的 loadedDetailsRef，分页一致）。 */
  const refreshStats = async () => {
    try {
      const p = (await platformApi.list()) || [];
      allPlatformsRef.current = p;
      const { statsMap, balanceMap } = await fetchGroupStats(loadedDetailsRef.current, p);
      setGroupStats(statsMap);
      setGroupBalance(balanceMap);
    } catch { /* ignore */ }
  };

  useEffect(() => { load(); }, []);

  // ── 触底加载 sentinel：滚到底部（含全屏视图退出后）拉下一页组 detail（J3 无限滚动）。
  // 依赖 hasMore/loadingMore/loading 重建 observer：避免在加载中/无更多时重复触发；root=null 用视口。
  useEffect(() => {
    const el = sentinelRef.current;
    if (!el || !hasMore || loading) return;
    const io = new IntersectionObserver(entries => {
      if (entries.some(e => e.isIntersecting) && !loadingMoreRef.current) {
        loadMore();
      }
    }, { rootMargin: "200px" });
    io.observe(el);
    return () => io.disconnect();
  }, [hasMore, loading, loadingMore]);

  // 父级跨组件刷新入口（全局 purge 后触发），绑定本组件 load() 重建分组卡内平台状态。
  useEffect(() => {
    if (!reloadRef) return;
    reloadRef.current = () => { load(); onGroupsChanged?.(); };
    return () => { reloadRef.current = null; };
  }, [reloadRef, load, onGroupsChanged]);

  // ── 分组展开区平台卡片：复用 PlatformCard + usePlatformCards（与 Platforms 主列表同款） ──
  // 单实例 hook 跨所有分组共享 state（quota/usage/expanded/test 按 platformId 索引）。
  const cards = usePlatformCards({ onNavigate, onEdit: onEditPlatform, setToast: onToast });
  // usePlatformCards 自身不拉 usage（依赖宿主注入），分组展开区卡片的用量区(请求数/token/cost)否则恒空。
  // quota(余额/coding plan) 走 computeQuotaDisplay 的 est 兜底分支即可展示，真查由卡片刷新按钮按需触发（已天然每卡独立）。
  // 平台级流式：usage(请求数/token/cost) 改为**每卡独立**异步加载（platform_usage_stats 单平台查询），
  // 替换原 usageStatsAll「攒齐全部平台一次性灌入」批量——某卡 usage 查询慢不阻塞同组其它卡上屏/填数。
  // 已加载过的平台不重复查（usageReqRef 去重）；卡片随 platforms 流式增加，本 effect 增量补查新卡。
  const cardsSetUsageMap = cards.setUsageMap;
  const usageReqRef = useRef<Set<number>>(new Set());
  useEffect(() => {
    if (platforms.length === 0) { usageReqRef.current = new Set(); return; }
    let alive = true;
    for (const p of platforms) {
      if (usageReqRef.current.has(p.id)) continue; // 本平台 usage 已发起，跳过
      usageReqRef.current.add(p.id);
      const pid = p.id;
      // 不 await、各卡并发独立：到一个填一个，单卡慢不阻塞其它卡。
      platformApi.usageStats(pid)
        .then(s => { if (alive && s) cardsSetUsageMap(prev => ({ ...prev, [pid]: s })); })
        .catch(() => { /* ignore：该卡 usage 缺失不影响其它卡 */ });
    }
    return () => { alive = false; };
  }, [platforms, cardsSetUsageMap]);
  // 分组展开态：默认全展开。追踪「已折叠」集（默认空 = 全展开），新分组天然展开，
  // 用户折叠状态跨 reload 保持；toggle 切换折叠集成员。
  const [collapsedGroups, setCollapsedGroups] = useState<Set<number>>(new Set());
  const toggleGroupExpanded = useCallback((id: number) => setCollapsedGroups(prev => {
    const s = new Set(prev); s.has(id) ? s.delete(id) : s.add(id); return s;
  }), []);
  // 分组卡片「移除平台」确认态：仅当平台只属当前一个分组（删除即销毁平台，破坏性）时弹确认；
  // 属多组时直接移出本组（保留平台与其他组关联）不弹窗。
  const [removeTarget, setRemoveTarget] = useState<{ platform: Platform; gid: number } | null>(null);

  // 平台所属分组数（按 platform_id 跨 details 计数），用于判定删除 vs 仅移出。
  const groupCountOf = useCallback((pid: number): number =>
    details.reduce((n, d) => n + (d.platforms.some(gp => gp.platform.id === pid) ? 1 : 0), 0),
  [details]);

  // 仅从当前分组移出该平台（不删平台、不动其他组）：用 group_set_platforms 重设本组平台集（去掉该平台）。
  // 不用 group_platform_move 到 group 0——那会 INSERT 一行 group_id=0 的幽灵关联（0 非真实分组）。
  const removePlatformFromGroup = useCallback(async (pid: number, gid: number) => {
    const detail = details.find(d => d.group.id === gid);
    if (!detail) return;
    const remaining = detail.platforms
      .filter(gp => gp.platform.id !== pid)
      .map((gp, i) => ({ platform_id: gp.platform.id, priority: i + 1, weight: gp.weight ?? 1 }));
    try {
      await groupApi.setPlatforms(gid, remaining);
      load(); onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      onToast?.({ text: `${t("group.removeFromGroupFailed", "移出分组失败")}: ${e}`, ok: false });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [details, load, t]);

  // 分组上下文「移除」语义：单属本组→确认后删平台；属多组→直接移出本组。
  const handleGroupRemovePlatform = useCallback((p: Platform, gid: number) => {
    if (groupCountOf(p.id) <= 1) {
      setRemoveTarget({ platform: p, gid });
    } else {
      removePlatformFromGroup(p.id, gid);
    }
  }, [groupCountOf, removePlatformFromGroup]);

  // 确认删除（仅属本组的平台）：走 delete_platform（连带清关联，后端 026289e 已处理）。
  const confirmDeletePlatform = useCallback(async () => {
    if (!removeTarget) return;
    await cards.handleDelete(removeTarget.platform.id);
    setRemoveTarget(null);
    load(); onGroupsChanged?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [removeTarget, cards, load]);

  // 分组上下文 card actions（按 gid 派生）：onDelete 改为「移除」语义（删 vs 移出二分）。
  // 拖拽 no-op（分组内禁拖拽）；启停后 load() 刷新本地 platforms。
  const makeGroupCardActions = useCallback((gid: number): PlatformCardActions => ({
    onPointerDown: () => {}, onPointerMove: () => {}, onPointerUp: () => {},
    onToggleExpanded: cards.toggleExpanded,
    onRefreshQuota: cards.refreshQuota,
    onToggleEnabled: async (p) => { await cards.handleToggle(p); load(); },
    onEdit: cards.handleEdit,
    onShare: cards.handleShare,
    onDuplicate: (p) => {
      if (onDuplicatePlatform) onDuplicatePlatform(p);
      else onNavigate?.("platforms", { platformId: p.id, platformName: p.name, duplicate: true });
    },
    onDelete: (id) => {
      const p = platforms.find(pp => pp.id === id);
      if (p) handleGroupRemovePlatform(p, gid);
    },
    onViewLogs: cards.handleViewLogs,
    onQuickTest: cards.handleQuickTest,
    onCustomTest: cards.handleCustomTest,
    onFaviconFailed: (id) => cards.onFaviconFailed(prev => new Set(prev).add(id)),
    // handlers 来自 usePlatformCards 的 useCallback（稳定）；load 内联故每次重算——分组展开非热路径，可接受
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [cards, load, platforms, handleGroupRemovePlatform, onDuplicatePlatform, onNavigate]);

  // ── per-group 优先级（level_priority）就地编辑：乐观更新 + 失败回滚 + toast ──
  const handleSetLevelPriority = useCallback((gid: number, pid: number, next: number) => {
    let prevValue: number | undefined;
    setDetails(prev => prev.map(d => {
      if (d.group.id !== gid) return d;
      return {
        ...d,
        platforms: d.platforms.map(gp => {
          if (gp.platform.id !== pid) return gp;
          prevValue = gp.level_priority;
          return { ...gp, level_priority: next };
        }),
      };
    }));
    groupDetailApi.setPlatformLevelPriority(gid, pid, next).catch(err => {
      console.error("[aidog] setPlatformLevelPriority failed", err);
      onToast?.({ text: t("group.levelPriorityFailed", "优先级保存失败: {{err}}", { err: String(err) }), ok: false });
      // 回滚到改前值
      setDetails(prev => prev.map(d => {
        if (d.group.id !== gid) return d;
        return {
          ...d,
          platforms: d.platforms.map(gp =>
            gp.platform.id === pid ? { ...gp, level_priority: prevValue } : gp),
        };
      }));
    });
  }, [onToast, t]);

  // ── 分组展开区平台拖拽（pointer 事件驱动，不依赖 HTML5 drop —— WKWebView 下 drop 不可靠） ──
  // 不与 dnd-kit 分组排序冲突：平台拖拽把手与分组排序把手是不同 DOM 节点，dnd-kit 只监听分组 handle。
  // 注：Platforms 主列表「未分组平台拖入分组」走 Platforms.tsx 自有 pointer 流（直接 movePlatform，fromGid=0），
  // 不经此处；此处只处理分组展开区内的组内重排 + 跨组移动。
  type DndPayload = { pid: number; fromGid: number };
  const [dropIndicator, setDropIndicator] = useState<{ gid: number; idx: number } | null>(null);
  // 拖拽悬停的分组（折叠态整体高亮，展开态配合 dropIndicator 精细指示）
  const [dragOverGroup, setDragOverGroup] = useState<number | null>(null);

  // ── 分组一键测试本组全部平台：有界并发跑 model_test，结果面板逐行实时刷新 ──
  const [groupTest, setGroupTest] = useState<{
    groupId: number; groupName: string; rows: GroupTestRow[]; running: boolean;
  } | null>(null);

  // 并发测试：与单平台快速测试同参（默认模型，max_tokens 后端默认 16），worker-pool（共享 index 游标 + N 个 worker）。
  // 不复用 usePlatformCards 的 testingId/testResults —— 那是单卡态，组级面板独立维护行集合。
  const handleTestGroup = useCallback(async (group: GroupDetail["group"], gps: GroupPlatformDetail[]) => {
    // 只测启用平台，跳过 disabled / auto_disabled（不出现在 rows、不发测试请求）
    const enabledGps = gps.filter(gp => gp.platform.status === "enabled");
    if (enabledGps.length === 0) return;
    const rows: GroupTestRow[] = enabledGps.map(gp => ({
      platformId: gp.platform.id, name: gp.platform.name, status: "pending",
    }));
    setGroupTest({ groupId: group.id, groupName: group.name, rows, running: true });
    // groupId guard：面板被中途关闭（groupTest=null）或被新一轮测试取代时，在途写回 no-op，不复活面板。
    const patchRow = (idx: number, patch: Partial<GroupTestRow>) =>
      setGroupTest(prev =>
        prev && prev.groupId === group.id
          ? { ...prev, rows: prev.rows.map((r, i) => i === idx ? { ...r, ...patch } : r) }
          : prev);
    const testOne = async (idx: number) => {
      const gp = enabledGps[idx];
      patchRow(idx, { status: "testing" });
      const defaultModel = gp.platform.models.default || gp.platform.available_models[0] || "";
      const start = Date.now();
      let success = false;
      try {
        const r = await modelTestApi.test({ platform_id: gp.platform.id, model: defaultModel });
        const durationMs = Date.now() - start;
        success = r.success;
        patchRow(idx, r.success
          ? { status: "ok", durationMs }
          : { status: "fail", durationMs, error: r.error || t("platform.testFail", "测试失败") });
      } catch (err: any) {
        patchRow(idx, { status: "fail", durationMs: Date.now() - start, error: err?.message || t("platform.testFail", "测试失败") });
      }
      // 携带 success：监听方（usePlatformCards/Platforms）据此写 testResults → 单卡 health 走 manual 分支即时变绿/红；
      // 同时驱动「最近测试」徽章刷新（不在 Groups 维护 lastTestMap）
      window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: gp.platform.id, success } }));
    };
    // 有界并发：共享游标 next，启 N 个 worker，各自循环领取下一个 idx 直到耗尽。
    let next = 0;
    const worker = async () => {
      while (next < enabledGps.length) {
        const idx = next++;
        await testOne(idx);
      }
    };
    const pool = Array.from(
      { length: Math.min(BATCH_TEST_CONCURRENCY, enabledGps.length) },
      () => worker(),
    );
    await Promise.all(pool);
    setGroupTest(prev => prev && prev.groupId === group.id ? { ...prev, running: false } : prev);
  }, [t]);

  // 基于 clientY 计算 drop 到容器内第 idx 张卡片前（末尾 = 卡片数）
  const computeDropIdx = (zoneEl: HTMLElement, clientY: number): number => {
    const cards = zoneEl.querySelectorAll<HTMLElement>("[data-gp-id]");
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (clientY < r.top + r.height / 2) return i;
    }
    return cards.length;
  };

  // ── pointer 拖拽：用 ref 记录当前在拖项 + 拖拽超阈标志（threshold 防误触把手当点击） ──
  // 不用 state 存「拖拽中」避免每次 pointermove rerender；只在跨过目标格变化时 setDropIndicator/setDragOverGroup。
  const platDragRef = useState<{
    payload: DndPayload | null;
    active: boolean;
    startX: number;
    startY: number;
  }>(() => ({ payload: null, active: false, startX: 0, startY: 0 }))[0];

  // 从 elementFromPoint 反查目标分组 + 插入位（命中分组 wrapper 的 data-group-id，
  // 容器内卡片 data-gp-id 算 idx）。命中分组外（其它区域）返回 null。
  const hitTestZone = (clientX: number, clientY: number): { gid: number; idx: number; zoneEl: HTMLElement } | null => {
    const el = document.elementFromPoint(clientX, clientY) as HTMLElement | null;
    if (!el) return null;
    const zoneEl = el.closest<HTMLElement>("[data-group-id]");
    if (!zoneEl) return null;
    const gid = Number(zoneEl.dataset.groupId);
    if (!Number.isFinite(gid)) return null;
    return { gid, idx: computeDropIdx(zoneEl, clientY), zoneEl };
  };

  // pointerup / 拖拽落定：按 payload + 目标 (gid, idx) 执行组内重排 / 跨组移动 / 未分组拖入。
  const commitPlatDrop = (gid: number, idx: number, payload: DndPayload) => {
    // 从 details 推导目标分组当前平台顺序
    const fullPlats = (details.find(d => d.group.id === gid)?.platforms ?? [])
      .map(gp => platforms.find(pp => pp.id === gp.platform.id))
      .filter((pp): pp is Platform => !!pp);

    if (payload.fromGid === gid) {
      // 组内重排
      const ids = fullPlats.map(p => p.id);
      const fromIdx = ids.indexOf(payload.pid);
      if (fromIdx < 0) return;
      let target = idx;
      if (fromIdx < idx) target = idx - 1; // 移除拖动项后位置左移
      if (target === fromIdx) return;
      const reordered = ids.filter(id => id !== payload.pid);
      reordered.splice(target, 0, payload.pid);
      setDetails(prev => prev.map(d => d.group.id !== gid ? d : {
        ...d,
        platforms: reordered.map((id, i) => {
          const gp = d.platforms.find(g => g.platform.id === id)!;
          return { ...gp, priority: i + 1 };
        }),
      }));
      groupDetailApi.reorderPlatforms(gid, reordered).catch(console.error);
    } else {
      if (payload.fromGid === 0) {
        // 从未分组列表拖入（fromGid=0，无源组）: 构造新明细乐观插入目标组
        const plat = platforms.find(pp => pp.id === payload.pid);
        if (plat) {
          setDetails(prev => prev.map(d => {
            if (d.group.id !== gid) return d;
            const newGp: GroupPlatformDetail = { platform: plat, priority: d.platforms.length + 1, weight: 1 };
            const gps = [...d.platforms];
            gps.splice(Math.min(idx, gps.length), 0, newGp);
            return { ...d, platforms: gps };
          }));
        }
        const gname = details.find(d => d.group.id === gid)?.group.name ?? `#${gid}`;
        groupDetailApi.movePlatform(payload.pid, 0, gid)
          .then(() => {
            onToast?.({ text: `已加入分组 ${gname}`, ok: true });
            load(); onGroupsChanged?.();
          })
          .catch((err) => {
            console.error("[aidog-dnd] movePlatform failed", err);
            onToast?.({ text: `加入分组失败: ${err}`, ok: false });
            load(); // 回滚乐观插入
          });
      } else {
        // 跨组移动
        let movedGp: GroupPlatformDetail | undefined;
        setDetails(prev => {
          const next = prev.map(d => {
            if (d.group.id === payload.fromGid) {
              const gps = d.platforms.filter(g => {
                if (g.platform.id === payload.pid) { movedGp = g; return false; }
                return true;
              });
              return { ...d, platforms: gps };
            }
            return d;
          });
          if (!movedGp) return next;
          return next.map(d => {
            if (d.group.id !== gid) return d;
            const newGp = { ...movedGp!, priority: d.platforms.length + 1 };
            const gps = [...d.platforms];
            const insertAt = Math.min(idx, gps.length);
            gps.splice(insertAt, 0, newGp);
            return { ...d, platforms: gps };
          });
        });
        groupDetailApi.movePlatform(payload.pid, payload.fromGid, gid)
          .then(() => load()).catch(console.error);
      }
    }
  };

  // 拖拽阈值（px）：pointermove 累计位移超过才视为拖拽，避免误触把手当点击。
  const PLAT_DRAG_THRESHOLD = 4;

  // pointermove：超阈后置 active，每帧 hit-test 更新 dropIndicator/dragOverGroup。
  const onPlatPointerMove = (ev: PointerEvent) => {
    const st = platDragRef;
    if (!st.payload) return;
    if (!st.active) {
      if (Math.abs(ev.clientX - st.startX) + Math.abs(ev.clientY - st.startY) < PLAT_DRAG_THRESHOLD) return;
      st.active = true;
    }
    ev.preventDefault();
    const hit = hitTestZone(ev.clientX, ev.clientY);
    if (!hit) {
      setDragOverGroup(prev => (prev === null ? prev : null));
      setDropIndicator(prev => (prev === null ? prev : null));
      return;
    }
    setDragOverGroup(prev => (prev === hit.gid ? prev : hit.gid));
    setDropIndicator(prev => (prev?.gid === hit.gid && prev?.idx === hit.idx) ? prev : { gid: hit.gid, idx: hit.idx });
  };

  // pointerup：落定（仅当超阈成拖拽且命中目标组）后清理监听与状态。
  const onPlatPointerUp = (ev: PointerEvent) => {
    const st = platDragRef;
    const payload = st.payload;
    document.removeEventListener("pointermove", onPlatPointerMove);
    document.removeEventListener("pointerup", onPlatPointerUp);
    document.removeEventListener("pointercancel", onPlatPointerUp);
    st.payload = null;
    const wasActive = st.active;
    st.active = false;
    setDropIndicator(null);
    setDragOverGroup(null);
    if (!payload || !wasActive) return;
    const hit = hitTestZone(ev.clientX, ev.clientY);
    if (!hit) return;
    commitPlatDrop(hit.gid, hit.idx, payload);
  };

  // pointerdown 起拖：记录 payload + 起点，挂 document 级 move/up 监听（elementFromPoint 跨组生效）。
  // useCallback([], ...) — 仅引用 ref（platDragRef）和不变函数（onPlatPointerMove/Up），无 state 依赖。
  const onPlatPointerDown = useCallback((ev: React.PointerEvent, pid: number, gid: number) => {
    ev.preventDefault();
    ev.stopPropagation(); // 不冒泡到 dnd-kit 分组排序把手
    const st = platDragRef;
    st.payload = { pid, fromGid: gid };
    st.active = false;
    st.startX = ev.clientX;
    st.startY = ev.clientY;
    document.addEventListener("pointermove", onPlatPointerMove);
    document.addEventListener("pointerup", onPlatPointerUp);
    document.addEventListener("pointercancel", onPlatPointerUp);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 取代理端口构造 base_url；失败保持兜底 7890。
  useEffect(() => {
    proxyApi.getSettings()
      .then(s => { if (s?.port) setProxyPort(s.port); })
      .catch(() => { /* 兜底 7890 */ });
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  // 监听跨组件分组变更（Platforms pointer 拖入分组后通知刷新；HTML5 DnD 跨区域在 WKWebView 失效，改 pointer + window 事件）
  useEffect(() => {
    const h = () => { load(); refreshStats(); };
    window.addEventListener("aidog-groups-changed", h);
    return () => window.removeEventListener("aidog-groups-changed", h);
  }, []);

  // ── Edit handlers ──

  const openEdit = useCallback((detail: GroupDetail) => {
    dispatchEdit({ type: "open", detail });
  }, []);

  const cancelEdit = useCallback(() => {
    dispatchEdit({ type: "reset" });
  }, []);

  const saveEdit = async () => {
    if (!editTarget) return;
    try {
      // Update group basic info + inline model mappings
      await groupApi.update({
        id: editTarget.group.id,
        name: editName,
        routing_mode: editMode,
        request_timeout_secs: editReqTimeout,
        connect_timeout_secs: editConnTimeout,
        max_retries: editMaxRetries,
        model_mappings: editMappings,
      });

      // Update platforms
      await groupApi.setPlatforms(
        editTarget.group.id,
        editPlatformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
      );

      const savedGid = editTarget.group.id;
      cancelEdit();
      // 编辑保存只动单组（基本信息/映射/平台关联）→ 单组就地刷新，不整列表重载（消除保存闪烁/卡顿）。
      refreshSingleGroup(savedGid);
      onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      alert(String(e) || "Failed to save group");
    }
  };

  // ── Create handler ──
  const handleCreateGroup = async () => {
    try {
      // 创建分组拿回新 group（含 id），再用现有 group_set_platforms 命令关联所选平台（无需改后端）。
      const group = await groupApi.create({ name: cName, group_key: cGroupKey.trim() || undefined, routing_mode: cMode });
      if (cPlatformIds.length > 0) {
        await groupApi.setPlatforms(
          group.id,
          cPlatformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
        );
      }
      setCName(""); setCGroupKey(""); setCMode("failover"); setCPlatformIds([]); setShowCreate(false);
      load();
      onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      onToast?.({ text: `${t("group.createFailed", "创建分组失败")}: ${e}`, ok: false });
    }
  };

  const handleDeleteGroup = useCallback(async (id: number) => {
    try {
      await groupApi.delete(id);
      load();
      onGroupsChanged?.();
    } catch (e) {
      alert(String(e) || "Failed to delete group");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [load, onGroupsChanged]);

  // 切换默认分组：单选。已是默认 → 取消默认；否则设为新默认。
  const handleToggleDefault = useCallback(async (group: GroupDetail["group"]) => {
    try {
      const nextId = group.is_default ? null : group.id;
      await groupApi.setDefault(nextId);
      load();
      onGroupsChanged?.();
    } catch (e) {
      alert(String(e) || "Failed to set default group");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [load, onGroupsChanged]);

  // ── Quick mapping (list view) — persists inline via group.update ──
  const handleAddMapping = useCallback(async () => {
    if (!mappingGroupId || !mSource || mTargetPlatform === "" || !mTargetModel) return;
    const detail = details.find(d => d.group.id === mappingGroupId);
    if (!detail) return;
    try {
      const next: ModelMapping[] = [
        ...detail.model_mappings,
        {
          source_model: mSource,
          target_platform_id: mTargetPlatform,
          target_model: mTargetModel,
          request_timeout_secs: 0,
          connect_timeout_secs: 0,
        },
      ];
      const gid = mappingGroupId;
      await groupApi.update({ id: gid, model_mappings: next });
      setMSource(""); setMTargetPlatform(""); setMTargetModel("");
      setMappingGroupId(null);
      refreshSingleGroup(gid); // 单组映射变更 → 就地刷新
      onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      onToast?.({ text: `${t("group.addMappingFailed", "添加映射失败")}: ${e}`, ok: false });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mappingGroupId, mSource, mTargetPlatform, mTargetModel, details, onGroupsChanged, onToast, t]);

  const handlePurgeDisabled = useCallback(async (gid: number) => {
    try {
      const r = await platformApi.purgeDisabled(gid);
      if (r.deletedIds.length === 0 && r.unassignedIds.length === 0) {
        onToast?.({ text: t("platform.purgeDisabledNone", "暂无失效平台"), ok: true });
      } else {
        onToast?.({ text: t("group.purgeDisabledDone", "已清理：删除 {{deleted}}，移除 {{unassigned}}", { deleted: r.deletedIds.length, unassigned: r.unassignedIds.length }), ok: true });
      }
      load();
      onGroupsChanged?.();
    } catch (err) {
      onToast?.({ text: `${t("group.purgeDisabled", "清理失效")}: ${err}`, ok: false });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t, onToast, onGroupsChanged, load]);

  const handleDeleteMapping = useCallback(async (groupId: number, index: number) => {
    const detail = details.find(d => d.group.id === groupId);
    if (!detail) return;
    try {
      const next = detail.model_mappings.filter((_, i) => i !== index);
      await groupApi.update({ id: groupId, model_mappings: next });
      refreshSingleGroup(groupId); // 单组映射删除 → 就地刷新
      onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      onToast?.({ text: `${t("group.deleteMappingFailed", "删除映射失败")}: ${e}`, ok: false });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [details, onGroupsChanged, onToast, t]);

  const selectedPlatform = platforms.find(p => p.id === mTargetPlatform);
  const availableModels = selectedPlatform ? allModelValues(selectedPlatform.models) : [];

  // SortableList items + group→index 映射：按 details 缓存，避免每渲染重建数组
  // 及列表项内 O(n) findIndex（原 details.findIndex 每项跑致 O(n²)）。
  const groupRows = useMemo<GroupRow[]>(
    () => details.map(d => ({ id: String(d.group.id), detail: d })),
    [details],
  );
  const groupIndexById = useMemo(() => {
    const m = new Map<number, number>();
    details.forEach((d, i) => m.set(d.group.id, i));
    return m;
  }, [details]);

  // ── Edit page ──
  if (editTarget) {
    const editPlatformOptions = platforms.filter(p => p.enabled);
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon" onClick={cancelEdit} title={t("action.cancel")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: F.title, fontWeight: 700 }}>{editName || t("group.edit")}</div>
            <div className="text-secondary" style={{ fontSize: F.hint, marginTop: 2 }}>#{editTarget.group.id}</div>
          </div>
          <CopyButton text={editTarget.group.group_key} label={t("group.apiKey", "API Key")} title={t("group.copyApiKeyTitle", "复制 API Key")} />
          <CopyButton text={buildClaudeCommand(editTarget.group.group_key)} icon={<img src={claudeIcon} width={14} height={14} alt="Claude" />} title={t("group.copyCommand", "复制 Claude Code 启动命令")} />
          <CopyButton text={buildCodexCommand(editTarget.group.group_key)} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} />
          <button className="btn" onClick={cancelEdit}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={saveEdit}
            disabled={!editName}>{t("action.save")}</button>
        </div>

        {/* Basic info */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

          {/* Name */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={editName} onChange={e => dispatchEdit({ type: "patch", patch: { name: e.target.value } })} />
          </div>

          {/* Group key（锁定，创建后不可改） */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.groupKey", "密钥")}</span>
            <div style={{ display: "flex", gap: 6, alignItems: "center", minWidth: 0 }}>
              <input className="input" style={{ fontSize: F.body, padding: S.inputPad, opacity: 0.7 }}
                value={editTarget.group.group_key} disabled
                title={t("group.groupKeyLocked", "分组密钥创建后锁定，不可修改")} />
              <CopyButton text={editTarget.group.group_key} title={t("group.copyApiKeyTitle", "复制 API Key")} size={14} />
            </div>
          </div>

          {/* Routing mode */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "start", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)", paddingTop: 6 }}>{t("group.routingMode", "路由模式")}</span>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 0 }}>
              <select className="input" style={{ fontSize: F.body, padding: S.inputPad }}
                value={editMode} onChange={e => dispatchEdit({ type: "patch", patch: { mode: e.target.value as RoutingMode } })}>
                {ROUTING_MODES.map(m => (
                  <option key={m} value={m}>{routingModeLabel(t, m)}</option>
                ))}
              </select>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{routingModeDesc(t, editMode)}</span>
            </div>
          </div>

          {/* Timeout */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.timeout", "超时")}</span>
            <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
              <input className="input" type="number" min={0} placeholder={t("group.reqTimeout", "请求(s)")}
                value={editReqTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { reqTimeout: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <input className="input" type="number" min={0} placeholder={t("group.connTimeout", "连接(s)")}
                value={editConnTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { connTimeout: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.timeoutDefault", "0 = 系统默认（秒）")}</span>
            </div>
          </div>

          {/* Max retries（多平台失败逐个重试上限） */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.maxRetries", "最大重试")}</span>
            <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
              <input className="input" type="number" min={0} max={10}
                value={editMaxRetries}
                onChange={e => dispatchEdit({ type: "patch", patch: { maxRetries: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.maxRetriesHint", "0 = 不重试，只试 1 个平台")}</span>
            </div>
          </div>

          {/* Auto badge */}
          {editTarget.group.auto_from_platform && (
            <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)" }}>
              <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 5px" }}>auto</span>
              {t("group.autoFromPlatform", "自动创建，部分字段不可编辑")}
            </div>
          )}
        </div>

        {/* Platforms */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.platforms", "关联平台")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("group.platformsHint", "选择并排序此分组使用的平台，顺序决定优先级")}
          </div>
          <PlatformPicker
            platformIds={editPlatformIds}
            options={editPlatformOptions}
            onChange={ids => dispatchEdit({ type: "patch", patch: { platformIds: ids } })}
            t={t}
          />
        </div>

        {/* Model Mappings */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.modelMappings", "模型映射")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("group.mappingsHint", "将源模型名映射到目标平台的具体模型")}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {editMappings.map((m, i) => {
              const targetPlat = platforms.find(p => p.id === m.target_platform_id);
              const models = targetPlat ? allModelValues(targetPlat.models) : [];
              return (
                <div key={i} style={{
                  display: "flex", gap: 8, alignItems: "center",
                  padding: "8px 12px", borderRadius: "var(--radius-sm)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                }}>
                  <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                    placeholder={t("mapping.source", "源模型")}
                    value={m.source_model}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], source_model: e.target.value };
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
                    }} />
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M2 6h8M8 4l2 2-2 2" />
                  </svg>
                  <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                    value={m.target_platform_id || ""}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_platform_id: e.target.value === "" ? 0 : Number(e.target.value), target_model: "" };
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
                    }}>
                    <option value="">{t("mapping.targetPlatform", "目标平台")}</option>
                    {platforms.filter(p => p.enabled).map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  {models.length > 0 ? (
                    <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                      value={m.target_model}
                      onChange={e => {
                        const ms = [...editMappings];
                        ms[i] = { ...ms[i], target_model: e.target.value };
                        dispatchEdit({ type: "patch", patch: { mappings: ms } });
                      }}>
                      <option value="">{t("mapping.target", "目标模型")}</option>
                      {models.map(m2 => <option key={m2} value={m2}>{m2}</option>)}
                    </select>
                  ) : (
                    <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                      placeholder={t("mapping.target", "目标模型")}
                      value={m.target_model}
                      onChange={e => {
                        const ms = [...editMappings];
                        ms[i] = { ...ms[i], target_model: e.target.value };
                        dispatchEdit({ type: "patch", patch: { mappings: ms } });
                      }} />
                  )}
                  <button type="button" onClick={() => dispatchEdit({ type: "patch", patch: { mappings: editMappings.filter((_, j) => j !== i) } })} style={{
                    background: "none", border: "none", cursor: "pointer",
                    color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1, flexShrink: 0,
                  }}><IconClose size={12} /></button>
                </div>
              );
            })}

            <button type="button" className="btn btn-ghost" style={{ fontSize: F.hint, padding: "6px 12px", alignSelf: "flex-start" }}
              onClick={() => dispatchEdit({ type: "patch", patch: { mappings: [...editMappings, { source_model: "", target_platform_id: 0, target_model: "", request_timeout_secs: 0, connect_timeout_secs: 0 }] } })}>
              + {t("mapping.add", "添加映射")}
            </button>
          </div>
        </div>

        {/* Middleware rules (group scope) */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("middleware.groupRules", "分组中间件规则")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("middleware.groupRulesHint", "仅本分组生效，就近覆盖全局同类型规则")}
          </div>
          <MiddlewareRulesPanel scope="group" scopeRef={editTarget.group.group_key} embedded />
        </div>
      </div>
    );
  }

  // ── Create page（独立视图态，复用编辑视图的全屏 + 返回箭头 Header 模式）──
  if (showCreate) {
    const closeCreate = () => { setCName(""); setCGroupKey(""); setCMode("failover"); setCPlatformIds([]); setShowCreate(false); };
    const createPlatformOptions = platforms.filter(p => p.enabled);
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon" onClick={closeCreate} title={t("action.cancel")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: F.title, fontWeight: 700 }}>{t("group.add")}</div>
          </div>
          <button className="btn" onClick={closeCreate}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={handleCreateGroup} disabled={!cName}>{t("action.create")}</button>
        </div>

        {/* Basic info */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

          {/* Name */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              placeholder={t("group.name", "分组名称")} value={cName}
              onChange={(e) => setCName(e.target.value)} />
            <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>
              {t("group.nameHint", "分组显示名（中文可读），用于界面展示。")}
            </div>
          </div>

          {/* Group key */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.groupKey", "密钥")}</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              placeholder={t("group.groupKey", "分组密钥（留空自动生成）")} value={cGroupKey}
              onChange={(e) => setCGroupKey(e.target.value.replace(/[^\w-]/g, ""))} />
            <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>
              {t("group.groupKeyHint", "分组密钥（= API Key / 路由识别键）。留空自动生成；创建后锁定不可修改。")}
            </div>
          </div>

          {/* Routing mode */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.routingMode", "路由模式")}</span>
            <select className="input" style={{ fontSize: F.body, padding: S.inputPad }} value={cMode} onChange={(e) => setCMode(e.target.value as RoutingMode)}>
              {ROUTING_MODES.map(m => (
                <option key={m} value={m}>{routingModeLabel(t, m)}</option>
              ))}
            </select>
            <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{routingModeDesc(t, cMode)}</div>
          </div>
        </div>

        {/* Platforms（与编辑视图共用 PlatformPicker；创建时选定，保存后一并关联） */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.platforms", "关联平台")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("group.platformsHint", "选择并排序此分组使用的平台，顺序决定优先级")}
          </div>
          <PlatformPicker
            platformIds={cPlatformIds}
            options={createPlatformOptions}
            onChange={setCPlatformIds}
            t={t}
          />
        </div>
      </div>
    );
  }

  // ── List view ──
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 子区块标题 + 操作栏 */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          {details.length > 0 && (
            <span style={{ fontSize: 13, color: "var(--text-tertiary)" }}>
              {details.length} {t("nav.groups").toLowerCase()}
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          {/* 代理 base_url：只读小字 + 复制按钮 */}
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <code style={{
              fontSize: 12, color: "var(--text-secondary)", background: "var(--bg-glass)",
              padding: "4px 8px", borderRadius: "var(--radius-sm)", whiteSpace: "nowrap",
            }}>{proxyBaseUrl}</code>
            <CopyButton text={proxyBaseUrl} label={t("group.copyBaseUrl", "复制代理地址")}
              title={t("group.copyBaseUrlTitle", "复制代理 base_url")} />
          </div>
        </div>
      </div>

      {/* 分组一键测试结果面板（有界并发执行，实时刷新行状态；running 态可中途关闭） */}
      {groupTest && (
        <GroupTestPanel
          groupName={groupTest.groupName}
          rows={groupTest.rows}
          running={groupTest.running}
          onClose={() => setGroupTest(null)}
          t={t}
        />
      )}

      {/* Group List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {details.length === 0 && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("group.empty")}</div>
            </div>
          )}
          <SortableList<GroupRow>
            items={groupRows}
            onReorder={handleReorderGroups}
            renderItem={(row, handle) => {
            const { group } = row.detail;
            const i = groupIndexById.get(group.id) ?? 0;
            const u = groupStats[group.group_key];
            const balance = groupBalance[group.id];
            const cardsSnap: CardsSnapshot = {
              quotaMap: cards.quotaMap,
              quotaRealIds: cards.quotaRealIds,
              quotaRefreshing: cards.quotaRefreshing,
              usageMap: cards.usageMap,
              expandedIds: cards.expandedIds,
              testResults: cards.testResults,
              testingId: cards.testingId,
              faviconFailed: cards.faviconFailed,
              lastTestMap: cards.lastTestMap,
            };
            const di = dropIndicator?.gid === group.id ? dropIndicator.idx : null;
            const fullPlatsLen = row.detail.platforms
              .map(gp => platforms.find(pp => pp.id === gp.platform.id))
              .filter(Boolean).length;
            return (
              <GroupListItem
                key={group.id}
                detail={row.detail}
                index={i}
                usageStat={u}
                balance={balance}
                platforms={platforms}
                isExpanded={!collapsedGroups.has(group.id)}
                isDragOver={dragOverGroup === group.id}
                dropIndicatorIdx={di}
                dropIndicatorTotal={fullPlatsLen}
                showMappingForm={mappingGroupId === group.id}
                mSource={mSource}
                mTargetPlatform={mTargetPlatform}
                mTargetModel={mTargetModel}
                availableModels={availableModels}
                groupTestRunning={groupTest?.running === true}
                cards={cardsSnap}
                actions={makeGroupCardActions(group.id)}
                t={t}
                onToggleExpanded={toggleGroupExpanded}
                onSetCollapsed={setCollapsedGroups}
                onEdit={openEdit}
                onDelete={handleDeleteGroup}
                onToggleDefault={handleToggleDefault}
                onTestGroup={handleTestGroup}
                onCreatePlatform={onCreatePlatform}
                onNavigate={onNavigate}
                onPlatPointerDown={onPlatPointerDown}
                onDeleteMapping={handleDeleteMapping}
                onSetMappingGroupId={setMappingGroupId}
                onSetMSource={setMSource}
                onSetMTargetPlatform={setMTargetPlatform}
                onSetMTargetModel={setMTargetModel}
                onAddMapping={handleAddMapping}
                onSetLevelPriority={handleSetLevelPriority}
                onPurgeDisabled={handlePurgeDisabled}
                handle={handle}
              />
            );
            }}
          />
          {/* 触底加载哨兵：进入视口触发 loadMore 拉下一页（hasMore 时常驻）。 */}
          {hasMore && details.length > 0 && (
            <div ref={sentinelRef} style={{ height: 1 }} aria-hidden="true" />
          )}
          {loadingMore && (
            <div className="text-tertiary" style={{ padding: 12, textAlign: "center", fontSize: 12 }}>
              {t("status.loading")}
            </div>
          )}
        </div>
      )}

      {/* 自定义测试弹窗（与 Platforms 主列表同款；handleCustomTest → testingPlatform）
          ModelTestPanel 自带 overlay 且经 createPortal 挂 body, 此处不再包外层遮罩。 */}
      {cards.testingPlatform !== null && (
        <ModelTestPanel
          platform={cards.testingPlatform}
          onClose={() => cards.setTestingPlatform(null)}
          onResult={(success) => {
            const tp = cards.testingPlatform;
            if (tp) cards.setTestResults(prev => ({ ...prev, [tp.id]: success ? "ok" : "fail" }));
          }}
        />
      )}

      {/* 分享弹窗（导出可分享配置 → 含明文 api_key 警示 + 多格式复制） */}
      {cards.shareData !== null && (
        <ShareModal
          share={cards.shareData.share}
          platformName={cards.shareData.name}
          onToast={(text, ok) => {
            onToast?.({ text, ok });
            setTimeout(() => onToast?.(null), 3000);
          }}
          onClose={() => cards.setShareData(null)}
        />
      )}

      {/* 删平台确认弹窗：仅当平台只属本组（删除即销毁平台，破坏性）时出现。
          属多组的平台走「仅移出本组」无需确认，不会进此分支。
          createPortal 挂 body 脱离 transform 祖先，参考 GroupTestPanel。 */}
      {removeTarget !== null && createPortal(
        <div onClick={() => setRemoveTarget(null)} style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.45)", zIndex: 1001,
          display: "flex", alignItems: "center", justifyContent: "center", padding: 20,
        }}>
          <div className="glass-surface" onClick={e => e.stopPropagation()} style={{
            width: "min(420px, 92vw)", display: "flex", flexDirection: "column", gap: 14, padding: 20,
            background: "var(--bg-floating)",
          }}>
            <div style={{ fontSize: 15, fontWeight: 700 }}>
              {t("group.deletePlatformTitle", "删除平台")}
            </div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {t("group.deletePlatformConfirm", "「{{name}}」仅属此分组，移除将彻底删除该平台及其所有关联，且无法撤销。确认删除？", { name: removeTarget.platform.name })}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button className="btn btn-ghost" onClick={() => setRemoveTarget(null)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" onClick={confirmDeletePlatform}>
                {t("group.deletePlatformAction", "删除平台")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
