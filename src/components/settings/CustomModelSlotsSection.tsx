// 自定义模型槽位定义（system tab）：槽名 + 匹配方式。各平台的值在平台编辑页填（票 02），路由见票 03。
// 删除前显示已填值平台数，确认后先写定义再清各平台 extra.custom_slots[槽名]。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { customSlotsApi, type CustomSlotDef, type CustomSlotMatch } from "../../services/api/customSlots";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/** 与 manual.ts `ModelSlot` 一致的内置槽名，自定义槽不得重名。 */
export const BUILTIN_SLOT_NAMES = ["default", "opus", "sonnet", "haiku", "gpt", "jev", "embedding", "rerank"];

export type SlotNameError = "invalid" | "builtin" | "duplicate";

export function validateSlotName(name: string, existing: string[]): SlotNameError | null {
  if (!/^[a-z0-9-]{1,24}$/.test(name)) return "invalid";
  if (BUILTIN_SLOT_NAMES.includes(name)) return "builtin";
  if (existing.includes(name)) return "duplicate";
  return null;
}

const MATCHES: CustomSlotMatch[] = ["contains", "exact"];

export function CustomModelSlotsSection() {
  const { t } = useTranslation();
  const [list, setList] = useState<CustomSlotDef[]>([]);
  const [name, setName] = useState("");
  const [match, setMatch] = useState<CustomSlotMatch>("contains");
  const [error, setError] = useState<SlotNameError | null>(null);
  const [saveError, setSaveError] = useState("");
  const [pending, setPending] = useState<{ name: string; count: number } | null>(null);

  useEffect(() => {
    customSlotsApi.get().then(setList).catch((e) => setSaveError(String(e)));
  }, []);

  const persist = useCallback(async (next: CustomSlotDef[]) => {
    try {
      setSaveError("");
      await customSlotsApi.set(next);
      setList(next);
      return true;
    } catch (e) {
      setSaveError(String(e));
      return false;
    }
  }, []);

  const matchLabel = (m: CustomSlotMatch) => (m === "exact" ? t("customSlots.matchExact") : t("customSlots.matchContains"));
  const errorLabel = (e: SlotNameError) =>
    e === "builtin" ? t("customSlots.errorBuiltin") : e === "duplicate" ? t("customSlots.errorDuplicate") : t("customSlots.errorInvalid");

  const add = async () => {
    const err = validateSlotName(name, list.map((s) => s.name));
    setError(err);
    if (err) return;
    if (await persist([...list, { name, match }])) setName("");
  };

  const askDelete = async (n: string) => {
    try {
      setPending({ name: n, count: await customSlotsApi.usage(n) });
    } catch (e) {
      setSaveError(String(e));
    }
  };

  const confirmDelete = async () => {
    if (!pending) return;
    const { name: n } = pending;
    setPending(null);
    // 先写定义再清平台值：写定义失败则平台值原样保留；清值失败只留下被后端忽略的孤儿键。
    if (!(await persist(list.filter((s) => s.name !== n)))) return;
    try {
      await customSlotsApi.purge(n);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  return (
    <div className="glass-surface" data-testid="custom-slots-section" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
      <div>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("customSlots.title")}</div>
        <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>{t("customSlots.desc")}</div>
      </div>
      {list.map((s) => (
        <div key={s.name} data-testid={`slot-row-${s.name}`} data-match={s.match} style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 14 }}>
          <span style={{ fontWeight: 600, minWidth: 120 }}>{s.name}</span>
          <select
            data-testid={`slot-match-${s.name}`}
            value={s.match}
            onChange={(e) => persist(list.map((x) => (x.name === s.name ? { ...x, match: e.target.value as CustomSlotMatch } : x)))}
          >
            {MATCHES.map((m) => <option key={m} value={m}>{matchLabel(m)}</option>)}
          </select>
          <Button variant="ghost" size="sm" data-testid={`slot-delete-${s.name}`} onClick={() => askDelete(s.name)}>
            {t("customSlots.delete")}
          </Button>
        </div>
      ))}
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <Input
          data-testid="slot-name-input"
          value={name}
          placeholder={t("customSlots.namePlaceholder")}
          style={{ maxWidth: 200 }}
          onChange={(e) => { setName(e.target.value); setError(null); }}
        />
        <select data-testid="slot-match-new" value={match} onChange={(e) => setMatch(e.target.value as CustomSlotMatch)}>
          {MATCHES.map((m) => <option key={m} value={m}>{matchLabel(m)}</option>)}
        </select>
        <Button size="sm" data-testid="slot-add" onClick={add}>{t("customSlots.add")}</Button>
      </div>
      {error && (
        <div data-testid="slot-error" data-state={error} style={{ fontSize: 14, color: "var(--destructive, #d33)" }}>
          {errorLabel(error)}
        </div>
      )}
      {saveError && <div data-testid="slot-save-error" style={{ fontSize: 14, color: "var(--destructive, #d33)" }}>{saveError}</div>}
      <AlertDialog open={pending !== null} onOpenChange={(o) => { if (!o) setPending(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("customSlots.deleteTitle", { name: pending?.name })}</AlertDialogTitle>
            <AlertDialogDescription data-testid="slot-delete-count" data-count={pending?.count}>
              {t("customSlots.deleteDesc", { n: pending?.count ?? 0 })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="slot-delete-cancel">{t("customSlots.cancel")}</AlertDialogCancel>
            <AlertDialogAction data-testid="slot-delete-confirm" onClick={confirmDelete}>{t("customSlots.delete")}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
