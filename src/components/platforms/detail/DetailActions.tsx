// 快捷操作板块 I：复用现有 handler；拉模型通过现有 platformApi.fetchModels + 拉取选择器写回回调。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Platform } from "../../../services/api";
import { platformApi } from "../../../services/api";
import { Button } from "@/components/ui/button";
import { Blk } from "./Blk";
import type { PlatformCardService } from "../PlatformCard";

export function DetailActions({ platform: p, onRefreshQuota, service }: {
  platform: Platform;
  onRefreshQuota?: (p: Platform) => void;
  service: PlatformCardService;
}) {
  const { t } = useTranslation();
  const [fetching, setFetching] = useState(false);
  const [fetchErr, setFetchErr] = useState("");
  const fetchModels = async () => {
    if (fetching) return;
    setFetching(true);
    setFetchErr("");
    try {
      const endpoint = p.endpoints?.[0];
      const protocol = endpoint?.protocol ?? p.platform_type;
      const baseUrl = endpoint?.base_url ?? p.base_url;
      await platformApi.fetchModels(protocol, baseUrl, p.api_key);
    } catch (e) {
      setFetchErr((e as Error)?.message ?? String(e));
    } finally {
      setFetching(false);
    }
  };

  return (
    <Blk title={t("platform.detail.actions", "Quick actions")}>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <Button data-testid="detail-act-test" variant="outline" size="sm" onClick={() => service.onQuickTest(p)}>
          {t("platform.quickTest", "Quick Test")}
        </Button>
        <Button data-testid="detail-act-quota" variant="outline" size="sm" onClick={() => onRefreshQuota?.(p)}>
          {t("platform.quotaRefresh", "Refresh quota")}
        </Button>
        <Button data-testid="detail-act-fetch" variant="outline" size="sm" disabled={fetching} onClick={fetchModels}>
          {fetching ? t("status.loading", "Loading...") : t("platform.fetchModels", "Fetch Models")}
        </Button>
        <Button data-testid="detail-act-logs" variant="outline" size="sm" onClick={() => service.onViewLogs(p)}>
          {t("platform.viewLogs", "View Logs")}
        </Button>
        <Button data-testid="detail-act-edit" variant="outline" size="sm" onClick={() => service.onEdit(p)}>
          {t("platform.editHint", "Edit platform")}
        </Button>
      </div>
      {fetchErr && (
        <div data-testid="detail-fetch-error" className="text-secondary" style={{ marginTop: 8, fontSize: 14, color: "var(--color-danger)", wordBreak: "break-all" }}>
          {fetchErr}
        </div>
      )}
    </Blk>
  );
}
