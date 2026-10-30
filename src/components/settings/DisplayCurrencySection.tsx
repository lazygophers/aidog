// 显示币种设置（system tab）：币种选择 + 汇率覆盖编辑。保存走 display_currency_set（票 01），
// 成功后用返回值写 store（票 02），同窗口其它订阅者即时刷新。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { displayCurrencyApi } from "../../services/api/currency";
import { setDisplayCurrency, useDisplayCurrency } from "../../utils/displayCurrency";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const parseRate = (v: string): number | null => {
  const n = Number(v);
  return v.trim() !== "" && Number.isFinite(n) && n > 0 ? n : null;
};

export function DisplayCurrencySection() {
  const { t } = useTranslation();
  const cur = useDisplayCurrency();
  const [code, setCode] = useState(cur.code);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, boolean>>({});
  const [saveError, setSaveError] = useState("");
  const [saved, setSaved] = useState(false);

  // store 更新（首次加载 / 保存成功 / 事件）时以其为准重置草稿
  useEffect(() => {
    setCode(cur.code);
    setDraft(Object.fromEntries(cur.table.map((r) => [r.code, String(r.rate)])));
    setErrors({});
  }, [cur]);

  const save = async () => {
    const rates: Record<string, number> = {};
    const bad: Record<string, boolean> = {};
    for (const r of cur.table) {
      if (r.code === "USD") continue;
      const n = parseRate(draft[r.code] ?? "");
      if (n === null) bad[r.code] = true;
      else if (n !== r.default_rate) rates[r.code] = n;
    }
    setErrors(bad);
    setSaved(false);
    if (Object.keys(bad).length) return;
    try {
      setSaveError("");
      setDisplayCurrency(await displayCurrencyApi.set(code, rates));
      setSaved(true);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  return (
    <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("displayCurrency.title")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t("displayCurrency.desc")}</div>
      </div>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("displayCurrency.select")}
        <select
          data-testid="currency-select"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          style={{ background: "transparent", border: "1px solid var(--border)", borderRadius: 6, padding: "4px 8px", color: "inherit" }}
        >
          {cur.table.map((r) => (
            <option key={r.code} value={r.code}>{r.symbol} {r.code}</option>
          ))}
        </select>
      </label>
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr auto auto", alignItems: "center", gap: "8px 12px", fontSize: 12 }}>
        {cur.table.map((r) => {
          const usd = r.code === "USD";
          return (
            <div key={r.code} style={{ display: "contents" }}>
              <span style={{ fontWeight: 600 }}>{r.symbol} {r.code}</span>
              <div>
                <Input
                  data-testid={`currency-rate-${r.code}`}
                  value={draft[r.code] ?? ""}
                  disabled={usd}
                  title={usd ? t("displayCurrency.usdFixed") : t("displayCurrency.rate")}
                  onChange={(e) => setDraft({ ...draft, [r.code]: e.target.value })}
                />
                {errors[r.code] && (
                  <div data-testid={`currency-error-${r.code}`} style={{ color: "var(--destructive, #d33)" }}>
                    {t("displayCurrency.invalidRate")}
                  </div>
                )}
              </div>
              <span className="text-secondary">{t("displayCurrency.defaultRate")} {r.default_rate}</span>
              {usd ? <span /> : (
                <Button
                  variant="ghost"
                  size="sm"
                  data-testid={`currency-restore-${r.code}`}
                  onClick={() => setDraft({ ...draft, [r.code]: String(r.default_rate) })}
                >
                  {t("displayCurrency.restore")}
                </Button>
              )}
            </div>
          );
        })}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button data-testid="currency-save" size="sm" onClick={save}>{t("displayCurrency.save")}</Button>
        {saved && <span className="text-secondary" style={{ fontSize: 12 }}>{t("displayCurrency.saved")}</span>}
        {saveError && <span data-testid="currency-save-error" style={{ fontSize: 12, color: "var(--destructive, #d33)" }}>{saveError}</span>}
      </div>
    </div>
  );
}
