import type { TFunction } from "i18next";
import type { Platform, RoutingMode } from "../../services/api";
import { F, S } from "../../domains/shared/tokens";
import { ROUTING_MODES, routingModeLabel, routingModeDesc, PlatformPicker } from "../../domains/groups";

export interface GroupCreateModalProps {
  cName: string;
  cGroupKey: string;
  cMode: RoutingMode;
  cPlatformIds: number[];
  platforms: Platform[];
  t: TFunction;
  onCName: (v: string) => void;
  onCGroupKey: (v: string) => void;
  onCMode: (v: RoutingMode) => void;
  onCPlatformIds: (v: number[]) => void;
  onClose: () => void;
  onCreate: () => void;
}

/** 分组创建全屏视图：基本信息 + 关联平台（复用 PlatformPicker，保存后一并关联）。 */
export function GroupCreateModal({
  cName, cGroupKey, cMode, cPlatformIds, platforms, t,
  onCName, onCGroupKey, onCMode, onCPlatformIds,
  onClose, onCreate,
}: GroupCreateModalProps) {
  const createPlatformOptions = platforms.filter(p => p.enabled);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <button className="btn btn-ghost btn-icon" onClick={onClose} title={t("action.cancel")}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.title, fontWeight: 700 }}>{t("group.add")}</div>
        </div>
        <button className="btn" onClick={onClose}>{t("action.cancel")}</button>
        <button className="btn btn-primary" onClick={onCreate} disabled={!cName}>{t("action.create")}</button>
      </div>

      {/* Basic info */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

        {/* Name */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
          <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
            placeholder={t("group.name", "分组名称")} value={cName}
            onChange={(e) => onCName(e.target.value)} />
          <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>
            {t("group.nameHint", "分组显示名（中文可读），用于界面展示。")}
          </div>
        </div>

        {/* Group key */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.groupKey", "密钥")}</span>
          <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
            placeholder={t("group.groupKey", "分组密钥（留空自动生成）")} value={cGroupKey}
            onChange={(e) => onCGroupKey(e.target.value.replace(/[^\w-]/g, ""))} />
          <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>
            {t("group.groupKeyHint", "分组密钥（= API Key / 路由识别键）。留空自动生成；创建后锁定不可修改。")}
          </div>
        </div>

        {/* Routing mode */}
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.routingMode", "路由模式")}</span>
          <select className="input" style={{ fontSize: F.body, padding: S.inputPad }} value={cMode} onChange={(e) => onCMode(e.target.value as RoutingMode)}>
            {ROUTING_MODES.map(m => (
              <option key={m} value={m}>{routingModeLabel(t, m)}</option>
            ))}
          </select>
          <div style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{routingModeDesc(t, cMode)}</div>
        </div>
      </div>

      {/* Platforms（与编辑视图共用 PlatformPicker；创建时选定，保存后一并关联） */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.platforms", "关联平台")}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
          {t("group.platformsHint", "选择并排序此分组使用的平台，顺序决定优先级")}
        </div>
        <PlatformPicker
          platformIds={cPlatformIds}
          options={createPlatformOptions}
          onChange={onCPlatformIds}
          t={t}
        />
      </div>
    </div>
  );
}
