import { IconClose } from "../../components/icons";
import { FilterDropdown } from "../../components/shared";
import { F } from "../../domains/shared/tokens";
import { LogRow, Pagination, FilterSelect, ThCell } from "./primitives";
import { NO_GROUP_SENTINEL, type TimePreset } from "./types";
import type { LogsData } from "./useLogsData";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TableHeader, TableBody } from "@/components/ui/table";
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

/**
 * 日志列表视图（自原 Logs.tsx L455-637 外迁）。
 * header + 筛选条 + 表格 + 分页；详情由 Logs.tsx 以 Sheet 叠加（本组件恒常渲染）。
 */
export function ListView({ d }: { d: LogsData }) {
  const {
    t, logs, total, offset, pageSize, loading, load, setOffset, setPageSize,
    platforms, groups, filterPlatform, filterGroup, filterStatus, filterTime,
    filterModelType, filterModelText, filterPath,
    setFilterPlatform, setFilterGroup, setFilterStatus, setFilterTime,
    setFilterModelType, setFilterModelText, setFilterPath,
    modelOptions, hasFilter, clearFilter, handleClear, handleCleanupExpired,
    showClearConfirm, setShowClearConfirm, cleanupMessage,
    openDetail, copyRow, platformMap, groupName,
  } = d;

  const totalPages = Math.ceil(total / pageSize);
  const currentPage = Math.floor(offset / pageSize) + 1;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.logs", "请求日志")}</div>
          <div className="section-desc">
            {total > 0 ? `${total} ${t("logs.total", "条记录")}` : t("logs.empty", "暂无日志")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {cleanupMessage && (
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{cleanupMessage}</span>
          )}
          <Button variant="default" onClick={() => load()} disabled={loading} style={{ fontSize: F.hint, height: "auto", padding: "4px 10px" }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
          </Button>
          {total > 0 && (
            <>
              <Button variant="default" onClick={handleCleanupExpired} style={{ fontSize: F.hint, height: "auto", padding: "4px 10px" }}>
                {t("logs.cleanupExpired", "清理过期")}
              </Button>
              <Button variant="destructive" onClick={() => setShowClearConfirm(true)} style={{ fontSize: F.hint, height: "auto", padding: "4px 10px" }}>
                {t("logs.clear", "清除全部")}
              </Button>
            </>
          )}
        </div>
      </div>

      {/* ── Filter bar ── */}
      <div className="glass-surface" style={{ padding: "12px 16", display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" }}>
        {/* Platform */}
        <FilterDropdown
          width={140}
          value={filterPlatform}
          onChange={setFilterPlatform}
          options={[
            ...platforms.map(p => ({ value: String(p.id), label: p.name })),
            // 隧道请求 host 未命中任何平台 → platform_id=0
            { value: "0", label: t("logs.noPlatform", "无平台") },
          ]}
          allLabel={t("logs.filterPlatform", "平台")}
          searchPlaceholder={t("stats.searchPlatform", "搜索平台")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
        />
        {/* Group */}
        <FilterDropdown
          width={140}
          value={filterGroup}
          onChange={setFilterGroup}
          options={[
            ...groups.map(g => ({ value: g.group.group_key, label: g.group.name })),
            // 隧道请求无 apikey → group_key=''（sentinel 映射见 activeFilter）
            { value: NO_GROUP_SENTINEL, label: t("logs.noGroup", "无分组") },
          ]}
          allLabel={t("logs.filterGroup", "分组")}
          searchPlaceholder={t("stats.searchGroup", "搜索分组")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
        />
        {/* Status */}
        <FilterSelect
          value={filterStatus}
          onChange={setFilterStatus}
          options={[
            { value: "success", label: t("logs.statusSuccess", "成功") },
            { value: "error", label: t("logs.statusError", "失败") },
          ]}
          placeholder={t("logs.filterStatus", "状态")}
        />
        {/* Time range */}
        <FilterSelect
          value={filterTime}
          onChange={v => setFilterTime(v as TimePreset)}
          options={[
            { value: "1h", label: "1h" },
            { value: "6h", label: "6h" },
            { value: "24h", label: "24h" },
            { value: "7d", label: "7d" },
            { value: "30d", label: "30d" },
          ]}
          placeholder={t("logs.filterTime", "时间")}
        />
        {/* Model type toggle */}
        <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: F.small }}>
          <Button
            variant="ghost"
            type="button"
            style={{ padding: "2px 8px", fontSize: F.small, height: "auto", fontWeight: filterModelType === "actual" ? 700 : 400, opacity: filterModelType === "actual" ? 1 : 0.6 }}
            onClick={() => setFilterModelType("actual")}
          >{t("logs.actualModel", "实际模型")}</Button>
          <Button
            variant="ghost"
            type="button"
            style={{ padding: "2px 8px", fontSize: F.small, height: "auto", fontWeight: filterModelType === "original" ? 700 : 400, opacity: filterModelType === "original" ? 1 : 0.6 }}
            onClick={() => setFilterModelType("original")}
          >{t("logs.model", "原始模型")}</Button>
        </div>
        {/* Model dropdown — options from unfiltered query */}
        <FilterDropdown
          width={170}
          value={filterModelText}
          onChange={setFilterModelText}
          options={modelOptions.map(m => ({ value: m, label: m }))}
          allLabel={t("logs.filterModel", "模型")}
          searchPlaceholder={t("stats.searchModel", "搜索模型")}
          emptyLabel={t("stats.noMatch", "无匹配结果")}
        />
        {/* Path search — LIKE match on request_url */}
        <Input
          type="text"
          value={filterPath}
          onChange={e => setFilterPath(e.target.value)}
          placeholder={t("logs.filterPath", "搜索路径（如 /v1/messages）")}
          style={{ fontSize: F.small, height: 30, maxWidth: 180, minWidth: 120 }}
        />
        {/* Clear */}
        {hasFilter && (
          <Button variant="ghost" type="button" onClick={clearFilter} style={{ fontSize: F.small, height: "auto", padding: "2px 8px", color: "var(--text-tertiary)" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}><IconClose size={11} /> {t("logs.clearFilter", "清除")}</span>
          </Button>
        )}
      </div>

      {/* Log Table */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : logs.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("logs.empty")}</div>
        </div>
      ) : (
        <>
          {/* ponytail: contain:paint 隔离滚动重绘范围，表格行多时减少合成层影响面（glass-surface 本身无 backdrop-filter，不含 GPU 合成叠加） */}
          <div className="glass-surface" style={{ overflow: "auto", contain: "paint" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
              <TableHeader>
                <ThCell>{t("logs.time")}</ThCell>
                <ThCell>{t("logs.group")}</ThCell>
                <ThCell>{t("logs.platform", "平台")}</ThCell>
                <ThCell>{t("logs.model", "原始模型")}</ThCell>
                <ThCell>{t("logs.actualModel", "实际模型")}</ThCell>
                <ThCell>{t("logs.status")}</ThCell>
                <ThCell>{t("logs.duration")}</ThCell>
                <ThCell>{t("logs.inputTokens")}</ThCell>
                <ThCell>{t("logs.outputTokens")}</ThCell>
                <ThCell sticky>{""}</ThCell>
              </TableHeader>
              <TableBody>
                {logs.map((log) => (
                  <LogRow
                    key={log.id}
                    log={log}
                    platformName={platformMap.get(log.platform_id) || "-"}
                    groupName={groupName(log.group_key)}
                    onOpen={openDetail}
                    onCopy={copyRow}
                    t={t}
                  />
                ))}
              </TableBody>
            </table>
          </div>

          {/* Pagination */}
          {total > 0 && (
            <Pagination
              currentPage={currentPage}
              totalPages={totalPages}
              total={total}
              pageSize={pageSize}
              onPageChange={page => setOffset((page - 1) * pageSize)}
              onPageSizeChange={setPageSize}
              t={t}
            />
          )}
        </>
      )}

      {/* 清空确认弹窗 — AlertDialog（Radix Portal，替代手工 createPortal）。
          原实现禁 window.confirm（破坏 Tauri）；AlertDialog 自带 Portal 居中，绕开祖先 transform/backdrop-filter 退化。 */}
      <AlertDialog open={showClearConfirm} onOpenChange={setShowClearConfirm}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 380, padding: 20 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 13, fontWeight: 600 }}>
              {t("logs.clearConfirmTitle", "清空全部日志")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("logs.clearConfirm", "确认清除所有日志？此操作不可撤销。")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ padding: "6px 14px", fontSize: 12 }}>
              {t("logs.cancel", "取消")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={handleClear}
              style={{ padding: "6px 14px", fontSize: 12, background: "var(--color-error, #ef4444)" }}
            >
              {t("logs.clear", "清除全部")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
