import { useTranslation } from "react-i18next";
import type { SystemSettings } from "./useSystemSettings";

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
        <div
          className={`toggle ${autostart ? "active" : ""}`}
          onClick={() => handleAutostartChange(!autostart)}
          role="switch"
          aria-checked={autostart}
          tabIndex={0}
        />
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
        <div
          className={`toggle ${bindLan ? "active" : ""}`}
          onClick={() => handleBindLanChange(!bindLan)}
          role="switch"
          aria-checked={bindLan}
          tabIndex={0}
        />
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
        <div
          className={`toggle ${autolaunch ? "active" : ""}`}
          onClick={() => handleAutolaunchChange(!autolaunch)}
          role="switch"
          aria-checked={autolaunch}
          tabIndex={0}
        />
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
        <div
          className={`toggle ${silentLaunch ? "active" : ""}`}
          onClick={() => handleSilentLaunchChange(!silentLaunch)}
          role="switch"
          aria-checked={silentLaunch}
          tabIndex={0}
        />
      </div>
      )}
    </>
  );
}
