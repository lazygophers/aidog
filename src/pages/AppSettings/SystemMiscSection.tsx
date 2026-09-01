import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

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
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.timeout", "超时设置")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("proxy.timeoutDesc", "系统默认超时，分组和模型级别可覆盖")}
        </div>
        <div style={{ display: "flex", gap: 16, alignItems: "center", marginTop: 4 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.requestTimeout", "请求超时")}
            </label>
            <Input
              type="number"
              min={0}
              value={reqTimeout}
              onChange={(e) => handleTimeoutChange(Math.max(0, Number(e.target.value)), connTimeout)}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("unit.sec", "秒")}</span>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.connectTimeout", "连接超时")}
            </label>
            <Input
              type="number"
              min={0}
              value={connTimeout}
              onChange={(e) => handleTimeoutChange(reqTimeout, Math.max(0, Number(e.target.value)))}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("unit.sec", "秒")}</span>
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.btcGlobal", "内置工具兼容总开关")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.btcGlobalDesc", "开启后所有平台转发时剔除 Claude Code 内置工具定义（ToolSearch/Read/Bash 等），用于不支持内置工具的第三方模型；默认关闭 = 请求原样转发")}
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
    dbCompacting, handleDbCompact,
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("settings.dbCompact", "立即压缩数据库")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("settings.dbCompactHint", "全量 VACUUM，期间代理请求将短暂排队")}
          </div>
        </div>
        <Button
          variant="outline"
          onClick={handleDbCompact}
          disabled={dbCompacting}
          style={{
            padding: "7px 16px", fontSize: 13,
            opacity: dbCompacting ? 0.6 : 1,
          }}
        >
          {dbCompacting ? t("common.loading", "加载中…") : t("settings.dbCompact", "立即压缩数据库")}
        </Button>
      </div>

      {/* Aggregate Stats — 聚合统计表保留与重建（与日志开关解耦：关日志也累计统计） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("stats.aggSettings", "聚合统计")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("stats.aggSettingsHint", "使用统计独立累计，不受请求日志开关影响")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
            {t("stats.aggRetention", "统计保留天数")}
          </label>
          <Input
            type="number"
            min={0}
            value={statsRetention}
            onChange={(e) => handleStatsRetentionChange(Math.max(0, Number(e.target.value)))}
            style={{ width: 70 }}
          />
          <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
            {statsRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("unit.days", "天")}
          </span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
          <div className="text-secondary" style={{ fontSize: 12 }}>
            {t("stats.rebuildHint", "从历史请求日志全量重建聚合统计表")}
          </div>
          <Button
            variant="outline"
            onClick={handleStatsRebuild}
            disabled={statsRebuilding}
            style={{
              padding: "7px 16px", fontSize: 13,
              opacity: statsRebuilding ? 0.6 : 1, whiteSpace: "nowrap",
            }}
          >
            {statsRebuilding ? t("common.loading", "加载中…") : t("stats.rebuild", "从日志重建统计")}
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("settings.autoUpdate")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("app.version")}</div>
          <div style={{
            fontSize: 13,
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            color: "var(--text-secondary)",
          }}>v{appVersion}</div>
        </div>
      )}
    </>
  );
}

