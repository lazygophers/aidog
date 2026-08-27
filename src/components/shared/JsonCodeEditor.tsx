// JSON 代码编辑器：全库唯一的 JSON 文本输入/展示控件（替代裸 textarea 与 <pre class="code-block">）。
//
// 能力（用户 2026-08-27 决策，四项全要）：
//   1. 搜索 / 替换 —— Cmd+F 开面板，Cmd+Opt+F 替换（@codemirror/search 的默认 keymap）
//   2. JSON 语法高亮 —— 颜色全部走主题 CSS 变量，浅色/深色自动跟随，不写死色值
//   3. 语法错误实时标红 —— jsonParseLinter 逐次输入解析，错误行标红 + gutter 图标
//   4. 格式化按钮 + 折叠 + 行号
//
// 只读模式（onChange 省略或 readOnly）供日志详情等展示场景用：同样能搜索、折叠、高亮。
//
// 主题：CodeMirror 不认 CSS 变量做「dark 判定」，故 dark 标志由 data-mode 属性读出（themes/index.ts
// applyTheme 写入），颜色值本身仍用 var() —— 切主题时无需重建 editor，浏览器自己重算。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { linter, lintGutter } from "@codemirror/lint";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { Button } from "@/components/ui/button";
import { F } from "../../domains/shared/tokens";

/** 语法高亮配色：一律引用主题变量，切主题/切明暗自动跟随。 */
const jsonHighlight = HighlightStyle.define([
  { tag: t.propertyName, color: "var(--accent)" },
  { tag: t.string, color: "var(--color-success)" },
  { tag: t.number, color: "var(--color-warning)" },
  { tag: [t.bool, t.null], color: "var(--color-danger)" },
  { tag: [t.punctuation, t.separator, t.brace, t.bracket], color: "var(--text-tertiary)" },
]);

/** 编辑器外观：底色/边框/gutter/搜索面板全部对齐 Liquid Glass 主题变量。 */
const baseTheme = EditorView.theme({
  "&": {
    backgroundColor: "transparent",
    color: "var(--text-primary)",
    fontSize: `${F.body}px`,
  },
  "&.cm-focused": { outline: "none" },
  ".cm-content": {
    fontFamily: '"SF Mono", "Fira Code", "Cascadia Code", monospace',
    padding: "8px 0",
  },
  ".cm-gutters": {
    backgroundColor: "transparent",
    color: "var(--text-tertiary)",
    border: "none",
    borderInlineEnd: "1px solid color-mix(in srgb, var(--border) 40%, transparent)",
  },
  ".cm-activeLine": { backgroundColor: "var(--accent-subtle)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--accent)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--primary) 22%, transparent)",
  },
  ".cm-cursor": { borderLeftColor: "var(--text-primary)" },
  // 搜索/替换面板：默认样式是浏览器原生灰，与玻璃主题割裂
  ".cm-panels": {
    backgroundColor: "var(--bg-surface)",
    color: "var(--text-primary)",
    borderTop: "1px solid color-mix(in srgb, var(--border) 55%, transparent)",
  },
  ".cm-panel.cm-search input, .cm-panel.cm-search button, .cm-panel.cm-search label": {
    fontSize: `${F.small}px`,
  },
  ".cm-panel.cm-search input": {
    backgroundColor: "var(--bg-base)",
    color: "var(--text-primary)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    padding: "3px 6px",
  },
  ".cm-panel.cm-search button": {
    backgroundColor: "var(--bg-glass)",
    color: "var(--text-secondary)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    backgroundImage: "none",
    padding: "3px 8px",
    cursor: "pointer",
  },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in srgb, var(--color-warning) 30%, transparent)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": {
    backgroundColor: "color-mix(in srgb, var(--color-warning) 55%, transparent)",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--bg-floating)",
    color: "var(--text-primary)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
  },
});

/** 读当前明暗模式（themes/index.ts 的 applyTheme 写在 <html data-mode>）。 */
function useDarkMode(): boolean {
  const read = () =>
    typeof document !== "undefined" &&
    document.documentElement.getAttribute("data-mode") === "dark";
  const [dark, setDark] = useState(read);
  useEffect(() => {
    if (typeof document === "undefined") return;
    const el = document.documentElement;
    const ob = new MutationObserver(() => setDark(read()));
    ob.observe(el, { attributes: true, attributeFilter: ["data-mode"] });
    return () => ob.disconnect();
  }, []);
  return dark;
}

export interface JsonCodeEditorProps {
  /** JSON 文本。受控值；父层负责持有字符串（而非对象），避免每次 render 重新 stringify 丢光标。 */
  value: string;
  /** 省略 = 只读模式（仍可搜索/折叠/复制）。 */
  onChange?: (value: string) => void;
  /** 显式只读（onChange 存在但当前禁编辑时用）。 */
  readOnly?: boolean;
  placeholder?: string;
  /** 撑满父容器高度（JSON 模式全屏编辑器用）；与 minHeight/maxHeight 互斥。 */
  fill?: boolean;
  minHeight?: number;
  maxHeight?: number;
  /** 显示格式化按钮（默认：可编辑时显示）。 */
  showFormat?: boolean;
  /** 语法错误标红（默认：可编辑时开）。只读展示非 JSON 文本（SSE / 纯文本）时保持关闭。 */
  lint?: boolean;
  /** 额外错误信息（父层保存失败等），显示在编辑器下方。 */
  error?: string;
  "aria-label"?: string;
}

export function JsonCodeEditor({
  value,
  onChange,
  readOnly,
  placeholder,
  fill,
  minHeight = 140,
  maxHeight,
  showFormat,
  lint,
  error,
  "aria-label": ariaLabel,
}: JsonCodeEditorProps) {
  const { t } = useTranslation();
  const dark = useDarkMode();
  const ref = useRef<ReactCodeMirrorRef>(null);
  const editable = !!onChange && !readOnly;
  const withFormat = showFormat ?? editable;

  // 只读展示默认不挂 linter：日志详情里的 SSE 流 / 纯文本响应体不是合法 JSON，
  // 挂上会满屏标红，对「只是想搜一个字段」的读者是纯噪声。需要时用 lint 显式打开。
  const withLint = lint ?? editable;

  const extensions = useMemo(
    () => [
      json(),
      ...(withLint ? [linter(jsonParseLinter()), lintGutter()] : []),
      syntaxHighlighting(jsonHighlight),
      baseTheme,
      EditorView.lineWrapping,
    ],
    [withLint],
  );

  const handleFormat = useCallback(() => {
    if (!onChange) return;
    try {
      onChange(JSON.stringify(JSON.parse(value), null, 2));
    } catch {
      // 非法 JSON 无法格式化：错误已由 linter 在行内标红，此处静默即可
    }
  }, [onChange, value]);

  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 6,
      ...(fill ? { flex: 1, minHeight: 0 } : {}),
    }}>
      {withFormat && (
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Button
            type="button"
            variant="ghost"
            onClick={handleFormat}
            style={{ fontSize: F.small, padding: "2px 8px", height: "auto" }}
          >
            {t("jsonEditor.format")}
          </Button>
          <span className="text-tertiary" style={{ fontSize: F.small }}>
            {t("jsonEditor.searchHint")}
          </span>
        </div>
      )}
      <div
        aria-label={ariaLabel}
        className="glass-surface"
        style={{
          overflow: "auto",
          ...(fill ? { flex: 1, minHeight: 0 } : { minHeight, ...(maxHeight ? { maxHeight } : {}) }),
        }}
      >
        <CodeMirror
          ref={ref}
          value={value}
          onChange={onChange}
          editable={editable}
          readOnly={!editable}
          placeholder={placeholder}
          theme={dark ? "dark" : "light"}
          height={fill ? "100%" : undefined}
          extensions={extensions}
          basicSetup={{
            lineNumbers: true,
            foldGutter: true,
            highlightActiveLine: editable,
            highlightActiveLineGutter: editable,
            autocompletion: false,
            // 搜索/替换 keymap 由 basicSetup 的 searchKeymap 提供（Cmd+F / Cmd+Opt+F）
            searchKeymap: true,
          }}
        />
      </div>
      {error && (
        <div style={{ fontSize: F.small, color: "var(--color-danger)", wordBreak: "break-all" }}>
          {error}
        </div>
      )}
    </div>
  );
}
