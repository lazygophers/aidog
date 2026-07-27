import { useTranslation } from "react-i18next";
import { makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import type { Group } from "@/services/api";
import { NONE, fieldLabel } from "./constants";

// 导入 Dialog，从 CliProxy/index.tsx 原地搬出（c10）。
interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  source: string;
  setSource: (v: string) => void;
  authDir: string;
  setAuthDir: (v: string) => void;
  groupId: number | "";
  setGroupId: (v: number | "") => void;
  groups: Group[];
  busy: boolean;
  onPickFile: () => void;
  onPickDir: () => void;
  onImport: () => void;
}

export function ImportDialog({
  open, onOpenChange, source, setSource, authDir, setAuthDir,
  groupId, setGroupId, groups, busy, onPickFile, onPickDir, onImport,
}: ImportDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="glass-elevated" style={{ maxWidth: 520, padding: 20 }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
            {t("cliProxy.import")}
          </DialogTitle>
        </DialogHeader>
        <label style={fieldLabel}>
          {t("cliProxy.importSource")}
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={source}
              onChange={e => setSource(e.target.value)}
              placeholder="config.yaml / .zip / .tgz / dir"
            />
            <Button
              variant="ghost"
              className="ripple"
              onClick={(e) => { makeRipple(e); onPickFile(); }}
              style={{ flexShrink: 0 }}
            >
              {t("cliProxy.importPickFile")}
            </Button>
          </div>
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.importAuthDir")}
          <div style={{ display: "flex", gap: 8 }}>
            <Input
              value={authDir}
              onChange={e => setAuthDir(e.target.value)}
              placeholder="~/.claude/auth.json dir (optional)"
            />
            <Button
              variant="ghost"
              className="ripple"
              onClick={(e) => { makeRipple(e); onPickDir(); }}
              style={{ flexShrink: 0 }}
            >
              {t("cliProxy.importPickDir")}
            </Button>
          </div>
        </label>
        <label style={fieldLabel}>
          {t("cliProxy.groupId")}
          <Select
            value={groupId === "" ? NONE : String(groupId)}
            onValueChange={v => setGroupId(v === NONE ? "" : Number(v))}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE}>—</SelectItem>
              {groups.map(g => (
                <SelectItem key={g.id} value={String(g.id)}>{g.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
        <DialogFooter>
          <Button
            variant="ghost"
            className="ripple"
            onClick={(e) => { makeRipple(e); onOpenChange(false); }}
            disabled={busy}
          >
            {t("cliProxy.cancel")}
          </Button>
          <Button
            variant="default"
            className="ripple"
            onClick={(e) => { makeRipple(e); onImport(); }}
            disabled={busy}
          >
            {t("cliProxy.import")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
