// PlatformListView — Platforms 列表态视图（showForm=false 时渲染）。
// ponytail: 从 Platforms 主组件抽出的纯展示组件。所有 state/handlers 经 props 从 usePlatformsState 传入。
//   渲染：页头（搜索 + 添加分组 + 添加平台 + 清理失效）+ GroupsEmbedded（分组段）+ 未分组平台列表 +
//   ModelTestPanel overlay + groupDrag portal + ShareModal + toast portal。
import React, { useEffect, useState } from "react";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { platformApi, type Platform, type PurgeCandidate } from "../../services/api";
import { IconClose, IconCheck } from "../../components/icons";
import { ModelTestPanel } from "../ModelTestPanel";
import { GroupsEmbedded } from "../Groups";
import type { PlatformCardService } from "../../components/platforms/PlatformCard";
import { PlatformRow, PlatformRowCols } from "./PlatformRow";
import { ShareModal } from "../../components/platforms/ShareModal";
import { useProtocolColorMap, useProtocolLabelMap } from "../../domains/platforms/useProtocolMeta";
import type { PlatformsState } from "./usePlatformsState";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

export function PlatformListView({ s, cardService, openCreateGroupRef }: {
  s: PlatformsState;
  cardService: PlatformCardService;
  openCreateGroupRef: React.MutableRefObject<(() => void) | null>;
}) {
  const { t, i18n } = useTranslation();
  // ponytail: 清理失效平台确认走 AlertDialog（禁原生 confirm，CLAUDE.md 硬规）
  const [purgeConfirmOpen, setPurgeConfirmOpen] = useState(false);
  const [purging, setPurging] = useState(false);
  // ponytail: 弹窗打开时拉一次 preview（非常驻轮询），列清单代替泛化确认文案。
  const [previewList, setPreviewList] = useState<PurgeCandidate[] | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  useEffect(() => {
    if (!purgeConfirmOpen) { setPreviewList(null); return; }
    let cancelled = false;
    setPreviewLoading(true);
    platformApi.purgeDisabledPreview().then((r) => { if (!cancelled) setPreviewList(r); })
      .catch(() => { if (!cancelled) setPreviewList([]); })
      .finally(() => { if (!cancelled) setPreviewLoading(false); });
    return () => { cancelled = true; };
  }, [purgeConfirmOpen]);
  // colorMap / labelMap：共享 hook（async 派生自 registry platform.json）；首帧空 map（fallback platform_type key）。
  const colorMap = useProtocolColorMap();
  const labelMap = useProtocolLabelMap(i18n.language);
  // ponytail: slice 化后字段拆在 list / form / drag / quota 四片，按消费就近解构。
  const {
    platforms, loading, headerActive, headerTotal,
    searchQuery, setSearchQuery,
    handleGroupsChanged, removePlatformsByIds,
    standalonePlatforms,
    expandedIds, testResults, testingId, toggleExpanded,
    lastTestMap, usageMap,
    setProgressiveCount,
    toast, setToast,
    handlePurgeDisabled,
  } = s.list;
  const {
    openCreatePlatform, handleEdit, handleDuplicate,
    resetForm, setShowForm,
    setGroupFullscreen, groupFullscreen,
    testingPlatform, setTestingPlatform,
    shareData, setShareData,
  } = s.form;
  const {
    platDrag, platListRef, groupDrag,
    onStandaloneGroupPointerDown, onStandaloneGroupPointerMove, onStandaloneGroupPointerUp,
  } = s.drag;
  const { quota, onNavigate } = s;
  // ponytail: ModelTestPanel onResult 写 list.testResults（form 经 listDeps 注入拿到同源 setter），
  //   本视图直接读 list.testResults 渲染，二者闭环；为可读性别名 setTestResults = list.setTestResults。
  const { setTestResults } = s.list;
  // 删单个平台也要先确认（2026-09-22 用户在 ask-ui 定）。
  //
  // 此前这里点了直接删，而同页其它破坏性操作（清理失效、批量删除）都有确认 ——
  // 单平台删除是唯一的例外，看着是当初漏了。删掉就得重填 base_url / key / 模型映射，
  // 一次手滑的代价远大于多点一下。Flutter 侧本来就有确认（`platforms.dart:347-354`）。
  const [deleteId, setDeleteId] = useState<number | null>(null);
  const deleteTarget = platforms.find((p) => p.id === deleteId) ?? null;

  // ponytail: purge 确认在 AlertDialog Action 触发（busy 期间禁按钮防双击）
  const onPurgeConfirm = async () => {
    setPurging(true);
    try { await handlePurgeDisabled(); } finally { setPurging(false); setPurgeConfirmOpen(false); }
  };

  return (
    <>
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header（稿 5 .top：大标题 + active 计数点 + 搜索 + 添加分组 / 添加平台 / 清理失效） */}
      <header data-testid="platforms-header" style={{ display: "flex", alignItems: "flex-end", justifyContent: "space-between", gap: 24, paddingBottom: 4, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 14 }}>
          <h1 style={{ fontSize: 21, fontWeight: 650, letterSpacing: "-0.01em", margin: 0 }}>{t("page.platforms")}</h1>
          <div data-testid="platforms-counter" style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12.5, color: "var(--text-secondary)" }}>
            <span style={{ width: 7, height: 7, borderRadius: "50%", background: "var(--color-success)", boxShadow: "0 0 6px color-mix(in srgb, var(--color-success) 70%, transparent)", display: "inline-block" }} />
            <b className="num" style={{ color: "var(--foreground)", fontWeight: 600, fontVariantNumeric: "tabular-nums" }}>{headerActive}</b>
            <span style={{ opacity: 0.6 }}>/</span>
            <span className="num" style={{ fontVariantNumeric: "tabular-nums" }}>{headerTotal}</span>
            <span>active</span>
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <Input
            className="input"
            placeholder={t("platform.searchPlaceholder", "搜索平台...")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{ width: 230, height: 32, fontSize: 12.5, borderRadius: 7 }}
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
      </header>

      {/* 分组段（内嵌） */}
      <GroupsEmbedded onNavigate={onNavigate} onGroupsChanged={handleGroupsChanged} onPlatformDeleted={(ids: number[]) => removePlatformsByIds(ids)} onCreatePlatform={openCreatePlatform} onEditPlatform={handleEdit} onDuplicatePlatform={handleDuplicate} onToast={setToast} onViewModeChange={setGroupFullscreen} openCreateGroupRef={openCreateGroupRef} reloadRef={s.groupsReloadRef} onCountChange={setProgressiveCount} searchQuery={searchQuery} />

      {/* 全屏视图态（创建/编辑分组）时隐藏分隔线 + 未分组平台列表，避免与全屏视图并列 */}
      {/* 未分组段（稿 5：分隔标题 + 列头 + 行式列表；全屏视图态隐藏） */}
      {!groupFullscreen && (<>
        <div data-testid="ungrouped-divider" style={{ display: "flex", alignItems: "center", gap: 12, margin: "26px 0 4px", color: "var(--text-tertiary)", fontSize: 12 }}>
          <span style={{ flex: 1, height: 1, background: "var(--border)" }} />
          {t("platform.ungrouped", "未分组")} · {standalonePlatforms.length} {t("group.platforms", "平台")}
          <span style={{ flex: 1, height: 1, background: "var(--border)" }} />
        </div>
        {loading ? (
          <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
        ) : (
          <div style={{ border: "1px solid var(--border)", borderRadius: 11, background: "var(--card)", overflow: "hidden", boxShadow: "var(--shadow-sm)" }}>
            {standalonePlatforms.length === 0 && (
              <div className="text-tertiary" style={{ padding: 40, textAlign: "center", fontSize: 13 }}>{t("platform.empty")}</div>
            )}
            {standalonePlatforms.length > 0 && <PlatformRowCols t={t} />}
            <div ref={platListRef} style={{ display: "flex", flexDirection: "column" }}>
              {standalonePlatforms.map((p, i) => {
                const isDragging = platDrag?.from === i;
                return (
                  <React.Fragment key={p.id}>
                    {platDrag && platDrag.to === i && standalonePlatforms[platDrag.from] && (
                      <div style={{
                        display: "flex", alignItems: "center", gap: 14,
                        padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                        background: "var(--glass-bg, rgba(255,255,255,0.06))",
                        border: "1.5px dashed var(--accent)",
                        opacity: 0.5, filter: "grayscale(0.8)",
                        pointerEvents: "none", transition: "all 150ms ease",
                      }}>
                        <div style={{ width: 10, height: 10, borderRadius: "50%", background: colorMap[standalonePlatforms[platDrag.from].platform_type] || "var(--accent)", flexShrink: 0 }} />
                        <span style={{ fontSize: 13, fontWeight: 600 }}>{standalonePlatforms[platDrag.from].name}</span>
                        <Badge variant="secondary" className="badge-muted" style={{ fontSize: 11 }}>{labelMap[standalonePlatforms[platDrag.from].platform_type] || standalonePlatforms[platDrag.from].platform_type}</Badge>
                      </div>
                    )}
                    {/* 未分组平台 pointer 拖拽加入分组（HTML5 DnD 在 WKWebView 失效故用 pointer events） */}
                    <div
                      onPointerDown={(e) => onStandaloneGroupPointerDown(e, p)}
                      onPointerMove={onStandaloneGroupPointerMove}
                      onPointerUp={onStandaloneGroupPointerUp}
                      style={{ cursor: groupDrag?.pid === p.id ? "grabbing" : undefined, opacity: isDragging ? 0.5 : undefined }}
                    >
                      <PlatformRow
                        platform={p}
                        usage={usageMap[p.id]}
                        quotaRaw={quota.quotaMap[p.id]}
                        quotaPreferReal={!!quota.quotaRealIds[p.id]}
                        lastTest={lastTestMap[p.id]}
                        manualResult={testResults[p.id]}
                        testing={testingId === p.id}
                        expanded={expandedIds.has(p.id)}
                        t={t}
                        lang={i18n.language}
                        onToggleExpanded={(next) => toggleExpanded(p.id, next)}
                        onToggle={(pp) => cardService.onToggleEnabled(pp)}
                        onEdit={(pp) => cardService.onEdit(pp)}
                        onShare={(pp) => cardService.onShare(pp)}
                        onDuplicate={(pp) => cardService.onDuplicate(pp)}
                        onViewLogs={(pp) => cardService.onViewLogs(pp)}
                        onQuickTest={(pp) => cardService.onQuickTest(pp)}
                        onDelete={(id) => setDeleteId(id)}
                        lead={
                          <span className="drag-handle drag-handle-inline" style={{ cursor: "grab", display: "inline-flex", alignItems: "center", opacity: 0.85, touchAction: "none" }}
                            title={t("group.dragPlatform", "拖拽加入分组")}>
                            <svg width="10" height="16" viewBox="0 0 10 16" fill="currentColor"><circle cx="3" cy="3" r="1.4" /><circle cx="7" cy="3" r="1.4" /><circle cx="3" cy="8" r="1.4" /><circle cx="7" cy="8" r="1.4" /><circle cx="3" cy="13" r="1.4" /><circle cx="7" cy="13" r="1.4" /></svg>
                          </span>
                        }
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
                    display: "flex", alignItems: "center", gap: 14,
                    padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                    background: "var(--glass-bg, rgba(255,255,255,0.06))",
                    border: "1.5px dashed var(--accent)",
                    opacity: 0.5, filter: "grayscale(0.8)",
                    pointerEvents: "none", transition: "all 150ms ease",
                  }}>
                    <div style={{ width: 10, height: 10, borderRadius: "50%", background: dc, flexShrink: 0 }} />
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{dp.name}</span>
                    <Badge variant="secondary" className="badge-muted" style={{ fontSize: 11 }}>{labelMap[dp.platform_type] || dp.platform_type}</Badge>
                  </div>
                );
              })()}
            </div>
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
          background: "var(--primary)", color: "var(--primary-foreground)", border: "1px solid var(--accent-edge)",
          fontSize: 12, fontWeight: 600,
          boxShadow: "0 4px 12px rgba(var(--shadow-rgb), 0.35)", opacity: 0.92,
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
          background: toast.ok ? "var(--color-success)" : "var(--color-danger)",
          color: "var(--primary-foreground)", fontSize: 13, fontWeight: 600,
          boxShadow: "0 4px 20px rgba(var(--shadow-rgb), 0.25)",
          opacity: 0.95,
          transition: "opacity 0.3s",
        }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>{toast.ok ? <IconCheck size={14} color="var(--primary-foreground)" /> : <IconClose size={14} color="var(--primary-foreground)" />} {toast.text}</span>
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
            <div style={{ maxHeight: 240, overflow: "auto", display: "flex", flexDirection: "column", gap: 6, margin: "4px 0" }}>
              {previewLoading ? (
                <div className="text-secondary" style={{ fontSize: 12, padding: "8px 0" }}>{t("status.loading", "处理中…")}</div>
              ) : !previewList || previewList.length === 0 ? (
                <div className="text-secondary" style={{ fontSize: 12, padding: "8px 0" }}>{t("platform.purgeDisabledNone", "暂无失效平台")}</div>
              ) : (
                <>
                  <div className="text-tertiary" style={{ fontSize: 11, fontWeight: 600 }}>{t("platform.purgeDisabledListTitle", "待清理平台清单")}</div>
                  {previewList.map((c) => (
                    <div key={c.id} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12 }}>
                      <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.name}</span>
                      <Badge variant="secondary" style={{ fontSize: 11, flexShrink: 0 }}>
                        {c.reason === "auth_failed" ? t("platform.purgeDisabledReasonAuthFailed", "认证失效（401/403）") : t("platform.purgeDisabledReasonExpired", "已过期")}
                      </Badge>
                    </div>
                  ))}
                </>
              )}
            </div>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={purging}>{t("action.cancel", "取消")}</AlertDialogCancel>
              <AlertDialogAction disabled={purging || previewLoading || !previewList || previewList.length === 0} onClick={onPurgeConfirm}>
                {purging ? t("status.loading", "处理中…") : t("action.confirm", "确认")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
      {/* 删单个平台的二次确认（2026-09-22 起两侧都有）。
          用 AlertDialog 不用 Dialog：点遮罩不许关，破坏性操作不接受误触。 */}
      {createPortal(
        <AlertDialog open={deleteId !== null} onOpenChange={(next) => { if (!next) setDeleteId(null); }}>
          <AlertDialogContent className="glass-elevated" style={{ maxWidth: 420, padding: "20px 22px" }}>
            <AlertDialogHeader>
              <AlertDialogTitle>{t("platform.deleteTitle", "删除平台")}</AlertDialogTitle>
              <AlertDialogDescription>
                {t("platform.deleteConfirm", "「{{name}}」将被彻底删除，配置、密钥与所有分组关联一并移除，且无法撤销。确认删除？")
                  .replace("{{name}}", deleteTarget?.name ?? "")}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{t("action.cancel", "取消")}</AlertDialogCancel>
              <AlertDialogAction
                onClick={() => {
                  const id = deleteId;
                  setDeleteId(null);
                  if (id !== null) void s.list.handleDelete(id);
                }}
              >
                {t("action.delete", "删除")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>,
        document.body,
      )}
    </>
  );
}
