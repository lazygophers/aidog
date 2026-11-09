// 「请求类型」过滤器（首页请求流量卡 / 分组请求流向面板共用）。value：'' = 全部，'unknown' = 未知，其余 = 原始类型值。
import { useTranslation } from "react-i18next";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { REQUEST_CLASSES, REQUEST_CLASS_UNKNOWN, requestClassLabel } from "../utils/requestClass";

// Radix SelectItem 不允许空串 value，「全部」在组件内用 "all" 表示，对外仍是 ''。
const ALL = "all";

export function RequestClassFilter({ value, onChange, labelColor }: {
  value: string;
  onChange: (v: string) => void;
  labelColor?: string;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
      <span style={{ fontSize: 11, color: labelColor ?? "var(--text-secondary)" }}>{t("logs.requestClassLabel", "请求类型")}</span>
      <Select value={value === "" ? ALL : value} onValueChange={v => onChange(v === ALL ? "" : v)}>
        <SelectTrigger data-testid="request-class-filter" className="h-7 w-auto min-w-[96px] text-xs"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value={ALL}>{t("flow.requestClass.all", "全部")}</SelectItem>
          {REQUEST_CLASSES.map(c => <SelectItem key={c} value={c}>{requestClassLabel(t, c)}</SelectItem>)}
          <SelectItem value={REQUEST_CLASS_UNKNOWN}>{t("flow.requestClass.unknown", "未知")}</SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}
