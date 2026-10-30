// ModelCombobox — 模型 id 组合框（输入 + 可搜索下拉）。ModelsMatrixSection 单元格与
// DirectModelsSection 共用；候选来源 = 拉取列表 availableModels，缺省回落 registry 默认列表。
import React, { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import type { Protocol } from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import { getDefaultModelList } from "../../domains/platforms";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { ModelPriceTag } from "../../components/shared";
import { useModelPrices } from "../../domains/platforms/modelPrices";

/** 候选列表：availableModels 非空用之，否则 registry 默认列表（getDefaultModelList async，effect 缓存）。 */
export function useModelCandidates(availableModels: string[], protocol: Protocol): string[] {
  const [defaultList, setDefaultList] = useState<string[]>([]);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const list = await getDefaultModelList(protocol);
      if (!cancelled) setDefaultList(list);
    })();
    return () => { cancelled = true; };
  }, [protocol]);
  return availableModels.length > 0 ? availableModels : defaultList;
}

export function ModelCombobox({
  value, onChange, onSelect, source, protocol, open: openProp, onOpenChange,
  placeholder, inputProps, inputStyle, t,
}: {
  value: string;
  onChange: (v: string) => void;
  onSelect: (v: string) => void;
  source: string[];
  protocol: Protocol;
  /** 受控开合（矩阵整表单开）；缺省走内部 state。 */
  open?: boolean;
  onOpenChange?: (o: boolean) => void;
  placeholder?: string;
  inputProps?: React.InputHTMLAttributes<HTMLInputElement> & { "data-testid"?: string };
  inputStyle?: React.CSSProperties;
  t: TFunction;
}) {
  const [innerOpen, setInnerOpen] = useState(false);
  const active = openProp ?? innerOpen;
  const setActive = onOpenChange ?? setInnerOpen;
  const priceMap = useModelPrices(protocol);
  const hasDropdown = source.length > 0;
  const query = value.trim().toLowerCase();
  const filtered = hasDropdown
    ? (query ? source.filter((m) => pinyinMatch(query, m)) : source)
    : [];
  const open = active && filtered.length > 0;
  // 选中格尾紧凑价：flex 兄弟节点右对齐占位，不叠加在文本上（无 maxWidth 省略）
  const cellPrice = value ? priceMap.get(value) : undefined;
  return (
    <Popover open={open} onOpenChange={(o) => { if (!o) setActive(false); }}>
      <PopoverAnchor asChild>
        <div style={{ display: "flex", alignItems: "center", gap: 4, width: "100%" }}>
          <div style={{ position: "relative", flex: 1, minWidth: 0 }}>
            <Input
              className="input"
              style={{ width: "100%", fontSize: 13, padding: "6px 8px", paddingRight: hasDropdown ? 24 : undefined, ...inputStyle }}
              placeholder={placeholder ?? t("platform.models_placeholder", "模型名")}
              {...inputProps}
              value={value}
              onChange={(e) => {
                onChange(e.target.value);
                if (hasDropdown) setActive(true);
              }}
              onFocus={() => {
                if (hasDropdown) setActive(true);
              }}
            />
            {hasDropdown && (
              <Button
                variant="ghost"
                size="icon"
                style={{
                  position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
                  width: 22, height: 22, minWidth: 22, padding: 0,
                  color: "var(--text-tertiary)", cursor: "pointer",
                }}
                onMouseDown={(e) => {
                  e.preventDefault();
                  setActive(!active);
                }}
                title={t("platform.selectModel")}
              >
                ▾
              </Button>
            )}
          </div>
          <ModelPriceTag info={cellPrice} compact />
        </div>
      </PopoverAnchor>
      <PopoverContent
        align="start"
        className="glass-elevated"
        style={{ width: "var(--radix-popover-trigger-width)", maxHeight: 200, overflowY: "auto", padding: 4 }}
        // 焦点留在 input 内以继续输入过滤（下拉在 portal，不抢焦点）。
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        {filtered.map((m) => (
          <Button
            key={m}
            variant="ghost"
            data-testid={`model-option-${m}`}
            style={{
              width: "100%",
              justifyContent: "flex-start",
              gap: 8,
              padding: "6px 12px",
              fontSize: 13,
              fontWeight: value === m ? 600 : 400,
              color: value === m ? "var(--accent)" : "var(--text-primary)",
              background: value === m ? "var(--accent-subtle)" : "transparent",
              borderRadius: "var(--radius-sm)",
            }}
            onMouseDown={(e) => {
              e.preventDefault();
              onSelect(m);
              setActive(false);
            }}
          >
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", textAlign: "left" }}>{m}</span>
            <ModelPriceTag info={priceMap.get(m)} />
          </Button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
