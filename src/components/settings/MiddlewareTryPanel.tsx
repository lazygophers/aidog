// 「试一试」：样例文本按当前启用的全部请求侧规则依次执行（后端与转发同一引擎），只读。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { middlewareApi } from "../../services/api/scheduling";
import type { MiddlewareTryResult } from "../../services/api";

const preStyle = { margin: 0, whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: 12 } as const;

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
      setError(`${t("middleware.try.failed", "运行失败")}: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="glass-surface glass-highlight" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("middleware.try.title", "试一试")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("middleware.try.desc", "输入样例文本，按当前启用的全部请求侧规则依次执行（与转发同一引擎），不会发出任何请求。")}
        </div>
      </div>
      <Textarea
        data-testid="mw-try-input"
        aria-label={t("middleware.try.input", "样例文本")}
        placeholder={t("middleware.try.placeholder", "粘贴一段含敏感信息的文本…")}
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={4}
      />
      <div>
        <Button data-testid="mw-try-run" onClick={run} disabled={running || !text}>
          {t("middleware.try.run", "运行")}
        </Button>
      </div>
      {error && <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{error}</div>}
      {result && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <div className="text-secondary" style={{ fontSize: 12 }}>{t("middleware.try.masked", "替换后文本")}</div>
            <pre data-testid="mw-try-masked" style={preStyle}>{result.masked}</pre>
          </div>
          <div>
            <div className="text-secondary" style={{ fontSize: 12 }}>{t("middleware.try.hits", "命中规则")}</div>
            {result.hits.length ? (
              <div data-testid="mw-try-hits" style={{ fontSize: 12 }}>{result.hits.join(", ")}</div>
            ) : (
              <div data-testid="mw-try-hits-empty" className="text-secondary" style={{ fontSize: 12 }}>
                {t("middleware.try.hitsEmpty", "无规则命中")}
              </div>
            )}
          </div>
          <div>
            <div className="text-secondary" style={{ fontSize: 12 }}>{t("middleware.try.restored", "还原后文本")}</div>
            <pre data-testid="mw-try-restored" style={preStyle}>{result.restored}</pre>
          </div>
        </div>
      )}
    </div>
  );
}
