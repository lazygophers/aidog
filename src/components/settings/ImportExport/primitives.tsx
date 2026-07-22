// 导入导出子系统的共享 UI primitives（纯展示组件）。
// 抽自原 ImportExport.tsx L700-1140。SectionHeader/TextButton 合并了 CcSwitchImport.tsx 的
// SectionHeaderSimple/TextButtonSimple（签名一致、实现等价），CcSwitchImport 改 import 消重。

import { useState } from "react";
import { SectionIcon } from "../editors";
import { IconCheck } from "../../icons";
import { Button } from "@/components/ui/button";

/** section 头：图标 + 标题 + 描述。 */
export function SectionHeader({ icon, title, desc }: { icon: string; title: string; desc: string }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionIcon name={icon} size={18} style={{ color: "var(--accent)" }} />
        <h3 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: "var(--text-primary)" }}>{title}</h3>
      </div>
      <p style={{ margin: 0, fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>{desc}</p>
    </div>
  );
}

/** 文字按钮（全选/反选/批量），accent 文字、无填充。 */
export function TextButton({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <Button variant="outline"
      onClick={onClick}
      style={{
        background: "transparent",
        border: "none",
        color: "var(--accent)",
        fontSize: 13,
        fontWeight: 500,
        cursor: "pointer",
        padding: 0,
      }}
    >
      {children}
    </Button>
  );
}

/** scope 选择卡：整卡可点 toggle，选中态 accent 边 + subtle 底 + 右上角 ✓。 */
export function ScopeCard({
  icon,
  label,
  desc,
  selected,
  indeterminate,
  onToggle,
}: {
  icon: string;
  label: string;
  desc: string;
  selected: boolean;
  indeterminate?: boolean;
  onToggle: () => void;
}) {
  const [hover, setHover] = useState(false);
  const on = selected || indeterminate;
  return (
    <div
      className="glass-surface"
      role="button"
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: "relative",
        padding: 14,
        borderRadius: "var(--radius-lg)",
        cursor: "pointer",
        border: `1px solid ${on ? "var(--primary)" : "var(--border)"}`,
        background: on ? "var(--accent-subtle)" : "transparent",
        boxShadow: hover ? "var(--shadow-md)" : "var(--shadow-sm)",
        transform: hover ? "translateY(-1px)" : "none",
        transition: "var(--transition)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      {/* 右上角选中指示 */}
      <span
        style={{
          position: "absolute",
          top: 10,
          right: 10,
          width: 18,
          height: 18,
          borderRadius: "50%",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          border: `1px solid ${on ? "var(--primary)" : "var(--border)"}`,
          background: on ? "var(--primary)" : "transparent",
          transition: "var(--transition)",
        }}
      >
        {selected && !indeterminate && <IconCheck size={12} color="var(--primary-foreground)" strokeWidth={2.5} />}
        {indeterminate && <span style={{ width: 8, height: 2, background: "var(--primary-foreground)", borderRadius: 1 }} />}
      </span>

      <SectionIcon name={icon} size={20} style={{ color: on ? "var(--primary)" : "var(--text-secondary)" }} />
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", paddingRight: 24 }}>{label}</div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{desc}</div>
    </div>
  );
}

/** 导出成功消息卡（含文件路径，语义成功色）。 */
export function SuccessPathCard({ message }: { message: string }) {
  return (
    <div
      className="glass-elevated"
      style={{
        padding: 12,
        borderRadius: "var(--radius-md)",
        border: "1px solid var(--color-success)",
        background: "var(--color-success-bg)",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <IconCheck size={16} color="var(--color-success)" strokeWidth={2.5} style={{ flexShrink: 0 }} />
      <span
        style={{
          fontSize: 13,
          color: "var(--text-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={message}
      >
        {message}
      </span>
    </div>
  );
}

/** 导入入口（虚线 glass 区）：点击触发 open；原生拖入时 active=true 高亮。 */
export function DropZone({ onClick, active, title, hint }: { onClick: () => void; active: boolean; title: string; hint: string }) {
  const [hover, setHover] = useState(false);
  const lit = hover || active;
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "28px 20px",
        borderRadius: "var(--radius-lg)",
        border: `1.5px dashed ${lit ? "var(--primary)" : "var(--border)"}`,
        background: lit ? "var(--accent-subtle)" : "var(--bg-glass)",
        cursor: "pointer",
        transition: "var(--transition)",
        transform: active ? "scale(1.01)" : "none",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
        textAlign: "center",
      }}
    >
      <SectionIcon name="file" size={28} style={{ color: lit ? "var(--primary)" : "var(--text-secondary)" }} />
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{title}</div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{hint}</div>
    </div>
  );
}

/** 小复选框（受控 √ 方块，accent 选中态）。 */
export function CheckBox({ checked, indeterminate }: { checked: boolean; indeterminate?: boolean }) {
  const on = checked || indeterminate;
  return (
    <span
      style={{
        width: 16,
        height: 16,
        flexShrink: 0,
        borderRadius: "var(--radius-sm)",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        border: `1px solid ${on ? "var(--primary)" : "var(--border)"}`,
        background: on ? "var(--primary)" : "transparent",
        transition: "var(--transition)",
      }}
    >
      {checked && !indeterminate && <IconCheck size={11} color="var(--primary-foreground)" strokeWidth={3} />}
      {indeterminate && <span style={{ width: 8, height: 2, background: "var(--primary-foreground)", borderRadius: 1 }} />}
    </span>
  );
}

/** 折叠箭头（▸ 旋转，open 时 90°）。 */
export function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      width={12}
      height={12}
      viewBox="0 0 24 24"
      fill="none"
      stroke="var(--text-tertiary)"
      strokeWidth={2.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ transform: open ? "rotate(90deg)" : "none", transition: "var(--transition)", flexShrink: 0 }}
    >
      <polyline points="9 18 15 12 9 6" />
    </svg>
  );
}

/** meta 行：左 label（次级）右 value（主）。 */
export function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", alignItems: "baseline", gap: 8, fontSize: 13 }}>
      <span style={{ color: "var(--text-tertiary)", minWidth: 72 }}>{label}</span>
      <span style={{ color: "var(--text-primary)", fontWeight: 500, wordBreak: "break-all" }}>{value}</span>
    </div>
  );
}

/** 3 段分段控件（覆盖/跳过/重命名）。 */
export function Segmented({
  value,
  options,
  onSelect,
}: {
  value: string;
  options: { id: string; label: string }[];
  onSelect: (id: string) => void;
}) {
  return (
    <div
      style={{
        display: "inline-flex",
        borderRadius: "var(--radius-sm)",
        border: "1px solid var(--border)",
        overflow: "hidden",
      }}
    >
      {options.map((opt, i) => {
        const active = value === opt.id;
        return (
          <Button variant="outline"
            key={opt.id}
            onClick={() => onSelect(opt.id)}
            style={{
              padding: "5px 12px",
              fontSize: 12,
              fontWeight: active ? 600 : 500,
              cursor: "pointer",
              border: "none",
              borderLeft: i > 0 ? "1px solid var(--border)" : "none",
              background: active ? "var(--accent-subtle)" : "transparent",
              color: active ? "var(--primary)" : "var(--text-secondary)",
              transition: "var(--transition)",
            }}
          >
            {opt.label}
          </Button>
        );
      })}
    </div>
  );
}
