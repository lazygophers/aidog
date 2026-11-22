// 设置 › 客户端会话读取：四个客户端读取开关 + 保留天数 + 「允许读取对话原文」开关。
// 读取开关由客户端统计页迁来（扩展-会话页 票 01）；开关即时写入，失败回滚并报错。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/button";
import { Card } from "../ui/card";
import { Input } from "../ui/input";
import { Switch } from "../ui/switch";
import { clientUsageApi, type ClientUsageConsent } from "../../services/api/clientUsage";

// 显示名为品牌专名不译。
export const USAGE_CLIENTS = [
  { code: "claude_code", name: "Claude Code" },
  { code: "codex", name: "Codex" },
  { code: "gemini_cli", name: "Gemini CLI" },
  { code: "opencode", name: "OpenCode" },
] as const;

export function ClientUsageSettings() {
  const { t } = useTranslation();
  const [consent, setConsent] = useState<ClientUsageConsent>({ asked: false, enabled: {} });
  const [transcript, setTranscript] = useState(false);
  const [days, setDays] = useState("");
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;
    clientUsageApi.consentGet().then((c) => { if (!cancelled) setConsent(c); }).catch((e) => setError(String(e)));
    clientUsageApi.retentionGet().then((d) => { if (!cancelled) setDays(String(d)); }).catch((e) => setError(String(e)));
    clientUsageApi.transcriptConsentGet().then((v) => { if (!cancelled) setTranscript(v); }).catch((e) => setError(String(e)));
    return () => { cancelled = true; };
  }, []);

  const toggleClient = async (code: string, on: boolean) => {
    setError("");
    const prev = consent;
    const next: ClientUsageConsent = { asked: true, enabled: { ...consent.enabled, [code]: on } };
    setConsent(next);
    try {
      await clientUsageApi.consentSet(next);
    } catch (e) {
      setConsent(prev);
      setError(String(e));
    }
  };

  const toggleTranscript = async (on: boolean) => {
    setError("");
    setTranscript(on);
    try {
      await clientUsageApi.transcriptConsentSet(on);
    } catch (e) {
      setTranscript(!on);
      setError(String(e));
    }
  };

  const saveRetention = async () => {
    setSaved(false);
    const n = Number(days);
    if (days.trim() === "" || !Number.isInteger(n) || n < 0) return;
    setError("");
    try {
      await clientUsageApi.retentionSet(n);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex flex-col gap-5" data-testid="client-usage-settings">
      <Card className="p-5 space-y-4">
        <div>
          <h2 className="font-medium">{t("clientUsage.consentTitle", "Read Authorization")}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.consentDesc", "Choose which clients to track. When enabled, the client's local session files are read and imported into the statistics.")}</p>
        </div>
        <div className="space-y-3">
          {USAGE_CLIENTS.map((c) => (
            <label key={c.code} className="flex items-center justify-between rounded-lg border p-3">
              <span className="text-sm">{c.name}</span>
              <Switch
                checked={consent.enabled[c.code] ?? false}
                onCheckedChange={(on) => void toggleClient(c.code, on)}
                aria-label={c.name}
                data-testid={`client-switch-${c.code}`}
              />
            </label>
          ))}
        </div>
      </Card>

      <Card className="p-5 space-y-3">
        <div>
          <h2 className="font-medium">{t("clientUsage.retentionTitle", "Retention days")}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.retentionDesc", "Call details older than this are cleaned up. 0 = keep forever; default 90 days.")}</p>
        </div>
        <div className="flex items-center gap-2">
          <Input
            data-testid="retention-days"
            type="number"
            min={0}
            value={days}
            className="w-24"
            aria-label={t("clientUsage.retentionTitle", "Retention days")}
            onChange={(e) => { setDays(e.target.value); setSaved(false); }}
          />
          <Button data-testid="retention-save" variant="outline" onClick={() => void saveRetention()}>
            {t("toolLog.save", "Save")}
          </Button>
          {saved && <span className="text-sm text-muted-foreground" data-testid="retention-saved">{t("toolLog.saved", "Saved")}</span>}
        </div>
      </Card>

      <Card className="p-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="font-medium">{t("clientUsage.transcriptTitle", "Allow reading conversation transcripts")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.transcriptDesc", "Transcripts are read only when you continue a session in another client, and are written to that client's own storage. They are never stored in the AiDog database. Off by default.")}</p>
          </div>
          <Switch
            checked={transcript}
            onCheckedChange={(on) => void toggleTranscript(on)}
            aria-label={t("clientUsage.transcriptTitle", "Allow reading conversation transcripts")}
            data-testid="transcript-consent"
          />
        </div>
      </Card>

      {error && <p role="alert" className="text-sm text-destructive" data-testid="client-usage-settings-error">{error}</p>}
    </div>
  );
}
