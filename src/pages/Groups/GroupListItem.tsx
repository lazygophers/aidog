import { Fragment, memo } from "react";
import type { TFunction } from "i18next";
import claudeIcon from "../../assets/platforms/claude_code.svg";
import codexIcon from "../../assets/platforms/openai.svg";
import type { GroupDetail, GroupPlatformDetail, Platform, PlatformUsageStats, PlatformQuota, LastTestResult } from "../../services/api";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../../utils/formatters";
import { CompactCard, StatChip, BalanceBar, CopyButton, successRateLevel, costLevel } from "../../components/shared";
import { IconCheck, IconHome, IconBolt, IconCost } from "../../components/icons";
import { PlatformCard, type PlatformCardActions } from "../../components/platforms/PlatformCard";
import type { DragHandleProps } from "../../components/SortableList";
import { buildClaudeCommand, buildCodexCommand, routingModeLabel, GroupIcon } from "../../domains/groups";

/** usePlatformCards 的卡片展示状态快照（memo 化子组件按需接收） */
export interface CardsSnapshot {
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
export const GroupListItem = memo(function GroupListItem({
  detail, index, usageStat: u, balance, platforms,
  isExpanded, isDragOver, dropIndicatorIdx, dropIndicatorTotal,
  showMappingForm, mSource, mTargetPlatform, mTargetModel, availableModels,
  visiblePlatformIds, forceExpanded,
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
        <CopyButton text={buildCodexCommand(group.group_key, group.env_vars)} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} size={14} />
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
    .filter((pp): pp is Platform => !!pp)
    // 搜索命中过滤：仅渲染命中平台卡（null = 无搜索，保留原行为）
    .filter(pp => !visiblePlatformIds || visiblePlatformIds.has(pp.id));

  return (
    <div
      className="animate-fade-in"
      data-group-id={group.id}
      style={{ animationDelay: `${index * 60}ms` }}
    >
      <CompactCard
        header={header}
        expanded={forceExpanded || isExpanded}
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
