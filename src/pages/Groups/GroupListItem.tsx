import { Fragment, memo, useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import type { GroupDetail, GroupPlatformDetail, Platform, PlatformUsageStats, PurgeCandidate, PlatformErrorStatus } from "../../services/api";
import { buildStatusOptions, idsForCode } from "./statusSelect";
import { platformApi } from "../../services/api";
import { formatCost, formatPercent, formatNumber, successRate as calcSuccessRate } from "../../utils/formatters";
import { makeRipple } from "../../components/shared";
import { PlatformCardActions, PlatformCardService } from "../../components/platforms/PlatformCard";
import { PlatformRow } from "../platforms/PlatformRow";
import type { DragHandleProps } from "../../components/SortableList";
import { buildClaudeCommand, buildCodexCommand, buildPiCommand, routingModeLabel, useProxyEnvVars } from "../../domains/groups";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem,
} from "@/components/ui/dropdown-menu";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/** usePlatformCards 的卡片展示状态快照（memo 化子组件按需接收） */
export interface CardsSnapshot {
  quotaMap: Record<number, import("../../services/api").PlatformQuota>;
  quotaRealIds: Record<number, boolean>;
  quotaRefreshing: Record<number, boolean>;
  usageMap: Record<number, PlatformUsageStats>;
  expandedIds: Set<number>;
  testResults: Record<number, "ok" | "fail">;
  testingId: number | null;
  faviconFailed: Set<number>;
  lastTestMap: Record<number, import("../../services/api").LastTestResult>;
}

/** GroupListItem props：每个分组行的全部渲染依赖，显式化为 props 以支持 React.memo 细粒度更新。 */
export interface GroupListItemProps {
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
  /** 搜索命中过滤：null/undefined = 不过滤（原行为），Set = 仅渲染 id 命中的平台卡 */
  visiblePlatformIds: Set<number> | null;
  /** 搜索态下强制展开（无视 collapsedGroups），让用户直接看到命中平台 */
  forceExpanded: boolean;
  // 测试状态（一键测试按钮 disabled）
  groupTestRunning: boolean;
  // cards 快照
  cards: CardsSnapshot;
  actions: PlatformCardActions;
  /** 卡片行为服务（票 12）：只吃 platform 的 7 个回调，页级一份 */
  cardService: PlatformCardService;
  // 稳定回调（父级 useCallback）
  t: TFunction;
  onToggleExpanded: (id: number) => void;
  onSetCollapsed: (updater: (prev: Set<number>) => Set<number>) => void;
  /** 打开独立详情页（观测台三栏） */
  onOpenDetail?: (id: number) => void;
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
  onPurgeDisabled: (gid: number) => void | Promise<void>;
  /** 批量删除：收 selectedIds + gid → 开 BatchDeleteModal（父级渲染 modal）。 */
  onBatchDelete: (ids: number[], gid: number) => void;
  /** 批量覆盖模型：收 selectedIds + gid → 开 BatchOverrideModelsModal（父级渲染 modal）。 */
  onBatchOverrideModels: (ids: number[], gid: number) => void;
  /** 批量改状态：收 selectedIds + gid → 开 BatchSetStatusModal（父级渲染 modal）。 */
  onBatchSetStatus: (ids: number[], gid: number) => void;
  /** 批量移组：收 selectedIds + gid → 开 BatchMoveGroupModal（父级渲染 modal）。 */
  onBatchMoveGroup: (ids: number[], gid: number) => void;
  /** 批量操作完成信号（每次成功 +1）；非删除类批量（平台仍存活，auto-exist effect 不触发）
   *  靠本信号强制退出多选模式。删除类靠平台从 gps 消失自动退出，不依赖本信号。 */
  batchDoneSignal?: number;
  // drag handle（来自 SortableList，每次由父 renderItem 传入，非稳定）
  handle: DragHandleProps;
}


/**
 * 单个分组段组件（G3 设计稿「平台管理页」.group 段式布局），React.memo 包裹。
 * 版式：group-head（折叠箭头 + 组名 + N 平台 + 今日/请求/成功率 + 多选/设置/⋯）
 * + 批量条 + 成员 PlatformRow 行 + 模型映射 chips。原卡片式渲染已按稿重写；
 * props 接口与全部 handler 保持不变（GroupListView 管线零改动）。
 */
export const GroupListItem = memo(function GroupListItem({
  detail, index, usageStat: u, balance, platforms,
  isExpanded, isDragOver, dropIndicatorIdx, dropIndicatorTotal,
  showMappingForm, mSource, mTargetPlatform, mTargetModel, availableModels,
  visiblePlatformIds, forceExpanded,
  groupTestRunning,
  cards, actions, cardService, t,
  onToggleExpanded: _onToggleExpanded, onSetCollapsed, onOpenDetail, onEdit, onDelete, onToggleDefault,
  onTestGroup, onCreatePlatform, onNavigate,
  onPlatPointerDown, onDeleteMapping, onSetMappingGroupId,
  onSetMSource, onSetMTargetPlatform, onSetMTargetModel, onAddMapping,
  onSetLevelPriority, onPurgeDisabled,
  onBatchDelete,
  onBatchOverrideModels,
  onBatchSetStatus,
  onBatchMoveGroup,
  batchDoneSignal,
  handle,
}: GroupListItemProps) {
  const { group, platforms: gps, model_mappings } = detail;
  // 代理 env（claude settings.json env 段），前置 export 注入 codex 启动命令（codex 无 config proxy）。
  const { i18n } = useTranslation();
  const proxyVars = useProxyEnvVars();

  // ── per-group 多选模式（本地 state, 不持久化）──
  const [mode, setMode] = useState<"view" | "select">("view");
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  // 按状态选数据（plat-select-status）：下拉首次展开拉一次；null = 未拉（含拉失败后重试语义）。
  const [errStatus, setErrStatus] = useState<PlatformErrorStatus[] | null>(null);

  // purge 确认走 AlertDialog（禁原生 confirm，CLAUDE.md 硬规）。
  const [purgeConfirmOpen, setPurgeConfirmOpen] = useState(false);
  const [purging, setPurging] = useState(false);
  // 弹窗打开时拉一次 preview（非常驻轮询），按 action 分「永久删除」「移出本分组」两段。
  const [previewList, setPreviewList] = useState<PurgeCandidate[] | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  useEffect(() => {
    if (!purgeConfirmOpen) { setPreviewList(null); return; }
    let cancelled = false;
    setPreviewLoading(true);
    platformApi.purgeDisabledPreview(group.id).then((r) => { if (!cancelled) setPreviewList(r); })
      .catch(() => { if (!cancelled) setPreviewList([]); })
      .finally(() => { if (!cancelled) setPreviewLoading(false); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [purgeConfirmOpen, group.id]);

  // 批量删除成功后：selectedIds 内平台从 gps 全消失 → 自动退出多选模式。
  useEffect(() => {
    if (mode !== "select" || selectedIds.size === 0) return;
    const currentIds = new Set(gps.map(gp => gp.platform.id));
    const anyAlive = [...selectedIds].some(id => currentIds.has(id));
    if (!anyAlive) {
      setMode("view");
      setSelectedIds(new Set());
    }
  }, [gps, mode, selectedIds]);

  // 非删除类批量成功：靠父级 batchDoneSignal 自增强制退出多选模式（初始 0 跳过）。
  const batchDoneSignalRef = useRef(0);
  useEffect(() => {
    if (batchDoneSignal === undefined) return;
    if (batchDoneSignal === batchDoneSignalRef.current) return;
    batchDoneSignalRef.current = batchDoneSignal;
    if (mode === "select") {
      setMode("view");
      setSelectedIds(new Set());
    }
  }, [batchDoneSignal, mode]);

  const fullPlats = gps
    .map(gp => platforms.find(pp => pp.id === gp.platform.id))
    .filter((pp): pp is Platform => !!pp)
    .filter(pp => !visiblePlatformIds || visiblePlatformIds.has(pp.id));

  // 按状态选选项（plat-select-status）。
  const statusOptions = errStatus !== null
    ? buildStatusOptions(errStatus, fullPlats.map(p => p.id))
    : [];

  const headBtn: React.CSSProperties = { fontSize: 12, height: 26, padding: "0 9px", borderRadius: 6 };
  // 稿 5 .batchbar .bact：批量条按钮基础款
  const bactBtn: React.CSSProperties = { fontSize: 12, padding: "3px 9px", height: "auto", borderRadius: 6, background: "var(--secondary)", border: "1px solid var(--border)" };

  const gstatStyle: React.CSSProperties = { fontSize: 12, color: "var(--text-secondary)", display: "inline-flex", gap: 18, marginLeft: 10 };

  return (
    <div
      data-group-id={group.id}
      data-testid="group-section"
      data-expanded={(forceExpanded || isExpanded || mode === "select") ? "true" : "false"}
      className="aidog-anim-rise"
      style={{
        // 稿 5 .group margin 26：父容器 gap 12 + 此处 14 = 净距 26（首组 0）
        marginTop: index === 0 ? 0 : 14,
        border: "1px solid var(--border)",
        borderRadius: 11,
        background: "var(--card)",
        overflow: "hidden",
        boxShadow: "var(--shadow-sm)",
        // 稿 5 stagger：组卡 60ms / 240ms（60 + index*180），组内行在 PlatformRow riseDelay
        animationDelay: `${60 + index * 180}ms`,
        ...(isDragOver ? { outline: "2px solid var(--accent)", outlineOffset: 2 } : {}),
        ...(handle.isDragging ? { opacity: 0.5 } : {}),
      }}
    >
      {/* ── group-head（稿 .group-head）：折叠 / 组名 / 计数 / 三统计 / 操作 ── */}
      <div
        data-testid="group-section-head"
        onClick={() => { if (mode !== "select") onSetCollapsed(prev => {
          const s = new Set(prev); isExpanded ? s.add(group.id) : s.delete(group.id); return s;
        }); }}
        style={{
          display: "flex", alignItems: "center", gap: 12, padding: "11px 16px",
          background: "var(--bg-chrome, var(--secondary))",
          borderBottom: (forceExpanded || isExpanded || mode === "select") ? "1px solid var(--border)" : undefined,
          cursor: mode === "select" ? "default" : "pointer",
        }}
      >
        {/* 折叠箭头（drag handle 复用此位：把手悬停浮现，点击区域折叠） */}
        <span
          ref={handle.ref}
          {...handle.attributes}
          {...handle.listeners}
          data-testid="group-section-chev"
          title={t("group.dragToReorder", "拖动排序")}
          onClick={(e: React.MouseEvent) => { if (handle.isDragging) e.stopPropagation(); }}
          style={{
            display: "inline-flex", color: "var(--text-tertiary)", cursor: "grab", touchAction: "none",
            transform: (forceExpanded || isExpanded || mode === "select") ? "rotate(90deg)" : undefined,
            transition: "transform .18s", flexShrink: 0,
          }}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M4 2.5L8 6l-4 3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
        </span>
        {/* 组名 + 默认星标 */}
        <span data-testid="group-section-name" style={{ fontSize: 13.5, fontWeight: 620, letterSpacing: "-0.005em", display: "inline-flex", alignItems: "center", gap: 5 }}>
          {group.name}
          {group.is_default && (
            <span title={t("group.isDefaultTitle", "默认分组")} style={{ color: "var(--color-warning)", fontSize: 10 }}>★</span>
          )}
          {group.auto_from_platform && (
            <Badge variant="secondary" style={{ fontSize: 11, padding: "0 5px", fontWeight: 500 }}>auto</Badge>
          )}
        </span>
        {/* 计数（稿 .gcount）+ 路由模式 */}
        <span style={{
          fontSize: 12, color: "var(--text-tertiary)", background: "var(--secondary)",
          border: "1px solid var(--border)", borderRadius: 99, padding: "1px 8px", whiteSpace: "nowrap",
        }}>
          {routingModeLabel(t, group.routing_mode)} · {gps.length} {t("group.platforms", "平台")}
        </span>
        {/* 三统计（稿 .gstats：今日 / 请求 / 成功率） */}
        <span style={gstatStyle}>
          <span><span style={{ color: "var(--text-tertiary)", marginRight: 5 }}>{t("group.detail.todayShort", "今")}</span><b className="num" style={{ color: "var(--foreground)", fontWeight: 560 }}>${u ? formatCost(u.today_cost) : "0"}</b></span>
          <span><span style={{ color: "var(--text-tertiary)", marginRight: 5 }}>{t("group.todayRequests", "今日请求")}</span><b className="num" style={{ color: "var(--foreground)", fontWeight: 560 }}>{u ? formatNumber(u.today_requests) : "0"}</b></span>
          <span><span style={{ color: "var(--text-tertiary)", marginRight: 5 }}>{t("group.todaySuccessRate", "今日成功率")}</span><b className="num" style={{ color: "var(--foreground)", fontWeight: 560 }}>{u && u.today_requests > 0 ? formatPercent(calcSuccessRate(u.today_success, u.today_requests), 1) : "—"}</b></span>
          {balance != null && (
            <span><span style={{ color: "var(--text-tertiary)", marginRight: 5 }}>{t("platform.col.balance", "余额")}</span><b className="num" style={{ color: "var(--foreground)", fontWeight: 560 }}>${formatCost(balance)}</b></span>
          )}
        </span>
        {/* 右侧操作（稿 .gactions：拖入提示 / 多选 / 设置 / ⋯） */}
        <span style={{ marginLeft: "auto", display: "inline-flex", gap: 6, alignItems: "center" }} onClick={e => e.stopPropagation()}>
          <span style={{
            marginLeft: 2, fontSize: 12, color: "var(--text-tertiary)",
            border: "1px dashed var(--input)", borderRadius: 6, padding: "2px 8px",
            display: "inline-flex", alignItems: "center", gap: 5,
          }} title={t("group.dragPlatform", "拖拽排序 / 移动到其他分组")}>⠿ {t("group.dropHint", "拖入平台")}</span>
          {gps.length > 0 && (
            <Button variant="ghost" className="ripple" style={{ ...headBtn, ...(mode === "select" ? { background: "var(--accent-subtle)", color: "var(--primary)", border: "1px solid color-mix(in srgb, var(--primary) 45%, transparent)" } : {}) }}
              onClick={(e) => { makeRipple(e); setMode("select"); setSelectedIds(new Set()); }}>
              {t("group.batchOps", "多选")}
            </Button>
          )}
          {onOpenDetail && (
            <Button variant="ghost" className="ripple" data-testid="group-section-detail" style={headBtn}
              onClick={(e) => { makeRipple(e); onOpenDetail(group.id); }}>
              {t("group.detail.openShort", "详情")}
            </Button>
          )}
          <Button variant="ghost" className="ripple" style={headBtn} onClick={(e) => { makeRipple(e); onEdit({ group, platforms: gps, model_mappings }); }}>
            {t("group.sectionSettings", "设置")}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button type="button" data-testid="group-section-menu" className="ripple" title={t("platform.more", "更多")}
                style={{ ...headBtn, display: "inline-flex", alignItems: "center", background: "transparent", border: "1px solid transparent", color: "var(--text-tertiary)", cursor: "pointer" }}
                onClick={(e) => { makeRipple(e); }}>
                ⋯
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" style={{ minWidth: 170 }}>
              {onTestGroup && (
                <DropdownMenuItem disabled={gps.filter(gp => gp.platform.status === "enabled").length === 0 || groupTestRunning}
                  onSelect={() => onTestGroup(group, gps)}>
                  {t("group.testAll", "一键测试本组全部平台")}
                </DropdownMenuItem>
              )}
              {onCreatePlatform && <DropdownMenuItem onSelect={() => onCreatePlatform([group.id], group.id)}>{t("group.addPlatform", "添加平台")}</DropdownMenuItem>}
              <DropdownMenuItem onSelect={() => onNavigate?.("stats", { groupId: String(group.id), groupKey: group.group_key })}>
                {t("group.viewStats", "查看统计")}
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => setPurgeConfirmOpen(true)}>{t("group.purgeDisabled", "清理失效")}</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => onToggleDefault(group)}>
                {group.is_default ? t("group.unsetDefault", "取消默认分组") : t("group.setAsDefault", "设为默认分组")}
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => { void navigator.clipboard?.writeText(group.group_key); }}>{t("group.menuCopyKey", "API Key")}</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => { void navigator.clipboard?.writeText(buildClaudeCommand(group.group_key)); }}>{t("group.menuCopyClaude", "Claude 启动命令")}</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => { void navigator.clipboard?.writeText(buildCodexCommand(group.group_key, [...(group.env_vars ?? []), ...proxyVars])); }}>{t("group.menuCopyCodex", "Codex 启动命令")}</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => { void navigator.clipboard?.writeText(buildPiCommand(group.group_key, [...(group.env_vars ?? []), ...proxyVars])); }}>{t("group.menuCopyPi", "pi 启动命令")}</DropdownMenuItem>
              {(!group.auto_from_platform || gps.length === 0) && (
                <DropdownMenuItem style={{ color: "var(--color-danger)" }} onSelect={() => onDelete(group.id)}>{t("group.delete", "删除分组")}</DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </span>
      </div>

      {/* ── 批量条（稿 .batchbar）：多选模式下成员行上方常驻 ── */}
      {mode === "select" && (
        <div data-testid="group-batchbar" style={{
          display: "flex", alignItems: "center", gap: 8, padding: "8px 14px", flexWrap: "wrap",
          background: "var(--accent-subtle)", borderBottom: "1px solid color-mix(in srgb, var(--primary) 35%, transparent)",
          fontSize: 12, color: "var(--text-secondary)",
        }} onClick={e => e.stopPropagation()}>
          <span style={{ fontWeight: 620, color: "var(--primary)" }}>{t("group.selectedCount", "已选 {{count}} 个", { count: selectedIds.size })}</span>
          {/* 稿 5 .batchbar .sep：竖分隔线 */}
          <span style={{ width: 1, height: 14, background: "color-mix(in srgb, var(--foreground) 12%, transparent)" }} />
          <Button variant="ghost" disabled={selectedIds.size === 0} style={{ ...bactBtn, color: "var(--color-danger)" }}
            onClick={() => onBatchDelete([...selectedIds], group.id)}>
            {t("group.batchDelete", "批量删除")}
          </Button>
          <Button variant="ghost" disabled={selectedIds.size === 0} style={bactBtn}
            onClick={() => onBatchMoveGroup([...selectedIds], group.id)}>
            {t("group.batchMoveGroup", "移动到分组")}
          </Button>
          <Button variant="ghost" disabled={selectedIds.size === 0} style={bactBtn}
            onClick={() => onBatchSetStatus([...selectedIds], group.id)}>
            {t("group.batchSetStatus", "改状态")}
          </Button>
          <Button variant="ghost" disabled={selectedIds.size === 0} style={bactBtn}
            onClick={() => onBatchOverrideModels([...selectedIds], group.id)}>
            {t("group.batchOverrideModels", "覆盖模型")}
          </Button>
          <Button variant="ghost" style={bactBtn}
            onClick={() => setSelectedIds(new Set(fullPlats.map(p => p.id)))}>
            {t("group.selectAll", "全选")}
          </Button>
          {/* 按状态选（plat-select-status）：动态错误码选项；口径 = 实时调度态优先回落 24h 日志最近失败码。 */}
          <Select
            onOpenChange={(open) => {
              if (open && errStatus === null) {
                platformApi.errorStatus()
                  .then(r => setErrStatus(r.filter(i => !(i.source === "cooldown" && i.until_ms !== null && i.until_ms < Date.now()))))
                  .catch(() => setErrStatus([]));
              }
            }}
            onValueChange={(v) => {
              const ids = idsForCode(errStatus ?? [], fullPlats.map(p => p.id), Number(v));
              if (ids.size > 0) setSelectedIds(ids);
            }}
          >
            <SelectTrigger style={{ fontSize: 12, height: "auto", padding: "3px 9px", minHeight: 26, width: "auto", background: "var(--secondary)" }}>
              <SelectValue placeholder={t("group.selectByStatus", "按状态选")} />
            </SelectTrigger>
            <SelectContent>
              {statusOptions.map(o => (
                <SelectItem key={o.code} value={String(o.code)} style={{ fontSize: 12 }}>
                  {`${o.code} · ${t(`group.err.${o.code}`, `HTTP ${o.code}`)}（${o.count}）`}
                </SelectItem>
              ))}
              {statusOptions.length === 0 && (
                <SelectItem value="__none__" disabled style={{ fontSize: 12 }}>
                  {errStatus === null ? "…" : t("group.selectByStatusNone", "无错误状态")}
                </SelectItem>
              )}
            </SelectContent>
          </Select>
          {/* 稿 5 .batchbar .cancel：取消多选靠右 */}
          <span data-testid="group-batch-cancel" onClick={() => { setMode("view"); setSelectedIds(new Set()); }}
            style={{ marginLeft: "auto", fontSize: 12, color: "var(--text-tertiary)", cursor: "pointer" }}>
            {t("group.cancelMultiSelect", "取消多选")}
          </span>
        </div>
      )}
      {/* ── 成员行（稿 .rows：PlatformRow 行式）+ 落点指示线 ── */}
      {(forceExpanded || isExpanded || mode === "select") && fullPlats.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column" }} onClick={e => e.stopPropagation()}>
          {fullPlats.map((p, idx) => (
            <Fragment key={p.id}>
              {dropIndicatorIdx === idx && (
                <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-1px 0", opacity: 0.7 }} />
              )}
              <PlatformRow
                platform={p}
                riseDelay={120 + index * 180 + idx * 60}
                usage={cards.usageMap[p.id]}
                quotaRaw={cards.quotaMap[p.id]}
                quotaPreferReal={!!cards.quotaRealIds[p.id]}
                lastTest={cards.lastTestMap[p.id]}
                manualResult={cards.testResults[p.id]}
                testing={cards.testingId === p.id}
                expanded={cards.expandedIds.has(p.id)}
                selecting={mode === "select"}
                selected={selectedIds.has(p.id)}
                levelPriority={gps.find(gp => gp.platform.id === p.id)?.level_priority ?? 5}
                t={t}
                lang={i18n.language}
                onSelect={() => setSelectedIds(prev => {
                  const s = new Set(prev);
                  s.has(p.id) ? s.delete(p.id) : s.add(p.id);
                  return s;
                })}
                onToggleExpanded={(next) => actions.onToggleExpanded(p.id, next)}
                onToggle={(pp) => cardService.onToggleEnabled(pp)}
                onEdit={(pp) => cardService.onEdit(pp)}
                onShare={(pp) => cardService.onShare(pp)}
                onDuplicate={(pp) => cardService.onDuplicate(pp)}
                onViewLogs={(pp) => cardService.onViewLogs(pp)}
                onQuickTest={(pp) => cardService.onQuickTest(pp)}
                onDelete={(id) => actions.onDelete(id)}
                onLevelPriorityChange={(v) => onSetLevelPriority(group.id, p.id, v)}
                onRefreshQuota={(pp) => actions.onRefreshQuota(pp)}
                refreshing={!!cards.quotaRefreshing[p.id]}
                lead={
                  <span
                    onPointerDown={(e) => onPlatPointerDown(e, p.id, group.id)}
                    className="drag-handle drag-handle-inline aidog-dragfade"
                    style={{ cursor: "grab", display: "inline-flex", alignItems: "center", touchAction: "none" }}
                    title={t("group.dragPlatform", "拖拽排序 / 移动到其他分组")}
                  >
                    <svg width="10" height="16" viewBox="0 0 10 16" fill="currentColor"><circle cx="3" cy="3" r="1.4" /><circle cx="7" cy="3" r="1.4" /><circle cx="3" cy="8" r="1.4" /><circle cx="7" cy="8" r="1.4" /><circle cx="3" cy="13" r="1.4" /><circle cx="7" cy="13" r="1.4" /></svg>
                  </span>
                }
              />
            </Fragment>
          ))}
          {dropIndicatorIdx === dropIndicatorTotal && (
            <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-1px 0", opacity: 0.7 }} />
          )}
        </div>
      )}
      {(forceExpanded || isExpanded || mode === "select") && fullPlats.length === 0 && (
        <div className="text-tertiary" style={{ padding: "14px 16px", fontSize: 12 }} onClick={e => e.stopPropagation()}>
          {t("group.noPlatforms", "暂无平台")}
        </div>
      )}

      {/* ── 模型映射 chips（稿 6 mp 样式）+ 快速添加 ── */}
      {(forceExpanded || isExpanded || mode === "select") && (
        <div style={{ borderTop: "1px solid var(--border)", padding: "8px 14px 10px" }} onClick={e => e.stopPropagation()}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
            <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)", letterSpacing: "1.2px", textTransform: "uppercase" }}>
              {t("group.detail.mappings", "模型映射")} · {model_mappings.length}
            </span>
            <Button variant="ghost" style={{ fontSize: 12, gap: 4, padding: "2px 8px", height: "auto", color: "var(--text-secondary)" }}
              onClick={() => onSetMappingGroupId(showMappingForm ? null : group.id)}>
              ＋ {t("mapping.add")}
            </Button>
          </div>
          {model_mappings.map((m, mi) => (
            <div key={mi} style={{ display: "flex", alignItems: "center", gap: 8, padding: "7px 0", fontSize: 12 }}>
              <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, border: "1px solid color-mix(in srgb, var(--primary) 35%, var(--border))", borderRadius: 6, padding: "2px 8px", color: "var(--primary)", background: "var(--secondary)" }}>{m.source_model}</span>
              <span style={{ color: "var(--text-tertiary)" }}>⟶</span>
              <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, border: "1px solid var(--border)", borderRadius: 6, padding: "2px 8px", color: "var(--text-secondary)", background: "var(--secondary)" }}>{m.target_model}</span>
              <span style={{ flex: 1 }} />
              <button type="button" className="ripple" title={t("group.deleteMapping", "删除映射")}
                style={{ background: "transparent", border: "none", cursor: "pointer", color: "var(--text-tertiary)", fontSize: 12, padding: "0 4px" }}
                onClick={(e) => { e.stopPropagation(); onDeleteMapping(group.id, mi); }}>✕</button>
            </div>
          ))}
          {showMappingForm && (
            <div className="animate-fade-in" style={{
              paddingTop: 10, display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap",
            }}>
              <Input style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                placeholder={t("mapping.source")} value={mSource}
                onChange={(e) => onSetMSource(e.target.value)} />
              <Select
                value={mTargetPlatform === "" ? "__none__" : String(mTargetPlatform)}
                onValueChange={(v) => { onSetMTargetPlatform(v === "__none__" ? "" : Number(v)); onSetMTargetModel(""); }}>
                <SelectTrigger style={{ fontSize: 12, width: 140 }}>
                  <SelectValue placeholder={t("mapping.targetPlatform")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">{t("mapping.targetPlatform")}</SelectItem>
                  {platforms.map((pp) => <SelectItem key={pp.id} value={String(pp.id)}>{pp.name}</SelectItem>)}
                </SelectContent>
              </Select>
              {availableModels.length > 0 ? (
                <Select
                  value={mTargetModel || "__none__"}
                  onValueChange={(v) => onSetMTargetModel(v === "__none__" ? "" : v)}>
                  <SelectTrigger style={{ flex: 1, minWidth: 100, fontSize: 12 }}>
                    <SelectValue placeholder={t("mapping.target")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">{t("mapping.target")}</SelectItem>
                    {availableModels.map(mm => <SelectItem key={mm} value={mm}>{mm}</SelectItem>)}
                  </SelectContent>
                </Select>
              ) : (
                <Input style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                  placeholder={t("mapping.target")} value={mTargetModel}
                  onChange={(e) => onSetMTargetModel(e.target.value)} />
              )}
              <Button style={{ fontSize: 12, padding: "6px 12px", height: "auto" }}
                onClick={onAddMapping}
                disabled={!mSource || !mTargetPlatform || !mTargetModel}>
                {t("action.create")}
              </Button>
            </div>
          )}
        </div>
      )}
      {/* 清理失效确认 AlertDialog（禁原生 confirm，CLAUDE.md 硬规）。 */}
      <AlertDialog open={purgeConfirmOpen} onOpenChange={(next) => { if (!next && !purging) setPurgeConfirmOpen(false); }}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 440, padding: "20px 22px" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("group.purgeDisabled", "清理失效")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("group.purgeDisabledConfirm", "将清理本分组失效平台（独占的永久删除，共享的仅从本分组移除），确定？")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div style={{ maxHeight: 240, overflow: "auto", display: "flex", flexDirection: "column", gap: 10, margin: "4px 0" }}>
            {previewLoading ? (
              <div className="text-secondary" style={{ fontSize: 12, padding: "8px 0" }}>{t("status.loading", "处理中…")}</div>
            ) : !previewList || previewList.length === 0 ? (
              <div className="text-secondary" style={{ fontSize: 12, padding: "8px 0" }}>{t("platform.purgeDisabledNone", "暂无失效平台")}</div>
            ) : (
              (["delete", "unassign"] as const).map((action) => {
                const items = previewList.filter(c => c.action === action);
                if (items.length === 0) return null;
                return (
                  <div key={action} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <div className="text-tertiary" style={{ fontSize: 11, fontWeight: 600 }}>
                      {action === "delete" ? t("platform.purgeDisabledActionDelete", "将永久删除") : t("platform.purgeDisabledActionUnassign", "将移出本分组")}
                    </div>
                    {items.map((c) => (
                      <div key={c.id} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12 }}>
                        <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.name}</span>
                        <Badge variant="secondary" style={{ fontSize: 11, flexShrink: 0 }}>
                          {c.reason === "auth_failed" ? t("platform.purgeDisabledReasonAuthFailed", "认证失效（401/403）") : t("platform.purgeDisabledReasonExpired", "已过期")}
                        </Badge>
                      </div>
                    ))}
                  </div>
                );
              })
            )}
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={purging}>{t("action.cancel", "取消")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={purging || previewLoading || !previewList || previewList.length === 0}
              onClick={async () => {
                setPurging(true);
                try { await onPurgeDisabled(group.id); } finally { setPurging(false); setPurgeConfirmOpen(false); }
              }}
            >
              {purging ? t("status.loading", "处理中…") : t("action.confirm", "确认")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
});
