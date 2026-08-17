import { useTranslation } from "react-i18next";
import piIcon from "../../assets/platforms/pi.svg";

/**
 * 「这个维度 pi 不支持」的说明条。
 *
 * 用在 MCP / Hooks 通知 / cc-switch 导入三处：那里对 Claude Code 与 Codex 有内容、
 * 对 pi 永远是空的。不写一句就像 aidog 坏了。样式刻意做成中性提示（不是错误态、
 * 也不是空状态），因为这是产品决定，不是缺陷。原因见
 * `docs/adr/0002-no-mcp-hooks-or-statusline-for-pi.md`。
 */
export function PiUnsupportedNote({ reasonKey, reasonFallback }: { reasonKey: string; reasonFallback: string }) {
  const { t } = useTranslation();
  return (
    <div
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        padding: "8px 12px",
        borderRadius: "var(--radius-md)",
        background: "var(--bg-subtle)",
        border: "1px solid var(--border-subtle)",
        color: "var(--text-secondary)",
        fontSize: 12,
        lineHeight: 1.5,
      }}
    >
      <img src={piIcon} width={16} height={16} alt="pi" style={{ flexShrink: 0, marginTop: 1 }} />
      <span>
        <strong style={{ color: "var(--text-primary)" }}>{t("pi.unsupportedTitle", "pi 不支持此项")}</strong>
        {" — "}
        {t(reasonKey, reasonFallback)}
      </span>
    </div>
  );
}
