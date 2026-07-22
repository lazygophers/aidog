// PlatformListView — Platforms 列表态视图（showForm=false 时渲染）。
// ponytail: 从 Platforms 主组件抽出的纯展示组件。所有 state/handlers 经 props 从 usePlatformsState 传入。
//   渲染：页头（搜索 + 添加分组 + 添加平台 + 清理失效）+ GroupsEmbedded（分组段）+ 未分组平台列表 +
//   ModelTestPanel overlay + groupDrag portal + ShareModal + toast portal。
import React, { useEffect, useState } from "react";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { type Platform } from "../../services/api";
import { IconClose, IconCheck } from "../../components/icons";
import { ModelTestPanel } from "../ModelTestPanel";
import { GroupsEmbedded } from "../Groups";
import { PlatformCard, type PlatformCardActions } from "../../components/platforms/PlatformCard";
import { ShareModal } from "../../components/platforms/ShareModal";
import { getProtocolColorMap, getProtocolLabelMap } from "../../domains/platforms/defaults";
import type { PlatformsState } from "./usePlatformsState";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

export function PlatformListView({ s, cardActions, openCreateGroupRef }: {
  s: PlatformsState;
  cardActions: PlatformCardActions;
  openCreateGroupRef: React.MutableRefObject<(() => void) | null>;
}) {
  const { t, i18n } = useTranslation();
  // ponytail: 清理失效平台确认走 AlertDialog（禁原生 confirm，CLAUDE.md 硬规）
  const [purgeConfirmOpen, setPurgeConfirmOpen] = useState(false);
  const [purging, setPurging] = useState(false);
  // colorMap / labelMap：async 派生自 platform-presets.json；首帧空 map（fallback platform_type key）。
  const [colorMap, setColorMap] = useState<Partial<Record<string, string>>>({});
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    Promise.all([getProtocolColorMap(), getProtocolLabelMap(i18n.language)]).then(([c, l]) => {
      if (!cancelled) { setColorMap(c); setLabelMap(l); }
    });
    return () => { cancelled = true; };
  }, [i18n.language]);
  const {
    platforms, loading, headerActive, headerTotal,
    searchQuery, setSearchQuery,
    handleGroupsChanged, openCreatePlatform, handleEdit, handleDuplicate,
    setGroupFullscreen, setProgressiveCount,
    groupFullscreen,
    platDrag, platListRef,
    standalonePlatforms,
    onStandaloneGroupPointerDown, onStandaloneGroupPointerMove, onStandaloneGroupPointerUp,
    groupDrag,
    quota, usageLoading, usageMap, expandedIds, testResults, testingId, faviconFailed, platformMembership, lastTestMap,
    resetForm, setShowForm,
    handlePurgeDisabled,
    testingPlatform, setTestingPlatform, setTestResults,
    shareData, setShareData,
    toast, setToast,
    onNavigate,
  } = s;
  // ponytail: purge 确认在 AlertDialog Action 触发（busy 期间禁按钮防双击）
  const onPurgeConfirm = async () => {
    setPurging(true);
    try { await handlePurgeDisabled(); } finally { setPurging(false); setPurgeConfirmOpen(false); }
  };

  return (
    <>
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.platforms")}</div>
          <div className="section-desc">
            {headerTotal > 0 ? `${headerActive} / ${headerTotal} active` : t("platform.empty")}
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Input
            className="input"
            placeholder={t("platform.searchPlaceholder", "搜索平台...")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{ width: 180, fontSize: 13 }}
          />
          <Button onClick={() => openCreateGroupRef.current?.()}>
            + {t("group.add", "添加分组")}
          </Button>
          <Button onClick={() => { resetForm(); setShowForm(true); }}>
            + {t("platform.add")}
          </Button>
          <Button
            variant="ghost"
            onClick={() => setPurgeConfirmOpen(true)}
            title={t("platform.purgeDisabled", "清理失效平台")}
          >
            {t("platform.purgeDisabled", "清理失效平台")}
          </Button>
        </div>
      </div>

      {/* 分组段（内嵌） */}
      <GroupsEmbedded onNavigate={onNavigate} onGroupsChanged={handleGroupsChanged} onPlatformDeleted={(ids: number[]) => s.removePlatformsByIds(ids)} onCreatePlatform={openCreatePlatform} onEditPlatform={handleEdit} onDuplicatePlatform={handleDuplicate} onToast={setToast} onViewModeChange={setGroupFullscreen} openCreateGroupRef={openCreateGroupRef} reloadRef={s.groupsReloadRef} onCountChange={setProgressiveCount} searchQuery={searchQuery} />

      {/* 全屏视图态（创建/编辑分组）时隐藏分隔线 + 未分组平台列表，避免与全屏视图并列 */}
      {!groupFullscreen && (<>
      {/* 分隔线 */}
      <div style={{ height: 1, background: "var(--border)", margin: "0 0 10px 0" }} />

      {/* Platform List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div ref={platListRef} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("platform.empty")}</div>
            </div>
          )}
          {standalonePlatforms.map((p, i) => {
            const isDragging = platDrag?.from === i;
            const draggedPlat = platDrag ? standalonePlatforms[platDrag.from] : null;
            const draggedColor = draggedPlat ? (colorMap[draggedPlat.platform_type] || "var(--accent)") : "";
            return (
              <React.Fragment key={p.id}>
                {/* Ghost card at insertion point */}
                {platDrag && platDrag.to === i && draggedPlat && (
                  <div style={{
                    display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                    padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                    background: "var(--glass-bg, rgba(255,255,255,0.06))",
                    border: "1.5px dashed var(--accent)",
                    opacity: 0.5, filter: "grayscale(0.8)",
                    pointerEvents: "none", transition: "all 150ms ease",
                  }}>
                    <div style={{ width: 10, height: 10, borderRadius: "50%", background: draggedColor, flexShrink: 0 }} />
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{draggedPlat.name}</span>
                    <Badge variant="secondary" className="badge-muted" style={{ fontSize: 10 }}>{labelMap[draggedPlat.platform_type] || draggedPlat.platform_type}</Badge>
                  </div>
                )}
                {/* 未分组平台 pointer 拖拽加入分组（按住卡片空白区拖到分组）；HTML5 DnD 跨区域在 WKWebView 失效故用 pointer events */}
                <div
                  onPointerDown={(e) => onStandaloneGroupPointerDown(e, p)}
                  onPointerMove={onStandaloneGroupPointerMove}
                  onPointerUp={onStandaloneGroupPointerUp}
                  style={{ cursor: groupDrag?.pid === p.id ? "grabbing" : undefined }}
                >
                <PlatformCard
                  platform={p}
                  index={i}
                  isDragging={isDragging}
                  dragActive={!!platDrag}
                  quotaRaw={quota.quotaMap[p.id]}
                  quotaPreferReal={!!quota.quotaRealIds[p.id]}
                  refreshing={!!quota.quotaRefreshing[p.id]}
                  quotaPending={!!quota.quotaPending[p.id]}
                  usagePending={usageLoading && !usageMap[p.id]}
                  usage={usageMap[p.id]}
                  expanded={expandedIds.has(p.id)}
                  manualResult={testResults[p.id]}
                  testing={testingId === p.id}
                  faviconFailed={faviconFailed.has(p.id)}
                  actions={cardActions}
                  platformMembership={platformMembership.get(p.id)}
                  lastTest={lastTestMap[p.id]}
                />
                </div>
              </React.Fragment>
            );
          })}
          {platDrag && (() => {
            if (platDrag.to !== standalonePlatforms.length) return null;
            const dp = standalonePlatforms[platDrag.from];
            const dc = colorMap[dp.platform_type] || "var(--accent)";
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                background: "var(--glass-bg, rgba(255,255,255,0.06))",
                border: "1.5px dashed var(--accent)",
                opacity: 0.5, filter: "grayscale(0.8)",
                pointerEvents: "none", transition: "all 150ms ease",
              }}>
                <div style={{ width: 10, height: 10, borderRadius: "50%", background: dc, flexShrink: 0 }} />
                <span style={{ fontSize: 13, fontWeight: 600 }}>{dp.name}</span>
                <Badge variant="secondary" className="badge-muted" style={{ fontSize: 10 }}>{labelMap[dp.platform_type] || dp.platform_type}</Badge>
              </div>
            );
          })()}
        </div>
      )}
      </>)}
    </div>

      {/* Custom test overlay — ModelTestPanel 自带 overlay 且经 createPortal 挂 body, 此处不再包外层遮罩。 */}
      {testingPlatform !== null && (
        <ModelTestPanel
          platform={testingPlatform as Platform}
          onClose={() => setTestingPlatform(null)}
          onResult={(success) => { if (testingPlatform) setTestResults(prev => ({ ...prev, [testingPlatform.id]: success ? "ok" : "fail" })); }}
        />
      )}

      {/* Test result toast — Portal 到 body, 脱离页面 transform 祖先(animate-fade-in 等)确保 fixed 相对窗口顶部 */}
      {groupDrag && createPortal(
        <div style={{
          position: "fixed", left: groupDrag.x + 14, top: groupDrag.y + 14,
          pointerEvents: "none", zIndex: 3000,
          padding: "6px 12px", borderRadius: 8,
          background: "var(--accent)", color: "#fff",
          fontSize: 12, fontWeight: 600,
          boxShadow: "0 4px 12px rgba(0,0,0,0.35)", opacity: 0.92,
        }}>
          {groupDrag.pname}
        </div>,
        document.body,
      )}
      {shareData && (
        <ShareModal
          share={shareData.share}
          title={shareData.name}
          urlScheme="aidog://platform/import"
          onToast={(text, ok) => { setToast({ text, ok }); setTimeout(() => setToast(null), 3000); }}
          onClose={() => setShareData(null)}
        />
      )}
      {toast && createPortal(
        <div style={{
          position: "fixed", top: 24, left: "50%", transform: "translateX(-50%)",
          zIndex: 2000, pointerEvents: "none",
          padding: "10px 20px", borderRadius: 10,
          background: toast.ok ? "var(--color-success, #22c55e)" : "var(--color-danger, #ef4444)",
          color: "#fff", fontSize: 13, fontWeight: 600,
          boxShadow: "0 4px 20px rgba(0,0,0,0.25)",
          opacity: 0.95,
          transition: "opacity 0.3s",
        }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>{toast.ok ? <IconCheck size={14} color="#fff" /> : <IconClose size={14} color="#fff" />} {toast.text}</span>
        </div>,
        document.body,
      )}
      {purgeConfirmOpen && (
        <AlertDialog open={purgeConfirmOpen} onOpenChange={(next) => { if (!next && !purging) setPurgeConfirmOpen(false); }}>
          <AlertDialogContent className="glass-elevated" style={{ maxWidth: 440, padding: "20px 22px" }}
            onEscapeKeyDown={(e) => { if (purging) e.preventDefault(); }}>
            <AlertDialogHeader>
              <AlertDialogTitle>{t("platform.purgeDisabled", "清理失效平台")}</AlertDialogTitle>
              <AlertDialogDescription>
                {t("platform.purgeDisabledConfirm", "将永久删除所有自动禁用态平台，此操作不可撤销。")}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={purging}>{t("action.cancel", "取消")}</AlertDialogCancel>
              <AlertDialogAction disabled={purging} onClick={onPurgeConfirm}>
                {purging ? t("status.loading", "处理中…") : t("action.confirm", "确认")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </>
  );
}
