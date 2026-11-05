import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelApi, proxyApi } from "../../services/api";
import catalogJson from "../../generated/apiCatalog.json";
import { FORWARD_API, LOCAL_API, curlForCommand, curlForStatic, type StaticEndpoint } from "./apiEndpoints";

interface Cmd {
  name: string; category: string; kind: "readonly" | "mutating" | null;
  exportsSecret: boolean; desktopOnly: boolean;
  params: { name: string; type: string }[]; returns: string;
}
const COMMANDS = catalogJson.commands as Cmd[];
const CATEGORIES = [...new Set(COMMANDS.map((c) => c.category))].sort();

function CurlBlock({ testId, text }: { testId: string; text: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  return (
    <div style={{ display: "flex", gap: 8, alignItems: "flex-start", marginTop: 4 }}>
      <code data-testid={testId} style={{ flex: 1, fontSize: 11, wordBreak: "break-all" }}>{text}</code>
      <button
        type="button"
        data-testid={`${testId}-copy`}
        style={{ fontSize: 11 }}
        onClick={() => {
          void navigator.clipboard?.writeText(text);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
      >
        {copied ? t("apiCatalog.copied") : t("apiCatalog.copy")}
      </button>
    </div>
  );
}

function StaticList({ group, items, port }: { group: "local" | "forward"; items: StaticEndpoint[]; port: number | null }) {
  const { t } = useTranslation();
  return (
    <div className="glass-surface" data-testid={`api-${group}`} style={{ padding: "16px 20px" }}>
      <div style={{ fontSize: 13, fontWeight: 600 }}>{t(`apiCatalog.${group}Title`)}</div>
      <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t(`apiCatalog.${group}Desc`)}</div>
      {items.map((e) => (
        <div key={e.id} data-testid={`api-${group}-${e.id}`} style={{ marginTop: 10, fontSize: 12 }}>
          <code>{`${e.method} ${e.path}`}</code>
          <CurlBlock testId={`api-${group}-curl-${e.id}`} text={curlForStatic(e, port)} />
        </div>
      ))}
    </div>
  );
}

/** 「API」分页接口清单：管理接口（构建期生成物）+ Local API + 转发接口。 */
export function ApiCatalogSection({ refreshKey = 0 }: { refreshKey?: number }) {
  const { t } = useTranslation();
  const [port, setPort] = useState<number | null>(null);
  const [hasToken, setHasToken] = useState(false);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");

  useEffect(() => {
    kernelApi.getSettings().then((s) => setHasToken(s.auth_token !== "")).catch(() => {});
    proxyApi.getSettings().then((p) => setPort(p.port)).catch(() => {});
  }, [refreshKey]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return COMMANDS.filter((c) => (!category || c.category === category) && (!q || c.name.toLowerCase().includes(q)));
  }, [query, category]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <div
        className="glass-surface"
        data-testid="api-catalog"
        data-state={rows.length ? "ready" : "empty"}
        data-port={port ?? ""}
        style={{ padding: "16px 20px" }}
      >
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("apiCatalog.title")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t("apiCatalog.desc")}</div>
        <div style={{ display: "flex", gap: 8, marginTop: 10, alignItems: "center" }}>
          <input
            data-testid="api-catalog-search"
            value={query}
            placeholder={t("apiCatalog.search")}
            onChange={(e) => setQuery(e.target.value)}
            style={{ flex: 1, fontSize: 12 }}
          />
          <select data-testid="api-catalog-category" value={category} onChange={(e) => setCategory(e.target.value)} style={{ fontSize: 12 }}>
            <option value="">{t("apiCatalog.allCategories")}</option>
            {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <span className="text-secondary" data-testid="api-catalog-count" style={{ fontSize: 12 }}>
            {t("apiCatalog.count", { count: rows.length })}
          </span>
        </div>
        {rows.length === 0 && (
          <div className="text-secondary" data-testid="api-catalog-empty" style={{ fontSize: 12, marginTop: 10 }}>
            {t("apiCatalog.empty")}
          </div>
        )}
        {rows.map((c) => (
          <div
            key={c.name}
            data-testid={`api-cmd-${c.name}`}
            data-kind={c.desktopOnly ? "desktop" : c.kind}
            data-secret={String(c.exportsSecret)}
            style={{ marginTop: 12, fontSize: 12 }}
          >
            <code style={{ fontWeight: 600 }}>{c.name}</code>
            <span className="text-secondary">{` · ${c.category} · `}</span>
            <span>
              {c.desktopOnly ? t("apiCatalog.desktopOnly") : c.kind === "readonly" ? t("apiCatalog.readonly") : t("apiCatalog.mutating")}
            </span>
            {c.exportsSecret && <span>{` · ${t("apiCatalog.secret")}`}</span>}
            <div className="text-secondary">
              {`${t("apiCatalog.params")}: ${c.params.length ? c.params.map((p) => `${p.name}: ${p.type}`).join(", ") : "-"}`}
            </div>
            <div className="text-secondary">{`${t("apiCatalog.returns")}: ${c.returns}`}</div>
            {!c.desktopOnly && <CurlBlock testId={`api-cmd-curl-${c.name}`} text={curlForCommand(c.name, c.params, { port, hasToken })} />}
          </div>
        ))}
      </div>
      <StaticList group="local" items={LOCAL_API} port={port} />
      <StaticList group="forward" items={FORWARD_API} port={port} />
    </div>
  );
}
