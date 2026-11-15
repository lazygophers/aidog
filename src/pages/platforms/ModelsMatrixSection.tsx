// ModelsMatrixSection — 矩阵 card：合并 ModelsSection（默认列）+ TimeModelsSection（时段档列）。
// ponytail: PRD 07-09 要求单 FormSection 内列=档（默认 + 时段档 rules[*]）、行=5 槽，
//   每格 input + 可搜索下拉（pinyinMatch）。默认列行为不退化（下拉 / fillAll / fetchModels），
//   时段档列头点弹窗（WindowsEditModal，ST4 产出）编辑 windows；矩阵主体只管模型值。
//   数据通道分离：默认列 = platform.models；时段档列 = platform.extra.time_windows.rules[*].models。
//   仍纯 props 驱动，无外部 state（矩阵内部仅 UI 临时态：activeCell / editingIdx / importModalOpen）。
import React, { type ReactNode, useState } from "react";
import { slotList } from "../../domains/platforms/slotValue";
import type { TFunction } from "i18next";
import {
  type Protocol, type ModelSlot, type TimeModelRule,
} from "../../services/api";
import type { TimeWindow } from "../../domains/platforms/defaults";
import { type TzMode, utcToDisplay } from "../../utils/timeWindow";
import { pad } from "../../utils/formatters";
import {
  MODEL_SLOTS, DEDICATED_MODEL_SLOTS, isOpenAIFamilyProtocol,
} from "../../domains/platforms";
import type { CustomSlotDef } from "../../services/api/customSlots";
import { FormSection } from "./formSections";
import { WindowsEditModal } from "./WindowsEditModal";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { makeRipple } from "../../components/shared";
import { useModelCandidates } from "./ModelCombobox";
import { ModelChainEditor } from "./ModelChainEditor";

// 单元格 key：默认列 = `d:<slot>`；时段档列 = `r<idx>:<slot>`。整矩阵单开（activeCell 唯一）。
type CellKey = string;

// 紧凑描述：单窗口取首窗口描述；多窗口加「+N」；空 → 「永不命中」占位。
//   时间格式：minute 缺省 → `H-H`（hour 精度）；任一带 minute → `HH:MM-HH:MM`；
//   days_of_week → `(周一,周三)`；days_of_month → `(每月1日)`；跨天 end<start 不特殊标记（描述用区间）；
//   全天 0-24 无 minute 无 day → `全天`。
function weekdayShort(t: TFunction, day: number): string {
  return t(`platform.weekday_short.${day}`);
}

function describeWindow(w: TimeWindow, tzMode: TzMode, t: TFunction): string {
  const isFullDay = w.start_hour === 0 && w.end_hour === 24
    && (w.start_minute ?? 0) === 0 && (w.end_minute ?? 0) === 0
    && !w.days_of_week && !w.days_of_month;
  if (isFullDay) {
    // 全天 + 无 day 过滤（原始 UTC 值判定，全天不受时区换算影响）
    return t("platform.window_all_day");
  }
  // 存储恒 UTC+0，展示按 tzMode 换算（同 WindowsEditModal 输入换算口径）
  const start = utcToDisplay(w.start_hour, w.start_minute ?? 0, tzMode);
  const end = utcToDisplay(w.end_hour, w.end_minute ?? 0, tzMode);
  const hourOnly = start.minute === 0 && end.minute === 0;
  const timePart = hourOnly
    ? `${start.hour}-${end.hour}`
    : `${pad(start.hour)}:${pad(start.minute)}-${pad(end.hour)}:${pad(end.minute)}`;
  let dayPart = "";
  if (w.days_of_week && w.days_of_week.length > 0) {
    dayPart = `(${w.days_of_week.map((d) => weekdayShort(t, d)).join(",")})`;
  } else if (w.days_of_month && w.days_of_month.length > 0) {
    dayPart = `(每月${w.days_of_month.join(",")}日)`;
  }
  const tzLabel = tzMode === "local"
    ? t("platform.timezone_local", "本地")
    : t("platform.timezone_utc", "UTC+0");
  return `${timePart}${dayPart}（${tzLabel}）`;
}

export function describeWindows(windows: TimeWindow[], tzMode: TzMode, t: TFunction): string {
  if (!windows || windows.length === 0) return t("platform.window_never");
  const first = describeWindow(windows[0], tzMode, t);
  if (windows.length === 1) return first;
  return `${first}+${windows.length - 1}`;
}

const LABEL_W = 150;
const rowStyle: React.CSSProperties = {
  display: "flex", gap: 8, width: "100%", alignItems: "center",
};

// 两个 tab 共用的槽位行：标签列（槽名 + 徽标 + 「N 个模型」计数）+ 模型链编辑器(children) + 可选提示 + 可选启用开关。
function SlotRow({ testId, name, count, badge, title, hint, toggle, cellMinWidth = 80, children, t }: {
  testId?: string;
  name: string;
  count: number;
  badge?: ReactNode;
  title?: string;
  /** 编辑器下方的提示（专用槽提示 / 空槽落点提示）。 */
  hint?: ReactNode;
  /** 行尾启用开关。 */
  toggle?: ReactNode;
  cellMinWidth?: number;
  children: ReactNode;
  t: TFunction;
}) {
  return (
    <div style={rowStyle} className="model-matrix-row" data-testid={testId}>
      <div style={{ width: LABEL_W, flexShrink: 0, display: "flex", flexDirection: "column", gap: 1 }} title={title}>
        <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 4, minWidth: 0 }}>
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{name}</span>
          {badge}
        </span>
        <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
          {t("platform.modelCount", "{{n}} 个模型", { n: count })}
        </span>
      </div>
      <div style={{ flex: 1, minWidth: cellMinWidth }}>
        {children}
        {hint}
      </div>
      {toggle}
    </div>
  );
}

export function ModelsMatrixSection({
  // 视图选择器："default" 只渲染默认列（平铺区），"time" 只渲染时段档列（计费 tab）。
  // 2026-10-08 改版：时段档 UI 挪进计费 tab，同一组件渲染两个实例、各取一列视图。
  columns = "default",
  // 默认列 props（沿用 ModelsSection 签名，行为不退化）
  models, onModelsChange,
  // activeDropdown / setActiveDropdown 接受签名兼容，矩阵内部用 activeCell 单开管理；
  //   外部 state 仅同步关闭（避免旧 dropdown 残留），不再驱动矩阵渲染。
  activeDropdown: _activeDropdown, setActiveDropdown: _setActiveDropdown,
  availableModels, protocol,
  // tab 实例（columns="time"）的 card 标题覆盖（缺省「模型」）
  titleOverride,
  fetchError, fetchInfo, fetching,
  onFillAll, onFetchModels, apiKeyMissing, endpointsCount, endpoints = [],
  // 时段档列 props
  rules, setRules, peak,
  // tz 展示模式：表单级共用一份（design.md §3.5 契约），与 PeakSection 的 tzMode/setTzMode 签名逐字一致
  tzMode, setTzMode,
  // 自定义槽位（全局定义 + 本平台值）：仅默认列视图渲染；缺省无自定义行
  customSlotDefs = [], customSlotValues = {}, onCustomSlotChange,
  disabledSlots = [], onSlotEnabledChange,
  platformCalendar,
  t,
}: {
  columns?: "default" | "time";
  titleOverride?: string;
  models: Record<ModelSlot, string[]>;
  onModelsChange: (slot: ModelSlot, chain: string[]) => void;
  activeDropdown: ModelSlot | null;
  setActiveDropdown: React.Dispatch<React.SetStateAction<ModelSlot | null>>;
  availableModels: string[];
  protocol: Protocol;
  fetchError: string;
  /** 一键获取非错误提示（如「已保留手填模型」）；空串不渲染 */
  fetchInfo: string;
  fetching: boolean;
  onFillAll: () => void;
  onFetchModels: () => void;
  apiKeyMissing: boolean;
  endpointsCount: number;
  /** embedding / rerank 提示用：缺省视为无 endpoints。 */
  endpoints?: { protocol: string }[];
  rules: TimeModelRule[];
  setRules: React.Dispatch<React.SetStateAction<TimeModelRule[]>>;
  peak: TimeWindow[];
  tzMode: TzMode;
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>;
  customSlotDefs?: CustomSlotDef[];
  customSlotValues?: Record<string, string[]>;
  onCustomSlotChange?: (name: string, chain: string[]) => void;
  /** 关闭的槽位名（内置 + 自定义）；命中该槽位的请求不会路由到本平台。时段档视图里仅用作新建档开关的预填来源。 */
  disabledSlots?: string[];
  onSlotEnabledChange?: (slot: string, enabled: boolean) => void;
  platformCalendar?: string;
  t: TFunction;
}) {
  // 矩阵内部 UI 临时态
  const [activeCell, setActiveCell] = useState<CellKey | null>(null);
  const [editingIdx, setEditingIdx] = useState<number | null>(null);
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [copyConfirmOpen, setCopyConfirmOpen] = useState(false);
  // 时段档视图（columns="time"）左菜单选中档
  const [selectedRuleIdx, setSelectedRuleIdx] = useState(0);
  // 候选列表（时段档列复用同源）。
  const dropdownSource = useModelCandidates(availableModels, protocol);

  // ─── 时段档列操作（从 TimeModelsSection 移植）───
  // 新建档的开关按「模型」tab 当前内置槽开关预填（自定义槽开关只存平台级，不进时段档）。
  const platformBuiltinOff = disabledSlots.filter((s) => MODEL_SLOTS.some((m) => m.key === s));

  const addRule = () => {
    setRules([...rules, { windows: [], models: {}, disabled_slots: platformBuiltinOff }]);
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

  // 本档开关；档缺省（升级迁移前的旧档）按平台级内置槽开关显示。
  const ruleOff = (rule: TimeModelRule) => rule.disabled_slots ?? platformBuiltinOff;

  const setRuleSlotEnabled = (idx: number, slot: string, enabled: boolean) => {
    const off = ruleOff(rules[idx]).filter((x) => x !== slot);
    updateRule(idx, { disabled_slots: enabled ? off : [...off, slot] });
  };

  const updateRuleModel = (idx: number, slot: ModelSlot, chain: string[]) => {
    setRules(rules.map((r, i) => {
      if (i !== idx) return r;
      const nextModels = { ...r.models };
      if (chain.length > 0) nextModels[slot] = chain;
      else delete nextModels[slot];
      return { ...r, models: nextModels };
    }));
  };

  // 「从模型配置复制」：「模型」tab 内置 8 槽链 + 内置槽开关整份写入本档（自定义槽不进时段档）。
  const copyFromModels = (idx: number) => {
    const copied: Partial<Record<ModelSlot, string[]>> = {};
    for (const { key } of MODEL_SLOTS) if (models[key].length > 0) copied[key] = [...models[key]];
    updateRule(idx, { models: copied, disabled_slots: platformBuiltinOff });
    setCopyConfirmOpen(false);
  };

  const importFromPeak = () => {
    if (peak.length === 0) return;
    const newRule: TimeModelRule = {
      windows: peak.map((w) => ({ ...w })),
      models: {},
      disabled_slots: platformBuiltinOff,
    };
    setRules([...rules, newRule]);
    setImportModalOpen(false);
  };

  // ─── 矩阵渲染 helpers ───
  // 每格 = 共用组合框；cellKey: "d:<slot>" | "r<idx>:<slot>"，整矩阵单开。
  const renderCell = (
    cellKey: string,
    value: string[],
    onChange: (next: string[]) => void,
  ) => (
    <ModelChainEditor
      testId={cellKey.replace(":", "-")} value={value} onChange={onChange}
      source={dropdownSource} protocol={protocol}
      open={activeCell === cellKey}
      onOpenChange={(o) => setActiveCell(o ? cellKey : null)}
      t={t}
    />
  );

  // 与 Rust `has_openai_family_endpoint` 同口径：endpoints 任一 OpenAI 系，或平台协议本身即 OpenAI 系
  const hasOpenAIEndpoint = isOpenAIFamilyProtocol(protocol)
    || endpoints.some((e) => isOpenAIFamilyProtocol(e.protocol));

  // 启用开关：关闭后平台不接收命中该槽位的请求（直接换下个平台，不计平台错误）。
  const enabledToggle = (
    slot: string,
    checked = !disabledSlots.includes(slot),
    onChange: (on: boolean) => void = (on) => onSlotEnabledChange?.(slot, on),
    testPrefix = "slot-enabled",
  ) => (
    <label
      title={t("platform.slotEnabledHint")}
      style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0, fontSize: 14, color: "var(--text-tertiary)", cursor: "pointer" }}
    >
      <Switch
        data-testid={`${testPrefix}-${slot}`}
        checked={checked}
        onCheckedChange={onChange}
      />
      {t("platform.slotEnabled")}
    </label>
  );

  // 专用槽徽标（embedding / rerank）。prefix 区分「模型」tab 与时段档视图的 testid。
  const dedicatedBadge = (key: ModelSlot, prefix = "slot-badge") => DEDICATED_MODEL_SLOTS.includes(key) ? (
    <span
      data-testid={`${prefix}-${key}`}
      style={{ fontSize: 14, lineHeight: "16px", padding: "0 5px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)", color: "var(--text-tertiary)", whiteSpace: "nowrap", flexShrink: 0 }}
    >
      {t("platform.slotDedicated", "专用")}
    </span>
  ) : null;

  // ── 时段档视图（columns === "time"，2026-10-08 三轮裁决）：左侧分层菜单 + 右侧选中档编辑器 ──
  if (columns === "time") {
    const selIdx = Math.min(selectedRuleIdx, rules.length - 1);
    const sel = rules[selIdx];
    const btnMini = {
      padding: "1px 4px", fontSize: 14, height: "auto", minWidth: "auto",
    } as const;
    return (
      <FormSection
        title={titleOverride ?? t("platform.time_windows_section_title", "时段档")}
        action={(
          <div style={{ display: "flex", gap: 6 }}>
            <Button
              variant="ghost"
              size="sm"
              style={{ fontSize: 14, padding: "4px 10px", whiteSpace: "nowrap" }}
              data-testid="time-rule-import-peak"
              disabled={peak.length === 0}
              title={peak.length === 0 ? t("platform.time_windows_no_peak", "当前无高峰时段配置") : ""}
              onClick={() => setImportModalOpen(true)}
            >
              {t("platform.time_windows_import_peak", "从高峰时段导入")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              style={{ fontSize: 14, padding: "4px 10px", color: "var(--accent)" }}
              data-testid="time-rule-add"
              onClick={addRule}
            >
              + {t("platform.time_windows_add_rule", "添加时段档")}
            </Button>
          </div>
        )}
      >
        {rules.length === 0 ? (
          <div style={{ fontSize: 14, color: "var(--text-tertiary)", fontStyle: "italic" }}>
            {t("platform.time_windows_empty", "未配置 → 全时段用默认模型档")}
          </div>
        ) : (
          <div style={{ display: "flex", gap: 16, alignItems: "flex-start" }}>
            {/* 左：时段分层菜单 */}
            <div style={{ flexShrink: 0, width: 200, display: "flex", flexDirection: "column", gap: 4 }}>
              {rules.map((rule, idx) => {
                const on = idx === selIdx;
                return (
                  <div
                    key={idx}
                    onClick={() => setSelectedRuleIdx(idx)}
                    style={{
                      display: "flex", flexDirection: "column", gap: 2,
                      padding: "7px 10px", borderRadius: "var(--radius-sm)", cursor: "pointer",
                      background: on ? "var(--accent-subtle)" : "transparent",
                      border: `1px solid ${on ? "var(--border)" : "transparent"}`,
                    }}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                      <span style={{ fontSize: 14, fontWeight: 600, color: on ? "var(--text-primary)" : "var(--text-secondary)", flex: 1 }}>
                        {t("platform.time_windows_rule_n", "档 {{n}}", { n: idx + 1 })}
                      </span>
                      <Button variant="ghost" size="icon" style={{ ...btnMini }} disabled={idx === 0} onClick={() => moveUp(idx)} title={t("action.moveUp", "上移")}>↑</Button>
                      <Button variant="ghost" size="icon" style={{ ...btnMini }} disabled={idx === rules.length - 1} onClick={() => moveDown(idx)} title={t("action.moveDown", "下移")}>↓</Button>
                      <Button variant="ghost" size="icon" className="btn-danger" style={{ ...btnMini, color: "var(--color-danger)" }} onClick={() => removeRule(idx)} title={t("action.delete", "删除")}>×</Button>
                    </div>
                    <span style={{ fontSize: 14, color: "var(--text-tertiary)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                      {describeWindows(rule.windows, tzMode, t)}
                    </span>
                  </div>
                );
              })}
            </div>
            {/* 右：选中档编辑器 */}
            <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 8 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
                  {describeWindows(sel.windows, tzMode, t)}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  style={{ fontSize: 14, padding: "4px 10px" }}
                  onClick={() => setEditingIdx(selIdx)}
                >
                  {t("platform.time_windows_edit_button", "编辑时段")}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  style={{ fontSize: 14, padding: "4px 10px" }}
                  data-testid="time-rule-copy-models"
                  onClick={() => (MODEL_SLOTS.some(({ key }) => slotList(sel.models[key]).length > 0)
                    ? setCopyConfirmOpen(true) : copyFromModels(selIdx))}
                >
                  {t("platform.time_windows_copy_models", "从模型配置复制")}
                </Button>
              </div>
              {MODEL_SLOTS.map(({ key, labelKey }) => (
                <SlotRow
                  key={key} testId={`time-slot-${key}`} name={t(labelKey)}
                  count={slotList(sel.models[key]).length} cellMinWidth={0}
                  badge={dedicatedBadge(key, "time-slot-badge")} t={t}
                  toggle={enabledToggle(key, !ruleOff(sel).includes(key), (on) => setRuleSlotEnabled(selIdx, key, on), "time-slot-enabled")}
                  hint={!DEDICATED_MODEL_SLOTS.includes(key) ? (
                    // 聊天空槽落点：命中本档后空槽落到本档 default 链首，default 也空 → 透传请求模型名
                    slotList(sel.models[key]).length === 0 && (() => {
                      const head = slotList(sel.models.default)[0];
                      return (
                        <div
                          id={`time-slot-fallback-${key}`}
                          data-state={head ? "default" : "passthrough"}
                          data-model={head}
                          style={{ fontSize: 14, color: "var(--text-tertiary)", marginTop: 2 }}
                        >
                          {head
                            ? t("platform.timeSlotFallbackDefault", "落到本档 default：{{model}}", { model: head })
                            : t("platform.timeSlotFallbackPassthrough", "透传请求模型名")}
                        </div>
                      );
                    })()
                  ) : (
                    slotList(sel.models[key]).length === 0 ? (
                      // 命中时段档整套替换：档内没填 = 时段内本平台不接此类请求
                      <div
                        id={`time-slot-hint-${key}`}
                        data-state="empty"
                        style={{ fontSize: 14, color: "var(--text-tertiary)", marginTop: 2 }}
                      >
                        {t("platform.timeSlotDedicatedEmptyHint", "时段内本平台不接此类请求")}
                      </div>
                    ) : !hasOpenAIEndpoint && (
                      <div
                        id={`time-slot-hint-${key}`}
                        data-state="warn"
                        style={{ fontSize: 14, color: "var(--text-tertiary)", marginTop: 2 }}
                      >
                        {t("platform.dedicatedSlotNoOpenAIEndpointHint")}
                      </div>
                    )
                  )}
                >
                  {renderCell(
                    `r${selIdx}:${key}`,
                    slotList(sel.models[key]),
                    (v) => updateRuleModel(selIdx, key, v),
                  )}
                </SlotRow>
              ))}
            </div>
          </div>
        )}

        <WindowsEditModal
          open={editingIdx !== null}
          windows={editingIdx !== null ? (rules[editingIdx]?.windows ?? []) : []}
          onSave={(w) => {
            if (editingIdx !== null) updateRule(editingIdx, { windows: w });
            setEditingIdx(null);
          }}
          onClose={() => setEditingIdx(null)}
          tzMode={tzMode}
          setTzMode={setTzMode}
          platformCalendar={platformCalendar}
          t={t}
        />

        <Dialog open={copyConfirmOpen} onOpenChange={setCopyConfirmOpen}>
          <DialogContent className="glass-elevated" style={{ maxWidth: 400 }}>
            <DialogHeader>
              <DialogTitle style={{ fontSize: 14 }}>
                {t("platform.time_windows_copy_confirm_title", "覆盖本档模型配置？")}
              </DialogTitle>
              <DialogDescription style={{ fontSize: 14 }}>
                {t("platform.time_windows_copy_confirm_body", "本档已有模型槽内容，将被「模型」tab 当前的内置槽模型与开关整份覆盖。")}
              </DialogDescription>
            </DialogHeader>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <Button variant="ghost" data-testid="time-rule-copy-cancel" onClick={() => setCopyConfirmOpen(false)}>
                {t("action.cancel", "取消")}
              </Button>
              <Button data-testid="time-rule-copy-confirm" className="ripple" onClick={(e) => { makeRipple(e); copyFromModels(selIdx); }}>
                {t("platform.time_windows_copy_confirm_button", "覆盖")}
              </Button>
            </div>
          </DialogContent>
        </Dialog>

        <Dialog open={importModalOpen} onOpenChange={(v) => { if (!v) setImportModalOpen(false); }}>
          <DialogContent className="glass-elevated" style={{ maxWidth: 400 }}>
            <DialogHeader>
              <DialogTitle style={{ fontSize: 14 }}>
                {t("platform.time_windows_import_confirm_title", "导入高峰时段配置？")}
              </DialogTitle>
              <DialogDescription style={{ fontSize: 14 }}>
                {t("platform.time_windows_import_confirm_body", "将基于当前高峰时段（{{count}} 个窗口）创建新规则，独立可编辑。").replace("{{count}}", String(peak.length))}
              </DialogDescription>
            </DialogHeader>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <Button variant="ghost" onClick={() => setImportModalOpen(false)}>
                {t("action.cancel", "取消")}
              </Button>
              <Button data-testid="time-rule-import-confirm" className="ripple" onClick={(e) => { makeRipple(e); importFromPeak(); }}>
                {t("platform.time_windows_import_confirm_button", "确认")}
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      </FormSection>
    );
  }

  return (
    <FormSection
      title={titleOverride ?? t("platform.models")}
      action={(
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          <Button
            variant="ghost"
            size="sm"
            style={{ fontSize: 14, gap: 4, padding: "4px 10px", color: "var(--text-secondary)" }}
            onClick={onFillAll}
            disabled={models.default.length === 0}
            title={t("platform.fillAllHint")}
          >
            {t("platform.fillAll")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            style={{ fontSize: 14, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
            onClick={onFetchModels}
            disabled={apiKeyMissing || endpointsCount === 0 || fetching}
          >
            {fetching ? t("status.loading") : t("platform.fetchModels")}
          </Button>
        </div>
      )}
    >
      {fetchError && (
        <div style={{ fontSize: 14, color: "var(--color-danger)", padding: "2px 0" }}>
          {fetchError}
        </div>
      )}
      {fetchInfo && (
        <div data-testid="fetch-info" style={{ fontSize: 14, color: "var(--text-secondary)", padding: "2px 0" }}>
          {fetchInfo}
        </div>
      )}

      {/* 矩阵主体：横向滚动，列宽固定 */}
      <div style={{ overflowX: "auto", paddingBottom: 4 }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 6, width: "100%" }}>
          {/* 列头行 */}
          <div style={rowStyle}>
            <div style={{ width: LABEL_W, flexShrink: 0 }} />
            {/* 默认列头 */}
            <div style={{ flex: 1, minWidth: 80, textAlign: "center", fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
              {t("platform.modelDefault")}
            </div>
          </div>

          {/* 5 槽行 */}
          {MODEL_SLOTS.map(({ key, labelKey }) => (
            <SlotRow
              key={key} testId={`model-slot-${key}`} name={t(labelKey)} count={models[key].length}
              badge={dedicatedBadge(key)} toggle={enabledToggle(key)} t={t}
              hint={DEDICATED_MODEL_SLOTS.includes(key) && models[key].length > 0 && !hasOpenAIEndpoint && (
                <div
                  id={`slot-hint-${key}`}
                  data-state="warn"
                  style={{ fontSize: 14, color: "var(--text-tertiary)", marginTop: 2 }}
                >
                  {t("platform.dedicatedSlotNoOpenAIEndpointHint")}
                </div>
              )}
            >
              {renderCell(`d:${key}`, models[key], (v) => onModelsChange(key, v))}
            </SlotRow>
          ))}

          {customSlotDefs.map(({ name, match }) => (
            <SlotRow
              key={name} testId={`custom-slot-${name}`} name={name} title={name}
              count={(customSlotValues[name] ?? []).length} toggle={enabledToggle(name)} t={t}
              badge={(
                <span
                  data-testid={`custom-slot-badge-${name}`} data-state={match}
                  style={{ fontSize: 14, lineHeight: "16px", padding: "0 5px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)", color: "var(--text-secondary)", whiteSpace: "nowrap", flexShrink: 0 }}
                >
                  {t(match === "exact" ? "customSlots.matchExact" : "customSlots.matchContains")}
                </span>
              )}
            >
              {renderCell(`c:${name}`, customSlotValues[name] ?? [], (v) => onCustomSlotChange?.(name, v))}
            </SlotRow>
          ))}

          {rules.length === 0 && (
            <div style={{ fontSize: 14, color: "var(--text-tertiary)", fontStyle: "italic", paddingLeft: LABEL_W + 8 }}>
              {t("platform.time_windows_empty", "未配置 → 全时段用默认模型档")}
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
        tzMode={tzMode}
        setTzMode={setTzMode}
        t={t}
      />

      {/* 导入高峰时段确认 modal */}
      <Dialog open={importModalOpen} onOpenChange={(v) => { if (!v) setImportModalOpen(false); }}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 400 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 14 }}>
              {t("platform.time_windows_import_confirm_title", "导入高峰时段配置？")}
            </DialogTitle>
            <DialogDescription style={{ fontSize: 14 }}>
              {t("platform.time_windows_import_confirm_body", "将基于当前高峰时段（{{count}} 个窗口）创建新规则，独立可编辑。").replace("{{count}}", String(peak.length))}
            </DialogDescription>
          </DialogHeader>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button
              variant="ghost"
              onClick={() => setImportModalOpen(false)}
            >
              {t("action.cancel", "取消")}
            </Button>
            <Button
              className="ripple"
              onClick={(e) => { makeRipple(e); importFromPeak(); }}
            >
              {t("platform.time_windows_import_confirm_button", "确认")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </FormSection>
  );
}
