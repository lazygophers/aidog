import { useTranslation } from "react-i18next";
import type { MockConfig, MockErrorMode } from "../../services/api";
import { MOCK_ERROR_MODES } from "./constants";

/** Mock 平台配置编辑器：编辑 platform.extra 的 mock 子对象 */
export interface MockConfigEditorProps {
  config: MockConfig;
  onChange: (next: MockConfig) => void;
}

export function MockConfigEditor({ config, onChange }: MockConfigEditorProps) {
  const { t } = useTranslation();
  const setField = <K extends keyof MockConfig>(key: K, value: MockConfig[K]) => {
    onChange({ ...config, [key]: value });
  };

  const numberField = (label: string, key: "status_code" | "delay_ms" | "input_tokens" | "output_tokens" | "cache_tokens" | "chunk_count", hint?: string) => (
    <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{label}</span>
      <input
        className="input"
        type="number"
        value={config[key]}
        onChange={(e) => setField(key, Number(e.target.value))}
      />
      {hint && <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>{hint}</span>}
    </label>
  );

  // stream_override: null=跟随请求 / true / false → 用三态下拉
  const streamValue = config.stream_override === null ? "follow" : config.stream_override ? "force_on" : "force_off";

  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 12,
      padding: 12, borderRadius: "var(--radius-sm)",
      background: "var(--bg-glass)", border: "1px solid var(--border)",
    }}>
      <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
        {t("platform.mockConfig")}（{t("platform.mockConfigHint")}）
      </div>

      {/* 响应文本 */}
      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockResponseText")}（response_text）</span>
        <textarea
          className="input"
          style={{ minHeight: 60, resize: "vertical" }}
          value={config.response_text}
          onChange={(e) => setField("response_text", e.target.value)}
        />
      </label>

      <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>finish_reason</span>
        <input
          className="input"
          value={config.finish_reason}
          onChange={(e) => setField("finish_reason", e.target.value)}
        />
      </label>

      {/* 数值字段网格 */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        {numberField(`${t("platform.mockStatusCode")}（status_code）`, "status_code")}
        {numberField(`${t("platform.mockDelayMs")}（delay_ms）`, "delay_ms")}
        {numberField(`${t("platform.mockInputTokens")}（input_tokens）`, "input_tokens")}
        {numberField(`${t("platform.mockOutputTokens")}（output_tokens）`, "output_tokens")}
        {numberField(`${t("platform.mockCacheTokens")}（cache_tokens）`, "cache_tokens")}
        {numberField(`${t("platform.mockChunkCount")}（chunk_count）`, "chunk_count")}
      </div>

      {/* error_mode + stream_override */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockErrorMode")}（error_mode）</span>
          <select
            className="input"
            value={config.error_mode}
            onChange={(e) => setField("error_mode", e.target.value as MockErrorMode)}
          >
            {MOCK_ERROR_MODES.map((m) => (
              <option key={m.value} value={m.value}>{t(m.labelKey)}</option>
            ))}
          </select>
        </label>
        <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{t("platform.mockStreamOverride")}（stream_override）</span>
          <select
            className="input"
            value={streamValue}
            onChange={(e) => {
              const v = e.target.value;
              setField("stream_override", v === "follow" ? null : v === "force_on");
            }}
          >
            <option value="follow">{t("platform.mockStreamFollow")}（null）</option>
            <option value="force_on">{t("platform.mockStreamForceOn")}（true）</option>
            <option value="force_off">{t("platform.mockStreamForceOff")}（false）</option>
          </select>
        </label>
      </div>
    </div>
  );
}
