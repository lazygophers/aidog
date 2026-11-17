import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { settingsApi } from "../services/api";
import { variablesApi, type Variable } from "../services/api/variables";
const KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]{0,127}$/;

export function Variables() {
  const { t } = useTranslation();
  const [variables, setVariables] = useState<Variable[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    settingsApi.get("global", "extension_variables")
      .then(saved => setVariables((saved ?? []) as Variable[]))
      .catch(e => setError(String(e)));
  }, []);

  const update = (index: number, patch: Partial<Variable>) => setVariables(current => current.map((item, i) => i === index ? { ...item, ...patch } : item));
  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const invalid = variables.find(v => !KEY_PATTERN.test(v.key) || new TextEncoder().encode(v.value).length > 8192);
      if (invalid) throw new Error("变量名须符合 [A-Za-z_][A-Za-z0-9_]*，最多 128 字符；变量值最多 8192 字节");
      await variablesApi.save(variables);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return <section className="space-y-5" data-testid="variables-page">
    <header><h1 className="text-xl font-semibold">{t("variables.title", "变量")}</h1><p className="mt-1 text-sm text-muted-foreground">{t("variables.desc", "环境变量独立管理：同名时附加并覆盖 Claude Code 与 Codex 的现有值，两边同时生效。清空值即撤销该变量（恢复覆盖前的原值）。")}</p></header>
    {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
    <div className="space-y-3">
      {variables.map((item, index) => <div key={index} className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,2fr)_auto] items-center gap-3 rounded-lg border p-3">
        <Input aria-label={t("variables.name", "变量名")} value={item.key} onChange={e => update(index, { key: e.target.value })} />
        <Input aria-label={t("variables.value", "变量值")} value={item.value} onChange={e => update(index, { value: e.target.value })} />
        <Button variant="ghost" onClick={() => setVariables(current => current.filter((_, i) => i !== index))} aria-label={t("action.remove", "移除")}>×</Button>
      </div>)}
    </div>
    <div className="flex gap-2">
      <Button data-testid="variable-add" variant="outline" onClick={() => setVariables(current => [...current, { key: "", value: "" }])}>{t("variables.add", "添加变量")}</Button>
      <Button data-testid="variables-save" disabled={busy} onClick={() => void save()}>{t("action.save", "保存")}</Button>
    </div>
  </section>;
}
