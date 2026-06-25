# 实施计划 — platform-expiry-toggle

读 prd.md。范围 = 前端 Platforms.tsx + platformPaste.ts + 主题 CSS + 8 locale。

## S1 — expiryEnabled state (Platforms.tsx)

1. line 1490 附近加 `const [expiryEnabled, setExpiryEnabled] = useState(false);`
2. 三处加载站点同步:
   - 1646: `if (r.expiresAt && r.expiresAt > 0) { setExpiresAt(r.expiresAt); setExpiryEnabled(true); }`
   - 2022/2100: `setExpiresAt(p.expires_at ?? 0); setExpiryEnabled((p.expires_at ?? 0) > 0);`
3. reset/新建路径 (grep setExpiresAt(0) reset 点): 同步 `setExpiryEnabled(false)`
4. **粘贴识别路径不改 expiryEnabled** (保持 false; expiresAt 由 paste 填入, toggle OFF 不显示)

## S2 — toggle UI + 条件渲染 (Platforms.tsx:3152-3180)

```tsx
<Field title={t("platform.expiresAt")} desc={t("platform.expiresAtHint")}>
  <label className="expiry-toggle-row">
    <input type="checkbox" checked={expiryEnabled}
      onChange={(e) => {
        const en = e.target.checked;
        setExpiryEnabled(en);
        if (!en) setExpiresAt(0);  // OFF → 清零
      }} />
    {t("platform.expiresAtEnable", "启用过期")}
  </label>
  {expiryEnabled && (
    <>
      <input type="datetime-local" value={expiresAt>0 ? toDatetimeLocal(expiresAt) : ""} ... />
      {expiresAt>0 && <清空按钮>}
      {expiresAt>0 && <临近过期提示>}
    </>
  )}
</Field>
```

- grep 现有 toggle/switch/checkbox 用法对齐风格 (Liquid Glass)
- toggle OFF → 隐藏 datetime-local 即使 state 有值
- 粘贴后 expiryEnabled=false → datetime-local 不显, 用户勾 toggle 才显 + 预填

## S3 — 日期粒度修正 (platformPaste.ts extractExpiryAt)

读 extractExpiryAt 全函数 (356-458)。识别日期候选时:
- 若候选 **无时间分量** (只到日, 如 "2026-07-15") → setHours(23,59,59,999) (本地当日结束)
- 若候选 **带时间** → 保持原时间

判定无时间分量: 候选正则未匹配 HH:MM。或构造 Date 后检测原文案是否含 `:` 时间标记。

## S5 — i18n × 8 locale

`src/locales/*.json` (确认实际 locale 数, CLAUDE.md 说 7 语言) 加:
```json
"expiresAtEnable": "<翻译>"
```
- zh-CN: 启用过期
- en-US: Enable expiry
- ar-SA: تمكين الانتهاء
- fr-FR: Activer l'expiration
- de-DE: Ablauf aktivieren
- ru-RU: Включить срок
- ja-JP: 期限を有効化

跑 `check-i18n.mjs` 验全覆盖。

## 验收

1. S1-S5 全做
2. `cargo test` + `cargo clippy --all-targets -- -D warnings` + `yarn build` + `check-i18n.mjs` 全绿
3. 无新 warning (block future-incompat 除外)
4. 手动验 prd 验收 1-10

## 执行顺序

单 agent 顺序 S1→S2→S3→S4→S5。完成后 main 跑 check。

## 禁

- 禁改后端 expires_at 语义
- 禁改 badge / candidate_state
- 禁 git commit / push
