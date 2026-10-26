import { useState } from "react";
// Platforms — 平台管理页（主组件）。
// ponytail: 阶段 4 二次拆分后的 orchestrator。state/handlers 全在 usePlatformsState（含 usePlatformQuota 子系统），
//   JSX 分两个区块子组件：PlatformEditForm（编辑/新建态）+ PlatformListView（列表态）。
//   本文件仅做：① 调 hook 拿 state ② 按 showForm 路由到对应子组件 ③ 构建 cardActions（latest-ref 稳定引用，
//   保 PlatformCard memo 生效）。
//
// 行为零变更（纯结构重构）：不改业务逻辑 / i18n key / Tauri command 签名。
// App.tsx import 路径 `./pages/Platforms` 保持不变。
import { useRef, useMemo } from "react";
import { DeletePlatformConfirm } from "../components/platforms/DeletePlatformConfirm";
import type { PlatformCardActions, PlatformCardService } from "../components/platforms/PlatformCard";
import { usePlatformsState } from "./platforms/usePlatformsState";
import { PlatformEditForm } from "./platforms/PlatformEditForm";
import { PlatformListView } from "./platforms/PlatformListView";

export function Platforms({ onNavigate, initialFilter }: { onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void; initialFilter?: { platformId?: number; platformName?: string; duplicate?: boolean } }) {
  // GroupsEmbedded「添加分组」弹窗触发 ref（按钮上移到本页页头）。
  const openCreateGroupRef = useRef<(() => void) | null>(null);
  // GroupsEmbedded 跨组件刷新入口（全局 purge 删平台后，触发分组卡内重建）。
  // ponytail: 该 ref 是 platform mutation 三连之一（见 usePlatformsState.ts 顶部「一致性规则」注释）。
  //   membership 写路径（handleDelete / handlePurgeDisabled / handleSave /
  //   runBatchCreateFromPaste）均须三连：handleGroupsChanged() + groupsReloadRef.current?.()
  //   + dispatch aidog-groups-changed。拖入分组特例跳过本 ref。
  const groupsReloadRef = useRef<(() => void) | null>(null);

  const s = usePlatformsState({ onNavigate, initialFilter, groupsReloadRef });
  const { list, drag, form } = s;

  // 卡片操作集合：用 latest-ref 持有最新闭包，对外暴露稳定引用，保证 PlatformCard memo 生效。
  // ponytail: 原代码每个回调显式包一层 actionsRef.current.X，迁移后保持完全等价的稳定引用语义。
  const actionsRef = useRef({
    s_handlePlatPointerDown: drag.handlePlatPointerDown,
    s_handlePlatPointerMove: drag.handlePlatPointerMove,
    s_handlePlatPointerUp: drag.handlePlatPointerUp,
    s_toggleExpanded: list.toggleExpanded,
    refreshQuota: s.quota.refreshQuota,
    s_handleToggle: list.handleToggle,
    s_handleEdit: form.handleEdit,
    s_handleShare: list.handleShare,
    s_handleDuplicate: form.handleDuplicate,
    s_handleDelete: list.handleDelete,
    s_handleViewLogs: form.handleViewLogs,
    s_handleQuickTest: list.handleQuickTest,
    s_setTestingPlatform: form.setTestingPlatform,
    s_setFaviconFailed: list.setFaviconFailed,
  });
  // 每次渲染把最新闭包刷入 ref（hook 返回的 handler 闭包随 state 更新，故每次都重写）。
  actionsRef.current = {
    s_handlePlatPointerDown: drag.handlePlatPointerDown,
    s_handlePlatPointerMove: drag.handlePlatPointerMove,
    s_handlePlatPointerUp: drag.handlePlatPointerUp,
    s_toggleExpanded: list.toggleExpanded,
    refreshQuota: s.quota.refreshQuota,
    s_handleToggle: list.handleToggle,
    s_handleEdit: form.handleEdit,
    s_handleShare: list.handleShare,
    s_handleDuplicate: form.handleDuplicate,
    s_handleDelete: list.handleDelete,
    s_handleViewLogs: form.handleViewLogs,
    s_handleQuickTest: list.handleQuickTest,
    s_setTestingPlatform: form.setTestingPlatform,
    s_setFaviconFailed: list.setFaviconFailed,
  };
  // 删除确认（G3 票 04）：⋯ 菜单删除先弹确认，确认后才走乐观删除链。
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);
  const cardActions = useMemo<PlatformCardActions>(() => ({
    onPointerDown: (e, index) => actionsRef.current.s_handlePlatPointerDown(e, index),
    onPointerMove: (e) => actionsRef.current.s_handlePlatPointerMove(e),
    onPointerUp: () => actionsRef.current.s_handlePlatPointerUp(),
    onToggleExpanded: (id, next) => actionsRef.current.s_toggleExpanded(id, next),
    onRefreshQuota: (p) => actionsRef.current.refreshQuota(p),
    onDelete: (id) => setConfirmDelete(id),
    onFaviconFailed: (id) => actionsRef.current.s_setFaviconFailed(prev => new Set(prev).add(id)),
  }), []);
  // 卡片行为服务（票 12）：只吃 platform 的 7 个回调独立成 service 对象，
  // 同样走 latest-ref 稳定引用，保 PlatformCard memo 生效。
  const cardService = useMemo<PlatformCardService>(() => ({
    onToggleEnabled: (p) => actionsRef.current.s_handleToggle(p),
    onEdit: (p) => actionsRef.current.s_handleEdit(p),
    onShare: (p) => actionsRef.current.s_handleShare(p),
    onDuplicate: (p) => actionsRef.current.s_handleDuplicate(p),
    onViewLogs: (p) => actionsRef.current.s_handleViewLogs(p),
    onQuickTest: (p) => actionsRef.current.s_handleQuickTest(p),
    onCustomTest: (p) => actionsRef.current.s_setTestingPlatform(p),
  }), []);

  // ── Edit / Add form (full page, no list) ──
  if (form.showForm) {
    return <PlatformEditForm s={s} />;
  }

  // ── List view ──
  return (
    <>
      <PlatformListView s={s} cardActions={cardActions} cardService={cardService} openCreateGroupRef={openCreateGroupRef} />
      {/* G3 票 04 / 旧票 03：删除平台先确认（密钥/端点/配置将删、日志保留、不可撤销） */}
      <DeletePlatformConfirm
        open={confirmDelete != null}
        name={confirmDelete != null ? (list.platforms.find(p => p.id === confirmDelete)?.platform_type ?? undefined) : undefined}
        onConfirm={() => { const id = confirmDelete; setConfirmDelete(null); if (id != null) actionsRef.current.s_handleDelete(id); }}
        onCancel={() => setConfirmDelete(null)}
      />
    </>
  );
}
