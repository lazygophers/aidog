// ─── 扩展 → 插件管理（Claude Code / Codex）────────────────────
// Claude Code：开关写全局 claude_code.enabledPlugins（与设置页 PluginsSection 同链）；
// Codex：经 codex CLI 管理 plugin / marketplace，操作后后端回传新列表。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import {
  pluginsApi,
  type ClaudePluginView,
  type CodexPluginsView,
} from "../services/api/plugins";

export function Plugins() {
  const { t } = useTranslation();
  const [claude, setClaude] = useState<ClaudePluginView[]>([]);
  const [codex, setCodex] = useState<CodexPluginsView | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [newPluginId, setNewPluginId] = useState("");
  const [newMarketplace, setNewMarketplace] = useState("");

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
      setClaude(await pluginsApi.claudeList());
      setCodex(await pluginsApi.codexList());
    });
  }, [run]);

  const setClaudeEnabled = (p: ClaudePluginView, enabled: boolean) =>
    void run(async () => setClaude(await pluginsApi.claudeSet(p.id, enabled)));

  const codexAction = (fn: () => Promise<CodexPluginsView>) =>
    run(async () => setCodex(await fn()));

  return (
    <div className="flex flex-1 flex-col gap-4 p-5" data-testid="plugins-page">
      <div>
        <h2 className="text-lg font-semibold">{t("plugins.title")}</h2>
        <p className="text-sm text-muted-foreground">{t("plugins.desc")}</p>
      </div>
      {error && <p className="text-sm text-destructive" data-testid="plugins-error">{error}</p>}
      <Tabs defaultValue="claude">
        <TabsList>
          <TabsTrigger value="claude" data-testid="plugins-tab-claude">Claude Code</TabsTrigger>
          <TabsTrigger value="codex" data-testid="plugins-tab-codex">Codex</TabsTrigger>
        </TabsList>
        <TabsContent value="claude" className="flex flex-col gap-2">
          {claude.length === 0 && <p className="text-sm text-muted-foreground">{t("plugins.emptyClaude")}</p>}
          {claude.map((p) => (
            <div key={p.id} data-testid={`plugin-row-${p.id}`} className="flex items-center gap-4 rounded-md border p-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-sm font-medium">{p.id}</span>
                  {p.version && <Badge variant="outline">v{p.version}</Badge>}
                </div>
                {p.lastUpdated && (
                  <div className="text-sm text-muted-foreground">
                    {t("plugins.installedAt", { time: p.lastUpdated })}
                  </div>
                )}
              </div>
              <Switch
                data-testid={`plugin-toggle-${p.id}`}
                checked={p.enabled}
                disabled={busy}
                onCheckedChange={(on) => setClaudeEnabled(p, on)}
              />
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
