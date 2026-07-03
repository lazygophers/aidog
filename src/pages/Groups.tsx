import { useState, useEffect, useReducer, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import type {
  GroupDetail, Platform, RoutingMode, ModelMapping,
} from "../services/api";
import { groupApi, groupDetailApi, platformApi } from "../services/api";
import { allModelValues } from "../domains/platforms";
import type { PlatformCardActions } from "../components/platforms/PlatformCard";
import { usePlatformCards } from "../components/platforms/usePlatformCards";
import {
  editReducer, EMPTY_EDIT,
  platformMatchesQuery, groupMatchesQuery,
  type GroupRow,
} from "../domains/groups";
import { GroupEditPanel } from "./Groups/GroupEditPanel";
import { GroupCreateModal } from "./Groups/GroupCreateModal";
import { GroupListView } from "./Groups/GroupListView";
import { useGroupData } from "./Groups/useGroupData";
import { useGroupTest } from "./Groups/useGroupTest";
import { usePlatformDrag } from "./Groups/usePlatformDrag";

/** 分组内嵌组件（供 Platforms 页使用） */
export function GroupsEmbedded({ onNavigate, onGroupsChanged, onCreatePlatform, onEditPlatform, onDuplicatePlatform, onToast, onViewModeChange, openCreateGroupRef, reloadRef, onCountChange, searchQuery }: {
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
  /** 平台搜索关键词（来自 Platforms 页头搜索框）：命中平台只展示命中项（同组其他折叠），
   *  命中分组名整组展开；空串 = 不过滤（原行为）。 */
  searchQuery?: string;
}) {
  const { t } = useTranslation();

  // ── 数据加载（分页 / 统计 / 余额聚合）──
  const {
    details, platforms, setDetails, groupStats, groupBalance,
    loading, loadingMore, hasMore, sentinelRef, proxyBaseUrl,
    load, refreshSingleGroup,
  } = useGroupData({ onCountChange });

  // Edit mode（8 字段合并为单 reducer）
  const [edit, dispatchEdit] = useReducer(editReducer, EMPTY_EDIT);
  const { target: editTarget } = edit;

  // Create mode
  const [showCreate, setShowCreate] = useState(false);
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

  // 父级跨组件刷新入口（全局 purge 后触发），绑定本组件 load() 重建分组卡内平台状态。
  useEffect(() => {
    if (!reloadRef) return;
    reloadRef.current = () => { load(); onGroupsChanged?.(); };
    return () => { reloadRef.current = null; };
  }, [reloadRef, load, onGroupsChanged]);

  // ── 分组展开区平台卡片：复用 PlatformCard + usePlatformCards（与 Platforms 主列表同款） ──
  const cards = usePlatformCards({ onNavigate, onEdit: onEditPlatform, setToast: onToast });
  const cardsSetUsageMap = cards.setUsageMap;
  const usageReqRef = useRef<Set<number>>(new Set());
  useEffect(() => {
    if (platforms.length === 0) { usageReqRef.current = new Set(); return; }
    let alive = true;
    for (const p of platforms) {
      if (usageReqRef.current.has(p.id)) continue;
      usageReqRef.current.add(p.id);
      const pid = p.id;
      platformApi.usageStats(pid)
        .then(s => { if (alive && s) cardsSetUsageMap(prev => ({ ...prev, [pid]: s })); })
        .catch(() => { /* ignore：该卡 usage 缺失不影响其它卡 */ });
    }
    return () => { alive = false; };
  }, [platforms, cardsSetUsageMap, usageReqRef]);

  // 分组展开态：默认全展开。追踪「已折叠」集（默认空 = 全展开）。
  const [collapsedGroups, setCollapsedGroups] = useState<Set<number>>(new Set());
  const toggleGroupExpanded = useCallback((id: number) => setCollapsedGroups(prev => {
    const s = new Set(prev); s.has(id) ? s.delete(id) : s.add(id); return s;
  }), []);

  // 分组卡片「移除平台」确认态：仅当平台只属当前一个分组（删除即销毁平台，破坏性）时弹确认。
  const [removeTarget, setRemoveTarget] = useState<{ platform: Platform; gid: number } | null>(null);

  // 平台所属分组数（按 platform_id 跨 details 计数），用于判定删除 vs 仅移出。
  const groupCountOf = useCallback((pid: number): number =>
    details.reduce((n, d) => n + (d.platforms.some(gp => gp.platform.id === pid) ? 1 : 0), 0),
  [details]);

  // 仅从当前分组移出该平台（不删平台、不动其他组）：用 group_set_platforms 重设本组平台集（去掉该平台）。
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
    groupDetailApi.setPlatformLevelPriority(gid, pid, next).catch((err: unknown) => {
      console.error("[aidog] setPlatformLevelPriority failed", err);
      onToast?.({ text: t("group.levelPriorityFailed", "优先级保存失败: {{err}}", { err: String(err) }), ok: false });
      setDetails(prev => prev.map(d => {
        if (d.group.id !== gid) return d;
        return {
          ...d,
          platforms: d.platforms.map(gp =>
            gp.platform.id === pid ? { ...gp, level_priority: prevValue } : gp),
        };
      }));
    });
  }, [onToast, t, setDetails]);

  // ── 一键测试本组全部平台 ──
  const { groupTest, setGroupTest, handleTestGroup } = useGroupTest();

  // ── 分组展开区平台拖拽（pointer 事件驱动） ──
  const { dropIndicator, dragOverGroup, onPlatPointerDown } = usePlatformDrag({
    details, platforms, setDetails, load, onToast, onGroupsChanged,
  });

  // ── 列表排序（dnd-kit）：搜索态下 no-op（搜索是临时视图，重排会丢未命中组）──
  const sq = (searchQuery ?? "").trim();
  const handleReorderGroups = useCallback((next: GroupRow[]) => {
    if (sq) return;
    const reordered = next.map(r => r.detail);
    setDetails(reordered);
    groupApi.reorder(reordered.map(d => d.group.id)).catch(console.error);
  }, [sq, setDetails]);

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
      // Update group basic info + inline model mappings + env vars
      await groupApi.update({
        id: editTarget.group.id,
        name: edit.name,
        routing_mode: edit.mode,
        request_timeout_secs: edit.reqTimeout,
        connect_timeout_secs: edit.connTimeout,
        max_retries: edit.maxRetries,
        model_mappings: edit.mappings,
        env_vars: edit.envVars,
      });
      // Update platforms
      await groupApi.setPlatforms(
        editTarget.group.id,
        edit.platformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
      );
      const savedGid = editTarget.group.id;
      cancelEdit();
      // 编辑保存只动单组 → 单组就地刷新，不整列表重载（消除保存闪烁/卡顿）。
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
      // ponytail: quick mapping 编辑走最小 update，但后端 UpdateGroup.env_vars 是
      // #[serde(default)] Vec（非 Option），缺省 = [] 会清空既有 env_vars。
      // 同 model_mappings 一并透传 detail 当前值，保持 partial update 语义。
      await groupApi.update({ id: gid, model_mappings: next, env_vars: detail.group.env_vars });
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
      // ponytail: 同 handleAddMapping —— env_vars 必须透传，否则被 default 清空。
      await groupApi.update({ id: groupId, model_mappings: next, env_vars: detail.group.env_vars });
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

  // per-group 搜索结果：命中分组名 → 整组（visibleIds=null 信号）；否则 → 命中平台 id 集（可能空）。
  const groupSearch = useMemo(() => {
    if (!sq) return null;
    const map = new Map<number, { visibleIds: Set<number> | null }>();
    for (const d of details) {
      if (groupMatchesQuery(d.group, sq)) {
        map.set(d.group.id, { visibleIds: null });
        continue;
      }
      const matched = new Set<number>();
      for (const gp of d.platforms) {
        const pp = platforms.find(p => p.id === gp.platform.id);
        if (pp && platformMatchesQuery(pp, sq)) matched.add(pp.id);
      }
      if (matched.size > 0) map.set(d.group.id, { visibleIds: matched });
    }
    return map;
  }, [sq, details, platforms]);

  // SortableList items + group→index 映射：搜索态下过滤掉无命中的组。
  const groupRows = useMemo<GroupRow[]>(
    () => details
      .filter(d => !groupSearch || groupSearch.has(d.group.id))
      .map(d => ({ id: String(d.group.id), detail: d })),
    [details, groupSearch],
  );
  const groupIndexById = useMemo(() => {
    const m = new Map<number, number>();
    details.forEach((d, i) => m.set(d.group.id, i));
    return m;
  }, [details]);

  // ── Edit page ──
  if (editTarget) {
    return (
      <GroupEditPanel
        edit={edit}
        dispatchEdit={dispatchEdit}
        platforms={platforms}
        t={t}
        onCancel={cancelEdit}
        onSave={saveEdit}
      />
    );
  }

  // ── Create page（独立视图态，复用编辑视图的全屏 + 返回箭头 Header 模式）──
  if (showCreate) {
    const closeCreate = () => { setCName(""); setCGroupKey(""); setCMode("failover"); setCPlatformIds([]); setShowCreate(false); };
    return (
      <GroupCreateModal
        cName={cName}
        cGroupKey={cGroupKey}
        cMode={cMode}
        cPlatformIds={cPlatformIds}
        platforms={platforms}
        t={t}
        onCName={setCName}
        onCGroupKey={setCGroupKey}
        onCMode={setCMode}
        onCPlatformIds={setCPlatformIds}
        onClose={closeCreate}
        onCreate={handleCreateGroup}
      />
    );
  }

  // ── List view ──
  return (
    <GroupListView
      details={details}
      platforms={platforms}
      t={t}
      loading={loading}
      loadingMore={loadingMore}
      hasMore={hasMore}
      sentinelRef={sentinelRef}
      proxyBaseUrl={proxyBaseUrl}
      groupRows={groupRows}
      groupIndexById={groupIndexById}
      groupStats={groupStats}
      groupBalance={groupBalance}
      groupSearch={groupSearch}
      collapsedGroups={collapsedGroups}
      setCollapsedGroups={setCollapsedGroups}
      toggleGroupExpanded={toggleGroupExpanded}
      mappingGroupId={mappingGroupId}
      mSource={mSource}
      mTargetPlatform={mTargetPlatform}
      mTargetModel={mTargetModel}
      availableModels={availableModels}
      setMappingGroupId={setMappingGroupId}
      setMSource={setMSource}
      setMTargetPlatform={setMTargetPlatform}
      setMTargetModel={setMTargetModel}
      dropIndicator={dropIndicator}
      dragOverGroup={dragOverGroup}
      onPlatPointerDown={onPlatPointerDown}
      cards={cards}
      makeGroupCardActions={makeGroupCardActions}
      groupTest={groupTest}
      setGroupTest={setGroupTest}
      removeTarget={removeTarget}
      setRemoveTarget={setRemoveTarget}
      confirmDeletePlatform={confirmDeletePlatform}
      onToast={onToast}
      handleReorderGroups={handleReorderGroups}
      openEdit={openEdit}
      handleDeleteGroup={handleDeleteGroup}
      handleToggleDefault={handleToggleDefault}
      handleTestGroup={handleTestGroup}
      handleDeleteMapping={handleDeleteMapping}
      handleAddMapping={handleAddMapping}
      handleSetLevelPriority={handleSetLevelPriority}
      handlePurgeDisabled={handlePurgeDisabled}
      onCreatePlatform={onCreatePlatform}
      onNavigate={onNavigate}
    />
  );
}
