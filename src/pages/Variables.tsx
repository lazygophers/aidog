import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { configApi, settingsApi, settingsPresetApi } from "../services/api";
import { variablesApi, type Variable } from "../services/api/variables";
const KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]{0,127}$/;

export function Variables() {
  const { t } = useTranslation();
  const [variables, setVariables] = useState<Variable[]>([]);
  const [claude, setClaude] = useState<Record<string, unknown>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    Promise.all([settingsApi.get("global", "claude_code"), settingsApi.get("global", "extension_variables")]).then(async ([config, saved]) => {
      // DB 空（从未保存过）时回落内置 oss 预设：保存是整份写回 claude_code，
      // 空对象起笔会让分组同步整份覆盖预设（hooks / env 等全丢），与插件页同病。
      let value = config ?? {};
      if (!Object.keys(value).length) {
        const presets = await settingsPresetApi.list("claude");
        const preset = presets.find(p => p.id === "oss");
        if (!preset) throw new Error("Missing oss settings preset");
        value = preset.config;
      }
      const env = (value.env ?? {}) as Record<string, string>;
      const records = (saved ?? []) as Variable[];
      const byKey = new Map(records.map(item => [item.key, item]));
      for (const [key, val] of Object.entries(env)) {
        byKey.set(key, { ...(byKey.get(key) ?? { key, value: val, codex: false }), key, value: val, claude: true });
      }
      for (const item of records) if (!(item.key in env)) byKey.set(item.key, { ...item, claude: false });
      setClaude(value);
      setVariables([...byKey.values()]);
    }).catch(e => setError(String(e)));
  }, []);

  const update = (index: number, patch: Partial<Variable>) => setVariables(current => current.map((item, i) => i === index ? { ...item, ...patch } : item));
  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const invalid = variables.find(v => !KEY_PATTERN.test(v.key) || new TextEncoder().encode(v.value).length > 8192);
      if (invalid) throw new Error("变量名须符合 [A-Za-z_][A-Za-z0-9_]*，最多 128 字符；变量值最多 8192 字节");
      const env = Object.fromEntries(variables.filter(v => v.claude && v.value !== "").map(v => [v.key, v.value]));
      const next = { ...claude, env };
      await settingsApi.set("global", "claude_code", next);
      setClaude(next);
      await configApi.syncGroupSettings();
      const conflicts = await variablesApi.saveCodex(variables);
      await settingsApi.set("global", "extension_variables", variables);
      if (conflicts.length) throw new Error(`Codex 中这些变量已由用户设置，未覆盖：${conflicts.join(", ")}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return <section className="space-y-5" data-testid="variables-page">
    <header><h1 className="text-xl font-semibold">{t("variables.title", "变量")}</h1><p className="mt-1 text-sm text-muted-foreground">{t("variables.desc", "为 Claude Code 与 Codex 管理环境变量")}</p></header>
    {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
    <div className="space-y-3">
      {variables.map((item, index) => <div key={index} className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,2fr)_auto_auto_auto] items-center gap-3 rounded-lg border p-3">
        <Input aria-label={t("variables.name", "变量名")} value={item.key} onChange={e => update(index, { key: e.target.value })} />
        <Input aria-label={t("variables.value", "变量值")} value={item.value} onChange={e => update(index, { value: e.target.value })} />
        <label className="flex items-center gap-2"><Switch checked={item.claude} onCheckedChange={claude => update(index, { claude })} data-testid="variable-claude" />Claude Code</label>
        <label className="flex items-center gap-2"><Switch checked={item.codex} onCheckedChange={codex => update(index, { codex })} data-testid="variable-codex" />Codex</label>
        <Button variant="ghost" onClick={() => setVariables(current => current.filter((_, i) => i !== index))} aria-label={t("action.remove", "移除")}>×</Button>
      </div>)}
    </div>
    <div className="flex gap-2">
      <Button data-testid="variable-add" variant="outline" onClick={() => setVariables(current => [...current, { key: "", value: "", claude: false, codex: false }])}>{t("variables.add", "添加变量")}</Button>
      <Button data-testid="variables-save" disabled={busy} onClick={() => void save()}>{t("action.save", "保存")}</Button>
    </div>
  </section>;
}
