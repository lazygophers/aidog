import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelApi, type KernelSettings } from "../../services/api";

/**
 * 无界面内核**管理面**设置（票 08，2026-09-03 审查后收窄）。
 *
 * 管理面永远只监听 127.0.0.1，**没有开放到局域网的开关**：它开放的是全部管理命令（改任意
 * 配置、读全部请求日志、执行脚本），而它唯一的鉴权是一个静态 Bearer，还带不进浏览器的文档
 * 导航与 EventSource。跨机访问交给用户自己架反向代理（nginx / caddy）负责 TLS 与鉴权。
 *
 * 这里只剩两件可配的事：端口、访问令牌。StartupSection 里代理的「局域网访问」开关管的是
 * 转发端口，是另一个维度，别把两者混为一谈。
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
      .catch(() => setSettings({ port: 9891, auth_token: "" }));
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
      {/* 监听地址是写死的事实，不是开关 —— 如实告诉用户，并给出跨机访问的正确做法 */}
      <div className="glass-surface" style={{ padding: "16px 20px" }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("kernel.loopbackOnly")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("kernel.loopbackOnlyDesc")}
        </div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("kernel.remoteAccess")}
        </div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("kernel.notProxy")}
        </div>
      </div>

      {/* 访问令牌 —— 反代回连本机时防同机其他进程 */}
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
