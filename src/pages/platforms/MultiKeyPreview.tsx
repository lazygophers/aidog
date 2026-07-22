// MultiKeyPreview — 多 apikey 批量创建实时预览（只读确认层）。
// ponytail: 纯展示组件，name 预览由 usePlatformForm 派生（previewBatchNames shared util 单源），
//   确认/取消回调交父组件处理（confirmBatchCreate / cancelBatchPreview）。无内部 state。
//
// 触发：创建态表单 apikey input 粘/输多 key（splitApiKeys.length>1）→ usePlatformForm.setBatchPreviewKeys(keys)。
// 渲染：表单下方「将创建 N 个平台」标题 + 列表（序号 + name 预览 + 协议 + base_url + key 尾4位掩码）
//   + 「确认批量创建 / 取消」按钮。只读（D2：不支持改 name / 跳过 key）。
import type { TFunction } from "i18next";
import type { Protocol } from "../../services/api";
import { Button } from "@/components/ui/button";

export interface MultiKeyPreviewProps {
  /** splitApiKeys 拆分后的 key 数组（与 previewNames 等长，用于显示 key 尾4位掩码）。 */
  keys: string[];
  /** previewBatchNames 派生的 name 预览列表（与 keys 等长，撞名追号已算入）。 */
  previewNames: string[];
  /** 当前表单协议（显示用，label 取 PROTOCOL_LABELS）。 */
  protocol: Protocol;
  /** 当前表单主 base_url（显示用，确认创建实际用 form state）。 */
  baseUrl: string;
  /** 确认按钮 → 父调 runBatchCreateFromPaste → resetForm + 关表单。 */
  onConfirm: () => void;
  /** 取消按钮 → 父清 batchPreviewKeys + apiKey 回单值。 */
  onCancel: () => void;
  t: TFunction;
}

/** key 尾4位掩码：前缀 •••• + 尾4位（短 key 原样显示）。复用 runBatchCreateFromPaste 的尾4位规则。 */
function maskTail(k: string): string {
  if (k.length <= 4) return k;
  return `••••${k.slice(-4)}`;
}

export function MultiKeyPreview({
  keys, previewNames, protocol, baseUrl, onConfirm, onCancel, t,
}: MultiKeyPreviewProps) {
  if (keys.length === 0) return null;
  return (
    <div
      className="animate-fade-in"
      style={{
        display: "flex", flexDirection: "column", gap: 10,
        padding: 14, borderRadius: "var(--radius-md)",
        background: "var(--bg-glass)", border: "1px solid var(--accent)",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <div className="section-title" style={{ fontSize: 14 }}>
          {t("platform.batch.previewTitle", "将创建 {{count}} 个平台", { count: keys.length })}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
          {t("platform.batch.previewHint", "确认后将批量创建，name 自动生成 {{base}}-key尾4位，撞名追号", { base: "{base}" })}
        </div>
      </div>

      {/* 只读预览列表（D2：不支持改 name / 跳过 key）。每行：#序号 + name 预览 + 协议 + base_url + key 尾4位掩码 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 4, overflowX: "auto" }}>
        {keys.map((k, i) => (
          <div
            key={i}
            style={{
              display: "grid",
              gridTemplateColumns: "24px minmax(120px, 1.5fr) minmax(80px, 1fr) minmax(120px, 1.5fr) minmax(90px, 1fr)",
              gap: 8, alignItems: "center",
              padding: "6px 8px", borderRadius: "var(--radius-sm)",
              background: "var(--bg-elevated)", fontSize: 12,
            }}
          >
            <span style={{ color: "var(--text-tertiary)" }}>#{i + 1}</span>
            <span style={{ fontWeight: 600, wordBreak: "break-all" }}>{previewNames[i] ?? ""}</span>
            <span style={{
              display: "inline-block", padding: "2px 6px", borderRadius: "var(--radius-sm)",
              background: "var(--bg-glass)", fontSize: 10, fontWeight: 700, justifySelf: "start",
            }}>{protocol.toUpperCase()}</span>
            <span style={{ color: "var(--text-secondary)", wordBreak: "break-all" }}>{baseUrl || "—"}</span>
            <span style={{ color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)" }}>{maskTail(k)}</span>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <Button variant="outline" onClick={onCancel}>{t("platform.batch.cancel", "取消")}</Button>
        <Button onClick={onConfirm}>{t("platform.batch.confirm", "确认批量创建")}</Button>
      </div>
    </div>
  );
}
