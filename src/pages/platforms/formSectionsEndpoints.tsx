// formSectionsEndpoints — 编辑页大分区（Endpoints / Models），从 formSections.tsx 拆出控制行数。
// ponytail: 这两个分区各 ~110-160 行，独立成文件使 formSections.tsx ≤600。仍纯 props 驱动。
import React from "react";
import type { TFunction } from "i18next";
import {
  type Protocol, type ModelSlot, type PlatformEndpoint, type ClientType,
} from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import {
  ENDPOINT_PROTOCOLS, CLIENT_TYPES, MODEL_SLOTS,
  getDefaultModelList, defaultClientForProtocol,
} from "../../domains/platforms";
import { FormSection } from "./formSections";

export function EndpointsSection({ endpoints, setEndpoints, t }: {
  endpoints: PlatformEndpoint[];
  setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.endpoints", "Protocol Endpoints")}
      desc={t("platform.endpointsHint", "Additional protocols this platform supports with different base URLs")}
      action={(
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
          onClick={() => setEndpoints([...endpoints, { protocol: "openai" as Protocol, base_url: "", client_type: defaultClientForProtocol("openai"), coding_plan: false }])}
        >
          + {t("platform.addEndpoint", "Add Endpoint")}
        </button>
      )}
    >
      {endpoints.length === 0 && (
        <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "4px 0", fontStyle: "italic" }}>
          {t("platform.noEndpoints", "No additional endpoints")}
        </div>
      )}
      {endpoints.map((ep, idx) => (
        <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <select
            className="input"
            style={{ width: 120, flexShrink: 0 }}
            value={ep.protocol}
            onChange={(e) => {
              const newProto = e.target.value as Protocol;
              const next = [...endpoints];
              next[idx] = { ...next[idx], protocol: newProto, client_type: defaultClientForProtocol(newProto) };
              setEndpoints(next);
            }}
          >
            {ENDPOINT_PROTOCOLS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
          <input
            className="input"
            style={{ flex: 1 }}
            placeholder="Endpoint Base URL"
            value={ep.base_url}
            onChange={(e) => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], base_url: e.target.value };
              setEndpoints(next);
            }}
          />
          <select
            className="input"
            style={{ width: 140, flexShrink: 0 }}
            value={ep.client_type || "default"}
            onChange={(e) => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], client_type: e.target.value as ClientType };
              setEndpoints(next);
            }}
            title={t("platform.clientType", "客户端模拟")}
          >
            <option value="default">{t(CLIENT_TYPES[0].labelKey!)}</option>
            {["Claude Code", "Codex", "IDE"].map(group => (
              <optgroup key={group} label={group}>
                {CLIENT_TYPES.filter(c => c.group === group).map(c => (
                  <option key={c.value} value={c.value}>{c.label}</option>
                ))}
              </optgroup>
            ))}
          </select>
          {/* Coding Plan 开关 */}
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{
              flexShrink: 0,
              width: 28, height: 28, minWidth: 28,
              padding: 0,
              fontSize: 11, fontWeight: 700,
              color: ep.coding_plan ? "var(--color-success, var(--color-success))" : "var(--text-tertiary)",
              background: ep.coding_plan ? "var(--color-success, var(--color-success))15" : "transparent",
              border: `1px solid ${ep.coding_plan ? "var(--color-success, var(--color-success))40" : "var(--border)"}`,
              borderRadius: "var(--radius-sm)",
            }}
            title={ep.coding_plan ? "Coding Plan ON" : "Coding Plan"}
            onClick={() => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], coding_plan: !next[idx].coding_plan };
              setEndpoints(next);
            }}
          >
            C
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-icon btn-danger"
            style={{ flexShrink: 0 }}
            onClick={() => setEndpoints(endpoints.filter((_, i) => i !== idx))}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
            </svg>
          </button>
        </div>
      ))}
    </FormSection>
  );
}

export function ModelsSection({ models, handleModelChange, handleModelSelect, activeDropdown, setActiveDropdown, availableModels, protocol, codingPlan, fetchError, fetching, onFillAll, onFetchModels, apiKeyMissing, endpointsCount, t }: {
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
  t: TFunction;
}) {
  return (
    <FormSection
      title={t("platform.models")}
      action={(
        <div style={{ display: "flex", gap: 6 }}>
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
        </div>
      )}
    >
      {fetchError && (
        <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
          {fetchError}
        </div>
      )}
      {MODEL_SLOTS.map(({ key, labelKey }) => {
        const query = models[key].trim().toLowerCase();
        // 下拉源：fetchModels 成功用 available_models，否则用内置候选列表（冷启动兜底）
        const dropdownSource = availableModels.length > 0
          ? availableModels
          : getDefaultModelList(protocol, codingPlan);
        const hasDropdown = dropdownSource.length > 0;
        const filtered = hasDropdown
          ? (query
            ? dropdownSource.filter(m => pinyinMatch(query, m))
            : dropdownSource)
          : [];
        return (
        <div key={key} style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{
            fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)",
            width: 56, textAlign: "right", flexShrink: 0,
          }}>
            {t(labelKey)}
          </span>
          <div style={{ position: "relative", flex: 1 }}>
            <input
              className="input"
              style={{ width: "100%", paddingRight: hasDropdown ? 28 : undefined }}
              placeholder={t(labelKey)}
              value={models[key]}
              onChange={(e) => {
                handleModelChange(key, e.target.value);
                if (hasDropdown) setActiveDropdown(key);
              }}
              onFocus={() => {
                if (hasDropdown) setActiveDropdown(key);
              }}
            />
            {hasDropdown && (
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                style={{
                  position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
                  width: 24, height: 24, minWidth: 24, padding: 0,
                  color: "var(--text-tertiary)", cursor: "pointer",
                }}
                onMouseDown={(e) => {
                  e.preventDefault();
                  setActiveDropdown(activeDropdown === key ? null : key);
                }}
                title={t("platform.selectModel")}
              >
                ▾
              </button>
            )}
            {/* 可搜索下拉列表 — 主题化 */}
            {activeDropdown === key && filtered.length > 0 && (
              <>
                <div
                  style={{ position: "fixed", inset: 0, zIndex: 99 }}
                  onMouseDown={() => setActiveDropdown(null)}
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
                        fontWeight: models[key] === m ? 600 : 400,
                        color: models[key] === m ? "var(--accent)" : "var(--text-primary)",
                        background: models[key] === m ? "var(--accent-subtle)" : "transparent",
                        borderRadius: "var(--radius-sm)",
                      }}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        handleModelSelect(key, m);
                        setActiveDropdown(null);
                      }}
                    >
                      {m}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
        );
      })}
    </FormSection>
  );
}
