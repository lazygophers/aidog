// ─── 扩展 → 子代理 ──────────────────────────────────────
// Claude Code（$CLAUDE_CONFIG_DIR/agents/<name>.md）与 Codex（$CODEX_HOME/agents/<name>.toml）
// 的子代理。每个 agent 一个开关；后端只撤回 aidog 写出且未被修改的文件，
// 用户改过标「已被修改」，同名外来文件标「同名文件已存在」（不覆盖）。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  subagentsApi,
  type Subagent,
  type SubagentAgent,
  type SubagentImportReport,
  type SubagentView,
} from "../services/api/subagents";
import { Button } from "../components/ui/button";
import { Badge } from "../components/ui/badge";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { Switch } from "../components/ui/switch";
import { Checkbox } from "../components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";

const AGENTS: SubagentAgent[] = ["claude-code", "codex"];
const AGENT_LABEL: Record<SubagentAgent, string> = {
  "claude-code": "mcp.agent.claude-code",
  codex: "mcp.agent.codex",
};
const STATUS_LABEL: Partial<Record<string, string>> = {
  modified: "subagents.status.modified",
  conflict: "subagents.status.conflict",
};
const EMPTY: Subagent = { name: "", description: "", prompt: "", agents: [] };

export function Subagents() {
  const { t } = useTranslation();
  const [list, setList] = useState<SubagentView[]>([]);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<{ old: string | null; draft: Subagent } | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [source, setSource] = useState("");
  const [importAgents, setImportAgents] = useState<SubagentAgent[]>([]);
  const [report, setReport] = useState<SubagentImportReport | null>(null);
  const [busy, setBusy] = useState(false);

  /** 返回是否成功；失败信息显示在页面。 */
  const run = useCallback(async (fn: () => Promise<void>) => {
    setError("");
    setBusy(true);
    try {
      await fn();
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  }, []);
  useEffect(() => {
    void run(async () => setList(await subagentsApi.list()));
  }, [run]);

  const save = (old: string | null, s: Subagent) =>
    run(async () => {
      try {
        setList(await subagentsApi.save(old, s));
      } catch (e) {
        // 写出部分失败时条目已落库，刷新拿真实状态
        await subagentsApi.list().then(setList, () => {});
        throw e;
      }
    });

  const toggle = (s: SubagentView, agent: SubagentAgent, on: boolean) => {
    const agents = on ? [...s.agents.filter((a) => a !== agent), agent] : s.agents.filter((a) => a !== agent);
    void save(s.name, { ...s, agents });
  };

  return (
    <div className="flex flex-1 flex-col gap-4 p-5" data-testid="subagents-page">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold">{t("subagents.title")}</h2>
          <p className="text-sm text-muted-foreground">{t("subagents.desc")}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" data-testid="subagent-import" onClick={() => { setReport(null); setImportOpen(true); }}>
            {t("subagents.import")}
          </Button>
          <Button data-testid="subagent-add" onClick={() => setEditing({ old: null, draft: { ...EMPTY } })}>
            {t("subagents.add")}
          </Button>
        </div>
      </div>
      {error && <p className="text-sm text-destructive" data-testid="subagent-error">{error}</p>}
      {list.length === 0 && <p className="text-sm text-muted-foreground">{t("subagents.empty")}</p>}
      <div className="flex flex-col gap-2">
        {list.map((s) => (
          <div key={s.name} data-testid={`subagent-row-${s.name}`} className="flex items-center gap-4 rounded-md border p-3">
            <div className="min-w-0 flex-1">
              <div className="font-mono text-sm font-medium">{s.name}</div>
              <div className="truncate text-sm text-muted-foreground">{s.description}</div>
              {s.from && <div className="truncate text-sm text-muted-foreground">{t("subagents.from", { from: s.from })}</div>}
            </div>
            {AGENTS.map((a) => {
              const label = STATUS_LABEL[s.status?.[a]];
              return (
                <label key={a} className="flex items-center gap-2 text-sm">
                  <Switch
                    data-testid={`subagent-toggle-${s.name}-${a}`}
                    checked={s.agents.includes(a)}
                    disabled={busy}
                    onCheckedChange={(on) => toggle(s, a, on)}
                  />
                  {t(AGENT_LABEL[a])}
                  {label && (
                    <Badge variant="outline" data-testid={`subagent-status-${s.name}-${a}`}>{t(label)}</Badge>
                  )}
                </label>
              );
            })}
            <Button size="sm" variant="ghost" onClick={() => setEditing({ old: s.name, draft: { ...s } })}>
              {t("subagents.edit")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setDeleting(s.name)}>
              {t("subagents.delete")}
            </Button>
          </div>
        ))}
      </div>

      <Dialog open={editing !== null} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t(editing?.old ? "subagents.edit" : "subagents.add")}</DialogTitle>
          </DialogHeader>
          {editing && (
            <div className="flex flex-col gap-3">
              <label className="flex flex-col gap-1 text-sm">
                {t("subagents.name")}
                <Input
                  data-testid="subagent-edit-name"
                  value={editing.draft.name}
                  placeholder={t("subagents.nameHint")}
                  onChange={(e) => setEditing({ ...editing, draft: { ...editing.draft, name: e.target.value } })}
                />
              </label>
              <label className="flex flex-col gap-1 text-sm">
                {t("subagents.description")}
                <Input
                  data-testid="subagent-edit-description"
                  value={editing.draft.description}
                  onChange={(e) => setEditing({ ...editing, draft: { ...editing.draft, description: e.target.value } })}
                />
              </label>
              <label className="flex flex-col gap-1 text-sm">
                {t("subagents.prompt")}
                <Textarea
                  data-testid="subagent-edit-prompt"
                  rows={8}
                  value={editing.draft.prompt}
                  onChange={(e) => setEditing({ ...editing, draft: { ...editing.draft, prompt: e.target.value } })}
                />
              </label>
              <label className="flex flex-col gap-1 text-sm">
                {t("subagents.codexModel")}
                <Input
                  data-testid="subagent-edit-codex-model"
                  value={editing.draft.codex_model ?? ""}
                  onChange={(e) => setEditing({ ...editing, draft: { ...editing.draft, codex_model: e.target.value } })}
                />
              </label>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditing(null)}>{t("subagents.cancel")}</Button>
            <Button
              data-testid="subagent-edit-save"
              disabled={busy}
              onClick={() => {
                if (!editing) return;
                const { old, draft } = editing;
                void save(old, draft).then((ok) => ok && setEditing(null));
              }}
            >
              {t("subagents.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleting !== null} onOpenChange={(o) => !o && setDeleting(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subagents.delete")}</DialogTitle>
          </DialogHeader>
          <p className="text-sm">{t("subagents.deleteConfirm", { name: deleting ?? "" })}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleting(null)}>{t("subagents.cancel")}</Button>
            <Button
              variant="destructive"
              data-testid="subagent-delete-confirm"
              onClick={() => {
                const name = deleting;
                setDeleting(null);
                if (!name) return;
                void run(async () => {
                  const kept = await subagentsApi.delete(name);
                  setList(await subagentsApi.list());
                  if (kept.length) setError(t("subagents.keptFiles", { paths: kept.join(", ") }));
                });
              }}
            >
              {t("subagents.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subagents.importTitle")}</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <Input
              data-testid="subagent-import-source"
              value={source}
              placeholder={t("subagents.importSource")}
              onChange={(e) => setSource(e.target.value)}
            />
            <div className="flex items-center gap-4 text-sm">
              {t("subagents.agents")}
              {AGENTS.map((a) => (
                <label key={a} className="flex items-center gap-2">
                  <Checkbox
                    data-testid={`subagent-import-agent-${a}`}
                    checked={importAgents.includes(a)}
                    onCheckedChange={(on) =>
                      setImportAgents(on ? [...importAgents, a] : importAgents.filter((x) => x !== a))
                    }
                  />
                  {t(AGENT_LABEL[a])}
                </label>
              ))}
            </div>
            {report && (
              <p className="text-sm" data-testid="subagent-import-result">
                {t("subagents.importResult", {
                  added: report.added.length,
                  updated: report.updated.length,
                  have: report.have.length,
                  edited: report.edited.length,
                })}
              </p>
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
          </div>
          <DialogFooter>
            <Button
              data-testid="subagent-import-run"
              disabled={busy || !source.trim()}
              onClick={() =>
                void run(async () => {
                  setReport(await subagentsApi.import(source.trim(), importAgents));
                  setList(await subagentsApi.list());
                })
              }
            >
              {t("subagents.importRun")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
export default Subagents;
