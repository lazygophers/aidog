// PathPickerHost — 浏览器形态的「选路径」弹窗（票 10）。
//
// 桌面版永远看不到它：`pickPath()` 在 Tauri 下直接弹原生文件对话框，压根不会碰这个组件。
// 浏览器里没有原生对话框，也拿不到 `<input type=file>` 的绝对路径，所以退化成
// **文本框 + 服务端补全**——复用设置页那个 `PathInput`（同一份补全逻辑、同样的 Tab 键行为）。
//
// 挂一次就够：`App.tsx` 根部挂 `<PathPickerHost/>`，`pickPath()` 通过模块级订阅推请求进来。
// 弹窗用项目里的 `Dialog`（Radix Portal 到 body，满足 `yarn check:modal`）。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { registerPathPickerHost, type PathPickerRequest } from "../../services/pathPicker";
import { PathInput } from "../settings/editors/_shared";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";

export function PathPickerHost() {
  const { t } = useTranslation();
  const [req, setReq] = useState<PathPickerRequest | null>(null);
  const [value, setValue] = useState("");

  useEffect(() => {
    registerPathPickerHost((incoming) => {
      setValue(incoming.options.defaultPath ?? "");
      setReq(incoming);
    });
    return () => registerPathPickerHost(null);
  }, []);

  if (!req) return null;

  const { options } = req;
  const finish = (path: string | null) => {
    req.resolve(path);
    setReq(null);
    setValue("");
  };

  const title =
    options.title ??
    (options.directory
      ? t("settings.editor.chooseDir", "选择目录")
      : t("settings.editor.chooseFile", "选择文件"));

  const extHint = options.filters?.flatMap((f) => f.extensions.map((e) => `.${e}`)).join(" / ");

  return (
    <Dialog open onOpenChange={(open) => { if (!open) finish(null); }}>
      <DialogContent style={{ maxWidth: 520 }}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            {options.save
              ? t("pathPicker.saveHint", "浏览器里没有系统「保存到」对话框，请直接填写要写入的完整路径。")
              : t("pathPicker.openHint", "浏览器里没有系统文件对话框，请直接输入路径（输入 ~/ 浏览主目录，Tab 补全）。")}
            {extHint ? ` (${extHint})` : ""}
          </DialogDescription>
        </DialogHeader>

        <PathInput
          value={value}
          onChange={(v) => setValue(v ?? "")}
          pathType={options.directory ? "directory" : "file"}
        />

        <DialogFooter>
          <Button variant="ghost" onClick={() => finish(null)}>
            {t("common.cancel", "取消")}
          </Button>
          <Button
            variant="default"
            disabled={!value.trim()}
            onClick={() => finish(value.trim())}
          >
            {t("action.confirm", "确认")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
