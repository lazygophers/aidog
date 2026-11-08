// agent 版本更新区块（settings「Coding 设置」tab，magpie 票 07）。
// 各 agent（claude / codex / pi）版本对照 + 按安装来源的单项更新（高危动作：AlertDialog 确认后逐个执行，无批量）。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { cliEnvApi, type CliTool, type AgentUpdateStatus, type AgentUpdateResult } from "../../services/api";
import { Button } from "@/components/ui/button";
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

/** CLI 名 → i18n key + 兜底展示名（后端 cli_env::TOOLS 真值源，与 About.tsx 同映射）。 */
const CLI_TOOL_LABELS: Record<CliTool, { key: string; fallback: string }> = {
  claude: { key: "about.localEnv.claudeCode", fallback: "Claude Code" },
  codex: { key: "about.localEnv.codex", fallback: "Codex CLI" },
  pi: { key: "about.localEnv.pi", fallback: "pi" },
};

/** 更新结果回显（按 tool 记忆）。 */
type Echo = { ok: boolean; text: string };

export function AgentUpdateSection() {
  const { t } = useTranslation();
  const [list, setList] = useState<AgentUpdateStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyTool, setBusyTool] = useState<CliTool | null>(null);
  const [pending, setPending] = useState<CliTool | null>(null);
  const [echoes, setEchoes] = useState<Record<string, Echo>>({});
  const [error, setError] = useState("");

  const reload = useCallback(() => {
    cliEnvApi
      .checkAgentUpdates()
      .then((rows) => {
        setList(rows);
        setError("");
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const apply = async (tool: CliTool) => {
    setPending(null);
    setBusyTool(tool);
    try {
      const r: AgentUpdateResult = await cliEnvApi.applyAgentUpdate(tool);
      const text = r.success
        ? t("agentUpdate.success", {
            before: r.before_version ?? "?",
            after: r.after_version ?? "?",
          })
        : `${t("agentUpdate.failed")}: ${r.message}`;
      setEchoes((prev) => ({ ...prev, [tool]: { ok: r.success, text } }));
      reload();
    } catch (e) {
      setEchoes((prev) => ({ ...prev, [tool]: { ok: false, text: `${t("agentUpdate.failed")}: ${e}` } }));
    } finally {
      setBusyTool(null);
    }
  };

  const toolLabel = (tool: string) => {
    const m = CLI_TOOL_LABELS[tool as CliTool];
    return m ? t(m.key, m.fallback) : tool;
  };

  return (
    <div
      className="glass-surface glass-highlight reveal in"
      data-testid="agent-update-section"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}
    >
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("agentUpdate.title", "Agent 版本更新")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 4 }}>
          {t("agentUpdate.desc", "检测各 agent 版本，并按安装来源执行对应的更新命令。")}
        </div>
      </div>

      {error && (
        <div style={{ fontSize: 12, color: "var(--color-danger)" }} data-testid="agent-update-error">
          {error}
        </div>
      )}

      {loading ? (
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("common.loading", "加载中…")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {list.map((s) => {
            const echo = echoes[s.tool];
            return (
              <div
                key={s.tool}
                data-testid={`agent-update-row-${s.tool}`}
                data-tool={s.tool}
                data-updatable={s.updatable ? "true" : "false"}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  fontSize: 13,
                  padding: "8px 10px",
                  borderRadius: 8,
                  border: "1px solid var(--border-subtle, rgba(0,0,0,0.08))",
                  flexWrap: "wrap",
                }}
              >
                <div style={{ fontWeight: 600, minWidth: 90 }}>{toolLabel(s.tool)}</div>
                <div className="text-secondary" style={{ fontSize: 12, fontFamily: "ui-monospace, Menlo, monospace" }}>
                  {s.installed ? `v${s.version ?? "?"}` : t("agentUpdate.notInstalled", "未安装")}
                  {s.latest_version ? ` → v${s.latest_version}` : s.installed ? ` → ${t("agentUpdate.unknown", "未知")}` : ""}
                </div>
                {s.source && (
                  <div
                    className="text-secondary"
                    style={{ fontSize: 11, border: "1px solid var(--border-subtle, rgba(0,0,0,0.08))", borderRadius: 6, padding: "1px 6px" }}
                  >
                    {s.source}
                  </div>
                )}
                <div style={{ flex: 1 }} />
                {echo && (
                  <div
                    data-testid={`agent-update-result-${s.tool}`}
                    style={{ fontSize: 12, color: echo.ok ? "var(--color-success)" : "var(--color-danger)" }}
                  >
                    {echo.text}
                  </div>
                )}
                {!s.installed ? null : s.updatable ? (
                  <Button
                    data-testid={`agent-update-btn-${s.tool}`}
                    size="sm"
                    variant="outline"
                    disabled={busyTool !== null || s.has_update !== true}
                    onClick={() => setPending(s.tool as CliTool)}
                  >
                    {busyTool === s.tool ? t("agentUpdate.updating", "更新中…") : t("agentUpdate.update", "更新")}
                  </Button>
                ) : (
                  <div className="text-secondary" data-testid={`agent-update-hint-${s.tool}`} style={{ fontSize: 12 }}>
                    {t("agentUpdate.manualHint", "该安装方式暂不支持自动更新，请手动升级")}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <AlertDialog open={pending !== null} onOpenChange={(o) => !o && setPending(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("agentUpdate.confirmTitle", { tool: pending ? toolLabel(pending) : "" })}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("agentUpdate.confirmDesc", "将按该工具的安装来源执行更新命令，可能需要下载。确认继续？")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="agent-update-cancel">{t("common.cancel", "取消")}</AlertDialogCancel>
            <AlertDialogAction data-testid="agent-update-confirm" onClick={() => pending && apply(pending)}>
              {t("agentUpdate.confirmOk", "确认更新")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
