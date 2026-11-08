// 隐私脱敏（system tab）：总开关 + 机密/个人信息两档 + 单类型细粒度覆盖 + 本地模型后端。
// 试一试预览走生产 scrub 路径（privacy_scrub_preview，票 04）。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { privacyApi, type PrivacyScrubPreview, type PrivacyScrubSettings } from "../../services/api/privacy";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";

/** 词表与 Rust `PiiKind::as_str` 逐字一致。 */
const KINDS: { key: string; labelKey: string; cat: "secrets" | "personal" }[] = [
  { key: "API_KEY", labelKey: "privacy.kindApiKey", cat: "secrets" },
  { key: "PRIVATE_KEY", labelKey: "privacy.kindPrivateKey", cat: "secrets" },
  { key: "BEARER_TOKEN", labelKey: "privacy.kindBearerToken", cat: "secrets" },
  { key: "EMAIL", labelKey: "privacy.kindEmail", cat: "personal" },
  { key: "BANK_CARD", labelKey: "privacy.kindBankCard", cat: "personal" },
  { key: "ID_NUMBER", labelKey: "privacy.kindIdNumber", cat: "personal" },
];

export function PrivacyScrubSection() {
  const { t } = useTranslation();
  const [s, setS] = useState<PrivacyScrubSettings | null>(null);
  const [loadError, setLoadError] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [previewInput, setPreviewInput] = useState("");
  const [preview, setPreview] = useState<PrivacyScrubPreview | null>(null);
  const [previewError, setPreviewError] = useState("");

  useEffect(() => {
    privacyApi.get().then(setS).catch((e) => setLoadError(String(e)));
  }, []);

  if (!s) {
    return (
      <div className="glass-surface" style={{ padding: "16px 20px" }}>
        {loadError && <span style={{ color: "var(--destructive, #d33)", fontSize: 12 }}>{loadError}</span>}
      </div>
    );
  }

  const kindOn = (k: (typeof KINDS)[number]) =>
    s.per_kind[k.key] ?? (k.cat === "secrets" ? s.secrets : s.personal);

  const save = async () => {
    setSaved(false);
    setSaveError("");
    try {
      await privacyApi.set(s);
      setSaved(true);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  const runPreview = async () => {
    setPreview(null);
    setPreviewError("");
    try {
      setPreview(await privacyApi.preview(previewInput));
    } catch (e) {
      setPreviewError(String(e));
    }
  };

  return (
    <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("privacy.title")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t("privacy.desc")}</div>
      </div>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        <Switch data-testid="privacy-enabled" checked={s.enabled} onCheckedChange={(v) => setS({ ...s, enabled: v })} />
        {t("privacy.enabled")}
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        <Switch data-testid="privacy-secrets" checked={s.secrets} onCheckedChange={(v) => setS({ ...s, secrets: v })} />
        {t("privacy.secrets")}
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        <Switch data-testid="privacy-personal" checked={s.personal} onCheckedChange={(v) => setS({ ...s, personal: v })} />
        {t("privacy.personal")}
      </label>
      <div style={{ display: "grid", gap: 6 }}>
        {KINDS.map((k) => {
          const on = kindOn(k);
          return (
            <label
              key={k.key}
              data-testid={`privacy-kind-${k.key}`}
              data-state={on ? "on" : "off"}
              style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12, paddingInlineStart: 16 }}
            >
              <Switch
                data-testid={`privacy-kind-switch-${k.key}`}
                checked={on}
                onCheckedChange={() => setS({ ...s, per_kind: { ...s.per_kind, [k.key]: !on } })}
              />
              {t(k.labelKey)}
            </label>
          );
        })}
        <div className="text-secondary" style={{ fontSize: 11 }}>{t("privacy.perKindHint")}</div>
      </div>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        <Switch
          data-testid="privacy-model-enabled"
          checked={s.model_enabled}
          onCheckedChange={(v) => setS({ ...s, model_enabled: v })}
        />
        {t("privacy.modelEnabled")}
      </label>
      <Input
        data-testid="privacy-model-endpoint"
        value={s.model_endpoint}
        disabled={!s.model_enabled}
        placeholder={t("privacy.modelEndpointPlaceholder")}
        onChange={(e) => setS({ ...s, model_endpoint: e.target.value })}
      />
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button data-testid="privacy-save" size="sm" onClick={save}>{t("privacy.save")}</Button>
        {saved && <span data-testid="privacy-saved" className="text-secondary" style={{ fontSize: 12 }}>{t("privacy.saved")}</span>}
        {saveError && (
          <span data-testid="privacy-save-error" style={{ fontSize: 12, color: "var(--destructive, #d33)" }}>
            {saveError}
          </span>
        )}
      </div>
      <div style={{ borderTop: "1px solid var(--border)", paddingTop: 12, display: "flex", flexDirection: "column", gap: 8 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("privacy.preview.title")}</div>
        <Textarea
          data-testid="privacy-preview-input"
          value={previewInput}
          rows={3}
          placeholder={t("privacy.preview.placeholder")}
          onChange={(e) => setPreviewInput(e.target.value)}
        />
        <div>
          <Button data-testid="privacy-preview-run" size="sm" disabled={!previewInput.trim()} onClick={runPreview}>
            {t("privacy.preview.run")}
          </Button>
        </div>
        {preview && (
          <>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
              <div>
                <div className="text-secondary" style={{ fontSize: 11 }}>{t("privacy.preview.original")}</div>
                <pre data-testid="privacy-preview-original" style={{ fontSize: 12, whiteSpace: "pre-wrap", wordBreak: "break-all", margin: 0 }}>{preview.original}</pre>
              </div>
              <div>
                <div className="text-secondary" style={{ fontSize: 11 }}>{t("privacy.preview.scrubbed")}</div>
                <pre data-testid="privacy-preview-scrubbed" style={{ fontSize: 12, whiteSpace: "pre-wrap", wordBreak: "break-all", margin: 0 }}>{preview.scrubbed}</pre>
              </div>
            </div>
            <span data-testid="privacy-preview-hits" data-count={preview.hits} className="text-secondary" style={{ fontSize: 12 }}>
              {t("privacy.preview.hits", { count: preview.hits })}
            </span>
          </>
        )}
        {previewError && (
          <span data-testid="privacy-preview-error" style={{ fontSize: 12, color: "var(--destructive, #d33)" }}>
            {previewError}
          </span>
        )}
      </div>
    </div>
  );
}
