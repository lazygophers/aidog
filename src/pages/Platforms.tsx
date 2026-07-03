// Platforms — 平台管理页（主组件）。
// ponytail: 阶段 4 二次拆分后的 orchestrator。state/handlers 全在 usePlatformsState（含 usePlatformQuota 子系统），
//   JSX 分两个区块子组件：PlatformEditForm（编辑/新建态）+ PlatformListView（列表态）。
//   本文件仅做：① 调 hook 拿 state ② 按 showForm 路由到对应子组件 ③ 构建 cardActions（latest-ref 稳定引用，
//   保 PlatformCard memo 生效）。
//
// 行为零变更（纯结构重构）：不改业务逻辑 / i18n key / Tauri command 签名。
// App.tsx import 路径 `./pages/Platforms` 保持不变。
import { useRef, useMemo } from "react";
import type { PlatformCardActions } from "../components/platforms/PlatformCard";
import { usePlatformsState } from "./platforms/usePlatformsState";
import { PlatformEditForm } from "./platforms/PlatformEditForm";
import { PlatformListView } from "./platforms/PlatformListView";

export function Platforms({ onNavigate, initialFilter }: { onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void; initialFilter?: { platformId?: number; platformName?: string; duplicate?: boolean } }) {
  // GroupsEmbedded「添加分组」弹窗触发 ref（按钮上移到本页页头）。
  const openCreateGroupRef = useRef<(() => void) | null>(null);
  // GroupsEmbedded 跨组件刷新入口（全局 purge 删平台后，触发分组卡内重建）。
  const groupsReloadRef = useRef<(() => void) | null>(null);

  const s = usePlatformsState({ onNavigate, initialFilter, groupsReloadRef });

  // 卡片操作集合：用 latest-ref 持有最新闭包，对外暴露稳定引用，保证 PlatformCard memo 生效。
  // ponytail: 原代码每个回调显式包一层 actionsRef.current.X，迁移后保持完全等价的稳定引用语义。
  const actionsRef = useRef({
    s_handlePlatPointerDown: s.handlePlatPointerDown,
    s_handlePlatPointerMove: s.handlePlatPointerMove,
    s_handlePlatPointerUp: s.handlePlatPointerUp,
    s_toggleExpanded: s.toggleExpanded,
    refreshQuota: s.quota.refreshQuota,
    s_handleToggle: s.handleToggle,
    s_handleEdit: s.handleEdit,
    s_handleShare: s.handleShare,
    s_handleDuplicate: s.handleDuplicate,
    s_handleDelete: s.handleDelete,
    s_handleViewLogs: s.handleViewLogs,
    s_handleQuickTest: s.handleQuickTest,
    s_setTestingPlatform: s.setTestingPlatform,
    s_setFaviconFailed: s.setFaviconFailed,
  });
  // 每次渲染把最新闭包刷入 ref（hook 返回的 handler 闭包随 state 更新，故每次都重写）。
  actionsRef.current = {
    s_handlePlatPointerDown: s.handlePlatPointerDown,
    s_handlePlatPointerMove: s.handlePlatPointerMove,
    s_handlePlatPointerUp: s.handlePlatPointerUp,
    s_toggleExpanded: s.toggleExpanded,
    refreshQuota: s.quota.refreshQuota,
    s_handleToggle: s.handleToggle,
    s_handleEdit: s.handleEdit,
    s_handleShare: s.handleShare,
    s_handleDuplicate: s.handleDuplicate,
    s_handleDelete: s.handleDelete,
    s_handleViewLogs: s.handleViewLogs,
    s_handleQuickTest: s.handleQuickTest,
    s_setTestingPlatform: s.setTestingPlatform,
    s_setFaviconFailed: s.setFaviconFailed,
  };
  const cardActions = useMemo<PlatformCardActions>(() => ({
    onPointerDown: (e, index) => actionsRef.current.s_handlePlatPointerDown(e, index),
    onPointerMove: (e) => actionsRef.current.s_handlePlatPointerMove(e),
    onPointerUp: () => actionsRef.current.s_handlePlatPointerUp(),
    onToggleExpanded: (id, next) => actionsRef.current.s_toggleExpanded(id, next),
    onRefreshQuota: (p) => actionsRef.current.refreshQuota(p),
    onToggleEnabled: (p) => actionsRef.current.s_handleToggle(p),
    onEdit: (p) => actionsRef.current.s_handleEdit(p),
    onShare: (p) => actionsRef.current.s_handleShare(p),
    onDuplicate: (p) => actionsRef.current.s_handleDuplicate(p),
    onDelete: (id) => actionsRef.current.s_handleDelete(id),
    onViewLogs: (p) => actionsRef.current.s_handleViewLogs(p),
    onQuickTest: (p) => actionsRef.current.s_handleQuickTest(p),
    onCustomTest: (p) => actionsRef.current.s_setTestingPlatform(p),
    onFaviconFailed: (id) => actionsRef.current.s_setFaviconFailed(prev => new Set(prev).add(id)),
  }), []);

  // ── Edit / Add form (full page, no list) ──
  if (s.showForm) {
    return <PlatformEditForm s={s} />;
  }

  // ── List view ──
  return <PlatformListView s={s} cardActions={cardActions} openCreateGroupRef={openCreateGroupRef} />;
}
