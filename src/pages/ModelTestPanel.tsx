import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  modelTestApi,
  type Platform,
  type ModelTestResult,
} from "../services/api";
import { IconClose, IconCheck } from "../components/icons";

interface Props {
  platform: Platform;
  onClose: () => void;
  onResult?: (success: boolean) => void;
}

type TestMode = "quick" | "single" | "batch" | "random" | "custom";

export function ModelTestPanel({ platform, onClose, onResult }: Props) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<TestMode>("quick");
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [customPrompt, setCustomPrompt] = useState("");
  const [results, setResults] = useState<ModelTestResult[]>([]);
  const [running, setRunning] = useState(false);
  const [currentIdx, setCurrentIdx] = useState(-1);

  const allModels = platform.available_models.length > 0
    ? platform.available_models
    : [platform.models.default, platform.models.sonnet, platform.models.opus, platform.models.haiku, platform.models.gpt].filter(Boolean) as string[];

  const defaultModel = platform.models.default || allModels[0] || "";

  const getModels = useCallback((): string[] => {
    switch (mode) {
      case "quick": return [defaultModel];
      case "single": return selectedModels.length > 0 ? [selectedModels[0]] : [defaultModel];
      case "batch": return selectedModels.length > 0 ? selectedModels : allModels.slice(0, 5);
      case "random": return allModels;
      case "custom": return selectedModels.length > 0 ? [selectedModels[0]] : [defaultModel];
    }
  }, [mode, selectedModels, allModels, defaultModel]);

  const toggleModel = (m: string) => {
    setSelectedModels(prev =>
      prev.includes(m) ? prev.filter(x => x !== m) : [...prev, m]
    );
  };

  const runTest = async () => {
    const models = getModels();
    if (models.length === 0) return;
    setRunning(true);
    setResults([]);
    const res: ModelTestResult[] = [];

    for (let i = 0; i < models.length; i++) {
      setCurrentIdx(i);
      try {
        const r = await modelTestApi.test({
          platform_id: platform.id,
          model: models[i],
          prompt: (mode === "custom" && customPrompt) ? customPrompt : undefined,
          max_tokens: 64,
        });
        res.push(r);
        setResults([...res]);
      } catch (e) {
        res.push({
          success: false, model: models[i], prompt_preview: "",
          response_preview: "", duration_ms: 0, input_tokens: 0, output_tokens: 0,
          error: String(e),
        });
        setResults([...res]);
      }
      // 每条测试落地（proxy_log source_protocol='test'）后派发全局事件：Platforms 页据此单卡刷新「最近测试」徽章 + health
      // success 取本轮最后一条结果（成功/失败两分支均已 push 到 res）
      window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: platform.id, success: res[res.length - 1]?.success ?? false } }));
    }
    setRunning(false);
    setCurrentIdx(-1);
    // Notify parent of overall test result
    if (onResult) {
      const allOk = res.length > 0 && res.every(r => r.success);
      onResult(allOk);
    }
  };

  const modes: { key: TestMode; label: string }[] = [
    { key: "quick", label: t("test.modeQuick", "快速测试") },
    { key: "single", label: t("test.modeSingle", "指定模型") },
    { key: "batch", label: t("test.modeBatch", "批量测试") },
    { key: "random", label: t("test.modeRandom", "随机提示词批量") },
    { key: "custom", label: t("test.modeCustom", "指定提示词") },
  ];

  const needsModelSelect = mode === "single" || mode === "batch" || mode === "custom";

  return createPortal((
    <div style={{
      position: "fixed", inset: 0, background: "rgba(0,0,0,0.4)", zIndex: 100,
      backdropFilter: "blur(4px)",
      display: "flex", alignItems: "center", justifyContent: "center",
    }} onClick={onClose}>
      <div className="glass-elevated" style={{
        width: 560, maxHeight: "80vh", overflow: "auto", padding: 24,
        borderRadius: 16, display: "flex", flexDirection: "column", gap: 16,
      }} onClick={e => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("test.title", "模型测试")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
              {platform.name} · {platform.platform_type.toUpperCase()}
            </div>
          </div>
          <button className="btn btn-ghost btn-icon" onClick={onClose}><IconClose size={16} /></button>
        </div>

        <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
          {modes.map(m => (
            <button key={m.key}
              className={mode === m.key ? "btn-active" : "btn"}
              style={{ fontSize: 11, padding: "4px 8px" }}
              onClick={() => { setMode(m.key); setSelectedModels([]); setResults([]); }}
            >{m.label}</button>
          ))}
        </div>

        {needsModelSelect && (
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap", maxHeight: 100, overflow: "auto" }}>
            {allModels.map(m => (
              <button key={m}
                className={selectedModels.includes(m) ? "btn-active" : "btn"}
                style={{ fontSize: 11, padding: "3px 8px" }}
                onClick={() => toggleModel(m)}
              >{m}</button>
            ))}
          </div>
        )}

        {mode === "custom" && (
          <textarea className="input" style={{ fontSize: 12, minHeight: 60, resize: "vertical" }}
            placeholder={t("test.promptPlaceholder", "输入测试提示词...")}
            value={customPrompt} onChange={e => setCustomPrompt(e.target.value)} />
        )}

        <button className="btn-active"
          style={{ fontSize: 13, padding: "8px 16px", alignSelf: "flex-start" }}
          onClick={runTest}
          disabled={running || (needsModelSelect && selectedModels.length === 0 && mode !== "batch" && allModels.length === 0)}
        >
          {running
            ? t("test.running", "测试中...") + (currentIdx >= 0 ? ` (${currentIdx + 1}/${getModels().length})` : "")
            : t("test.run", "开始测试")}
        </button>

        {results.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("test.results", "测试结果")}</div>
            {results.map((r, i) => (
              <div key={i} className="glass-surface" style={{
                padding: "10px 14px",
                borderLeft: `3px solid ${r.success ? "var(--success, #22c55e)" : "var(--danger, #ef4444)"}`,
              }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div style={{ fontWeight: 600, fontSize: 12 }}>{r.model}</div>
                  <div style={{ display: "flex", gap: 8, fontSize: 11, color: "var(--text-secondary)" }}>
                    <span style={{ display: "inline-flex" }}>{r.success ? <IconCheck size={12} color="var(--success, #22c55e)" /> : <IconClose size={12} color="var(--danger, #ef4444)" />}</span>
                    {r.duration_ms > 0 && <span>{r.duration_ms}ms</span>}
                    {r.output_tokens > 0 && <span>{r.input_tokens + r.output_tokens} tok</span>}
                  </div>
                </div>
                {r.error && <div style={{ fontSize: 11, color: "var(--danger, #ef4444)", marginTop: 4 }}>{r.error}</div>}
                {r.response_preview && (
                  <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 4, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                    {r.response_preview}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  ), document.body);
}
