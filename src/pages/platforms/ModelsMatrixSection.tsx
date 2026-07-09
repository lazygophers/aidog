// ModelsMatrixSection — 矩阵 card：合并 ModelsSection（默认列）+ TimeModelsSection（时段档列）。
// ponytail: PRD 07-09 要求单 FormSection 内列=档（默认 + 时段档 rules[*]）、行=5 槽，
//   每格 input + 可搜索下拉（pinyinMatch）。默认列行为不退化（下拉 / fillAll / fetchModels），
//   时段档列头点弹窗（WindowsEditModal，ST4 产出）编辑 windows；矩阵主体只管模型值。
//   数据通道分离：默认列 = platform.models；时段档列 = platform.extra.time_models.rules[*].models。
//   仍纯 props 驱动，无外部 state（矩阵内部仅 UI 临时态：activeCell / editingIdx / importModalOpen）。
import React, { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { TFunction } from "i18next";
import {
  type Protocol, type ModelSlot, type TimeModelRule,
} from "../../services/api";
import type { PeakWindow } from "../../domains/platforms/defaults";
import { pinyinMatch } from "../../utils/pinyin";
import {
  MODEL_SLOTS, getDefaultModelList,
} from "../../domains/platforms";
import { FormSection } from "./formSections";
import { WindowsEditModal } from "./WindowsEditModal";

// 单元格 key：默认列 = `d:<slot>`；时段档列 = `r<idx>:<slot>`。整矩阵单开（activeCell 唯一）。
type CellKey = string;

// 紧凑描述：单窗口取首窗口描述；多窗口加「+N」；空 → 「永不命中」占位。
//   时间格式：minute 缺省 → `H-H`（hour 精度）；任一带 minute → `HH:MM-HH:MM`；
//   days_of_week → `(周一,周三)`；days_of_month → `(每月1日)`；跨天 end<start 不特殊标记（描述用区间）；
//   全天 0-24 无 minute 无 day → `全天`。
const WEEKDAY_ZH = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function describeWindow(w: PeakWindow): string {
  const sMin = w.start_minute ?? 0;
  const eMin = w.end_minute ?? 0;
  const hourOnly = sMin === 0 && eMin === 0;
  const timePart = hourOnly
    ? `${w.start_hour}-${w.end_hour}`
    : `${pad2(w.start_hour)}:${pad2(sMin)}-${pad2(w.end_hour)}:${pad2(eMin)}`;
  const isFullDay = w.start_hour === 0 && w.end_hour === 24 && hourOnly
    && !w.days_of_week && !w.days_of_month;
  if (isFullDay) {
    // 全天 + 无 day 过滤
    return "全天";
  }
  let dayPart = "";
  if (w.days_of_week && w.days_of_week.length > 0) {
    dayPart = `(${w.days_of_week.map((d) => WEEKDAY_ZH[d] ?? String(d)).join(",")})`;
  } else if (w.days_of_month && w.days_of_month.length > 0) {
    dayPart = `(每月${w.days_of_month.join(",")}日)`;
  }
  return `${timePart}${dayPart}`;
}

export function describeWindows(windows: PeakWindow[]): string {
  if (!windows || windows.length === 0) return "永不命中";
  const first = describeWindow(windows[0]);
  if (windows.length === 1) return first;
  return `${first}+${windows.length - 1}`;
}

export function ModelsMatrixSection({
  // 默认列 props（沿用 ModelsSection 签名，行为不退化）
  models, handleModelChange, handleModelSelect,
  // activeDropdown / setActiveDropdown 接受签名兼容，矩阵内部用 activeCell 单开管理；
  //   外部 state 仅同步关闭（避免旧 dropdown 残留），不再驱动矩阵渲染。
  activeDropdown: _activeDropdown, setActiveDropdown: _setActiveDropdown,
  availableModels, protocol, codingPlan,
  fetchError, fetching,
  onFillAll, onFetchModels, apiKeyMissing, endpointsCount,
  // 时段档列 props
  rules, setRules, peakHours,
  t,
}: {
  models: Record<ModelSlot, string>;
  handleModelChange: (slot: ModelSlot, value: string) => void;
  handleModelSelect: (slot: ModelSlot, value: string) => void;
  activeDropdown: ModelSlot | null;
  setActiveDropdown: React.Dispatch<React.SetStateAction<ModelSlot | null>>;
  availableModels: string[];
  protocol: Protocol;
  codingPlan: boolean;
  fetchError: string;
  fetching: boolean;
  onFillAll: () => void;
  onFetchModels: () => void;
  apiKeyMissing: boolean;
  endpointsCount: number;
  rules: TimeModelRule[];
  setRules: React.Dispatch<React.SetStateAction<TimeModelRule[]>>;
  peakHours: PeakWindow[];
  t: TFunction;
}) {
  // 矩阵内部 UI 临时态
  const [activeCell, setActiveCell] = useState<CellKey | null>(null);
  const [editingIdx, setEditingIdx] = useState<number | null>(null);
  const [importModalOpen, setImportModalOpen] = useState(false);

  // 默认列候选列表：getDefaultModelList async 化后 effect 缓存（同 ModelsSection 原逻辑）。
  //   时段档列复用同源（availableModels || defaultList）。
  const [defaultList, setDefaultList] = useState<string[]>([]);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const list = await getDefaultModelList(protocol, codingPlan);
      if (!cancelled) setDefaultList(list);
    })();
    return () => { cancelled = true; };
  }, [protocol, codingPlan]);

  // ─── 时段档列操作（从 TimeModelsSection 移植）───
  const addRule = () => {
    setRules([...rules, { windows: [], models: {} }]);
  };

  const removeRule = (idx: number) => {
    setRules(rules.filter((_, i) => i !== idx));
  };

  const moveUp = (idx: number) => {
    if (idx === 0) return;
    setRules((prev) => {
      const next = [...prev];
      [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
      return next;
    });
  };

  const moveDown = (idx: number) => {
    if (idx === rules.length - 1) return;
    setRules((prev) => {
      const next = [...prev];
      [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
      return next;
    });
  };

  const updateRule = (idx: number, patch: Partial<TimeModelRule>) => {
    setRules(rules.map((r, i) => (i === idx ? { ...r, ...patch } : r)));
  };

  const updateRuleModel = (idx: number, slot: ModelSlot, value: string) => {
    setRules(rules.map((r, i) => {
      if (i !== idx) return r;
      const nextModels = { ...r.models };
      if (value) nextModels[slot] = value;
      else delete nextModels[slot];
      return { ...r, models: nextModels };
    }));
  };

  const importFromPeakHours = () => {
    if (peakHours.length === 0) return;
    const newRule: TimeModelRule = {
      windows: peakHours.map((w) => ({ ...w })),
      models: {},
    };
    setRules([...rules, newRule]);
    setImportModalOpen(false);
  };

  // ─── 矩阵渲染 helpers ───
  const dropdownSource = availableModels.length > 0 ? availableModels : defaultList;
  const hasDropdown = dropdownSource.length > 0;

  // 每格 input + 可搜索下拉（复用 ModelsSection 的 pinyinMatch 模式）。
  //   cellKey: "d:<slot>" | "r<idx>:<slot>"
  const renderCell = (
    cellKey: string,
    value: string,
    onChange: (v: string) => void,
    onSelect: (v: string) => void,
  ) => {
    const query = value.trim().toLowerCase();
    const filtered = hasDropdown
      ? (query ? dropdownSource.filter((m) => pinyinMatch(query, m)) : dropdownSource)
      : [];
    const open = activeCell === cellKey && filtered.length > 0;
    return (
      <div style={{ position: "relative", width: "100%" }}>
        <input
          className="input"
          style={{ width: "100%", fontSize: 11, padding: "4px 6px", paddingRight: hasDropdown ? 22 : undefined }}
          placeholder={t("platform.models_placeholder", "模型名")}
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
            if (hasDropdown) setActiveCell(cellKey);
          }}
          onFocus={() => {
            if (hasDropdown) setActiveCell(cellKey);
          }}
        />
        {hasDropdown && (
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{
              position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
              width: 20, height: 20, minWidth: 20, padding: 0,
              color: "var(--text-tertiary)", cursor: "pointer",
            }}
            onMouseDown={(e) => {
              e.preventDefault();
              setActiveCell(activeCell === cellKey ? null : cellKey);
            }}
            title={t("platform.selectModel")}
          >
            ▾
          </button>
        )}
        {open && (
          <>
            <div
              style={{ position: "fixed", inset: 0, zIndex: 99 }}
              onMouseDown={() => setActiveCell(null)}
            />
            <div
              className="glass-elevated"
              style={{
                position: "absolute",
                top: "100%",
                left: 0,
                right: 0,
                marginTop: 4,
                maxHeight: 200,
                overflowY: "auto",
                zIndex: 100,
                padding: 4,
                animation: "fadeIn 150ms ease both",
              }}
            >
              {filtered.map((m) => (
                <button
                  key={m}
                  type="button"
                  className="btn btn-ghost"
                  style={{
                    width: "100%",
                    justifyContent: "flex-start",
                    padding: "6px 10px",
                    fontSize: 12,
                    fontWeight: value === m ? 600 : 400,
                    color: value === m ? "var(--accent)" : "var(--text-primary)",
                    background: value === m ? "var(--accent-subtle)" : "transparent",
                    borderRadius: "var(--radius-sm)",
                  }}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onSelect(m);
                    setActiveCell(null);
                  }}
                >
                  {m}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    );
  };

  // 列宽：固定 160px（PRD spec ~160px）；slot label 列 64px 固定。
  const COL_W = 160;
  const LABEL_W = 64;
  const rowStyle: React.CSSProperties = {
    display: "flex", gap: 8, minWidth: "max-content", alignItems: "center",
  };

  return (
    <FormSection
      title={t("platform.models")}
      action={(
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--text-secondary)" }}
            onClick={onFillAll}
            disabled={!models.default.trim()}
            title={t("platform.fillAllHint")}
          >
            {t("platform.fillAll")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
            onClick={onFetchModels}
            disabled={apiKeyMissing || endpointsCount === 0 || fetching}
          >
            {fetching ? t("status.loading") : t("platform.fetchModels")}
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            style={{ fontSize: 12, padding: "4px 10px", whiteSpace: "nowrap" }}
            disabled={peakHours.length === 0}
            title={peakHours.length === 0 ? t("platform.time_models_no_peak", "当前无高峰时段配置") : ""}
            onClick={() => setImportModalOpen(true)}
          >
            {t("platform.time_models_import_peak", "从高峰时段导入")}
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            style={{ fontSize: 12, padding: "4px 10px", color: "var(--accent)" }}
            onClick={addRule}
          >
            + {t("platform.time_models_add_rule", "添加时段档")}
          </button>
        </div>
      )}
    >
      {fetchError && (
        <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
          {fetchError}
        </div>
      )}

      {/* 矩阵主体：横向滚动，列宽固定 */}
      <div style={{ overflowX: "auto", paddingBottom: 4 }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 6, minWidth: "max-content" }}>
          {/* 列头行 */}
          <div style={rowStyle}>
            <div style={{ width: LABEL_W, flexShrink: 0 }} />
            {/* 默认列头 */}
            <div style={{ width: COL_W, flexShrink: 0, textAlign: "center", fontSize: 11, fontWeight: 600, color: "var(--text-secondary)" }}>
              {t("platform.modelDefault")}
            </div>
            {/* 时段档列头 */}
            {rules.map((rule, idx) => (
              <div
                key={`hdr-${idx}`}
                style={{
                  width: COL_W, flexShrink: 0, display: "flex", flexDirection: "column",
                  gap: 2, alignItems: "stretch",
                }}
              >
                <button
                  type="button"
                  className="btn btn-ghost"
                  style={{
                    fontSize: 10, padding: "3px 4px", cursor: "pointer",
                    color: "var(--text-secondary)", whiteSpace: "nowrap",
                    overflow: "hidden", textOverflow: "ellipsis",
                    border: "1px solid var(--border)", borderRadius: "var(--radius-sm)",
                  }}
                  title={t("platform.time_models_edit_windows", "点击编辑时段窗口")}
                  onClick={() => setEditingIdx(idx)}
                >
                  {describeWindows(rule.windows)}
                </button>
                <div style={{ display: "flex", gap: 2, justifyContent: "center" }}>
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon"
                    style={{ padding: "1px 4px", fontSize: 10 }}
                    disabled={idx === 0}
                    onClick={() => moveUp(idx)}
                    title={t("action.moveUp", "上移")}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon"
                    style={{ padding: "1px 4px", fontSize: 10 }}
                    disabled={idx === rules.length - 1}
                    onClick={() => moveDown(idx)}
                    title={t("action.moveDown", "下移")}
                  >
                    ↓
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon btn-danger"
                    style={{ padding: "1px 4px", fontSize: 10 }}
                    onClick={() => removeRule(idx)}
                    title={t("action.delete", "删除")}
                  >
                    ×
                  </button>
                </div>
              </div>
            ))}
          </div>

          {/* 5 槽行 */}
          {MODEL_SLOTS.map(({ key, labelKey }) => (
            <div key={key} style={rowStyle}>
              {/* 左侧固定 slot 标签 */}
              <div style={{
                width: LABEL_W, flexShrink: 0,
                fontSize: 11, fontWeight: 500, color: "var(--text-tertiary)",
                textAlign: "right",
              }}>
                {t(labelKey)}
              </div>
              {/* 默认列单元格 */}
              <div style={{ width: COL_W, flexShrink: 0 }}>
                {renderCell(
                  `d:${key}`,
                  models[key],
                  (v) => handleModelChange(key, v),
                  (v) => handleModelSelect(key, v),
                )}
              </div>
              {/* 时段档列单元格 */}
              {rules.map((rule, idx) => (
                <div key={`r${idx}-${key}`} style={{ width: COL_W, flexShrink: 0 }}>
                  {renderCell(
                    `r${idx}:${key}`,
                    rule.models[key] ?? "",
                    (v) => updateRuleModel(idx, key, v),
                    (v) => updateRuleModel(idx, key, v),
                  )}
                </div>
              ))}
            </div>
          ))}

          {rules.length === 0 && (
            <div style={{ fontSize: 11, color: "var(--text-tertiary)", fontStyle: "italic", paddingLeft: LABEL_W + 8 }}>
              {t("platform.time_models_empty", "未配置 → 全时段用默认模型档")}
            </div>
          )}
        </div>
      </div>

      {/* 列头编辑 windows 弹窗（ST4 产出，此接口对齐） */}
      <WindowsEditModal
        open={editingIdx !== null}
        windows={editingIdx !== null ? (rules[editingIdx]?.windows ?? []) : []}
        onSave={(w) => {
          if (editingIdx !== null) updateRule(editingIdx, { windows: w });
          setEditingIdx(null);
        }}
        onClose={() => setEditingIdx(null)}
        t={t}
      />

      {/* 导入高峰时段确认 modal */}
      {importModalOpen && createPortal(
        <div style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)", display: "flex",
          alignItems: "center", justifyContent: "center", zIndex: 9999,
        }}>
          <div style={{
            background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
            padding: 16, maxWidth: 400, width: "100%", border: "1px solid var(--border)",
          }}>
            <h3 style={{ margin: "0 0 12px", fontSize: 14, fontWeight: 600 }}>
              {t("platform.time_models_import_confirm_title", "导入高峰时段配置？")}
            </h3>
            <p style={{ margin: "0 0 16px", fontSize: 12, color: "var(--text-secondary)" }}>
              {t("platform.time_models_import_confirm_body", "将基于当前高峰时段（{{count}} 个窗口）创建新规则，独立可编辑。").replace("{{count}}", String(peakHours.length))}
            </p>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button
                type="button"
                className="btn btn-ghost"
                onClick={() => setImportModalOpen(false)}
              >
                {t("action.cancel", "取消")}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={importFromPeakHours}
              >
                {t("platform.time_models_import_confirm_button", "确认")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </FormSection>
  );
}
