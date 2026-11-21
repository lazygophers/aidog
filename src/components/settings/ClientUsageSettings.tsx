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
          <h2 className="font-medium">{t("clientUsage.consentTitle", "读取授权")}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.consentDesc", "选择要统计的客户端。开启后读取对应客户端本地的会话文件并导入统计。")}</p>
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
          <h2 className="font-medium">{t("clientUsage.retentionTitle", "保留天数")}</h2>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.retentionDesc", "超过天数的调用明细会被清理；0 = 永久保留，缺省 90 天。")}</p>
        </div>
        <div className="flex items-center gap-2">
          <Input
            data-testid="retention-days"
            type="number"
            min={0}
            value={days}
            className="w-24"
            aria-label={t("clientUsage.retentionTitle", "保留天数")}
            onChange={(e) => { setDays(e.target.value); setSaved(false); }}
          />
          <Button data-testid="retention-save" variant="outline" onClick={() => void saveRetention()}>
            {t("toolLog.save", "保存")}
          </Button>
          {saved && <span className="text-sm text-muted-foreground" data-testid="retention-saved">{t("toolLog.saved", "已保存")}</span>}
        </div>
      </Card>

      <Card className="p-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="font-medium">{t("clientUsage.transcriptTitle", "允许读取对话原文")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.transcriptDesc", "仅在「换客户端继续」时读取会话原文并写入目标客户端；原文不存入 AiDog 数据库。默认关闭。")}</p>
          </div>
          <Switch
            checked={transcript}
            onCheckedChange={(on) => void toggleTranscript(on)}
            aria-label={t("clientUsage.transcriptTitle", "允许读取对话原文")}
            data-testid="transcript-consent"
          />
        </div>
      </Card>

      {error && <p role="alert" className="text-sm text-destructive" data-testid="client-usage-settings-error">{error}</p>}
    </div>
  );
}
