// ── Claude Code 订阅平台 MITM 接入形态（cc-sub-mitm 票 12）──
// 开关本体存 platform.extra.mitm_stats（usePlatformForm serialize）；本组件只做呈现：
// Root CA 启用引导（读 mitm_status）+ export 语句（cc_proxy_export，密码/端口与 sync 同源）
// + 三条接入须知（env 只读一次 / 组名字符集 / Desktop 需 export）。
// 数据打开表单时拉一次，无轮询。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { mitmApi, mitmStatsApi } from "../../services/api";
import { writeText } from "../../services/platform";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { FormSection } from "./formSections";

export function CcMitmAccessSection({
  enabled,
  onToggle,
  groupName,
}: {
  enabled: boolean;
  onToggle: (next: boolean) => void;
  /** 编辑态平台的所属分组名（claude_code 独占分组 = 单值）；新建态 undefined → 不显 export 行。 */
  groupName?: string;
}) {
  const { t } = useTranslation();
  const [caReady, setCaReady] = useState<boolean | null>(null);
  const [exportLine, setExportLine] = useState("");

  useEffect(() => {
    let cancelled = false;
    mitmApi.status()
      .then(s => { if (!cancelled) setCaReady(s.enabled && s.ca_installed); })
      .catch(() => { if (!cancelled) setCaReady(false); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!groupName) { setExportLine(""); return; }
    mitmStatsApi.ccProxyExport(groupName)
      .then(line => { if (!cancelled) setExportLine(line); })
      .catch(() => { if (!cancelled) setExportLine(""); });
    return () => { cancelled = true; };
  }, [groupName]);

  return (
    <FormSection title={t("platform.mitmSection", "订阅透传 MITM 统计")}>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Switch checked={enabled} onCheckedChange={onToggle} />
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("platform.mitmStatsToggle", "启用（经 CONNECT 代理解密统计用量与参考成本）")}
          </span>
        </div>

        {/* Root CA 启用引导：MITM 未启用 / CA 未装时指路（装 CA 流程在 设置 → MITM，不在表单里重做） */}
        {enabled && caReady === false && (
          <div style={{
            fontSize: 12, lineHeight: 1.5, color: "var(--text-secondary)",
            padding: "8px 10px", borderRadius: "var(--radius-sm)",
            background: "color-mix(in srgb, var(--color-warning) 10%, transparent)",
            border: "1px solid color-mix(in srgb, var(--color-warning) 30%, transparent)",
          }}>
            {t("platform.mitmCaHint", "MITM 隧道未就绪：请到 设置 → MITM 启用隧道并安装假根证书（CA），否则流量盲转、无统计。")}
          </div>
        )}

        {/* export 语句（Desktop / 多 shell；密码/端口与 sync 注入 settings.json 的同一份） */}
        {exportLine && (
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
              {t("platform.mitmExportLabel", "终端接入（Claude Desktop 忽略配置文件 env 块，须用 export 语句）：")}
            </span>
            <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <code style={{
                flex: 1, minWidth: 0, fontSize: 11, padding: "6px 8px",
                fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                background: "var(--bg-glass)", border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)", wordBreak: "break-all",
                color: "var(--text-primary)",
              }}>
                {exportLine}
              </code>
              <Button variant="outline" size="sm" style={{ fontSize: 11, height: "auto", padding: "5px 10px" }}
                onClick={() => { writeText(exportLine); }}>
                {t("action.copy", "复制")}
              </Button>
            </div>
          </div>
        )}

        {/* 三条接入须知（#96258 / undici decodeURIComponent / CC 启动只读一次 env） */}
        <ul style={{ margin: 0, paddingLeft: 18, fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.7 }}>
          <li>{t("platform.mitmNoteRestart", "Claude Code 启动时只读一次代理环境变量：换组后须重启 Claude Code。")}</li>
          <li>{t("platform.mitmNoteName", "组名仅限字母、数字与 - _ . ；含其它字符的组改名后才能订阅透传。")}</li>
          <li>{t("platform.mitmNoteDesktop", "Claude Desktop 忽略配置文件的 env 块，须在终端用上面的 export 语句接入。")}</li>
        </ul>
      </div>
    </FormSection>
  );
}
