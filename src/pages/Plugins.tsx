// ─── 扩展 → 插件管理（Claude 插件捆绑包 / Codex）───────────────
// Claude：文件夹 / git / marketplace 安装 → 拆包分发到 agent 自己的位置；
// Codex：经 codex CLI 管理 plugin / marketplace，操作后后端回传新列表。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import {
  pluginsApi,
  type ClaudePluginCandidate,
  type ClaudePluginView,
  type ClaudePluginViewList,
  type CodexPluginsView,
} from "../services/api/plugins";

export function Plugins() {
  const { t } = useTranslation();
  const [claude, setClaude] = useState<ClaudePluginView[]>([]);
  const [codex, setCodex] = useState<CodexPluginsView | null>(null);
  const [error, setError] = useState("");
  const [warnings, setWarnings] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [newPluginId, setNewPluginId] = useState("");
  const [newMarketplace, setNewMarketplace] = useState("");
  const [source, setSource] = useState("");
  const [candidates, setCandidates] = useState<ClaudePluginCandidate[] | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [installAgents, setInstallAgents] = useState<string[]>(["claude-code", "codex"]);

  const applyClaude = useCallback((res: ClaudePluginViewList) => {
    setClaude(res.plugins);
    setWarnings(res.warnings);
  }, []);

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
    void run(async () => {
      applyClaude(await pluginsApi.claudeList());
      setCodex(await pluginsApi.codexList());
    });
  }, [run, applyClaude]);

  const setClaudeEnabled = (p: ClaudePluginView, enabled: boolean) =>
    void run(async () => applyClaude(await pluginsApi.claudeSet(p.name, enabled)));

  const toggleAgent = (p: ClaudePluginView, agent: string, on: boolean) => {
    const agents = on ? [...p.agents.filter((a) => a !== agent), agent] : p.agents.filter((a) => a !== agent);
    void run(async () => applyClaude(await pluginsApi.claudeSetAgents(p.name, agents)));
  };

  const codexAction = (fn: () => Promise<CodexPluginsView>) =>
    run(async () => setCodex(await fn()));

  return (
    <div className="flex flex-1 flex-col gap-4 p-5" data-testid="plugins-page">
      <div>
        <h2 className="text-lg font-semibold">{t("plugins.title")}</h2>
        <p className="text-sm text-muted-foreground">{t("plugins.desc")}</p>
      </div>
      {error && <p className="text-sm text-destructive" data-testid="plugins-error">{error}</p>}
      {warnings.length > 0 && <div className="text-sm text-muted-foreground" data-testid="plugins-warnings">
        {warnings.map((w) => <p key={w}>{w}</p>)}
      </div>}
      <Tabs defaultValue="claude">
        <TabsList>
          <TabsTrigger value="claude" data-testid="plugins-tab-claude">Claude Code</TabsTrigger>
          <TabsTrigger value="codex" data-testid="plugins-tab-codex">Codex</TabsTrigger>
        </TabsList>
        <TabsContent value="claude" className="flex flex-col gap-4">
          <div className="flex flex-col gap-2 rounded-md border p-3">
            <div className="flex items-end gap-2">
              <label className="flex min-w-0 flex-1 flex-col gap-1 text-sm" htmlFor="claude-plugin-source">
                {t("plugins.claudeSource")}
                <Input
                  id="claude-plugin-source"
                  data-testid="claude-plugin-source"
                  value={source}
                  onChange={(e) => { setSource(e.target.value); setCandidates(null); setSelected([]); }}
                />
              </label>
              <Button
                type="button"
                variant="outline"
                data-testid="claude-plugin-probe"
                disabled={busy || !source.trim()}
                onClick={() => void run(async () => {
                  const c = await pluginsApi.claudeProbe(source.trim());
                  setCandidates(c);
                  setSelected(c.filter((p) => !p.installed).map((p) => p.name));
                })}
              >
                {t("plugins.claudeProbe")}
              </Button>
            </div>
            {candidates !== null && <>
              <fieldset className="flex flex-col gap-1">
                <legend className="text-sm font-medium">{t("plugins.claudeProbe")}</legend>
                {candidates.map((c) => {
                  const id = `claude-plugin-candidate-${c.name}`;
                  return <div key={c.name} className="flex min-h-11 items-center gap-2 text-sm">
                    <Checkbox
                      id={id}
                      data-testid={id}
                      checked={selected.includes(c.name)}
                      onCheckedChange={(on) => setSelected(on ? [...selected, c.name] : selected.filter((n) => n !== c.name))}
                    />
                    <label htmlFor={id} className="flex min-h-11 min-w-0 items-center gap-2">
                      <span className="font-mono">{c.name}</span>
                      {c.description && <span className="truncate text-muted-foreground">{c.description}</span>}
                      {c.version && <Badge variant="outline">v{c.version}</Badge>}
                      {c.installed && <Badge variant="outline">{t("plugins.claudeAlreadyInstalled")}</Badge>}
                    </label>
                  </div>;
                })}
                {candidates.length === 0 && <p className="text-sm text-muted-foreground">{t("plugins.claudeNoCandidates")}</p>}
              </fieldset>
              <fieldset className="flex flex-wrap items-center gap-4 text-sm">
                <legend className="text-sm font-medium">{t("plugins.claudeAgents")}</legend>
                {["claude-code", "codex"].map((a) => {
                  const id = `claude-plugin-install-agent-${a}`;
                  return <div key={a} className="flex min-h-11 items-center gap-2">
                    <Checkbox
                      id={id}
                      data-testid={id}
                      checked={installAgents.includes(a)}
                      onCheckedChange={(on) => setInstallAgents(on ? [...installAgents, a] : installAgents.filter((x) => x !== a))}
                    />
                    <label htmlFor={id}>{a === "claude-code" ? "Claude Code" : "Codex"}</label>
                  </div>;
                })}
              </fieldset>
              <Button
                type="button"
                data-testid="claude-plugin-install"
                disabled={busy || selected.length === 0}
                onClick={() => void run(async () => {
                  applyClaude(await pluginsApi.claudeInstall(source.trim(), installAgents, selected));
                  setCandidates(null);
                  setSelected([]);
                })}
              >
                {t("plugins.addPlugin")}
              </Button>
            </>}
          </div>
          {claude.length === 0 && <p className="text-sm text-muted-foreground">{t("plugins.emptyClaude")}</p>}
          {claude.map((p) => (
            <div key={p.name} data-testid={`plugin-row-${p.name}`} className="flex flex-col gap-2 rounded-md border p-3">
              <div className="flex items-center gap-4">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-sm font-medium">{p.name}</span>
                    <Badge variant="outline">{p.on ? t("plugins.enabled") : t("plugins.disabled")}</Badge>
                  </div>
                  <div className="truncate text-sm text-muted-foreground">{p.source}</div>
                </div>
                <Switch
                  aria-label={p.name}
                  data-testid={`plugin-toggle-${p.name}`}
                  checked={p.on}
                  disabled={busy}
                  onCheckedChange={(on) => setClaudeEnabled(p, on)}
                />
                <Button size="sm" variant="outline" data-testid={`plugin-update-${p.name}`} disabled={busy} onClick={() => void run(async () => applyClaude(await pluginsApi.claudeUpdate(p.name)))}>{t("plugins.claudeUpdate")}</Button>
                <Button size="sm" variant="ghost" data-testid={`plugin-remove-${p.name}`} disabled={busy} onClick={() => void run(async () => applyClaude(await pluginsApi.claudeRemove(p.name)))}>{t("plugins.remove")}</Button>
              </div>
              <div className="flex items-center gap-4 text-sm">
                {t("plugins.claudeAgents")}
                {["claude-code", "codex"].map((a) => <label key={a} className="flex items-center gap-2">
                  <Checkbox
                    data-testid={`plugin-agent-${p.name}-${a}`}
                    checked={p.agents.includes(a)}
                    disabled={busy}
                    onCheckedChange={(on) => toggleAgent(p, a, !!on)}
                  />
                  {a === "claude-code" ? "Claude Code" : "Codex"}
                </label>)}
              </div>
              <div className="flex flex-wrap gap-2 text-sm text-muted-foreground">
                <span>{t("plugins.claudeParts", { skills: p.skills.length, subagents: p.subagents.length, hooks: p.hooks.length, servers: p.servers.length })}</span>
              </div>
              {p.left.length > 0 && <div className="text-sm text-muted-foreground" data-testid={`plugin-left-${p.name}`}>
                <span>{t("plugins.claudeLeft")}</span>
                {p.left.map((l) => <p key={l}>{l}</p>)}
              </div>}
            </div>
          ))}
        </TabsContent>
        <TabsContent value="codex" className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">{t("plugins.codexHint")}</p>
          <div className="flex flex-col gap-2">
            {codex?.installed.length === 0 && <p className="text-sm text-muted-foreground">{t("plugins.emptyCodex")}</p>}
            {codex?.installed.map((p) => (
              <div key={p.pluginId} data-testid={`codex-plugin-row-${p.pluginId}`} className="flex items-center gap-4 rounded-md border p-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-sm font-medium">{p.pluginId}</span>
                    {p.version && <Badge variant="outline">v{p.version}</Badge>}
                    <Badge variant="outline">{p.enabled ? t("plugins.enabled") : t("plugins.disabled")}</Badge>
                  </div>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  data-testid={`codex-plugin-remove-${p.pluginId}`}
                  onClick={() => void codexAction(() => pluginsApi.codexRemove(p.pluginId))}
                >
                  {t("plugins.remove")}
                </Button>
              </div>
            ))}
          </div>
          <div className="flex gap-2">
            <Input
              data-testid="codex-plugin-add-input"
              value={newPluginId}
              placeholder={t("plugins.addPluginPlaceholder")}
              onChange={(e) => setNewPluginId(e.target.value)}
            />
            <Button
              data-testid="codex-plugin-add"
              disabled={busy || !newPluginId.trim()}
              onClick={() =>
                void run(async () => {
                  setCodex(await pluginsApi.codexAdd(newPluginId.trim()));
                  setNewPluginId("");
                })
              }
            >
              {t("plugins.addPlugin")}
            </Button>
          </div>
          <div className="flex flex-col gap-2">
            {codex?.marketplaces.length === 0 && (
              <p className="text-sm text-muted-foreground">{t("plugins.emptyMarkets")}</p>
            )}
            {codex?.marketplaces.map((m) => (
              <div key={m.name} data-testid={`codex-mkt-row-${m.name}`} className="flex items-center gap-4 rounded-md border p-3">
                <div className="min-w-0 flex-1">
                  <div className="font-mono text-sm font-medium">{m.name}</div>
                  <div className="truncate text-sm text-muted-foreground">{m.source}</div>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  data-testid={`codex-mkt-remove-${m.name}`}
                  onClick={() => void codexAction(() => pluginsApi.codexMarketplaceRemove(m.name))}
                >
                  {t("plugins.remove")}
                </Button>
              </div>
            ))}
          </div>
          <div className="flex gap-2">
            <Input
              data-testid="codex-mkt-add-input"
              value={newMarketplace}
              placeholder={t("plugins.addMarketplacePlaceholder")}
              onChange={(e) => setNewMarketplace(e.target.value)}
            />
            <Button
              data-testid="codex-mkt-add"
              disabled={busy || !newMarketplace.trim()}
              onClick={() =>
                void run(async () => {
                  setCodex(await pluginsApi.codexMarketplaceAdd(newMarketplace.trim()));
                  setNewMarketplace("");
                })
              }
            >
              {t("plugins.addMarketplace")}
            </Button>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}
export default Plugins;
