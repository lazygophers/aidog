// ─── MITM 解密隧道配置 UI（P3 ST7）──────────────────────────────
// 假 CA 安装状态/引导 + 全局白名单增删改。
//
// 流程（design.md D1/D7/D8）：
//   启用 MITM → 后端 ensure_root_ca 生成 CA → 写 ca.pem 到数据目录
//   → 前端用 @tauri-apps/plugin-shell Command.create(name, args).execute() 装信任库
//     （OS 原生提权：macOS osascript admin / Windows UAC / Linux pkexec，零背景用户无需手敲 sudo）
//   → exit=0 回写 ca_installed=true；非 0 / reject → classifyTrustError 分类（取消/密码错/无 agent/命令失败）
//     + 弹窗给 manual_display 真实 sudo 命令 + 路径引导手动装（D8 兜底）
//
// 消费 services/api.ts mitmApi 契约（ST7 冻结），只读不改。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Command } from "@tauri-apps/plugin-shell";
import {
  mitmApi,
  type MitmStatus,
  type CaCommandSpec,
  type TrustErrorKind,
} from "../../services/api";

/// CA 安装失败分类后端化（阶段 B）：分类逻辑真源在后端 `classify_trust_error`（Rust 纯函数 +
/// 单测矩阵），前端 invoke `mitm_classify_trust_error` 取 TrustErrorKind，消除前后端双源。
/// 见 ca.rs `classify_trust_error` / `classify_trust_error_linux/macos/windows`。

export function MitmConfigTab() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<MitmStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [newPattern, setNewPattern] = useState("");
  // 手动装命令兜底弹窗（D8）：shell execute 失败 / reject 时展示。
  const [manualInstall, setManualInstall] = useState<CaCommandSpec | null>(null);
  // 阶段A 诊断：存 execute() 的原始输出（code/stderr/stdout/signal），兜底弹窗里展示给用户复现。
  const [installResult, setInstallResult] = useState<{ code: number | null; stderr: string; stdout: string; signal: number | null } | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await mitmApi.status());
    } catch (e) {
      console.error("mitm status failed", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // ── 启用 / 禁用 ────────────────────────────────────────────
  const handleToggle = async () => {
    if (!status) return;
    setBusy(true); setError("");
    try {
      if (status.enabled) {
        await mitmApi.disable();
      } else {
        await mitmApi.enable();
      }
      await refresh();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };

  // ── 装 CA（D1/D8）─────────────────────────────────────────
  const handleInstallCa = async () => {
    setBusy(true); setError(""); setManualInstall(null); setInstallResult(null);
    try {
      const spec = await mitmApi.installCaPrepare();
      // tauri-plugin-shell：capability mitm-ca.json 按 name 限定（cmd 键），OS 原生提权包装（osascript/UAC/pkexec）。
      const out = await Command.create(spec.name, spec.args).execute();
      const ok = out.code === 0;
      // 阶段A 诊断：不论 ok 都打全字段（code/stderr/stdout/signal），定位 osascript 失败真实根因。
      console.error("[ca-install]", {
        ok,
        code: out.code,
        stderr: out.stderr,
        stdout: out.stdout,
        signal: out.signal,
        name: spec.name,
        args: spec.args,
      });
      setInstallResult({ code: out.code, stderr: out.stderr, stdout: out.stdout, signal: out.signal });
      await mitmApi.setCaInstalled(ok);
      if (!ok) {
        // 失败兜底（D8）：分类错误 + 给 manual_display 真实 sudo 命令引导手动装。
        setManualInstall(spec);
        // 阶段 B：分类后端化，invoke 后端 classify_trust_error（Rust 真源 + 单测矩阵）。
        // code 显式 null 兜底（Tauri shell plugin reject/signal kill 路径 code 可能为 null）。
        const kind: TrustErrorKind = await mitmApi.classifyTrustError(spec.name, out.code, out.stderr ?? "");
        const base =
          kind === "cancel"
            ? t("mitm.installCancel", "已取消安装（未输入密码或点了取消）")
            : kind === "auth_fail"
              ? t("mitm.installAuthFail", "密码错误或鉴权被拒，请重试")
              : kind === "no_agent"
                ? t("mitm.installNoAgent", "Linux 缺少 polkit 鉴权 agent，请用下方命令手动装")
                : t("mitm.installFailed", "装信任库失败（exit={{code}}）", { code: String(out.code) });
        // cmd_fail 附原始 stderr 辅助诊断；其余分类不堆栈 stderr（用户无需看）。
        // 阶段A 兜底：分类逻辑产出空串（out.code 异常 / i18n 缺失）时也必给非空文案，禁 toast 无文案再现。
        const detail = kind === "cmd_fail" && out.stderr ? `${base}\n${out.stderr}` : base;
        setError(detail && detail.trim()
          ? detail
          : `exit=${String(out.code)} stderr=${out.stderr ?? "(empty)"} stdout=${out.stdout ?? "(empty)"}`);
      }
      await refresh();
    } catch (e) {
      // 阶段A 诊断：打印完整 reject 原因（含 capability scope 错误 / 用户取消 sudo 等）。
      console.error("[ca-install] reject", e);
      // Command.create reject（capability 拒绝 / 用户取消 sudo）→ 兜底手动装。
      // 阶段A 兜底：reject 的 e 可能无 message（空 reject）→ 也必给非空文案。
      setError(String(e) || "(reject 无 message)");
      try {
        const spec = await mitmApi.installCaPrepare();
        setManualInstall(spec);
      } catch { /* ignore secondary error */ }
    } finally { setBusy(false); }
  };

  // ── 白名单增删改 ──────────────────────────────────────────
  const handleAdd = async () => {
    const p = newPattern.trim();
    if (!p) return;
    setBusy(true); setError("");
    try {
      await mitmApi.whitelistAdd(p);
      setNewPattern("");
      await refresh();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };

  const handleToggleEntry = async (pattern: string, enabled: boolean) => {
    setError("");
    try {
      await mitmApi.whitelistToggle(pattern, enabled);
      await refresh();
    } catch (e) { setError(String(e)); }
  };

  const handleRemove = async (pattern: string) => {
    setError("");
    try {
      await mitmApi.whitelistRemove(pattern);
      await refresh();
    } catch (e) { setError(String(e)); }
  };

  if (loading) {
    return (
      <div className="text-secondary" style={{ padding: 20 }}>
        {t("status.loading", "加载中…")}
      </div>
    );
  }

  const enabled = status?.enabled ?? false;
  const caPresent = status?.ca_present ?? false;
  const caInstalled = status?.ca_installed ?? false;
  const fingerprint = status?.ca_fingerprint ?? "";
  const whitelist = status?.whitelist ?? [];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 主开关 */}
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("mitm.masterToggle", "MITM 解密隧道")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("mitm.masterToggleDesc", "启用后 CONNECT 隧道内 HTTPS 流量经 AirDog 解密采集（需装假 CA）")}
          </div>
        </div>
        <div
          className={`toggle ${enabled ? "active" : ""}`}
          onClick={() => !busy && handleToggle()}
          role="switch"
          aria-checked={enabled}
          tabIndex={0}
        />
      </div>

      {/* 风险提示（启用后展示）*/}
      {enabled && (
        <div
          className="glass-surface"
          style={{
            padding: "14px 16px",
            borderLeft: "3px solid var(--color-warning, #f0a020)",
            fontSize: 12,
            color: "var(--text-secondary)",
          }}
        >
          <div style={{ fontWeight: 600, marginBottom: 4, color: "var(--text-primary)" }}>
            {t("mitm.riskTitle", "安全提示")}
          </div>
          {t("mitm.riskDesc", "假 CA 私钥明文存于本机数据库；私钥泄露 = 白名单内 HTTPS 可被解密。仅启用必要 host。")}
        </div>
      )}

      {/* CA 安装状态 / 操作 */}
      {enabled && (
        <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
          <div>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("mitm.caCard", "假根证书 CA")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
              {t("mitm.caCardDesc", "装到系统信任库后，客户端才会信任 AirDog 签的 host 证书")}
            </div>
          </div>

          <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", fontSize: 12 }}>
            <span style={{ color: "var(--text-secondary)" }}>
              {t("mitm.caPresent", "已生成：")}
              <strong style={{ color: caPresent ? "var(--color-success)" : "var(--text-tertiary)" }}>
                {caPresent ? t("common.yes", "是") : t("common.no", "否")}
              </strong>
            </span>
            <span style={{ color: "var(--text-secondary)" }}>
              {t("mitm.caInstalled", "已装信任库：")}
              <strong style={{ color: caInstalled ? "var(--color-success)" : "var(--text-tertiary)" }}>
                {caInstalled ? t("common.yes", "是") : t("common.no", "否")}
              </strong>
            </span>
          </div>

          {fingerprint && (
            <div style={{ fontSize: 11, fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", color: "var(--text-tertiary)", wordBreak: "break-all" }}>
              {t("mitm.fingerprint", "指纹：")} {fingerprint}
            </div>
          )}

          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <button
              className="btn btn-primary"
              onClick={handleInstallCa}
              disabled={busy || !caPresent}
              style={{ padding: "7px 16px", fontSize: 13, opacity: busy || !caPresent ? 0.6 : 1 }}
            >
              {busy ? t("common.loading", "加载中…") : t("mitm.installCa", "安装 CA")}
            </button>
            {caInstalled && (
              <span style={{ fontSize: 12, color: "var(--color-success)" }}>
                {t("mitm.installedHint", "已装，客户端应已信任")}
              </span>
            )}
          </div>

          {/* 手动装兜底（D8）*/}
          {manualInstall && (
            <div style={{
              marginTop: 4, padding: 12, borderRadius: "var(--radius-md)",
              background: "color-mix(in srgb, var(--color-warning, #f0a020) 10%, transparent)",
              border: "1px solid color-mix(in srgb, var(--color-warning, #f0a020) 30%, transparent)",
              fontSize: 12,
            }}>
              <div style={{ fontWeight: 600, marginBottom: 6 }}>
                {t("mitm.manualInstallTitle", "自动安装失败，请手动执行：")}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div>
                  <span style={{ color: "var(--text-secondary)" }}>CA PEM: </span>
                  <code style={{ fontFamily: "ui-monospace, monospace" }}>{manualInstall.ca_pem_path}</code>
                </div>
                <div>
                  <span style={{ color: "var(--text-secondary)" }}>{t("mitm.command", "命令：")}</span>
                  <code style={{ fontFamily: "ui-monospace, monospace", wordBreak: "break-all" }}>
                    {manualInstall.manual_display}
                  </code>
                </div>
                {/* 阶段A 诊断：弹窗内直接显 execute() 真实 code/stderr/stdout/signal，用户复现无需开 devtools。 */}
                {installResult && (
                  <div style={{
                    marginTop: 4, padding: 8, borderRadius: "var(--radius-sm)",
                    background: "color-mix(in srgb, var(--color-error, #ef4444) 8%, transparent)",
                    border: "1px solid color-mix(in srgb, var(--color-error, #ef4444) 25%, transparent)",
                    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                    fontSize: 11, color: "var(--text-secondary)", whiteSpace: "pre-wrap", wordBreak: "break-all",
                  }}>
                    <div>exit={String(installResult.code)} signal={String(installResult.signal)}</div>
                    <div>stderr: {installResult.stderr || "(empty)"}</div>
                    <div>stdout: {installResult.stdout || "(empty)"}</div>
                  </div>
                )}
                <div style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
                  {t("mitm.manualInstallHint", "复制上方命令到终端执行（需输入管理员密码）")}
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 白名单（启用后展示）*/}
      {enabled && (
        <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
          <div>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("mitm.whitelistTitle", "解密白名单")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
              {t("mitm.whitelistDesc", "命中的 host 走 MITM 解密；未命中的走 P1 盲转。支持 *.domain 通配。")}
            </div>
          </div>

          {/* 添加输入 */}
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              className="input"
              value={newPattern}
              onChange={(e) => setNewPattern(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleAdd(); }}
              placeholder={t("mitm.addPlaceholder", "*.anthropic.com")}
              style={{ flex: 1, maxWidth: 320 }}
            />
            <button
              className="btn"
              onClick={handleAdd}
              disabled={busy || !newPattern.trim()}
              style={{ padding: "7px 14px", fontSize: 13, opacity: busy || !newPattern.trim() ? 0.6 : 1 }}
            >
              {t("common.add", "添加")}
            </button>
          </div>

          {/* 列表 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {whitelist.length === 0 && (
              <div className="text-tertiary" style={{ fontSize: 12 }}>
                {t("mitm.whitelistEmpty", "（无白名单条目）")}
              </div>
            )}
            {whitelist.map((e) => (
              <div
                key={e.host_pattern}
                style={{
                  display: "flex", alignItems: "center", gap: 10,
                  padding: "8px 10px", borderRadius: "var(--radius-md)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                }}
              >
                <code style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", fontSize: 12, flex: 1, wordBreak: "break-all" }}>
                  {e.host_pattern}
                </code>
                <span
                  style={{
                    fontSize: 10, padding: "2px 6px", borderRadius: 4,
                    background: e.source === "default"
                      ? "color-mix(in srgb, var(--color-info, #3b82f6) 15%, transparent)"
                      : "color-mix(in srgb, var(--text-tertiary) 15%, transparent)",
                    color: "var(--text-secondary)",
                  }}
                >
                  {e.source === "default" ? t("mitm.sourceDefault", "默认") : t("mitm.sourceUser", "自定义")}
                </span>
                {/* rule_type 标签（domain/suffix/keyword/ipcidr）— source 同风格不同色 */}
                {(() => {
                  const rt = e.rule_type ?? "suffix";
                  const label = rt === "domain" ? t("mitm.ruleDomain", "域名")
                    : rt === "suffix" ? t("mitm.ruleSuffix", "后缀")
                    : rt === "keyword" ? t("mitm.ruleKeyword", "关键字")
                    : rt === "ipcidr" ? t("mitm.ruleIpcidr", "IP 段")
                    : rt;
                  return (
                    <span
                      style={{
                        fontSize: 10, padding: "2px 6px", borderRadius: 4,
                        background: "color-mix(in srgb, var(--color-success, #10b981) 15%, transparent)",
                        color: "var(--text-secondary)",
                      }}
                    >
                      {label}
                    </span>
                  );
                })()}
                <div
                  className={`toggle ${e.enabled ? "active" : ""}`}
                  onClick={() => handleToggleEntry(e.host_pattern, !e.enabled)}
                  role="switch"
                  aria-checked={e.enabled}
                  tabIndex={0}
                  style={{ transform: "scale(0.85)" }}
                />
                <button
                  onClick={() => handleRemove(e.host_pattern)}
                  aria-label={t("common.delete", "删除")}
                  style={{
                    background: "transparent", border: "none", cursor: "pointer",
                    color: "var(--text-tertiary)", padding: "2px 6px", fontSize: 14,
                  }}
                >
                  ✕
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{error}</div>
      )}
    </div>
  );
}
