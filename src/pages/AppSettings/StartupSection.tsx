import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";
import { Switch } from "@/components/ui/switch";

/**
 * Autostart + Bind LAN + Autolaunch + Silent Launch 四个启动相关 section（原 L316-404）。
 */
export function StartupSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    autostart, bindLan, autolaunch, silentLaunch,
    handleAutostartChange, handleBindLanChange,
    handleAutolaunchChange, handleSilentLaunchChange,
  } = s;

  return (
    <>
      {/* Autostart */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.autostart")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.autostartDesc")}
          </div>
        </div>
        <Switch checked={autostart} onCheckedChange={handleAutostartChange} />
      </div>

      {/* Bind LAN — allow other devices on the local network to connect */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.bindLan")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.bindLanDesc")}
          </div>
        </div>
        <Switch checked={bindLan} onCheckedChange={handleBindLanChange} />
      </div>

      {/* Autolaunch — OS login auto start */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.autolaunch")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.autolaunchDesc")}
          </div>
        </div>
        <Switch checked={autolaunch} onCheckedChange={handleAutolaunchChange} />
      </div>

      {/* Silent Launch — start minimized to tray; 仅在 autolaunch (开机自启) 开启时展示 */}
      {autolaunch && (
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.silentLaunch")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.silentLaunchDesc")}
          </div>
        </div>
        <Switch checked={silentLaunch} onCheckedChange={handleSilentLaunchChange} />
      </div>
      )}
    </>
  );
}
