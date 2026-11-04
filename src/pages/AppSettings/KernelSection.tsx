import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelApi, proxyApi, type KernelSettings } from "../../services/api";
import { Switch } from "@/components/ui/switch";

/**
 * 管理接口设置（ADR 0010）：开关、可选凭据、当前地址。
 *
 * 管理接口挂在代理端口 /api/ 下（POST /api/rpc/<命令>、GET /api/events），只接受本机连接，
 * 与代理的「局域网访问」开关无关。
 */
export function KernelSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<KernelSettings | null>(null);
  const [token, setToken] = useState("");
  const [port, setPort] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState("");

  useEffect(() => {
    kernelApi
      .getSettings()
      .then((s) => {
        setSettings(s);
        setToken(s.auth_token);
      })
      .catch(() => setSettings({ api_enabled: true, auth_token: "" }));
    proxyApi.getSettings().then((p) => setPort(p.port)).catch(() => {});
  }, []);

  if (!settings) return null;

  const flash = (msg: string) => {
    setSaved(msg);
    setTimeout(() => setSaved(""), 2000);
  };

  const persist = async (next: KernelSettings) => {
    setError("");
    try {
      await kernelApi.setSettings(next);
      setSettings(next);
      flash(t("kernel.saved"));
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.toString?.() ?? "");
    }
  };

  return (
    <>
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("kernel.enabled")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("kernel.enabledDesc")}
          </div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("kernel.loopbackOnly")}
          </div>
        </div>
        <Switch
          checked={settings.api_enabled}
          onCheckedChange={(v) => persist({ ...settings, api_enabled: v })}
        />
      </div>

      {port !== null && (
        <div className="glass-surface" style={{ padding: "16px 20px" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("kernel.address")}</div>
          <code style={{ display: "block", marginTop: 6, fontSize: 12 }}>
            {`http://127.0.0.1:${port}/api/rpc/<command>`}
          </code>
          <code style={{ display: "block", marginTop: 2, fontSize: 12 }}>
            {`http://127.0.0.1:${port}/api/events`}
          </code>
        </div>
      )}

      <div className="glass-surface" style={{ padding: "16px 20px" }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("kernel.authToken")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("kernel.authTokenDesc")}
        </div>
        <input
          type="password"
          value={token}
          placeholder={t("kernel.authTokenPlaceholder")}
          onChange={(e) => setToken(e.target.value)}
          onBlur={() => {
            if (token !== settings.auth_token) persist({ ...settings, auth_token: token });
          }}
          style={{ marginTop: 10, width: "100%", fontSize: 13, padding: "6px 10px" }}
        />
      </div>

      {(error || saved) && (
        <div className="text-secondary" style={{ fontSize: 12, padding: "0 20px" }}>
          {error || saved}
        </div>
      )}
    </>
  );
}
