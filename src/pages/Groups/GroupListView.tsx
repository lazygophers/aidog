import type { TFunction } from "i18next";
import type { GroupDetail, Platform, PlatformUsageStats } from "../../services/api";
import type { PlatformCardActions } from "../../components/platforms/PlatformCard";
import type { usePlatformCards } from "../../components/platforms/usePlatformCards";
import { SortableList } from "../../components/SortableList";
import { CopyButton, StatChip, successRateLevel, costLevel } from "../../components/shared";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../../utils/formatters";
import { IconBolt, IconCost, IconCheck } from "../../components/icons";
import { ModelTestPanel } from "../ModelTestPanel";
import { ShareModal } from "../../components/platforms/ShareModal";
import { BatchDeleteModal } from "../../components/platforms/BatchDeleteModal";
import { BatchOverrideModelsModal } from "../../components/platforms/BatchOverrideModelsModal";
import { BatchSetStatusModal } from "../../components/platforms/BatchSetStatusModal";
import { BatchMoveGroupModal } from "../../components/platforms/BatchMoveGroupModal";
import type { PlatformModels } from "../../services/api";
import { GroupTestPanel, type GroupRow } from "../../domains/groups";
import { GroupListItem, type CardsSnapshot } from "./GroupListItem";
import type { GroupTestState } from "./useGroupTest";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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

export interface GroupListViewProps {
  details: GroupDetail[];
  platforms: Platform[];
  t: TFunction;
  // 视图状态
  loading: boolean;
  loadingMore: boolean;
  hasMore: boolean;
  sentinelRef: React.RefObject<HTMLDivElement | null>;
  proxyBaseUrl: string;
  // 列表派生
  groupRows: GroupRow[];
  groupIndexById: Map<number, number>;
  groupStats: Record<string, PlatformUsageStats>;
  groupBalance: Record<number, number>;
  /** 虚拟桶「未匹配」（MITM 解密非 API fallback 直通）：undefined = 无此类请求，不渲染卡片 */
  unmatchedStat: PlatformUsageStats | undefined;
  groupSearch: Map<number, { visibleIds: Set<number> | null }> | null;
  collapsedGroups: Set<number>;
  setCollapsedGroups: React.Dispatch<React.SetStateAction<Set<number>>>;
  toggleGroupExpanded: (id: number) => void;
  // 映射表单（列表内快速添加）
  mappingGroupId: number | null;
  mSource: string;
  mTargetPlatform: number | "";
  mTargetModel: string;
  availableModels: string[];
  setMappingGroupId: (id: number | null) => void;
  setMSource: (v: string) => void;
  setMTargetPlatform: (v: number | "") => void;
  setMTargetModel: (v: string) => void;
  // 拖拽
  dropIndicator: { gid: number; idx: number } | null;
  dragOverGroup: number | null;
  onPlatPointerDown: (e: React.PointerEvent, pid: number, gid: number) => void;
  // 卡片系统
  cards: ReturnType<typeof usePlatformCards>;
  makeGroupCardActions: (gid: number) => PlatformCardActions;
  // 测试
  groupTest: GroupTestState | null;
  setGroupTest: (v: GroupTestState | null) => void;
  // 删平台确认态：groupCount/groupNames 作 modal 展示
  removeTarget: { platform: Platform; gid: number; groupCount: number; groupNames: string[] } | null;
  setRemoveTarget: React.Dispatch<React.SetStateAction<{ platform: Platform; gid: number; groupCount: number; groupNames: string[] } | null>>;
  confirmDeletePlatform: () => Promise<void>;
  /** 多组场景「移出本组」按钮接线 */
  removePlatformFromGroup: (pid: number, gid: number) => Promise<void>;
  // 稳定回调（父级 useCallback）
  onToast?: (toast: { text: string; ok: boolean } | null) => void;
  handleReorderGroups: (next: GroupRow[]) => void;
  openEdit: (detail: GroupDetail) => void;
  handleDeleteGroup: (id: number) => void;
  handleToggleDefault: (group: GroupDetail["group"]) => void;
  handleTestGroup: (group: GroupDetail["group"], gps: GroupDetail["platforms"]) => void;
  handleDeleteMapping: (groupId: number, index: number) => void;
  handleAddMapping: () => void;
  handleSetLevelPriority: (gid: number, pid: number, v: number) => void;
  handlePurgeDisabled: (gid: number) => void;
  onCreatePlatform?: (presetGroupIds?: number[], lockedGroupId?: number) => void;
  onNavigate?: (id: string, context?: { groupId?: string; groupKey?: string; platformId?: number; platformName?: string; duplicate?: boolean }) => void;
  // 批量删除（group-batch-ops s3）
  onBatchDelete: (ids: number[], gid: number) => void;
  batchDeleteTarget: { platforms: Platform[]; groupNamesByPlatform: Record<number, string[]> } | null;
  batchDeleteBusy: boolean;
  confirmBatchDelete: () => Promise<void>;
  setBatchDeleteTarget: React.Dispatch<React.SetStateAction<{ platforms: Platform[]; groupNamesByPlatform: Record<number, string[]> } | null>>;
  // 批量覆盖模型（group-batch-ops s4）
  onBatchOverrideModels: (ids: number[], gid: number) => void;
  batchOverrideTarget: { platforms: Platform[] } | null;
  batchOverrideBusy: boolean;
  confirmBatchOverrideModels: (models: PlatformModels) => Promise<void>;
  setBatchOverrideTarget: React.Dispatch<React.SetStateAction<{ platforms: Platform[] } | null>>;
  /** 非删除类批量完成信号（GroupListItem 监听退出多选）。 */
  batchDoneSignal?: number;
  // 批量改状态（group-batch-ops s5）
  onBatchSetStatus: (ids: number[], gid: number) => void;
  batchSetStatusTarget: { platforms: Platform[]; groupEnabledIds: number[] } | null;
  batchSetStatusBusy: boolean;
  confirmBatchSetStatus: (status: "enabled" | "disabled") => Promise<void>;
  setBatchSetStatusTarget: React.Dispatch<React.SetStateAction<{ platforms: Platform[]; groupEnabledIds: number[] } | null>>;
  // 批量移组（group-batch-ops s5）
  onBatchMoveGroup: (ids: number[], gid: number) => void;
  batchMoveGroupTarget: { platforms: Platform[]; gid: number } | null;
  batchMoveGroupBusy: boolean;
  confirmBatchMoveGroup: (targetGroupId: number, mode: "move" | "add") => Promise<void>;
  setBatchMoveGroupTarget: React.Dispatch<React.SetStateAction<{ platforms: Platform[]; gid: number } | null>>;
  /** 全部分组（目标组下拉数据源）。 */
  allGroups: { id: number; name: string }[];
}

/** 分组列表视图：页头操作栏 + 测试面板 + SortableList + 加载哨兵 + 弹窗（自定义测试 / 分享 / 删平台确认）。 */
export function GroupListView(props: GroupListViewProps) {
  const {
    details, platforms, t, loading, loadingMore, hasMore, sentinelRef, proxyBaseUrl,
    groupRows, groupIndexById, groupStats, groupBalance, unmatchedStat, groupSearch,
    collapsedGroups, setCollapsedGroups, toggleGroupExpanded,
    mappingGroupId, mSource, mTargetPlatform, mTargetModel, availableModels,
    setMappingGroupId, setMSource, setMTargetPlatform, setMTargetModel,
    dropIndicator, dragOverGroup, onPlatPointerDown,
    cards, makeGroupCardActions,
    groupTest, setGroupTest,
    removeTarget, setRemoveTarget, confirmDeletePlatform, removePlatformFromGroup,
    onToast,
    handleReorderGroups, openEdit, handleDeleteGroup, handleToggleDefault,
    handleTestGroup, handleDeleteMapping, handleAddMapping,
    handleSetLevelPriority, handlePurgeDisabled,
    onCreatePlatform, onNavigate,
    onBatchDelete, batchDeleteTarget, batchDeleteBusy, confirmBatchDelete, setBatchDeleteTarget,
    onBatchOverrideModels, batchOverrideTarget, batchOverrideBusy, confirmBatchOverrideModels, setBatchOverrideTarget,
    batchDoneSignal,
    onBatchSetStatus, batchSetStatusTarget, batchSetStatusBusy, confirmBatchSetStatus, setBatchSetStatusTarget,
    onBatchMoveGroup, batchMoveGroupTarget, batchMoveGroupBusy, confirmBatchMoveGroup, setBatchMoveGroupTarget,
    allGroups,
  } = props;

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
            const gs = groupSearch?.get(group.id) ?? null;
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
                visiblePlatformIds={gs?.visibleIds ?? null}
                forceExpanded={!!gs}
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
                onBatchDelete={onBatchDelete}
                onBatchOverrideModels={onBatchOverrideModels}
                onBatchSetStatus={onBatchSetStatus}
                onBatchMoveGroup={onBatchMoveGroup}
                batchDoneSignal={batchDoneSignal}
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
          {/* 虚拟桶「未匹配」只读卡片：MITM 解密非 API 流量 fallback 直通的统计，无平台/余额/编辑。 */}
          {unmatchedStat && unmatchedStat.total_requests > 0 && (
            <UnmatchedBucketCard stat={unmatchedStat} t={t} />
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
          title={cards.shareData.name}
          urlScheme="aidog://platform/import"
          onToast={(text, ok) => {
            onToast?.({ text, ok });
            setTimeout(() => onToast?.(null), 3000);
          }}
          onClose={() => cards.setShareData(null)}
        />
      )}

      {/* 删平台确认弹窗：总弹（根因旁路，去 count 决定行为）。
          单组 → 单按钮「删除平台」（销毁平台）；多组 → 双按钮「移出本组」+「删除平台（全部组）」。
          AlertDialog 走 Radix Portal（脱离 transform 祖先，参考 BatchDeleteModal）。 */}
      <AlertDialog open={removeTarget !== null} onOpenChange={(next) => { if (!next) setRemoveTarget(null); }}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 420, padding: "20px 22px" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {removeTarget?.groupCount && removeTarget.groupCount > 1
                ? t("group.deletePlatformMultiTitle", "移出或删除平台")
                : t("group.deletePlatformTitle", "删除平台")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {removeTarget?.groupCount && removeTarget.groupCount > 1
                ? t("group.deletePlatformMultiDesc",
                    "「{{name}}」属 {{count}} 个分组：{{groups}}。选择操作：",
                    { name: removeTarget.platform.name, count: removeTarget.groupCount, groups: removeTarget.groupNames.join("、") })
                : t("group.deletePlatformConfirm",
                    "「{{name}}」仅属此分组，移除将彻底删除该平台及其所有关联，且无法撤销。确认删除？",
                    { name: removeTarget?.platform.name ?? "" })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("action.cancel", "取消")}</AlertDialogCancel>
            {removeTarget?.groupCount && removeTarget.groupCount > 1 && (
              <Button variant="outline" onClick={() => {
                removePlatformFromGroup(removeTarget.platform.id, removeTarget.gid);
                setRemoveTarget(null);
              }}>
                {t("group.removeFromGroupAction", "移出本组")}
              </Button>
            )}
            <AlertDialogAction
              style={{ backgroundColor: "var(--color-danger)", color: "var(--destructive-foreground)" }}
              onClick={() => { void confirmDeletePlatform(); }}>
              {removeTarget?.groupCount && removeTarget.groupCount > 1
                ? t("group.deleteFromAllGroupsAction", "删除平台（全部组）")
                : t("group.deletePlatformAction", "删除平台")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 批量删除平台确认弹窗（group-batch-ops s3）：全列可滚 + 跨组警告 + 原子事务物理删。 */}
      <BatchDeleteModal
        open={batchDeleteTarget !== null}
        platforms={batchDeleteTarget?.platforms ?? []}
        groupNamesByPlatform={batchDeleteTarget?.groupNamesByPlatform ?? {}}
        onConfirm={() => void confirmBatchDelete()}
        onClose={() => { if (!batchDeleteBusy) setBatchDeleteTarget(null); }}
        busy={batchDeleteBusy}
        t={t}
      />

      {/* 批量覆盖平台模型弹窗（group-batch-ops s4）：三来源 radio + 全 diff + 原子事务整体覆盖。 */}
      <BatchOverrideModelsModal
        open={batchOverrideTarget !== null}
        platforms={batchOverrideTarget?.platforms ?? []}
        allPlatforms={platforms}
        onConfirm={(m) => void confirmBatchOverrideModels(m)}
        onClose={() => { if (!batchOverrideBusy) setBatchOverrideTarget(null); }}
        busy={batchOverrideBusy}
        t={t}
      />

      {/* 批量改状态弹窗（group-batch-ops s5）：启用/禁用 radio + 无候选警告 + 原子事务。 */}
      <BatchSetStatusModal
        open={batchSetStatusTarget !== null}
        platforms={batchSetStatusTarget?.platforms ?? []}
        groupEnabledIds={batchSetStatusTarget?.groupEnabledIds ?? []}
        onConfirm={(s) => void confirmBatchSetStatus(s)}
        onClose={() => { if (!batchSetStatusBusy) setBatchSetStatusTarget(null); }}
        busy={batchSetStatusBusy}
        t={t}
      />

      {/* 批量移组弹窗（group-batch-ops s5）：目标组下拉 + move/add radio + 原子事务。 */}
      <BatchMoveGroupModal
        open={batchMoveGroupTarget !== null}
        platforms={batchMoveGroupTarget?.platforms ?? []}
        groups={allGroups}
        currentGroupId={batchMoveGroupTarget?.gid ?? 0}
        onConfirm={(gid, mode) => void confirmBatchMoveGroup(gid, mode)}
        onClose={() => { if (!batchMoveGroupBusy) setBatchMoveGroupTarget(null); }}
        busy={batchMoveGroupBusy}
        t={t}
      />
    </div>
  );
}


/** 虚拟桶「未匹配」只读卡片：MITM 解密非 API 流量未匹配分组 → fallback 直通原 host 的统计。
 *  非真实分组（不入 groups 表），仅展示请求数/token/cost(恒0)/成功率，灰色徽标区分，无编辑/平台/余额。 */
function UnmatchedBucketCard({ stat: u, t }: { stat: PlatformUsageStats; t: TFunction }) {
  const totalTokens = u.total_input_tokens + u.total_output_tokens;
  const sRate = calcSuccessRate(u.success_count, u.total_requests);
  return (
    <Card className="glass-surface" style={{
      padding: "14px 18px", display: "flex", flexDirection: "column", gap: 8,
      opacity: 0.85, border: "1px dashed var(--border)", borderRadius: "var(--radius-md)",
    }}>
      <CardHeader style={{ padding: 0, gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <CardTitle style={{ fontSize: 14, fontWeight: 600 }}>{t("group.unmatched", "未匹配")}</CardTitle>
          <Badge variant="secondary" style={{ fontSize: 10, padding: "0 6px", fontWeight: 500 }}>
            {t("group.unmatchedBadge", "虚拟桶")}
          </Badge>
        </div>
      </CardHeader>
      <CardContent style={{ padding: 0, display: "flex", flexDirection: "column", gap: 8 }}>
        <div className="text-tertiary" style={{ fontSize: 12, lineHeight: 1.5 }}>
          {t("group.unmatchedHint", "MITM 解密的非 API 流量未匹配分组时透明转发到原 host，不计费，仅统计请求数。")}
        </div>
        <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
          <StatChip icon={<IconBolt size={13} />} value={formatNumber(totalTokens)} label="tokens" />
          <StatChip icon={<IconCost size={13} />} value={`$${formatCost(u.total_cost)}`} label="cost" level={costLevel(u.total_cost)} />
          {u.total_requests > 0 && (
            <StatChip icon={<IconCheck size={13} />} value={formatPercent(sRate, 0)} label="ok"
              level={successRateLevel(sRate, u.total_requests)} />
          )}
        </div>
      </CardContent>
    </Card>
  );
}
