// 模型详情弹窗：一个 canonical 模型的全部维度，按平台分 tab 比价。
// Dialog 走 Radix Portal（挂 document.body），liquid glass 主题下的 fixed 居中由 Portal 保证。

import { useTranslation } from "react-i18next";
import type { ModelEntry, ModelEntryGroup, Protocol } from "../../services/api";
import { F } from "../../domains/shared/tokens";
import { ProtocolLogo } from "../../domains/platforms/ProtocolLogo";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { fmtPricePerM, fmtTokens, parsePriceData, type PriceTier } from "./priceData";
import { CapabilityBadges } from "./CapabilityBadges";
import { nameParts } from "./ModelName";
import { CopyButton } from "../../components/shared";

export function ModelDetailDialog({ group, labelMap, pricingOnly, onClose }: {
  /** null = 关闭态（父组件用「选中哪个 canonical」单一状态驱动开合）。 */
  group: ModelEntryGroup | null;
  labelMap: Record<string, string>;
  /** 只提供比价条目、不可选为平台的来源 code（litellm / meta / mistral）。 */
  pricingOnly: Set<string>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  if (!group) return null;
  // 可选平台在前，比价参考来源排最后并单独标注，避免被当成「能选的平台」。
  const entries = [...group.entries].sort(
    (a, b) => Number(pricingOnly.has(a.platform_code)) - Number(pricingOnly.has(b.platform_code)),
  );
  const title = nameParts(group.display_name, group.canonical_model);

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent style={{ maxWidth: 720, maxHeight: "82vh", overflow: "auto" }}>
        <DialogHeader>
          <DialogTitle style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap" }}>
            <span>{title.primary}</span>
            {/* 展示名与 canonical id 同串时（读取层回落）只留标题，不把同一串再写一遍 */}
            {title.secondary !== null && (
              <span className="text-tertiary" style={{ fontSize: F.small, fontWeight: 400 }}>
                {t("modelInfo.canonical")}: <code>{title.secondary}</code>
              </span>
            )}
          </DialogTitle>
        </DialogHeader>

        <Tabs defaultValue={group.primary_platform || entries[0]?.platform_code || ""}>
          <TabsList style={{ display: "flex", flexWrap: "wrap", height: "auto" }}>
            {entries.map(e => (
              <TabsTrigger key={e.platform_code} value={e.platform_code} style={{ fontSize: F.small }}>
                <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
                  <ProtocolLogo protocol={e.platform_code as Protocol} size={14} />
                  {labelMap[e.platform_code] ?? e.platform_code}
                  {pricingOnly.has(e.platform_code) && (
                    <span className="text-tertiary">{t("modelInfo.priceRefOnly")}</span>
                  )}
                </span>
              </TabsTrigger>
            ))}
          </TabsList>
          {entries.map(e => (
            <TabsContent key={e.platform_code} value={e.platform_code}>
              <EntryDetail entry={e} />
            </TabsContent>
          ))}
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

function EntryDetail({ entry }: { entry: ModelEntry }) {
  const { t } = useTranslation();
  const price = parsePriceData(entry.price_data);
  const tiers = price.context_tiers ?? [];
  // secondary === null → 展示名就是请求名本身，只留「请求名」一个 Field，不重复渲染同一串
  const { secondary } = nameParts(entry.display_name, entry.model_id);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 14, paddingTop: 8 }}>
      <Section title={t("modelInfo.basics")}>
        <Field label={t("modelInfo.requestName")}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
            <code>{entry.model_id}</code>
            <CopyButton text={entry.model_id} title={t("modelInfo.copyRequestName")} size={12} />
          </span>
        </Field>
        {secondary !== null && (
          <Field label={t("modelInfo.displayName")}>{entry.display_name}</Field>
        )}
        <Field label={t("modelInfo.official")}>
          {entry.official ? t("modelInfo.officialYes") : t("modelInfo.officialNo")}
        </Field>
      </Section>

      <Section title={t("modelInfo.versionChain")}>
        <Field label={t("modelInfo.family")}>{entry.family || "-"}</Field>
        <Field label={t("modelInfo.version")}>{entry.version || "-"}</Field>
        <Field label={t("modelInfo.predecessor")}>{entry.predecessor || "-"}</Field>
      </Section>

      <Section title={t("modelInfo.capabilities")}>
        <CapabilityBadges capabilities={entry.capabilities} />
        <Field label={t("modelInfo.builtinTools")}>
          {entry.builtin_tools_excluded.length === 0
            ? t("modelInfo.builtinToolsAll")
            : entry.builtin_tools_excluded.map(tool => (
              <Badge key={tool} variant="secondary" style={{ fontSize: 10, marginInlineEnd: 4 }}>{tool}</Badge>
            ))}
        </Field>
      </Section>

      <Section title={t("modelInfo.limits")}>
        <Field label={t("modelInfo.maxInput")}>{fmtTokens(entry.max_input_tokens)}</Field>
        <Field label={t("modelInfo.maxOutput")}>{fmtTokens(entry.max_output_tokens)}</Field>
        <Field label={t("modelInfo.contextWindow")}>{fmtTokens(entry.context_window)}</Field>
      </Section>

      <Section title={t("modelInfo.prices")}>
        <PriceRow label={t("modelInfo.priceDefault")} tier={price} />
        {price.peak && <PriceRow label={t("modelInfo.pricePeak")} tier={price.peak} />}
        {tiers.map((tier, i) => (
          <PriceRow
            key={i}
            label={t("modelInfo.priceContextTier").replace("{tokens}", fmtTokens(tier.min_tokens))}
            tier={tier}
          />
        ))}
      </Section>
    </div>
  );
}

function PriceRow({ label, tier }: { label: string; tier: PriceTier }) {
  const { t } = useTranslation();
  return (
    <Field label={label}>
      <span style={{ display: "inline-flex", gap: 12, flexWrap: "wrap" }}>
        <span>{t("modelInfo.colInput")}: {fmtPricePerM(tier.input_cost_per_token)}</span>
        <span>{t("modelInfo.colOutput")}: {fmtPricePerM(tier.output_cost_per_token)}</span>
        <span>{t("modelInfo.colCacheRead")}: {fmtPricePerM(tier.cache_read_input_token_cost)}</span>
      </span>
    </Field>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div style={{ fontSize: F.small, fontWeight: 700, color: "var(--text-secondary)" }}>{title}</div>
      {/* 分区内容落在一块半透明面板上：弹窗里原本是一串裸字段，没有层次也没有玻璃质感 */}
      <div style={{
        display: "flex", flexDirection: "column", gap: 6,
        padding: "10px 12px", borderRadius: "var(--radius-sm)",
        background: "color-mix(in srgb, var(--bg-surface) 60%, transparent)",
        border: "1px solid color-mix(in srgb, var(--border) 40%, transparent)",
      }}>
        {children}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", gap: 10, fontSize: F.hint, alignItems: "baseline", flexWrap: "wrap" }}>
      <span className="text-tertiary" style={{ minWidth: 96 }}>{label}</span>
      <span>{children}</span>
    </div>
  );
}
