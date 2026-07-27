import { useTranslation } from "react-i18next";
import { makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { CreateCliProxyProvider, Group } from "@/services/api";
import { quotaTypeOf } from "./quotaTypeOf";
import { NONE, fieldLabel } from "./constants";

// 编辑/新增 inline form，从 CliProxy/index.tsx 原地搬出（c10）。
interface ProviderFormPanelProps {
  editingId: number | "new";
  form: CreateCliProxyProvider;
  setForm: (f: CreateCliProxyProvider) => void;
  modelsText: string;
  setModelsText: (v: string) => void;
  groups: Group[];
  busy: boolean;
  onCancel: () => void;
  onSave: () => void;
}

export function ProviderFormPanel({
  editingId, form, setForm, modelsText, setModelsText, groups, busy, onCancel, onSave,
}: ProviderFormPanelProps) {
  const { t } = useTranslation();
  return (
    <div className="glass-surface" style={{
      padding: 16,
      display: "flex", flexDirection: "column", gap: 12,
    }}>
      <div style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>
        {editingId === "new" ? t("cliProxy.add") : t("cliProxy.edit")}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 12 }}>
        <label style={fieldLabel}>
          {t("cliProxy.name")}
          <Input
            value={form.name}
            onChange={e => setForm({ ...form, name: e.target.value })}
            placeholder="claude-official"
          />
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.wireProtocol")}
          <Select
            value={form.wire_protocol}
            onValueChange={v => setForm({ ...form, wire_protocol: v })}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {["anthropic", "openai", "openai_responses", "openai_completions", "gemini", "glm_coding"].map(v => (
                <SelectItem key={v} value={v}>{v}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
        <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
          {t("cliProxy.baseUrl")}
          <Input
            value={form.base_url}
            onChange={e => setForm({ ...form, base_url: e.target.value })}
            placeholder="https://api.anthropic.com/v1"
          />
        </label>
        <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
          {t("cliProxy.apiKey")}
          <Input
            type="password"
            value={form.api_key ?? ""}
            onChange={e => setForm({ ...form, api_key: e.target.value })}
            placeholder="sk-..."
          />
        </label>
        <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
          {t("cliProxy.models")}
          <Textarea
            style={{ minHeight: 80, resize: "vertical" }}
            value={modelsText}
            onChange={e => setModelsText(e.target.value)}
            placeholder={t("cliProxy.modelsHint")}
          />
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.status")}
          <Select
            value={form.status ?? "active"}
            onValueChange={v => setForm({ ...form, status: v })}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="active">{t("cliProxy.statusActive")}</SelectItem>
              <SelectItem value="disabled">{t("cliProxy.statusDisabled")}</SelectItem>
            </SelectContent>
          </Select>
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.groupId")}
          <Select
            value={form.group_id == null ? NONE : String(form.group_id)}
            onValueChange={v => setForm({ ...form, group_id: v === NONE ? null : Number(v) })}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE}>—</SelectItem>
              {groups.map(g => (
                <SelectItem key={g.id} value={String(g.id)}>{g.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
        <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
          {t("cliProxy.extra")}
          <Input
            value={form.extra ?? ""}
            onChange={e => setForm({ ...form, extra: e.target.value })}
            placeholder="{}"
          />
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.quotaType")}
          <Select
            value={quotaTypeOf(form.quota)}
            onValueChange={v => setForm({ ...form, quota: JSON.stringify({ type: v }) })}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">{t("cliProxy.quotaTypeNone")}</SelectItem>
              <SelectItem value="newapi">{t("cliProxy.quotaTypeNewapi")}</SelectItem>
            </SelectContent>
          </Select>
        </label>
      </div>
      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <Button
          variant="ghost"
          className="ripple"
          onClick={(e) => { makeRipple(e); onCancel(); }}
          disabled={busy}
        >
          {t("cliProxy.cancel")}
        </Button>
        <Button
          variant="default"
          className="ripple"
          onClick={(e) => { makeRipple(e); onSave(); }}
          disabled={busy}
        >
          {t("cliProxy.save")}
        </Button>
      </div>
    </div>
  );
}
