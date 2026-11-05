import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelApi } from "../../services/api";
import type { ApiTestResult } from "../../services/api/types";
import catalogJson from "../../generated/apiCatalog.json";
import { formatDurationMs } from "../../utils/formatters";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { paramsSkeleton } from "./apiEndpoints";

interface Cmd { name: string; kind: "readonly" | "mutating" | null; desktopOnly: boolean; params: { name: string; type: string }[] }
const COMMANDS = (catalogJson.commands as Cmd[]).filter((c) => !c.desktopOnly);

type Outcome = { kind: "ok" | "error"; res: ApiTestResult } | { kind: "proxy-down" } | { kind: "failed"; msg: string };

/** 「API」分页测试面板：真实 HTTP 经代理端口打管理接口；会改数据的命令先确认。 */
export function ApiTestPanel({ refreshKey = 0, onSent }: { refreshKey?: number; onSent?: () => void }) {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [command, setCommand] = useState(COMMANDS[0]?.name ?? "");
  const [params, setParams] = useState(() => (COMMANDS[0] ? paramsSkeleton(COMMANDS[0].params) : "{}"));
  const [confirming, setConfirming] = useState(false);
  const [sending, setSending] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);

  useEffect(() => {
    kernelApi.getSettings().then((s) => setEnabled(s.api_enabled)).catch(() => setEnabled(true));
  }, [refreshKey]);

  const cmd = COMMANDS.find((c) => c.name === command);

  const onPick = (name: string) => {
    setCommand(name);
    const c = COMMANDS.find((x) => x.name === name);
    setParams(c ? paramsSkeleton(c.params) : "{}");
  };

  const send = async () => {
    setConfirming(false);
    setSending(true);
    try {
      const res = await kernelApi.testSend(command, params);
      setOutcome({ kind: res.status >= 200 && res.status < 300 ? "ok" : "error", res });
    } catch (e) {
      const msg = typeof e === "string" ? e : String(e);
      setOutcome(msg === "proxy_unreachable" ? { kind: "proxy-down" } : { kind: "failed", msg });
    } finally {
      setSending(false);
      onSent?.();
    }
  };

  const state = enabled === false ? "disabled" : enabled === null ? "loading" : "ready";

  return (
    <div className="glass-surface" data-testid="api-test" data-state={state} style={{ padding: "16px 20px" }}>
      <div style={{ fontSize: 13, fontWeight: 600 }}>{t("apiTest.title")}</div>
      <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t("apiTest.desc")}</div>
      {enabled === false && (
        <div data-testid="api-test-disabled-hint" style={{ fontSize: 12, marginTop: 10 }}>{t("apiTest.disabledHint")}</div>
      )}
      <div style={{ display: "flex", gap: 8, marginTop: 10, alignItems: "center" }}>
        <select data-testid="api-test-command" value={command} onChange={(e) => onPick(e.target.value)} style={{ fontSize: 12, flex: 1 }}>
          {COMMANDS.map((c) => <option key={c.name} value={c.name}>{c.name}</option>)}
        </select>
        <span className="text-secondary" data-testid="api-test-kind" data-kind={cmd?.kind ?? ""} style={{ fontSize: 12 }}>
          {cmd?.kind === "mutating" ? t("apiCatalog.mutating") : t("apiCatalog.readonly")}
        </span>
        <Button
          type="button"
          data-testid="api-test-send"
          disabled={enabled !== true || sending || !cmd}
          onClick={() => (cmd?.kind === "mutating" ? setConfirming(true) : void send())}
        >
          {t("apiTest.send")}
        </Button>
      </div>
      <textarea
        data-testid="api-test-params"
        value={params}
        onChange={(e) => setParams(e.target.value)}
        rows={4}
        spellCheck={false}
        style={{ width: "100%", marginTop: 8, fontSize: 12, fontFamily: "monospace" }}
      />
      {outcome && (
        <div data-testid="api-test-result" data-state={outcome.kind} style={{ marginTop: 10, fontSize: 12 }}>
          {outcome.kind === "proxy-down" && <div data-testid="api-test-proxy-down">{t("apiTest.proxyDown")}</div>}
          {outcome.kind === "failed" && <div data-testid="api-test-failed">{outcome.msg}</div>}
          {(outcome.kind === "ok" || outcome.kind === "error") && (
            <>
              <div>
                <span data-testid="api-test-status">{outcome.res.status}</span>
                {" · "}
                <span data-testid="api-test-duration">{formatDurationMs(outcome.res.duration_ms)}</span>
              </div>
              <pre data-testid="api-test-body" style={{ whiteSpace: "pre-wrap", wordBreak: "break-all", margin: "4px 0 0" }}>
                {outcome.res.body}
              </pre>
            </>
          )}
        </div>
      )}
      <Dialog open={confirming} onOpenChange={(o) => !o && setConfirming(false)}>
        <DialogContent data-testid="api-test-confirm">
          <DialogHeader>
            <DialogTitle>{t("apiTest.confirmTitle")}</DialogTitle>
            <DialogDescription>{t("apiTest.confirmDesc")}</DialogDescription>
          </DialogHeader>
          <code style={{ fontSize: 12, fontWeight: 600 }}>{command}</code>
          <pre style={{ whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: 12, margin: 0 }}>{params}</pre>
          <DialogFooter>
            <Button type="button" variant="outline" data-testid="api-test-confirm-cancel" onClick={() => setConfirming(false)}>
              {t("apiTest.cancel")}
            </Button>
            <Button type="button" data-testid="api-test-confirm-ok" onClick={() => void send()}>
              {t("apiTest.confirmSend")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
