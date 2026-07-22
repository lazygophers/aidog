// formSectionsEndpoints — 编辑页 Endpoints 大分区，从 formSections.tsx 拆出控制行数。
// ponytail: ModelsSection 已被 ModelsMatrixSection 取代（PRD 07-09 合并矩阵 card）。
import React from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import {
  type Protocol, type PlatformEndpoint, type ClientType,
} from "../../services/api";
import {
  ENDPOINT_PROTOCOLS,
  defaultClientForProtocol,
  buildClientTypesFromPresets,
} from "../../domains/platforms";
import { FormSection } from "./formSections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue,
} from "@/components/ui/select";

export function EndpointsSection({ endpoints, setEndpoints, t }: {
  endpoints: PlatformEndpoint[];
  setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  t: TFunction;
}) {
  const { i18n } = useTranslation();
  // CLIENT_TYPES 删除（JSON 派生）：仿 C3 buildProtocolsFromPresets 范式 — useState 空初始
  // + useEffect + cancelled flag + locale key [i18n.language]，确保切语言时重拉派生层。
  // 禁前端直读 github / 文件系统，一律 invoke get_client_types_json（封装在 buildClientTypesFromPresets 内）。
  const [clientTypeOptions, setClientTypeOptions] = React.useState<Array<{ value: ClientType; group: string; label: string }>>([]);
  React.useEffect(() => {
    let cancelled = false;
    buildClientTypesFromPresets(i18n.language).then((list) => {
      if (!cancelled) setClientTypeOptions(list);
    }).catch(() => { /* best-effort，空列表保下拉不崩 */ });
    return () => { cancelled = true; };
  }, [i18n.language]);

  return (
    <FormSection
      title={t("platform.endpoints", "Protocol Endpoints")}
      desc={t("platform.endpointsHint", "Additional protocols this platform supports with different base URLs")}
      action={(
        <Button
          variant="ghost"
          size="sm"
          style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
          onClick={() => {
            // defaultClientForProtocol async 化后取默认客户端类型（仅一处调用，不阻塞渲染）
            defaultClientForProtocol("openai").then((ct) =>
              setEndpoints([...endpoints, { protocol: "openai" as Protocol, base_url: "", client_type: ct, coding_plan: false }]),
            );
          }}
        >
          + {t("platform.addEndpoint", "Add Endpoint")}
        </Button>
      )}
    >
      {endpoints.length === 0 && (
        <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "4px 0", fontStyle: "italic" }}>
          {t("platform.noEndpoints", "No additional endpoints")}
        </div>
      )}
      {endpoints.map((ep, idx) => (
        <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <Select
            value={ep.protocol}
            onValueChange={(newProto) => {
              // defaultClientForProtocol async 化后异步取默认客户端类型，更新该 endpoint。
              defaultClientForProtocol(newProto as Protocol).then((ct) => {
                setEndpoints((prev) => {
                  const next = [...prev];
                  next[idx] = { ...next[idx], protocol: newProto as Protocol, client_type: ct };
                  return next;
                });
              });
            }}
          >
            <SelectTrigger className="input" style={{ width: 120, flexShrink: 0 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {ENDPOINT_PROTOCOLS.map((p) => (
                <SelectItem key={p.value} value={p.value}>{p.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Input
            className="input"
            style={{ flex: 1 }}
            placeholder="Endpoint Base URL"
            value={ep.base_url}
            onChange={(e) => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], base_url: e.target.value };
              setEndpoints(next);
            }}
          />
          <Select
            value={ep.client_type || "default"}
            onValueChange={(v) => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], client_type: v as ClientType };
              setEndpoints(next);
            }}
          >
            <SelectTrigger className="input" style={{ width: 140, flexShrink: 0 }} title={t("platform.clientType", "客户端模拟")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {/* 默认条目（group === ""）：单独置顶，沿用 platform.mockDefault i18n 文案 */}
              {clientTypeOptions
                .filter((c) => c.group === "")
                .map((c) => (
                  <SelectItem key={c.value} value={c.value}>
                    {t("platform.mockDefault", c.label)}
                  </SelectItem>
                ))}
              {/* 分组条目：按 group SelectGroup 聚合（Claude Code / Codex / IDE） */}
              {Object.entries(
                clientTypeOptions
                  .filter((c) => c.group !== "")
                  .reduce<Record<string, typeof clientTypeOptions>>((acc, c) => {
                    (acc[c.group] ||= []).push(c);
                    return acc;
                  }, {}),
              ).map(([group, items]) => (
                <SelectGroup key={group}>
                  <SelectLabel>{group}</SelectLabel>
                  {items.map((c) => (
                    <SelectItem key={c.value} value={c.value}>{c.label}</SelectItem>
                  ))}
                </SelectGroup>
              ))}
            </SelectContent>
          </Select>
          {/* Coding Plan 开关 */}
          <Button
            variant="ghost"
            size="icon"
            style={{
              flexShrink: 0,
              width: 28, height: 28, minWidth: 28,
              padding: 0,
              fontSize: 11, fontWeight: 700,
              color: ep.coding_plan ? "var(--color-success)" : "var(--text-tertiary)",
              background: ep.coding_plan ? "color-mix(in srgb, var(--color-success) 8%, transparent)" : "transparent",
              border: `1px solid ${ep.coding_plan ? "color-mix(in srgb, var(--color-success) 25%, transparent)" : "var(--border)"}`,
              borderRadius: "var(--radius-sm)",
            }}
            title={ep.coding_plan ? "Coding Plan ON" : "Coding Plan"}
            onClick={() => {
              const next = [...endpoints];
              next[idx] = { ...next[idx], coding_plan: !next[idx].coding_plan };
              setEndpoints(next);
            }}
          >
            C
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="btn-danger"
            style={{ flexShrink: 0, color: "var(--color-danger)" }}
            onClick={() => setEndpoints(endpoints.filter((_, i) => i !== idx))}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
            </svg>
          </Button>
        </div>
      ))}
    </FormSection>
  );
}
