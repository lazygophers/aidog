import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

/**
 * Proxy Status section（原 L258-314）。
 * 仅状态卡 + 端口/启停；Upstream Proxy 单独成组件以保留原视觉顺序。
 */
export function ProxyStatusSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    running, proxyPort, setProxyPort,
    handleProxyStart, handleProxyStop,
  } = s;

  return (
    <div
      className={`glass glass-highlight ${running ? "" : ""}`}
      style={{
        padding: "24px 20px",
        display: "flex",
        alignItems: "center",
        gap: 20,
        ...(running ? { animation: running ? "pulseGlow 3s ease-in-out infinite" : undefined } : {}),
      }}
    >
      <div style={{
        width: 44, height: 44, borderRadius: 22,
        flexShrink: 0,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: running
          ? "linear-gradient(135deg, color-mix(in srgb, var(--color-success) 20%, transparent), color-mix(in srgb, var(--color-success) 5%, transparent))"
          : "var(--bg-glass)",
        border: `1px solid ${running ? "color-mix(in srgb, var(--color-success) 20%, transparent)" : "var(--border)"}`,
        transition: "all 400ms ease",
      }}>
        <span className={`status-dot ${running ? "status-dot-active" : "status-dot-inactive"}`}
          style={{ width: 16, height: 16 }} />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 700 }}>
          {running ? t("proxy.running") : t("proxy.stopped")}
        </div>
        {running && (
          <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 2 }}>
            localhost:{proxyPort}
          </div>
        )}
      </div>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
          {t("proxy.port")}
        </label>
        <Input
          type="number"
          value={proxyPort}
          onChange={(e) => setProxyPort(Number(e.target.value))}
          disabled={running}
          style={{ width: 80 }}
        />
        {!running ? (
          <Button variant="default" onClick={handleProxyStart}>
            {t("proxy.start")}
          </Button>
        ) : (
          <Button variant="destructive" onClick={handleProxyStop}>
            {t("proxy.stop")}
          </Button>
        )}
      </div>
    </div>
  );
}

/**
 * Upstream Proxy section（原 L406-517）。操作 proxyClient。
 */
export function UpstreamProxySection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { proxyClient, handleProxyClientChange } = s;

  return (
    <div className="glass-surface" style={{
      padding: "16px 20px",
      display: "flex",
      flexDirection: "column",
      gap: 12,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.upstreamProxy")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.upstreamProxyDesc")}
          </div>
        </div>
        <div
          className={`toggle ${proxyClient.enabled ? "active" : ""}`}
          onClick={() => handleProxyClientChange({ enabled: !proxyClient.enabled })}
          role="switch"
          aria-checked={proxyClient.enabled}
          tabIndex={0}
        />
      </div>

      {proxyClient.enabled && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10, paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.proxyType", "协议")}
              </label>
              <Select value={proxyClient.proxy_type} onValueChange={(v) => handleProxyClientChange({ proxy_type: v })}>
                <SelectTrigger style={{ width: 100, height: 28, fontSize: 12 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="socks5">SOCKS5</SelectItem>
                  <SelectItem value="http">HTTP</SelectItem>
                  <SelectItem value="https">HTTPS</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.proxyHost", "地址")}
              </label>
              <Input
                value={proxyClient.host}
                onChange={(e) => handleProxyClientChange({ host: e.target.value })}
                style={{ width: 120, height: 28, fontSize: 12 }}
              />
            </div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.proxyPort", "端口")}
              </label>
              <Input
                type="number"
                value={proxyClient.port}
                onChange={(e) => handleProxyClientChange({ port: Number(e.target.value) || 7890 })}
                style={{ width: 70, height: 28, fontSize: 12 }}
              />
            </div>
          </div>
          <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.proxyUser", "用户名")}
              </label>
              <Input
                value={proxyClient.username}
                onChange={(e) => handleProxyClientChange({ username: e.target.value })}
                placeholder={t("proxy.proxyUserPlaceholder", "可选")}
                style={{ width: 100, height: 28, fontSize: 12 }}
              />
            </div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.proxyPass", "密码")}
              </label>
              <Input
                type="password"
                value={proxyClient.password}
                onChange={(e) => handleProxyClientChange({ password: e.target.value })}
                placeholder={t("proxy.proxyPassPlaceholder", "可选")}
                style={{ width: 100, height: 28, fontSize: 12 }}
              />
            </div>
          </div>
          {proxyClient.proxy_type === "socks5" && (
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600 }}>{t("proxy.dnsOverProxy", "DNS 走代理")}</div>
                <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1 }}>
                  {t("proxy.dnsOverProxyDesc", "SOCKS5h: DNS 解析也走代理解析")}
                </div>
              </div>
              <div
                className={`toggle ${proxyClient.dns_over_proxy ? "active" : ""}`}
                onClick={() => handleProxyClientChange({ dns_over_proxy: !proxyClient.dns_over_proxy })}
                role="switch"
                aria-checked={proxyClient.dns_over_proxy}
                tabIndex={0}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
