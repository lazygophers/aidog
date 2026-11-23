import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
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
import { makeRipple } from "@/components/shared";

/**
 * Timeout + DB Maintenance + Aggregate Stats + App version + toast（原 L519-560 + L680-755 + L820-837）。
 */
export function SystemMiscSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { reqTimeout, connTimeout, handleTimeoutChange, btcGlobalEnabled, handleBtcGlobalChange } = s;

  return (
    <>
      {/* Timeout */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("proxy.timeout", "Timeout Settings")}</div>
        <div className="text-secondary" style={{ fontSize: 14 }}>
          {t("proxy.timeoutDesc", "System default timeouts; group and model levels can override")}
        </div>
        <div style={{ display: "flex", gap: 16, alignItems: "center", marginTop: 4 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.requestTimeout", "Request Timeout")}
            </label>
            <Input
              type="number"
              min={0}
              value={reqTimeout}
              onChange={(e) => handleTimeoutChange(Math.max(0, Number(e.target.value)), connTimeout)}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>{t("unit.sec", "sec")}</span>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.connectTimeout", "Connect Timeout")}
            </label>
            <Input
              type="number"
              min={0}
              value={connTimeout}
              onChange={(e) => handleTimeoutChange(reqTimeout, Math.max(0, Number(e.target.value)))}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>{t("unit.sec", "sec")}</span>
          </div>
        </div>
      </div>

      {/* Builtin tool compat global switch — 全局总开关（两级 AND 的第一级，默认关闭） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        gap: 12,
      }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("proxy.btcGlobal", "Builtin tool compat master switch")}</div>
          <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
            {t("proxy.btcGlobalDesc", "When on, Claude Code builtin tool definitions (ToolSearch/Read/Bash, etc.) are stripped from every platform’s outbound request; off = forward unchanged")}
          </div>
        </div>
        <Switch checked={btcGlobalEnabled} onCheckedChange={handleBtcGlobalChange} />
      </div>
    </>
  );
}

/**
 * DB Maintenance + Aggregate Stats（原 L680-755）。
 * 与 Timeout/App version 拆为两组以便顺序编排（Timeout 先, DB/Stats 中段, App version + toast 尾）。
 */
export function DbStatsSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    dbCompacting, dbCompactConfirm, setDbCompactConfirm, requestDbCompact, runDbCompact,
    statsRetention, statsRebuilding, handleStatsRetentionChange, handleStatsRebuild,
  } = s;

  return (
    <>
      {/* DB Maintenance — 全量 VACUUM 压缩数据库（Tier 1 手动回收入口） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("settings.dbCompact", "Compact database now")}</div>
          <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
            {t("settings.dbCompactHint", "Full VACUUM; proxy requests will briefly queue during compaction")}
          </div>
        </div>
        <Button
          variant="outline"
          onClick={requestDbCompact}
          disabled={dbCompacting}
          style={{
            padding: "7px 16px", fontSize: 14,
            opacity: dbCompacting ? 0.6 : 1,
          }}
        >
          {dbCompacting ? t("common.loading", "Loading...") : t("settings.dbCompact", "Compact database now")}
        </Button>
      </div>

      {/* 压缩确认弹窗 — AlertDialog（Radix Portal，替代原生 window.confirm：破坏 Tauri，项目禁用）。 */}
      <AlertDialog open={dbCompactConfirm} onOpenChange={setDbCompactConfirm}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 380, padding: 20 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 14, fontWeight: 600 }}>
              {t("settings.dbCompact", "Compact database now")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("settings.dbCompactHint", "Full VACUUM; proxy requests will briefly queue during compaction")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ padding: "6px 14px", fontSize: 14 }}>
              {t("common.cancel", "Cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              className="ripple"
              onClick={(e) => { makeRipple(e); runDbCompact(); }}
              style={{ padding: "6px 14px", fontSize: 14 }}
            >
              {t("settings.dbCompact", "Compact database now")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Aggregate Stats — 聚合统计表保留与重建（与日志开关解耦：关日志也累计统计） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("stats.aggSettings", "Aggregate Stats")}</div>
          <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
            {t("stats.aggSettingsHint", "Usage stats are accumulated independently, unaffected by the request log toggle")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
            {t("stats.aggRetention", "Stats retention days")}
          </label>
          <Input
            type="number"
            min={0}
            value={statsRetention}
            onChange={(e) => handleStatsRetentionChange(Math.max(0, Number(e.target.value)))}
            style={{ width: 70 }}
          />
          <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
            {statsRetention === 0 ? t("proxy.logRetentionForever", "Keep forever") : t("unit.days", "days")}
          </span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
          <div className="text-secondary" style={{ fontSize: 14 }}>
            {t("stats.rebuildHint", "Fully rebuild the aggregate stats table from historical request logs")}
          </div>
          <Button
            variant="outline"
            onClick={handleStatsRebuild}
            disabled={statsRebuilding}
            style={{
              padding: "7px 16px", fontSize: 14,
              opacity: statsRebuilding ? 0.6 : 1, whiteSpace: "nowrap",
            }}
          >
            {statsRebuilding ? t("common.loading", "Loading...") : t("stats.rebuild", "Rebuild from logs")}
          </Button>
        </div>
      </div>
    </>
  );
}

/**
 * App version + toast（原 L820 + L822-837）。
 */
export function VersionToastSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { message, appVersion, autoUpdateEnabled, handleAutoUpdateChange } = s;

  return (
    <>
      {message && <div className="toast">{message}</div>}

      {/* Auto-update toggle — 关闭仅跳过启动 daily check；手动按钮仍可查。
          ponytail: toggle 紧跟版本行，复用 StartupSection 同款 toggle 组件 */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("settings.autoUpdate")}</div>
          <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
            {t("settings.autoUpdateHint")}
          </div>
        </div>
        <Switch checked={autoUpdateEnabled} onCheckedChange={handleAutoUpdateChange} />
      </div>

      {/* App version — 只读展示, 单一事实源 = tauri.conf.json (经 getVersion API) */}
      {appVersion && (
        <div className="glass-surface" style={{
          padding: "16px 20px",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("app.version")}</div>
          <div style={{
            fontSize: 14,
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            color: "var(--text-secondary)",
          }}>v{appVersion}</div>
        </div>
      )}
    </>
  );
}

