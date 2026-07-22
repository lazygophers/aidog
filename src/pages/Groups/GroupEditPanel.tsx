import type { TFunction } from "i18next";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import claudeIcon from "../../assets/platforms/claude_code.svg";
import codexIcon from "../../assets/platforms/openai.svg";
import type { Platform, RoutingMode } from "../../services/api";
import type { EditState, EditAction } from "../../domains/groups";
import { allModelValues, getProtocolLabelMap } from "../../domains/platforms";
import { F, S } from "../../domains/shared/tokens";
import { ROUTING_MODES, routingModeLabel, routingModeDesc, buildClaudeCommand, buildCodexCommand, PlatformPicker, useProxyEnvVars } from "../../domains/groups";
import { CopyButton } from "../../components/shared";
import { IconClose } from "../../components/icons";
import { MiddlewareRulesPanel } from "../../components/settings/MiddlewareRules";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export interface GroupEditPanelProps {
  edit: EditState;
  dispatchEdit: (a: EditAction) => void;
  platforms: Platform[];
  t: TFunction;
  onCancel: () => void;
  onSave: () => void;
}

/** 分组编辑全屏视图：基本信息 + 关联平台 + 模型映射 + 环境变量 + 中间件规则。 */
export function GroupEditPanel({ edit, dispatchEdit, platforms, t, onCancel, onSave }: GroupEditPanelProps) {
  const { target: editTarget, name: editName, mode: editMode, platformIds: editPlatformIds,
    mappings: editMappings, envVars: editEnvVars, reqTimeout: editReqTimeout,
    connTimeout: editConnTimeout, maxRetries: editMaxRetries } = edit;
  const editPlatformOptions = platforms.filter(p => p.enabled);
  const { i18n } = useTranslation();
  // 代理 env（claude settings.json env 段），前置 export 注入 codex 启动命令（codex 无 config proxy）。
  const proxyVars = useProxyEnvVars();
  // 协议 label 全表（一次 RPC，i18n.language 变化重取；PlatformPicker 下拉 option 共享）
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolLabelMap(i18n.language).then(m => { if (!cancelled) setLabelMap(m); });
    return () => { cancelled = true; };
  }, [i18n.language]);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button variant="ghost" size="icon" style={{ height: "auto" }} onClick={onCancel} title={t("action.cancel")}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </Button>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.title, fontWeight: 700 }}>{editName || t("group.edit")}</div>
          <div className="text-secondary" style={{ fontSize: F.hint, marginTop: 2 }}>#{editTarget!.group.id}</div>
        </div>
        <CopyButton text={editTarget!.group.group_key} label={t("group.apiKey", "API Key")} title={t("group.copyApiKeyTitle", "复制 API Key")} />
        <CopyButton text={buildClaudeCommand(editTarget!.group.group_key)} icon={<img src={claudeIcon} width={14} height={14} alt="Claude" />} title={t("group.copyCommand", "复制 Claude Code 启动命令")} />
        <CopyButton text={buildCodexCommand(editTarget!.group.group_key, [...editEnvVars, ...proxyVars])} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} />
        <Button variant="outline" onClick={onCancel}>{t("action.cancel")}</Button>
        <Button onClick={onSave} disabled={!editName}>{t("action.save")}</Button>
      </div>

      {/* Basic info */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

        {/* Name */}
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
          <Input style={{ fontSize: F.body, padding: S.inputPad }}
            value={editName} onChange={e => dispatchEdit({ type: "patch", patch: { name: e.target.value } })} />
        </div>

        {/* Group key（锁定，创建后不可改） */}
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.groupKey", "密钥")}</span>
          <div style={{ display: "flex", gap: 6, alignItems: "center", minWidth: 0 }}>
            <Input style={{ fontSize: F.body, padding: S.inputPad, opacity: 0.7 }}
              value={editTarget!.group.group_key} disabled
              title={t("group.groupKeyLocked", "分组密钥创建后锁定，不可修改")} />
            <CopyButton text={editTarget!.group.group_key} title={t("group.copyApiKeyTitle", "复制 API Key")} size={14} />
          </div>
        </div>

        {/* Routing mode */}
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "start", gap: 12 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", paddingTop: 6 }}>{t("group.routingMode", "路由模式")}</span>
          <div style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 0 }}>
            <Select value={editMode} onValueChange={(v) => dispatchEdit({ type: "patch", patch: { mode: v as RoutingMode } })}>
              <SelectTrigger style={{ fontSize: F.body, padding: S.inputPad }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ROUTING_MODES.map(m => (
                  <SelectItem key={m} value={m}>{routingModeLabel(t, m)}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{routingModeDesc(t, editMode)}</span>
          </div>
        </div>

        {/* Timeout */}
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.timeout", "超时")}</span>
          <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
            <Input type="number" min={0} placeholder={t("group.reqTimeout", "请求(s)")}
              value={editReqTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { reqTimeout: Math.max(0, Number(e.target.value)) } })}
              style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
            <Input type="number" min={0} placeholder={t("group.connTimeout", "连接(s)")}
              value={editConnTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { connTimeout: Math.max(0, Number(e.target.value)) } })}
              style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.timeoutDefault", "0 = 系统默认（秒）")}</span>
          </div>
        </div>

        {/* Max retries（多平台失败逐个重试上限） */}
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.maxRetries", "最大重试")}</span>
          <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
            <Input type="number" min={0} max={10}
              value={editMaxRetries}
              onChange={e => dispatchEdit({ type: "patch", patch: { maxRetries: Math.max(0, Number(e.target.value)) } })}
              style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
            <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.maxRetriesHint", "0 = 不重试，只试 1 个平台")}</span>
          </div>
        </div>

        {/* Auto badge */}
        {editTarget!.group.auto_from_platform && (
          <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)" }}>
            <Badge variant="secondary" style={{ fontSize: 10, padding: "0 5px" }}>auto</Badge>
            {t("group.autoFromPlatform", "自动创建，部分字段不可编辑")}
          </div>
        )}
      </div>

      {/* Platforms */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.platforms", "关联平台")}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
          {t("group.platformsHint", "选择并排序此分组使用的平台，顺序决定优先级")}
        </div>
        <PlatformPicker
          platformIds={editPlatformIds}
          options={editPlatformOptions}
          onChange={ids => dispatchEdit({ type: "patch", patch: { platformIds: ids } })}
          t={t}
          labelMap={labelMap}
        />
      </div>

      {/* Model Mappings */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.modelMappings", "模型映射")}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
          {t("group.mappingsHint", "将源模型名映射到目标平台的具体模型")}
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {editMappings.map((m, i) => {
            const targetPlat = platforms.find(p => p.id === m.target_platform_id);
            const models = targetPlat ? allModelValues(targetPlat.models) : [];
            return (
              <div key={i} style={{
                display: "flex", gap: 8, alignItems: "center",
                padding: "8px 12px", borderRadius: "var(--radius-sm)",
                background: "var(--bg-glass)", border: "1px solid var(--border)",
              }}>
                <Input style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                  placeholder={t("mapping.source", "源模型")}
                  value={m.source_model}
                  onChange={e => {
                    const ms = [...editMappings];
                    ms[i] = { ...ms[i], source_model: e.target.value };
                    dispatchEdit({ type: "patch", patch: { mappings: ms } });
                  }} />
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                  <path d="M2 6h8M8 4l2 2-2 2" />
                </svg>
                {/* radix Select 禁 value="" → __none__ 哨兵映射回 0（= 未选目标平台） */}
                <Select
                  value={m.target_platform_id ? String(m.target_platform_id) : "__none__"}
                  onValueChange={(v) => {
                    const ms = [...editMappings];
                    ms[i] = { ...ms[i], target_platform_id: v === "__none__" ? 0 : Number(v), target_model: "" };
                    dispatchEdit({ type: "patch", patch: { mappings: ms } });
                  }}>
                  <SelectTrigger style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}>
                    <SelectValue placeholder={t("mapping.targetPlatform", "目标平台")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">{t("mapping.targetPlatform", "目标平台")}</SelectItem>
                    {platforms.filter(p => p.enabled).map(p => <SelectItem key={p.id} value={String(p.id)}>{p.name}</SelectItem>)}
                  </SelectContent>
                </Select>
                {models.length > 0 ? (
                  <Select
                    value={m.target_model || "__none__"}
                    onValueChange={(v) => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_model: v === "__none__" ? "" : v };
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
                    }}>
                    <SelectTrigger style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}>
                      <SelectValue placeholder={t("mapping.target", "目标模型")} />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">{t("mapping.target", "目标模型")}</SelectItem>
                      {models.map(m2 => <SelectItem key={m2} value={m2}>{m2}</SelectItem>)}
                    </SelectContent>
                  </Select>
                ) : (
                  <Input style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                    placeholder={t("mapping.target", "目标模型")}
                    value={m.target_model}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_model: e.target.value };
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
                    }} />
                )}
                <Button type="button" variant="ghost" size="icon" style={{ height: "auto", color: "var(--text-tertiary)", padding: 4, minWidth: "auto" }} onClick={() => dispatchEdit({ type: "patch", patch: { mappings: editMappings.filter((_, j) => j !== i) } })}>
                  <IconClose size={12} />
                </Button>
              </div>
            );
          })}

          <Button type="button" variant="ghost" style={{ fontSize: F.hint, padding: "6px 12px", height: "auto", alignSelf: "flex-start" }}
            onClick={() => dispatchEdit({ type: "patch", patch: { mappings: [...editMappings, { source_model: "", target_platform_id: 0, target_model: "", request_timeout_secs: 0, connect_timeout_secs: 0 }] } })}>
            + {t("mapping.add", "添加映射")}
          </Button>
        </div>
      </div>

      {/* Environment Variables（分组维度；sync 注入 Claude settings.env + Codex 复制命令前置 export） */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.envVars", "环境变量")}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
          {t("group.envVarsHint", "本分组会话级环境变量；sync 时注入 Claude settings.env，复制 Codex 命令时前置 export。")}
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {editEnvVars.map((ev, i) => {
            const reserved = ev.key === "ANTHROPIC_BASE_URL" || ev.key === "ANTHROPIC_AUTH_TOKEN" || ev.key === "AIDOG_KEY";
            return (
              <div key={i} style={{
                display: "flex", gap: 8, alignItems: "center",
                padding: "8px 12px", borderRadius: "var(--radius-sm)",
                background: "var(--bg-glass)", border: `1px solid ${reserved ? "var(--color-warning)" : "var(--border)"}`,
              }}>
                <Input style={{ fontSize: F.hint, padding: "6px 10px", width: 200, flexShrink: 0, fontFamily: "var(--font-mono, monospace)" }}
                  placeholder={t("group.envVarKey", "变量名 (如 ANTHROPIC_DEFAULT_OPUS_MODEL)")}
                  value={ev.key}
                  onChange={e => {
                    const next = [...editEnvVars];
                    next[i] = { ...next[i], key: e.target.value };
                    dispatchEdit({ type: "patch", patch: { envVars: next } });
                  }} />
                <Input style={{ fontSize: F.hint, padding: "6px 10px", flex: 1, fontFamily: "var(--font-mono, monospace)" }}
                  placeholder={t("group.envVarValue", "变量值")}
                  value={ev.value}
                  onChange={e => {
                    const next = [...editEnvVars];
                    next[i] = { ...next[i], value: e.target.value };
                    dispatchEdit({ type: "patch", patch: { envVars: next } });
                  }} />
                <Button type="button" variant="ghost" size="icon" style={{ height: "auto", color: "var(--text-tertiary)", padding: 4, minWidth: "auto" }} onClick={() => dispatchEdit({ type: "patch", patch: { envVars: editEnvVars.filter((_, j) => j !== i) } })}>
                  <IconClose size={12} />
                </Button>
              </div>
            );
          })}
          {editEnvVars.some(ev => ev.key === "ANTHROPIC_BASE_URL" || ev.key === "ANTHROPIC_AUTH_TOKEN" || ev.key === "AIDOG_KEY") && (
            <div style={{ fontSize: F.small, color: "var(--color-warning)", lineHeight: 1.4 }}>
              {t("group.envVarReservedHint", "ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN / AIDOG_KEY 为 aidog 路由字段，保存时将被丢弃。")}
            </div>
          )}

          <Button type="button" variant="ghost" style={{ fontSize: F.hint, padding: "6px 12px", height: "auto", alignSelf: "flex-start" }}
            onClick={() => dispatchEdit({ type: "patch", patch: { envVars: [...editEnvVars, { key: "", value: "" }] } })}>
            + {t("group.addEnvVar", "添加环境变量")}
          </Button>
        </div>
      </div>

      {/* Middleware rules (group scope) */}
      <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
        <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("middleware.groupRules", "分组中间件规则")}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
          {t("middleware.groupRulesHint", "仅本分组生效，就近覆盖全局同类型规则")}
        </div>
        <MiddlewareRulesPanel scope="group" scopeRef={editTarget!.group.group_key} embedded />
      </div>
    </div>
  );
}
