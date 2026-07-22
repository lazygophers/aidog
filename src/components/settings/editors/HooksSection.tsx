// ─── Hooks Section (main editor) ──────────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { F, S } from "./tokens";
import { SectionIcon } from "./icons";
import { Section, Toggle, FieldRow } from "./_shared";
import {
  type HooksConfig,
  type HookHandler,
  type HandlerType,
  HOOK_EVENTS,
  HANDLER_TYPES,
  HANDLER_LABELS,
} from "./hooks-types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export function HooksSection({
  hooksValue,
  updateField,
  t,
}: {
  hooksValue: HooksConfig | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const hooks: HooksConfig = hooksValue ?? {};
  const [userToggles, setUserToggles] = useState<Record<string, boolean>>({});

  // Count total hooks for badge
  const totalHooks = Object.values(hooks).reduce((sum, groups) => sum + groups.reduce((s, g) => s + g.hooks.length, 0), 0);

  const syncHooks = (updated: HooksConfig) => {
    const cleaned: HooksConfig = {};
    for (const [evt, groups] of Object.entries(updated)) {
      const nonEmpty = groups.filter(g => g.hooks.length > 0);
      if (nonEmpty.length > 0) cleaned[evt] = nonEmpty;
    }
    updateField("hooks", Object.keys(cleaned).length > 0 ? cleaned : undefined);
  };

  const addMatcherGroup = (eventId: string) => {
    const updated = { ...hooks };
    const existing = updated[eventId] ?? [];
    updated[eventId] = [...existing, { matcher: "", hooks: [{ type: "command" as HandlerType, command: "" }] }];
    syncHooks(updated);
    setUserToggles((prev) => ({ ...prev, [eventId]: true }));
  };

  const removeMatcherGroup = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups.splice(groupIdx, 1);
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateMatcher = (eventId: string, groupIdx: number, matcher: string) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups[groupIdx] = { ...groups[groupIdx], matcher };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const addHandler = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const group = { ...groups[groupIdx], hooks: [...groups[groupIdx].hooks, { type: "command" as HandlerType, command: "" }] };
    groups[groupIdx] = group;
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const removeHandler = (eventId: string, groupIdx: number, handlerIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers.splice(handlerIdx, 1);
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateHandler = (eventId: string, groupIdx: number, handlerIdx: number, patch: Partial<HookHandler>) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers[handlerIdx] = { ...handlers[handlerIdx], ...patch };
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const eventHookCount = (eventId: string) => {
    const groups = hooks[eventId];
    if (!groups) return 0;
    return groups.reduce((s, g) => s + g.hooks.length, 0);
  };

  const inputStyle: React.CSSProperties = {
    fontSize: F.body,
    padding: S.inputPad,
    minWidth: 0,
  };

  return (
    <Section title={`${t("settings.sectionHooks")}${totalHooks > 0 ? ` (${totalHooks})` : ""}`} defaultOpen={totalHooks > 0}>
      {/* Event selector — add new hook */}
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <Select


          // radix Select 空值哨兵：value="" 会抛，用 __none__ 映射。
          value="__none__"
          onValueChange={(v) => {
            if (v && v !== "__none__") addMatcherGroup(v);
          }}
        >
<SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 200 }}><SelectValue/></SelectTrigger>
<SelectContent>
          <SelectItem value="__none__">{t("settings.hooks.addEvent", "+ 添加 Hook 事件…")}</SelectItem>
          {HOOK_EVENTS.map(ev => (
            <SelectItem key={ev.id} value={ev.id}>
              {ev.id} — {t(`settings.hooks.event.${ev.id}.desc`, ev.desc)}
            </SelectItem>
          ))}
        </SelectContent>
</Select>
      </div>

      {/* Hint */}
      {totalHooks === 0 && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("settings.hooks.introLine1", "Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。")}
          <br />{t("settings.hooks.introLine2", "选择事件类型开始配置。")}
        </div>
      )}

      {/* Configured events */}
      {Object.entries(hooks).map(([eventId, groups]) => {
        const eventMeta = HOOK_EVENTS.find(e => e.id === eventId);
        const isExpanded = eventId in userToggles ? userToggles[eventId] : groups.length > 0;
        const count = eventHookCount(eventId);

        return (
          <div
            key={eventId}
            style={{
              background: "var(--bg-glass)",
              border: "1px solid var(--border)",
              borderRadius: "var(--radius-md)",
              padding: "16px 20px",
              display: "flex",
              flexDirection: "column",
              gap: 14,
            }}
          >
            {/* Event header */}
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{ cursor: "pointer", userSelect: "none", fontSize: F.small, color: "var(--text-tertiary)",
                  transition: "transform 0.2s", transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)"
                }}
                onClick={() => setUserToggles((prev) => ({ ...prev, [eventId]: !isExpanded }))}
              >
                ▶
              </span>
              <span style={{ fontSize: 16, fontWeight: 600, color: "var(--accent)" }}>
                {eventId}
              </span>
              {eventMeta && (
                <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
                  — {t(`settings.hooks.event.${eventMeta.id}.desc`, eventMeta.desc)}
                </span>
              )}
              <span style={{
                fontSize: 12, fontWeight: 600, padding: "2px 10px", borderRadius: 10,
                background: "var(--accent-subtle)", color: "var(--accent)", marginLeft: "auto",
              }}>
                {count} handler{count !== 1 ? "s" : ""}
              </span>
              <Button variant="ghost"
                type="button"
                
                style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                onClick={() => {
                  const updated = { ...hooks };
                  delete updated[eventId];
                  syncHooks(updated);
                }}
                title={t("settings.hooks.deleteEvent", "删除此事件所有 hooks")}
              >
                ×
              </Button>
            </div>

            {/* Matcher groups */}
            {isExpanded && groups.map((group, gi) => {
              // Parse current matcher into selected tags
              const matcherTags = group.matcher ? group.matcher.split("|").map(s => s.trim()).filter(Boolean) : [];
              const toggleMatcherTag = (tag: string) => {
                const next = matcherTags.includes(tag)
                  ? matcherTags.filter(t => t !== tag)
                  : [...matcherTags, tag];
                updateMatcher(eventId, gi, next.join("|"));
              };

              return (
              <div
                key={gi}
                style={{
                  borderLeft: "3px solid var(--accent)",
                  paddingLeft: 16,
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
                }}
              >
                {/* Matcher: tag chips or freeform input */}
                <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, fontWeight: 500 }}>
                    {t("settings.hooks.matcher", "匹配器")}
                  </span>
                  {eventMeta && eventMeta.matcherOptions.length > 0 ? (
                    <>
                      {eventMeta.matcherOptions.map(opt => {
                        const selected = matcherTags.includes(opt);
                        return (
                          <Button variant="ghost"
                            key={opt}
                            type="button"
                            
                            style={{
                              fontSize: 13,
                              padding: "4px 12px",
                              borderRadius: 16,
                              fontWeight: selected ? 600 : 400,
                              background: selected ? "var(--accent-subtle)" : "transparent",
                              color: selected ? "var(--primary)" : "var(--text-secondary)",
                              border: selected ? "1px solid var(--accent)" : "1px solid var(--border)",
                              transition: "all 150ms",
                            }}
                            onClick={() => toggleMatcherTag(opt)}
                          >
                            {opt}
                          </Button>
                        );
                      })}
                      {/* Selected indicator */}
                      {matcherTags.length > 0 && !matcherTags.every(t => eventMeta.matcherOptions.includes(t)) && (
                        <span style={{ fontSize: F.hint, color: "var(--accent)" }}>
                          {t("settings.hooks.customMatcher", "+ 自定义")}: {matcherTags.filter(t => !eventMeta.matcherOptions.includes(t)).join(", ")}
                        </span>
                      )}
                    </>
                  ) : eventMeta?.matcherFreeform ? (
                    <Input
                      
                      style={{ ...inputStyle, flex: 1 }}
                      placeholder={eventMeta?.id === "FileChanged" ? t("settings.hooks.matcherFilePh", "文件名，如 .envrc|.env") : t("settings.hooks.matcherToolPh", "工具名称或正则，多个用 | 分隔")}
                      value={group.matcher}
                      onChange={(e) => updateMatcher(eventId, gi, e.target.value)}
                    />
                  ) : (
                    <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.matchAll", "匹配所有")}</span>
                  )}
                  <Button variant="ghost"
                    type="button"
                    
                    style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                    onClick={() => removeMatcherGroup(eventId, gi)}
                    title={t("settings.hooks.deleteMatcherGroup", "删除此匹配器组")}
                  >
                    ×
                  </Button>
                </div>

                {/* Handlers — each in its own sub-card */}
                {group.hooks.map((handler, hi) => (
                  <div
                    key={hi}
                    style={{
                      marginLeft: 72,
                      background: "var(--bg-surface)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius-sm)",
                      padding: "14px 16px",
                      display: "flex",
                      flexDirection: "column",
                      gap: 10,
                    }}
                  >
                    {/* Header: type selector + delete */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <span style={{
                        fontSize: 13, fontWeight: 600, padding: "3px 10px", borderRadius: 6,
                        background: "var(--bg-glass)", color: "var(--accent)", border: "1px solid var(--border)",
                        flexShrink: 0,
                      }}>
                        {t(`settings.hooks.handler.${handler.type}`, HANDLER_LABELS[handler.type])}
                      </span>
                      <Select
                        
                        
                        value={handler.type}
                        onValueChange={(v) => updateHandler(eventId, gi, hi, { type: v as HandlerType })}
                      >
<SelectTrigger style={{ ...inputStyle, width: 130, flexShrink: 0 }}><SelectValue/></SelectTrigger>
<SelectContent>
                        {HANDLER_TYPES.map(ht => (
                          <SelectItem key={ht} value={ht}>{t(`settings.hooks.handler.${ht}`, HANDLER_LABELS[ht])}</SelectItem>
                        ))}
                      </SelectContent>
</Select>
                      <Button variant="ghost"
                        type="button"
                        
                        style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                        onClick={() => removeHandler(eventId, gi, hi)}
                        title={t("settings.hooks.deleteHandler", "删除此处理器")}
                      >
                        ×
                      </Button>
                    </div>

                    {/* Command — textarea + shell selector on own row */}
                    {handler.type === "command" && (
                      <>
                        <FieldRow label={t("settings.hooks.fieldCommand", "命令")} icon={<SectionIcon name="bolt" size={13} />}>
                          <Textarea
                            
                            style={{
                              flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                              fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                              minHeight: 56, resize: "vertical",
                            }}
                            placeholder={t("settings.hooks.commandPh", "命令或脚本路径，如 ./scripts/check.sh&#10;支持多行命令，每行独立执行")}
                            value={handler.command ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { command: e.target.value || undefined })}
                          />
                        </FieldRow>
                        <FieldRow label="Shell" icon={<SectionIcon name="advanced" size={13} />}>
                          <Select


                            value={!handler.shell ? "__none__" : handler.shell}
                            onValueChange={(v) => updateHandler(eventId, gi, hi, { shell: v === "__none__" ? undefined : v })}
                          >
<SelectTrigger style={{ ...inputStyle, width: 140 }}><SelectValue/></SelectTrigger>
<SelectContent>
                            <SelectItem value="__none__">Bash</SelectItem>
                            <SelectItem value="powershell">PowerShell</SelectItem>
                          </SelectContent>
</Select>
                        </FieldRow>
                      </>
                    )}
                    {/* HTTP URL */}
                    {handler.type === "http" && (
                      <FieldRow label="URL" icon={<SectionIcon name="network" size={13} />}>
                        <Input
                          
                          style={{ ...inputStyle, flex: 1 }}
                          placeholder={t("settings.hooks.urlPh", "HTTP URL，如 http://localhost:8080/hooks/pre-tool-use")}
                          value={handler.url ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}
                    {/* MCP Tool — server + tool each on own row */}
                    {handler.type === "mcp_tool" && (
                      <>
                        <FieldRow label={t("settings.hooks.fieldServer", "服务器")} icon={<SectionIcon name="network" size={13} />}>
                          <Input
                            
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder={t("settings.hooks.serverPh", "MCP 服务器名称")}
                            value={handler.server ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })}
                          />
                        </FieldRow>
                        <FieldRow label={t("settings.hooks.fieldTool", "工具")} icon={<SectionIcon name="advanced" size={13} />}>
                          <Input
                            
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder={t("settings.hooks.toolPh", "工具名称")}
                            value={handler.tool ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })}
                          />
                        </FieldRow>
                      </>
                    )}
                    {/* Prompt / Agent — textarea */}
                    {(handler.type === "prompt" || handler.type === "agent") && (
                      <FieldRow label={t("settings.hooks.fieldPrompt", "提示")} icon={<SectionIcon name="behavior" size={13} />}>
                        <Textarea
                          
                          style={{
                            flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                            fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                            minHeight: 56, resize: "vertical",
                          }}
                          placeholder={t("settings.hooks.promptPh", "提示文本，用 $ARGUMENTS 插入 hook 输入数据&#10;支持多行提示内容")}
                          value={handler.prompt ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}

                    {/* ── Auxiliary options, each on its own row ── */}
                    {eventMeta?.hasMatcher && (
                      <FieldRow label={t("settings.hooks.fieldIf", "条件 if")} icon={<SectionIcon name="permissions" size={13} />}>
                        <Input
                          
                          style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                          placeholder={t("settings.hooks.ifPh", "匹配条件，如 Bash(rm *)")}
                          value={handler["if"] ?? ""}
                          onChange={(e) => {
                            const patch: Partial<HookHandler> = {};
                            if (e.target.value) (patch as any)["if"] = e.target.value;
                            else (patch as any)["if"] = undefined;
                            updateHandler(eventId, gi, hi, patch);
                          }}
                        />
                      </FieldRow>
                    )}
                    <FieldRow label={t("settings.hooks.fieldTimeout", "超时")} icon={<SectionIcon name="status" size={13} />}>
                      <Input
                        
                        style={{ ...inputStyle, width: 80, fontSize: F.hint }}
                        type="number"
                        placeholder="600"
                        value={handler.timeout ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })}
                      />
                      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.seconds", "秒")}</span>
                    </FieldRow>
                    {handler.type === "command" && (
                      <FieldRow label="async" icon={<SectionIcon name="ui" size={13} />}>
                        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                          <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                          {t("settings.hooks.asyncDesc", "后台运行（不阻塞主流程）")}
                        </label>
                      </FieldRow>
                    )}
                    <FieldRow label={t("settings.hooks.fieldStatus", "状态")} icon={<SectionIcon name="status" size={13} />}>
                      <Input
                        
                        style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                        placeholder={t("settings.hooks.statusPh", "运行时显示的状态消息")}
                        value={handler.statusMessage ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })}
                      />
                    </FieldRow>
                  </div>
                ))}

                {/* Add handler button */}
                <Button variant="ghost"
                  type="button"
                  
                  style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start", marginLeft: 72 }}
                  onClick={() => addHandler(eventId, gi)}
                >
                  {t("settings.hooks.addHandler", "+ 处理器")}
                </Button>
              </div>
            );
            })}


            {/* Add matcher group to existing event */}
            {isExpanded && (
              <Button variant="ghost"
                type="button"
                
                style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start" }}
                onClick={() => addMatcherGroup(eventId)}
              >
                {t("settings.hooks.addMatcherGroup", "+ 匹配器组")}
              </Button>
            )}
          </div>
        );
      })}
    </Section>
  );
}

