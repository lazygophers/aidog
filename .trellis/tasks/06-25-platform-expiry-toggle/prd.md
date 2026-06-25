# 平台过期时间: 启用 toggle + 粘贴识别规则 + 主题适配

## 需求 (用户澄清)

1. **手动添加**: 默认 **无** 过期时间 (toggle OFF, datetime-local 隐藏)。用户勾选 "启用过期" toggle → 才显示 datetime-local。
2. **粘贴识别**: 识别到的过期时间 **填入 state**, 但 **toggle 保持 OFF** (不自动启用)。用户手动勾选 toggle → 显示已填值的 datetime-local + 生效。
3. **日期粒度修正**: 识别到日期级 (无时间, 如 "27日") → expiresAt = 该日 23:59:59.999 (当日结束, = 次日前一秒), 不是 00:00:00。
4. **主题适配**: 拆到独立 task `datetime-local-theme` (本 task 不含)。

## 现状

`src/pages/Platforms.tsx`:
- `expiresAt` state 默认 0 (line 1490)
- datetime-local 直接暴露 (3152-3180), value = `expiresAt>0 ? toDatetimeLocal : ""`
- 已有清空按钮
- 加载平台: `setExpiresAt(p.expires_at ?? 0)` (1646/2022/2100)
- 主题: `style={{ colorScheme: themeMode }}` (3162) — **无效 (用户确认全部主题不对)**

`src/utils/platformPaste.ts`:
- `extractExpiryAt` 收紧模式 (356-458, 语义词 + 60 字符)
- line 505: `expiresAt: extractExpiryAt(text)` — 粘贴时填入

## 方案

### S1 — expiryEnabled state (Platforms.tsx)

```ts
const [expiryEnabled, setExpiryEnabled] = useState(false);
```
- 手动添加 / 新建: `expiryEnabled=false`
- 加载已有平台: `setExpiryEnabled((p.expires_at ?? 0) > 0)` (老平台 expires_at>0 → ON)
- 粘贴识别: **不改 expiryEnabled (保持当前, 通常 false)**; 但 expiresAt 填入识别值 (state 有值, toggle OFF 不显示)
- toggle 切换: ON→OFF `setExpiresAt(0)` (清零, 不生效); OFF→ON 显示 datetime-local (若 expiresAt 已有识别值则预填)

三处加载站点 (1646/2022/2100) + reset 路径同步 expiryEnabled。

### S2 — toggle UI + 条件渲染 (Platforms.tsx:3152-3180)

```tsx
<Field title={expiresAt} desc={expiresAtHint}>
  <Toggle checked={expiryEnabled} onChange={...}>  {/* 启用过期 */}
  {expiryEnabled && (
    <>
      <input type="datetime-local" ... />
      {expiresAt>0 && <清空按钮>}
      {expiresAt>0 && <临近过期提示>}
    </>
  )}
</Field>
```

- toggle OFF → 隐藏 datetime-local, 即便 expiresAt state 有识别值也不显示
- toggle ON → 显示 datetime-local (预填 expiresAt 若有)
- toggle ON→OFF: setExpiresAt(0) (清零不生效)
- 粘贴识别后: expiryEnabled 仍 false, expiresAt 有值 → toggle OFF 状态, 用户勾 toggle → datetime-local 出现带预填值

### S3 — 日期粒度修正 (platformPaste.ts extractExpiryAt)

识别到日期级 (无时间分量, 如 "2026-07-15" 或 "7月15日") → expiresAt = 该日本地 23:59:59.999:

```ts
// extractExpiryAt 内, 构造日期时:
// 若原文案无时间分量 → 设为当日 end-of-day
date.setHours(23, 59, 59, 999);
```

识别到带时间 (如 "2026-07-15 18:00") → 保持原时间。

### S5 — i18n × 8 locale

新 key `platform.expiresAtEnable` (启用过期) 加到 `src/locales/*.json` 全语言。

## 验收

1. 手动添加: 过期区只显 toggle (OFF), 无 datetime-local
2. 勾 toggle → datetime-local 出现, 选日期, 保存, 重开仍 ON + 日期在
3. 粘贴识别 (文案含过期词 + 日期): expiresAt 填入识别值, 但 **toggle OFF**, datetime-local 不显示
4. 粘贴后勾 toggle → datetime-local 出现 + 预填识别值
5. 粘贴识别日期级 (无时间) → expiresAt = 当日 23:59:59.999 (28日前一秒验证)
6. 老平台 (expires_at>0): toggle 默认 ON + 日期显示
7. 取消 toggle → datetime-local 隐藏 + expiresAt 清 0
8. badge / candidate_state / smart paste 收紧逻辑不受影响
9. `yarn build` + `cargo test` + `cargo clippy -- -D warnings` + `check-i18n.mjs` 全绿, 无新 warning
10. datetime-local 主题适配不在本 task (拆到 `datetime-local-theme`)

## 不改

- 后端 expires_at 字段语义 (0=永不过期)
- badge / candidate_state 路由过滤
- 清空按钮 (toggle ON 时仍可用)
