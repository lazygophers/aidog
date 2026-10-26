// GroupConsoleList — G3 设计稿「整体风格与分组页」分组独立页（.scratch/重设计-终稿/6-整体风格与分组页.html）。
// pgtop 工具栏（分组 h1 + N 分组 · M 关联 + 清理失效 + ＋新建分组）+ grp-card 紧凑组卡列表
// （nm+★默认 / routing·N 平台 / 今 tokens · $费用 / hp 健康条）+ newgrp 虚线新建卡。
// 点组卡进观测台三栏详情页（Groups.tsx detailTarget 分支，本组件只发 onOpenDetail）。
import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import type { GroupDetail, PlatformUsageStats, PurgeCandidate } from "../../services/api";
import { platformApi } from "../../services/api";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../../utils/formatters";
import { CopyButton } from "../../components/shared";
import { Button } from "@/components/ui/button";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";

export interface GroupConsoleListProps {
  details: GroupDetail[];
  groupStats: Record<string, PlatformUsageStats>;
  loading: boolean;
  t: TFunction;
  proxyBaseUrl: string;
  onOpenDetail: (id: number) => void;
  onCreateGroup: () => void;
  /** 清理失效：逐组执行（独占永久删 / 共享移出本组），确认弹窗列全局 preview。 */
  onPurgeAllGroups: () => Promise<void>;
}

export function GroupConsoleList({ details, groupStats, loading, t, proxyBaseUrl, onOpenDetail, onCreateGroup, onPurgeAllGroups }: GroupConsoleListProps) {
  // 全局清理失效（稿 pgtop「清理失效」）：preview 一次，确认后逐组执行。
  const [purgeOpen, setPurgeOpen] = useState(false);
  const [purging, setPurging] = useState(false);
  const [previewList, setPreviewList] = useState<PurgeCandidate[] | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  useEffect(() => {
    if (!purgeOpen) { setPreviewList(null); return; }
    let cancelled = false;
    setPreviewLoading(true);
    platformApi.purgeDisabledPreview().then((r) => { if (!cancelled) setPreviewList(r); })
      .catch(() => { if (!cancelled) setPreviewList([]); })
      .finally(() => { if (!cancelled) setPreviewLoading(false); });
    return () => { cancelled = true; };
  }, [purgeOpen]);

  const totalLinks = details.reduce((n, d) => n + d.platforms.length, 0);
  const seclab: React.CSSProperties = {
    fontSize: 10, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)",
    letterSpacing: "1.2px", textTransform: "uppercase",
  };

  return (
    <div data-testid="groups-console" style={{ display: "flex", flexDirection: "column", width: "100%", maxWidth: 980, margin: "0 auto" }}>
      {/* pgtop（稿 .pgtop） */}
      <div data-testid="groups-console-top" style={{ display: "flex", gap: 10, alignItems: "center", padding: "12px 16px", border: "1px solid var(--border)", borderBottom: "none", borderRadius: "10px 10px 0 0", background: "var(--bg-chrome, var(--secondary))", flexWrap: "wrap" }}>
        <h1 style={{ fontSize: 15, fontWeight: 700, margin: 0 }}>{t("nav.groups", "分组")}</h1>
        <span style={{ fontSize: 11.5, color: "var(--text-tertiary)" }}>
          {details.length} {t("group.detail.countGroups", "个分组")} · {totalLinks} {t("group.detail.countLinks", "个平台关联")}
        </span>
        <div style={{ flex: 1 }} />
        <code style={{ fontSize: 12, color: "var(--text-secondary)", background: "var(--secondary)", padding: "4px 8px", borderRadius: "var(--radius-sm)", whiteSpace: "nowrap" }}>{proxyBaseUrl}</code>
        <CopyButton text={proxyBaseUrl} label={t("group.copyBaseUrl", "复制代理地址")} title={t("group.copyBaseUrlTitle", "复制代理 base_url")} />
        <Button variant="ghost" onClick={() => setPurgeOpen(true)}>{t("group.purgeDisabled", "清理失效")}</Button>
        <Button onClick={onCreateGroup}>{t("group.detail.newGroup", "＋ 新建分组")}</Button>
      </div>
      {/* pgbody 列表区（稿 .pgbody 底色渐变） */}
      <div style={{ border: "1px solid var(--border)", borderRadius: "0 0 10px 10px", padding: "14px 16px", background: "var(--background)" }}>
        {loading ? (
          <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
            <div style={{ ...seclab, marginBottom: 4 }}>Groups</div>
            {details.length === 0 && (
              <div className="text-tertiary" style={{ padding: 40, textAlign: "center", fontSize: 13 }}>{t("group.empty", "暂无分组")}</div>
            )}
            {details.map((d, i) => {
              const u = groupStats[d.group.group_key];
              const sRate = u && u.total_requests > 0 ? calcSuccessRate(u.success_count, u.total_requests) : null;
              const hpColor = sRate == null ? "var(--text-tertiary)" : sRate >= 98 ? "var(--color-success)" : sRate >= 90 ? "var(--color-warning)" : "var(--color-danger)";
              return (
                <div key={d.group.id} data-testid="group-card" data-gid={d.group.id}
                  onClick={() => onOpenDetail(d.group.id)}
                  className="ripple"
                  style={{
                    border: "1px solid var(--border)", borderRadius: 8, padding: "9px 11px",
                    cursor: "pointer", background: "var(--card)", transition: "border-color .13s",
                    animationDelay: `${i * 60}ms`,
                  }}
                  onMouseEnter={e => { (e.currentTarget as HTMLElement).style.borderColor = "var(--input)"; }}
                  onMouseLeave={e => { (e.currentTarget as HTMLElement).style.borderColor = "var(--border)"; }}
                >
                  <div style={{ fontWeight: 650, fontSize: 12.5, display: "flex", alignItems: "center", gap: 5 }}>
                    {d.group.name}
                    {d.group.is_default && <span title={t("group.isDefaultTitle", "默认分组")} style={{ color: "var(--color-warning)", fontSize: 10 }}>★</span>}
                  </div>
                  <div style={{ fontSize: 10, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)", marginTop: 3 }}>
                    {t(`group.routingMode.${d.group.routing_mode}`, d.group.routing_mode)} · {d.platforms.length} {t("group.platforms", "平台")}
                  </div>
                  <div style={{ fontSize: 10, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)", marginTop: 3 }}>
                    {t("group.detail.todayShort", "今")} {u ? formatNumber(u.today_tokens) : 0} · ${u ? formatCost(u.today_cost) : "0"}
                  </div>
                  <div title={t("group.health.title", "成功率 {{rate}}", { rate: sRate != null ? formatPercent(sRate, 1) : "—" })}
                    style={{ height: 4, borderRadius: 2, background: "var(--secondary)", marginTop: 6, overflow: "hidden" }}>
                    <div data-testid="group-card-hp" style={{ height: "100%", width: `${sRate ?? 0}%`, background: hpColor }} />
                  </div>
                </div>
              );
            })}
            {/* newgrp（稿 .newgrp 虚线新建卡） */}
            <div data-testid="group-card-new" onClick={onCreateGroup}
              style={{
                border: "1px dashed var(--input)", borderRadius: 8, padding: 8, textAlign: "center",
                fontSize: 11.5, color: "var(--text-tertiary)", cursor: "pointer", marginTop: 2,
              }}
              onMouseEnter={e => { (e.currentTarget as HTMLElement).style.color = "var(--primary)"; (e.currentTarget as HTMLElement).style.borderColor = "var(--primary)"; }}
              onMouseLeave={e => { (e.currentTarget as HTMLElement).style.color = "var(--text-tertiary)"; (e.currentTarget as HTMLElement).style.borderColor = "var(--input)"; }}
            >
              {t("group.detail.newGroup", "＋ 新建分组")}
            </div>
          </div>
        )}
      </div>

      {/* 清理失效确认（全局 preview：独占组永久删 / 共享组仅移出关联） */}
      <AlertDialog open={purgeOpen} onOpenChange={(next) => { if (!next && !purging) setPurgeOpen(false); }}>
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
                try { await onPurgeAllGroups(); } finally { setPurging(false); setPurgeOpen(false); }
              }}
            >
              {purging ? t("status.loading", "处理中…") : t("action.confirm", "确认")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
