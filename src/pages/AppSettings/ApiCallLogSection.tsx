import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelApi, type ApiCallLog } from "../../services/api";
import { formatDateTime, formatDurationMs } from "../../utils/formatters";

/** 最近的管理接口调用记录（只含元数据，不含请求与响应正文，保留 7 天）。 */
export function ApiCallLogSection({ refreshKey = 0 }: { refreshKey?: number }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<ApiCallLog[] | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(() => {
    setFailed(false);
    kernelApi
      .callLog(100)
      .then(setRows)
      .catch(() => {
        setRows([]);
        setFailed(true);
      });
  }, []);

  useEffect(load, [load, refreshKey]);

  const state = rows === null ? "loading" : failed ? "error" : rows.length === 0 ? "empty" : "ready";

  return (
    <div className="glass-surface" data-testid="api-call-log" data-state={state} style={{ padding: "16px 20px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("apiCallLog.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("apiCallLog.desc")}
          </div>
        </div>
        <button type="button" data-testid="api-call-log-refresh" onClick={load} style={{ fontSize: 12 }}>
          {t("apiCallLog.refresh")}
        </button>
      </div>
      {failed && (
        <div className="text-secondary" data-testid="api-call-log-error" style={{ fontSize: 12, marginTop: 10 }}>
          {t("apiCallLog.loadFailed")}
        </div>
      )}
      {state === "empty" && (
        <div className="text-secondary" data-testid="api-call-log-empty" style={{ fontSize: 12, marginTop: 10 }}>
          {t("apiCallLog.empty")}
        </div>
      )}
      {state === "ready" && rows && (
        <table style={{ width: "100%", marginTop: 10, fontSize: 12, borderCollapse: "collapse" }}>
          <thead>
            <tr className="text-secondary" style={{ textAlign: "start" }}>
              <th>{t("apiCallLog.colTime")}</th>
              <th>{t("apiCallLog.colCommand")}</th>
              <th>{t("apiCallLog.colResult")}</th>
              <th>{t("apiCallLog.colDuration")}</th>
              <th>{t("apiCallLog.colUserAgent")}</th>
              <th>{t("apiCallLog.colSource")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.id}
                data-testid={`api-call-log-row-${r.id}`}
                data-state={r.success ? "ok" : "failed"}
                data-source={r.source}
              >
                <td>{formatDateTime(r.created_at)}</td>
                <td><code>{r.command}</code></td>
                <td>{`${r.status_code} ${r.success ? t("apiCallLog.resultOk") : t("apiCallLog.resultFailed")}`}</td>
                <td>{formatDurationMs(r.duration_ms)}</td>
                <td style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={r.user_agent}>
                  {r.user_agent || "-"}
                </td>
                <td>{r.source === "settings_test" ? t("apiCallLog.sourceSettingsTest") : t("apiCallLog.sourceThirdParty")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
