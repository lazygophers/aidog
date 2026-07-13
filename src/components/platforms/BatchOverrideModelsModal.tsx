// ─── 批量覆盖平台模型弹窗（group-batch-ops s4）──────────────────
// 三来源 radio 切换：手输 / preset 下拉（按协议默认模型）/ 从别平台复制。
// 全 diff：选中平台当前模型 vs 将覆盖为的新模型（5 槽 per-platform 可滚展示）。
// 确认 → 调 batch_override_models 原子事务（PlatformModels 整体覆盖 5 槽）。
// 复用 shared/Modal 基元（createPortal document.body，liquid glass 居中）。

import { Fragment, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import type { Platform, PlatformModels, Protocol, ModelSlot } from "../../services/api";
import { Modal } from "../shared/Modal";
import { MODEL_SLOTS } from "../../domains/platforms/constants";
import { getDefaultModels, buildProtocolsFromPresets } from "../../domains/platforms/defaults";
import type { ProtocolOption } from "../../domains/platforms/constants";

type Source = "manual" | "preset" | "copy";

const EMPTY_MODELS: PlatformModels = { default: "", sonnet: "", opus: "", haiku: "", gpt: "" };

/** 把 Partial<Record<ModelSlot,string>> 归一为 5 槽全字符串对象（缺槽 = ""）。 */
function normalize(partial: Partial<Record<ModelSlot, string>> | PlatformModels | undefined): PlatformModels {
  return {
    default: partial?.default ?? "",
    sonnet: partial?.sonnet ?? "",
    opus: partial?.opus ?? "",
    haiku: partial?.haiku ?? "",
    gpt: partial?.gpt ?? "",
  };
}

export interface BatchOverrideModelsModalProps {
  open: boolean;
  /** 待覆盖平台（展示 current models + diff）。 */
  platforms: Platform[];
  /** 全库平台列表（"从别平台复制" 来源下拉数据源）。 */
  allPlatforms: Platform[];
  /** 确认覆盖：父级执行 invoke + 刷新 + toast，完成后 onClose。 */
  onConfirm: (models: PlatformModels) => void;
  onClose: () => void;
  /** 覆盖中（invoke 等），禁用按钮 + 切文案。 */
  busy?: boolean;
  t: TFunction;
}

export function BatchOverrideModelsModal({
  open,
  platforms,
  allPlatforms,
  onConfirm,
  onClose,
  busy = false,
  t,
}: BatchOverrideModelsModalProps) {
  const { i18n } = useTranslation();
  const locale = i18n.language;
  const [source, setSource] = useState<Source>("manual");
  const [slots, setSlots] = useState<PlatformModels>(EMPTY_MODELS);
  const [presetProtocol, setPresetProtocol] = useState<Protocol | "">("");
  const [copyPlatformId, setCopyPlatformId] = useState<number | "">("");
  const [protocolOptions, setProtocolOptions] = useState<ProtocolOption[]>([]);

  // preset 协议列表（mount / open 时拉一次；buildProtocolsFromPresets 单次 RPC 复用 docPromise 缓存）
  useEffect(() => {
    if (!open) return;
    let alive = true;
    buildProtocolsFromPresets(locale).then(opts => {
      if (alive) setProtocolOptions(opts);
    }).catch(() => { /* 单次 RPC 失败静默：下拉空，用户走手输/复制 */ });
    return () => { alive = false; };
  }, [open, locale]);

  // 切来源时重置槽位（避免来源间残留串读；user 重新选来源 = 重新输入意图）
  useEffect(() => {
    setSlots(EMPTY_MODELS);
    setPresetProtocol("");
    setCopyPlatformId("");
  }, [source]);

  // preset: 选协议 → 拉默认模型填槽（codingPlan=false, isPeak=false：基础默认分支）
  useEffect(() => {
    if (source !== "preset" || presetProtocol === "") return;
    let alive = true;
    getDefaultModels(presetProtocol).then(m => {
      if (alive) setSlots(normalize(m));
    }).catch(() => { if (alive) setSlots(EMPTY_MODELS); });
    return () => { alive = false; };
  }, [source, presetProtocol]);

  // copy: 选平台 → 直接取该平台 models 填槽
  useEffect(() => {
    if (source !== "copy" || copyPlatformId === "") return;
    const src = allPlatforms.find(p => p.id === copyPlatformId);
    if (src) setSlots(normalize(src.models));
  }, [source, copyPlatformId, allPlatforms]);

  // 是否所有槽位都空（此时确认无意义）
  const allEmpty = useMemo(
    () => MODEL_SLOTS.every(s => !(slots[s.key] ?? "").trim()),
    [slots],
  );

  return (
    <Modal
      open={open}
      onClose={busy ? () => {} : onClose}
      className="glass-elevated"
      maxWidth={560}
      maxHeight="85vh"
      style={{ padding: "20px 22px" }}
      closeOnBackdrop={!busy}
      closeOnEscape={!busy}
    >
      {/* 标题 */}
      <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 4 }}>
        {t("group.batchOverrideModelsTitle", "批量覆盖模型")}
      </div>
      <div style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 12 }}>
        {t("group.batchOverrideModelsDesc", "将以下 {{count}} 个平台的模型整体覆盖为新值（5 槽位全量替换）。", { count: platforms.length })}
      </div>

      {/* 来源 radio */}
      <div style={{ display: "flex", gap: 16, marginBottom: 10, flexWrap: "wrap" }}>
        {(["manual", "preset", "copy"] as const).map(s => (
          <label key={s} style={{ display: "inline-flex", alignItems: "center", gap: 5, fontSize: 13, cursor: "pointer" }}>
            <input
              type="radio"
              name="batch-override-source"
              checked={source === s}
              onChange={() => setSource(s)}
              style={{ accentColor: "var(--accent)" }}
            />
            {s === "manual" && t("group.batchOverrideSourceManual", "手输")}
            {s === "preset" && t("group.batchOverrideSourcePreset", "preset 下拉")}
            {s === "copy" && t("group.batchOverrideSourceCopy", "从别平台复制")}
          </label>
        ))}
      </div>

      {/* 来源子控件 */}
      {source === "preset" && (
        <select
          className="input"
          style={{ fontSize: 13, width: "100%", marginBottom: 10 }}
          value={presetProtocol}
          onChange={e => setPresetProtocol(e.target.value as Protocol)}
        >
          <option value="">{t("group.batchOverridePresetSelect", "选择协议…")}</option>
          {protocolOptions.map(o => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>
      )}
      {source === "copy" && (
        <select
          className="input"
          style={{ fontSize: 13, width: "100%", marginBottom: 10 }}
          value={copyPlatformId}
          onChange={e => setCopyPlatformId(e.target.value === "" ? "" : Number(e.target.value))}
        >
          <option value="">{t("group.batchOverrideCopySelect", "选择平台…")}</option>
          {allPlatforms.map(p => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      )}

      {/* 5 槽输入（所有来源共用：preset/copy 填充后仍可手改） */}
      <div style={{
        display: "grid", gridTemplateColumns: "auto 1fr", gap: "6px 10px",
        padding: "8px 10px", borderRadius: "var(--radius-sm)",
        background: "var(--bg-glass)", border: "1px solid var(--border)",
        marginBottom: 12, alignItems: "center",
      }}>
        {MODEL_SLOTS.map(s => (
          <Fragment key={s.key}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)", fontWeight: 500 }}>
              {t(s.labelKey, s.key)}
            </span>
            <input
              className="input"
              style={{ fontSize: 12, padding: "4px 8px" }}
              value={slots[s.key] ?? ""}
              onChange={e => setSlots(prev => ({ ...prev, [s.key]: e.target.value }))}
            />
          </Fragment>
        ))}
      </div>

      {/* 全 diff：选中平台 current → new（5 槽 per-platform 可滚） */}
      <div style={{ fontSize: 12.5, color: "var(--text-secondary)", marginBottom: 6 }}>
        {t("group.batchOverrideDiffTitle", "覆盖预览（当前 → 新值）：")}
      </div>
      <div style={{
        display: "flex", flexDirection: "column", gap: 6,
        maxHeight: "32vh", overflowY: "auto",
        padding: "6px 10px", borderRadius: "var(--radius-sm)",
        background: "var(--bg-glass)", border: "1px solid var(--border)",
      }}>
        {platforms.map(p => (
          <div key={p.id} style={{
            padding: "5px 0", borderBottom: "1px solid var(--border)",
            display: "flex", flexDirection: "column", gap: 2,
          }}>
            <div style={{ fontSize: 12.5, fontWeight: 600 }}>{p.name}</div>
            {MODEL_SLOTS.map(s => {
              const cur = p.models?.[s.key] ?? "";
              const next = slots[s.key] ?? "";
              const changed = cur !== next;
              const empty = !cur && !next;
              if (empty) return null; // 双空槽不展示，减噪
              return (
                <div key={s.key} style={{
                  display: "grid", gridTemplateColumns: "auto 1fr auto 1fr",
                  gap: "2px 6px", fontSize: 11.5, alignItems: "baseline",
                  paddingLeft: 8,
                }}>
                  <span style={{ color: "var(--text-tertiary)", fontWeight: 500 }}>
                    {t(s.labelKey, s.key)}
                  </span>
                  <span style={{
                    color: cur ? "var(--text-secondary)" : "var(--text-tertiary)",
                    fontStyle: cur ? "normal" : "italic",
                    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  }}>
                    {cur || t("group.batchOverrideEmpty", "（空）")}
                  </span>
                  <span style={{ color: changed ? "var(--accent)" : "var(--text-tertiary)" }}>→</span>
                  <span style={{
                    color: next ? (changed ? "var(--accent)" : "var(--text-secondary)") : "var(--text-tertiary)",
                    fontStyle: next ? "normal" : "italic",
                    fontWeight: changed ? 600 : 400,
                    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  }}>
                    {next || t("group.batchOverrideEmpty", "（空）")}
                  </span>
                </div>
              );
            })}
          </div>
        ))}
      </div>

      {/* 操作按钮 */}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
        <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose} disabled={busy}>
          {t("action.cancel", "取消")}
        </button>
        <button
          className="btn btn-primary"
          style={{ fontSize: 13, padding: "6px 14px" }}
          onClick={() => onConfirm(slots)}
          disabled={busy || allEmpty}
          title={allEmpty ? t("group.batchOverrideAllEmptyHint", "所有槽位为空，请至少填一个") : undefined}
        >
          {busy
            ? t("group.batchOverrideApplying", "覆盖中…")
            : t("group.batchOverrideConfirm", "覆盖 {{count}} 个平台", { count: platforms.length })}
        </button>
      </div>
    </Modal>
  );
}
