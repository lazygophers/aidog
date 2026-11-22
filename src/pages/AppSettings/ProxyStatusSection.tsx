import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/**
 * Proxy Status section（原 L258-314）。
 * 仅状态卡 + 端口/启停；Upstream Proxy 单独成组件以保留原视觉顺序。
 */
export function ProxyStatusSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    running, proxyPort, setProxyPort,
    handleProxyStart, handleProxyStop, proxyStartError,
  } = s;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div
        className={`glass glass-highlight ${running ? "" : ""}`}
        style={{
          padding: "24px 20px",
          display: "flex",
          alignItems: "center",
          gap: 20,
        }}
      >
        {/* 运行中外发光：独立叠层动 opacity，不动宿主 .glass 的 box-shadow */}
        {running && <div className="proxy-status-glow" />}
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
            <div style={{ fontSize: 14, color: "var(--text-secondary)", marginTop: 2 }}>
              localhost:{proxyPort}
            </div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
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

      {/* 手动启动失败错误条（proxy-port-no-drift s2）：持久展示，非 toast/一闪而过。
          单一 proxyStartError state，重试成功清空、仍失败原样保留，不堆叠第二条。 */}
      {proxyStartError && (
        <div
          className="glass-surface"
          style={{
            padding: "12px 16px",
            display: "flex",
            alignItems: "center",
            gap: 12,
            border: "1px solid var(--color-danger)",
            background: "var(--color-danger-bg)",
          }}
        >
          <span style={{ fontSize: 14, color: "var(--color-danger)", flex: 1 }}>
            {proxyStartError.kind === "addr_in_use"
              ? t("proxy.startFailedPortInUse", "Port {{port}} is already in use, proxy did not start", { port: proxyStartError.port })
              : t("proxy.startFailedOther", "Failed to bind port {{port}}, proxy did not start", { port: proxyStartError.port })}
          </span>
          <Button variant="outline" onClick={handleProxyStart}>
            {t("proxy.retry", "Retry")}
          </Button>
        </div>
      )}
    </div>
  );
}

/**
 * 代理配置 section（issue #64 T02，原「上游代理」）。
 * 无启用总开关：配置输入恒可见；「其它请求使用代理」独立开关（others_enabled，
 * 管不属于任何平台的出站请求）；「为全部平台开启」批量写各平台 extra.proxy_enabled。
 */
export function UpstreamProxySection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { proxyClient, handleProxyClientChange } = s;
  const { proxyAllConfirm, setProxyAllConfirm, proxyAllBusy, runEnableProxyForAll } = s;

  return (
    <div className="glass-surface" style={{
      padding: "16px 20px",
      display: "flex",
      flexDirection: "column",
      gap: 12,
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("proxy.upstreamProxy")}</div>
          <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
            {t("proxy.upstreamProxyDesc")}
          </div>
        </div>
        <Button data-testid="enable-proxy-all-button" variant="outline" disabled={proxyAllBusy}
          onClick={() => setProxyAllConfirm(true)}>
          {proxyAllBusy
            ? t("proxy.enableForAllBusy", "Writing…")
            : t("proxy.enableForAllPlatforms", "Enable for all platforms")}
        </Button>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.proxyType", "Protocol")}
            </label>
            <Select value={proxyClient.proxy_type} onValueChange={(v) => handleProxyClientChange({ proxy_type: v })}>
              <SelectTrigger style={{ width: 100, height: 28, fontSize: 14 }}>
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
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.proxyHost", "Host")}
            </label>
            <Input
              data-testid="proxy-host-input"
              value={proxyClient.host}
              onChange={(e) => handleProxyClientChange({ host: e.target.value })}
              placeholder="127.0.0.1"
              style={{ width: 120, height: 28, fontSize: 14 }}
            />
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.proxyPort", "Port")}
            </label>
            <Input
              data-testid="proxy-port-input"
              type="number"
              value={proxyClient.port}
              onChange={(e) => handleProxyClientChange({ port: Number(e.target.value) || 7890 })}
              placeholder="7890"
              style={{ width: 70, height: 28, fontSize: 14 }}
            />
          </div>
        </div>
        <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.proxyUser", "Username")}
            </label>
            <Input
              value={proxyClient.username}
              onChange={(e) => handleProxyClientChange({ username: e.target.value })}
              placeholder={t("proxy.proxyUserPlaceholder", "Optional")}
              style={{ width: 100, height: 28, fontSize: 14 }}
            />
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 14, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.proxyPass", "Password")}
            </label>
            <Input
              type="password"
              value={proxyClient.password}
              onChange={(e) => handleProxyClientChange({ password: e.target.value })}
              placeholder={t("proxy.proxyPassPlaceholder", "Optional")}
              style={{ width: 100, height: 28, fontSize: 14 }}
            />
          </div>
        </div>
        {proxyClient.proxy_type === "socks5" && (
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontSize: 14, fontWeight: 600 }}>{t("proxy.dnsOverProxy", "DNS over Proxy")}</div>
              <div className="text-tertiary" style={{ fontSize: 14, marginTop: 1 }}>
                {t("proxy.dnsOverProxyDesc", "SOCKS5h: resolve DNS through the proxy")}
              </div>
            </div>
            <Switch checked={proxyClient.dns_over_proxy} onCheckedChange={(c) => handleProxyClientChange({ dns_over_proxy: c })} />
          </div>
        )}
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 14, color: "var(--text-secondary)" }}>
            {t("proxy.noProxy", "Bypass List")}
          </label>
          <Input
            value={proxyClient.no_proxy}
            onChange={(e) => handleProxyClientChange({ no_proxy: e.target.value })}
            placeholder={t("proxy.noProxyPlaceholder", "Empty = default: LAN, loopback, .cn, China platform domains")}
            style={{ width: "100%", height: 28, fontSize: 14 }}
          />
          <div className="text-tertiary" style={{ fontSize: 14 }}>
            {t("proxy.noProxyDesc", "Comma-separated: domain suffixes, IPs, CIDRs; matching hosts connect directly")}
          </div>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10, paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <div>
            <div style={{ fontSize: 14, fontWeight: 600 }}>{t("proxy.othersUseProxy", "Use proxy for other requests")}</div>
            <div className="text-tertiary" style={{ fontSize: 14, marginTop: 1 }}>
              {t("proxy.othersUseProxyDesc", "Outbound requests not tied to any platform, e.g. logo sync, MCP suggestions, skill installation")}
            </div>
          </div>
          <Switch data-testid="others-use-proxy" checked={proxyClient.others_enabled} onCheckedChange={(c) => handleProxyClientChange({ others_enabled: c })} />
        </div>
      </div>

      <AlertDialog open={proxyAllConfirm} onOpenChange={setProxyAllConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("proxy.enableForAllConfirmTitle", "Enable proxy for all platforms?")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("proxy.enableForAllConfirmDesc", "This turns on \"Use proxy\" for every platform. The proxy address is not changed.")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="enable-proxy-all-cancel" onClick={() => setProxyAllConfirm(false)}>
              {t("action.cancel", "Cancel")}
            </AlertDialogCancel>
            <AlertDialogAction data-testid="enable-proxy-all-confirm" onClick={() => { void runEnableProxyForAll(); }}>
              {t("action.confirm", "Confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
