// 分页导航（自 PricingTab 迁入，逻辑零变更）：页码按钮 + 跳页 + 每页数量。
// 模型信息页的数据源是一次 snapshot 全量，分页纯客户端切片。

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export const PAGE_SIZE_OPTIONS = [20, 50, 100, 200];

export function Pagination({
  currentPage, totalPages, total, pageSize, pageSizeOptions,
  jumpPage, onJumpPageChange, onJump, onPageSizeChange, onPageChange,
}: {
  currentPage: number;
  totalPages: number;
  total: number;
  pageSize: number;
  pageSizeOptions: number[];
  jumpPage: string;
  onJumpPageChange: (v: string) => void;
  onJump: () => void;
  onPageSizeChange: (size: number) => void;
  onPageChange: (page: number) => void;
}) {
  const rangeStart = total === 0 ? 0 : (currentPage - 1) * pageSize + 1;
  const rangeEnd = Math.min(currentPage * pageSize, total);

  const pages: (number | "ellipsis")[] = [];
  if (totalPages <= 7) {
    for (let i = 1; i <= totalPages; i++) pages.push(i);
  } else {
    pages.push(1);
    if (currentPage > 3) pages.push("ellipsis");
    const start = Math.max(2, currentPage - 1);
    const end = Math.min(totalPages - 1, currentPage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (currentPage < totalPages - 2) pages.push("ellipsis");
    pages.push(totalPages);
  }

  const btnStyle: React.CSSProperties = {
    fontSize: 12, padding: "4px 8px", minWidth: 28, textAlign: "center",
  };

  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span className="text-tertiary" style={{ fontSize: 12 }}>
          {rangeStart}–{rangeEnd} / {total}
        </span>
        <Select value={String(pageSize)} onValueChange={v => onPageSizeChange(Number(v))}>
          <SelectTrigger style={{ fontSize: 12, padding: "2px 6px", width: 80, height: 28 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {pageSizeOptions.map(s => <SelectItem key={s} value={String(s)}>{s}/page</SelectItem>)}
          </SelectContent>
        </Select>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
        <Button variant="ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(1)} title="First">⟪</Button>
        <Button variant="ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(currentPage - 1)}>←</Button>
        {pages.map((p, i) =>
          p === "ellipsis" ? (
            <span key={`e${i}`} className="text-tertiary" style={{ fontSize: 12, padding: "0 4px" }}>…</span>
          ) : (
            <Button key={p} variant={p === currentPage ? "default" : "ghost"}
              // 当前页走 default variant（primary 底），文字色交给 variant 自己的
              // primary-foreground —— 之前额外覆写 color: var(--accent) 等于把强调色写在
              // 强调色底上，两种模式下都近乎看不清。
              style={p === currentPage ? { ...btnStyle, fontWeight: 700 } : btnStyle}
              onClick={() => onPageChange(p)}>{p}</Button>
          ),
        )}
        <Button variant="ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(currentPage + 1)}>→</Button>
        <Button variant="ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(totalPages)} title="Last">⟫</Button>

        <div style={{ display: "flex", alignItems: "center", gap: 4, marginLeft: 8 }}>
          <Input
            type="number" min={1} max={totalPages} value={jumpPage}
            onChange={e => onJumpPageChange(e.target.value)}
            onKeyDown={e => { if (e.key === "Enter") onJump(); }}
            placeholder="#"
            style={{ width: 50, fontSize: 12, padding: "3px 6px", textAlign: "center", height: 28 }}
          />
          <Button variant="ghost" style={btnStyle} onClick={onJump}>Go</Button>
        </div>
      </div>
    </div>
  );
}
