// Platforms — 平台管理页（主组件）。
// ponytail: 阶段 4 二次拆分后的 orchestrator。state/handlers 全在 usePlatformsState（含 usePlatformQuota 子系统），
//   JSX 分两个区块子组件：PlatformEditForm（编辑/新建态）+ PlatformListView（列表态）。
//   本文件仅做：① 调 hook 拿 state ② 按 showForm 路由到对应子组件 ③ 构建 cardActions（latest-ref 稳定引用，
//   保 PlatformCard memo 生效）。
//
// 行为零变更（纯结构重构）：不改业务逻辑 / i18n key / Tauri command 签名。
// App.tsx import 路径 `./pages/Platforms` 保持不变。
import { useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";

import type { PlatformCardService } from "../components/platforms/PlatformCard";
import { usePlatformsState } from "./platforms/usePlatformsState";
import { PlatformEditForm } from "./platforms/PlatformEditForm";
import { PlatformListView } from "./platforms/PlatformListView";
import { Sheet, SheetContent, SheetTitle } from "@/components/ui/sheet";

export function Platforms({ onNavigate, initialFilter }: { onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void; initialFilter?: { platformId?: number; platformName?: string; duplicate?: boolean } }) {
  const { t } = useTranslation();
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

  // ── List view + 编辑态抽屉（重设计稿 5：点行内「编辑」右侧滑入抽屉，页面其余压暗）──
  // showForm 由 handleEdit / handleDuplicate / openCreatePlatform 置位；关闭走 resetForm 复位表单。
  return (
    <>
      <PlatformListView s={s} cardService={cardService} openCreateGroupRef={openCreateGroupRef} />
      <Sheet open={form.showForm} onOpenChange={(open) => { if (!open) form.resetForm(); }}>
        <SheetContent
          side="right"
          className="w-[400px] sm:max-w-[400px] p-4 pt-12"
          data-testid="platform-edit-drawer"
        >
          <SheetTitle className="sr-only">{form.editing?.name ?? t("platform.add")}</SheetTitle>
          <PlatformEditForm s={s} variant="drawer" />
        </SheetContent>
      </Sheet>
    </>
  );
}
