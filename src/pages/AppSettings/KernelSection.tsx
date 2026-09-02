import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Switch } from "@/components/ui/switch";
import { kernelApi, KERNEL_BIND_REQUIRES_AUTH, type KernelSettings } from "../../services/api";

/**
 * 无界面内核**管理面**设置（票 08）。
 *
 * 与 StartupSection 里代理的「局域网访问」开关**分列两处、互不读取**：那个开放的是转发
 * 端口，这个开放的是管理接口（210 个命令，含改配置、读全部请求日志、执行脚本）。文案必须
 * 把这件事写明，否则用户会以为两个开关是一回事。
 *
 * 开启的硬前提：先填访问令牌。没填就切开关，后端 reject（消息 = `kernel.bindLanRequiresAuth`），
 * 这里把它翻译成人话展示，并把开关弹回关的状态。
 */
export function KernelSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<KernelSettings | null>(null);
  const [token, setToken] = useState("");
  const [error, setError] = useState("");
  const [saved, setSaved] = useState("");

  useEffect(() => {
    kernelApi
      .getSettings()
      .then((s) => {
        setSettings(s);
        setToken(s.auth_token);
      })
      .catch(() => setSettings({ port: 9891, bind_lan: false, auth_token: "" }));
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
      const raw = typeof e === "string" ? e : e?.toString?.() ?? "";
      setError(raw.includes(KERNEL_BIND_REQUIRES_AUTH) ? t("kernel.bindLanRequiresAuth") : raw);
    }
  };

  const rowStyle = {
    padding: "16px 20px",
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
  } as const;

  return (
    <>
      {/* 管理面绑定开关 —— 独立于代理的 bind_lan */}
      <div className="glass-surface" style={rowStyle}>
        <div style={{ paddingRight: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("kernel.bindLan")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("kernel.bindLanDesc")}
          </div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("kernel.bindLanSecurity")}
          </div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("kernel.bindLanNotProxy")}
          </div>
        </div>
        <Switch
          checked={settings.bind_lan}
          onCheckedChange={(val) => persist({ ...settings, bind_lan: val })}
        />
      </div>

      {/* 访问令牌 —— 开启上面那个开关的前提 */}
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
