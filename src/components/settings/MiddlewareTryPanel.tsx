// 「试一试」：样例文本按当前启用的全部请求侧规则依次执行（后端与转发同一引擎），只读。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { middlewareApi } from "../../services/api/scheduling";
import type { MiddlewareTryResult } from "../../services/api";

const preStyle = { margin: 0, whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: 14 } as const;

export function MiddlewareTryPanel() {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [result, setResult] = useState<MiddlewareTryResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");

  const run = async () => {
    setRunning(true);
    setError("");
    try {
      setResult(await middlewareApi.tryRun(text));
    } catch (e) {
      setError(`${t("middleware.try.failed", "Run failed")}: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="glass-surface glass-highlight" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("middleware.try.title", "Try it")}</div>
        <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
          {t("middleware.try.desc", "Enter sample text; all enabled request-side rules run in order through the same engine as forwarding. No request is sent.")}
        </div>
      </div>
      <Textarea
        data-testid="mw-try-input"
        aria-label={t("middleware.try.input", "Sample text")}
        placeholder={t("middleware.try.placeholder", "Paste text containing sensitive data…")}
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={4}
      />
      <div>
        <Button data-testid="mw-try-run" onClick={run} disabled={running || !text}>
          {t("middleware.try.run", "Run")}
        </Button>
      </div>
      {error && <div className="toast" style={{ fontSize: 14, wordBreak: "break-all" }}>{error}</div>}
      {result && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <div className="text-secondary" style={{ fontSize: 14 }}>{t("middleware.try.masked", "Masked text")}</div>
            <pre data-testid="mw-try-masked" style={preStyle}>{result.masked}</pre>
          </div>
          <div>
            <div className="text-secondary" style={{ fontSize: 14 }}>{t("middleware.try.hits", "Matched rules")}</div>
            {result.hits.length ? (
              <div data-testid="mw-try-hits" style={{ fontSize: 14 }}>{result.hits.join(", ")}</div>
            ) : (
              <div data-testid="mw-try-hits-empty" className="text-secondary" style={{ fontSize: 14 }}>
                {t("middleware.try.hitsEmpty", "No rule matched")}
              </div>
            )}
          </div>
          <div>
            <div className="text-secondary" style={{ fontSize: 14 }}>{t("middleware.try.restored", "Restored text")}</div>
            <pre data-testid="mw-try-restored" style={preStyle}>{result.restored}</pre>
          </div>
        </div>
      )}
    </div>
  );
}
