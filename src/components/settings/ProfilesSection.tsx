// 配置方案（Profile）区块（settings「Coding 设置」tab，magpie 票 06）。
// 保存当前 agent 配置 + 分组映射为快照；应用前先 dry_run 弹差异清单二次确认；
// 删除两步确认（只删快照文件，不碰当前配置）。
import { useCallback, useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { profileApi, type ProfileDiffItem, type ProfileMeta } from "../../services/api/profile";
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

/** 差异 target 的人读标签；动态文件 key（如 aidog_script:xxx）原样展示。 */
function targetLabel(t: TFunction, target: string): string {
  if (target === "db:groups") return t("profile.targetDbGroups", "分组与映射");
  return target;
}

const OP_KEY: Record<ProfileDiffItem["op"], string> = {
  create: "profile.opCreate",
  update: "profile.opUpdate",
  delete: "profile.opDelete",
};

export function ProfilesSection() {
  const { t } = useTranslation();
  const [list, setList] = useState<ProfileMeta[]>([]);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  /** 待覆盖保存的方案名（同名保存被拒后弹确认）。 */
  const [overwrite, setOverwrite] = useState<string | null>(null);
  /** 待删除的方案名。 */
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  /** 已跑 dry-run 的差异清单（确认后执行）。 */
  const [diff, setDiff] = useState<{ name: string; items: ProfileDiffItem[] } | null>(null);

  const reload = useCallback(() => {
    profileApi.list().then(setList).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const flash = (msg: string) => {
    setMessage(msg);
    setError("");
  };

  const save = async (target: string, force: boolean) => {
    if (!target.trim() || busy) return;
    setBusy(true);
    try {
      await profileApi.save(target, force);
      setName("");
      flash(t("profile.saved", { name: target }));
      reload();
    } catch (e) {
      const msg = String(e);
      if (msg.includes("exists")) setOverwrite(target);
      else setError(msg);
    } finally {
      setBusy(false);
    }
  };

  const askApply = async (target: string) => {
    setBusy(true);
    try {
      const items = await profileApi.apply(target, true);
      setDiff({ name: target, items });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const runApply = async () => {
    if (!diff) return;
    const target = diff.name;
    setDiff(null);
    setBusy(true);
    try {
      const items = await profileApi.apply(target, false);
      const failed = items.filter((i) => i.error);
      if (failed.length > 0) {
        setError(failed.map((i) => `${i.target}: ${i.error}`).join("\n"));
      } else {
        flash(t("profile.applied", { name: target }));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!pendingDelete) return;
    const target = pendingDelete;
    setPendingDelete(null);
    setBusy(true);
    try {
      await profileApi.remove(target);
      flash(t("profile.deleted", { name: target }));
      reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="glass-surface glass-highlight reveal in"
      data-testid="profiles-section"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}
    >
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("profile.title")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 4 }}>
          {t("profile.desc")}
        </div>
      </div>

      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <Input
          data-testid="profiles-name-input"
          value={name}
          placeholder={t("profile.namePlaceholder")}
          onChange={(e) => setName(e.target.value)}
          style={{ maxWidth: 240 }}
        />
        <Button
          data-testid="profiles-save-btn"
          size="sm"
          disabled={busy || !name.trim()}
          onClick={() => save(name.trim(), false)}
        >
          {t("profile.save")}
        </Button>
      </div>

      {list.length === 0 ? (
        <div className="text-secondary" style={{ fontSize: 12 }} data-testid="profiles-empty">
          {t("profile.empty")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {list.map((p) => (
            <div
              key={p.name}
              data-testid={`profiles-row-${p.name}`}
              data-name={p.name}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                fontSize: 13,
                padding: "8px 10px",
                borderRadius: 8,
                border: "1px solid var(--border)",
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontWeight: 600 }}>{p.name}</div>
                <div className="text-secondary" style={{ fontSize: 11 }}>
                  {t("profile.created", { date: new Date(p.created_at * 1000).toLocaleString() })}
                </div>
              </div>
              <Button
                data-testid={`profiles-apply-${p.name}`}
                size="sm"
                variant="outline"
                disabled={busy}
                onClick={() => askApply(p.name)}
              >
                {t("profile.apply")}
              </Button>
              <Button
                data-testid={`profiles-delete-${p.name}`}
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => setPendingDelete(p.name)}
              >
                {t("profile.delete")}
              </Button>
            </div>
          ))}
        </div>
      )}

      {message && (
        <div className="toast" data-testid="profiles-message" style={{ fontSize: 12 }}>
          {message}
        </div>
      )}
      {error && (
        <div role="alert" data-testid="profiles-error" style={{ color: "var(--color-danger)", fontSize: 12, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
          {error}
        </div>
      )}

      {/* 同名覆盖确认 */}
      <AlertDialog open={overwrite != null} onOpenChange={(o) => !o && setOverwrite(null)}>
        <AlertDialogContent data-testid="profiles-overwrite-dialog">
          <AlertDialogHeader>
            <AlertDialogTitle>{t("profile.overwriteTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("profile.overwriteDesc", { name: overwrite ?? "" })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="profiles-overwrite-cancel">{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              data-testid="profiles-overwrite-confirm"
              onClick={() => {
                const target = overwrite;
                setOverwrite(null);
                if (target) save(target, true);
              }}
            >
              {t("profile.overwriteConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 应用前差异清单（dry_run 结果） */}
      <AlertDialog open={diff != null} onOpenChange={(o) => !o && setDiff(null)}>
        <AlertDialogContent data-testid="profiles-diff-dialog" style={{ maxHeight: "70vh", overflow: "auto" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("profile.diffTitle", { name: diff?.name ?? "" })}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("profile.diffDesc", { count: diff?.items.length ?? 0 })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, margin: "8px 0" }}>
            {(diff?.items ?? []).map((item, i) => (
              <div key={`${item.target}-${i}`} data-testid={`profiles-diff-item-${i}`} data-op={item.op} style={{ display: "flex", gap: 8, alignItems: "baseline" }}>
                <span
                  style={{
                    fontFamily: "ui-monospace, monospace",
                    fontSize: 11,
                    color: item.op === "delete" ? "var(--color-danger)" : "inherit",
                    flexShrink: 0,
                  }}
                >
                  {t(OP_KEY[item.op])}
                </span>
                <span style={{ fontWeight: 600, wordBreak: "break-all" }}>{targetLabel(t, item.target)}</span>
                <span className="text-secondary" style={{ wordBreak: "break-all" }}>{item.detail}</span>
              </div>
            ))}
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="profiles-diff-cancel">{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction data-testid="profiles-diff-confirm" onClick={runApply}>
              {t("profile.diffConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 删除确认 */}
      <AlertDialog open={pendingDelete != null} onOpenChange={(o) => !o && setPendingDelete(null)}>
        <AlertDialogContent data-testid="profiles-delete-dialog">
          <AlertDialogHeader>
            <AlertDialogTitle>{t("profile.deleteTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("profile.deleteDesc", { name: pendingDelete ?? "" })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="profiles-delete-cancel">{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction data-testid="profiles-delete-confirm" onClick={remove}>
              {t("profile.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
