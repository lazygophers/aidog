# 保留时间单位选择器 — 详细设计

## 数据模型

### Rust enum（`aidog_core/src/gateway/models/proxy_log.rs`）
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetentionUnit {
    Hour,
    Day,
    Week,
}
impl Default for RetentionUnit { fn default() -> Self { Self::Day } }
impl RetentionUnit {
    pub fn secs(self, value: u32) -> u64 {
        match self {
            Self::Hour => 3600 * value as u64,
            Self::Day  => 86400 * value as u64,
            Self::Week => 604800 * value as u64,
        }
    }
}
```

### ProxyLogSettings 加 3 字段（紧随现有 `*_retention_days`）
```rust
#[serde(default)]
pub user_request_retention_unit: RetentionUnit,   // default Day (老配置兼容)
#[serde(default)]
pub upstream_request_retention_unit: RetentionUnit,
#[serde(default)]
pub retention_unit: RetentionUnit,
```
默认值改：`default_user_req_retention()` 7→6；`default_upstream_req_retention()` 7→6；`default_retention_days()` 90→6。单位默认仍 Day（serde default），**但 Default impl 里显式设 Hour + 6**（新装走 Default，老配置走 serde default Day → 保持原值语义需老配置 value 仍是 7/7/90）。

**关键矛盾解决**：serde default = Day（老配置无 unit 字段 → Day），但老配置 value 已是 7/7/90（天）→ 7天/7天/90天 ✅ 不变。新装 Default impl → unit=Hour, value=6 → 6h ✅。

### retention_cutoff 改造（`maintenance.rs`）
现有：
```rust
pub(crate) fn retention_cutoff(days: u32) -> Option<i64> {
    if days == 0 { return None; }
    Some((now - Duration::days(days)).timestamp_millis())
}
```
改签名接 secs：
```rust
pub(crate) fn retention_cutoff_secs(secs: u64) -> Option<i64> {
    if secs == 0 { return None; }
    Some((now - Duration::seconds(secs as i64)).timestamp_millis())
}
```
3 处 caller 改：
- `cleanup_user_request_fields(db, value, unit)` → 内部 `retention_cutoff_secs(unit.secs(value))`
- `cleanup_upstream_request_fields` 同
- `cleanup_proxy_logs` 同

## 前端

### TS 类型（`src/services/api/types/part2.ts`）
```typescript
export type RetentionUnit = "hour" | "day" | "week";
export interface ProxyLogSettings {
  // ... 现有字段
  user_request_retention_days: number;
  user_request_retention_unit: RetentionUnit;   // 新
  upstream_request_retention_days: number;
  upstream_request_retention_unit: RetentionUnit; // 新
  retention_days: number;
  retention_unit: RetentionUnit;                  // 新
}
```

### useSystemSettings.ts
- 加 3 state: `userReqRetentionUnit` / `upstreamReqRetentionUnit` / `logRetentionUnit`
- load: 从 `ls.user_request_retention_unit ?? "day"` 读（老后端无字段时 default day — 但后端 serde default Day 后 JSON 必带，无需 ??）
- updateLogSettings payload 加 unit 字段

### LogSettingsSection.tsx
三项各加 `<select>`：
```tsx
<input type="number" ... />
<select
  className="input"
  value={userReqRetentionUnit}
  onChange={(e) => { setUserReqRetentionUnit(e.target.value); updateLogSettings({ user_request_retention_unit: e.target.value }); }}
  style={{ width: 70 }}
>
  <option value="hour">{t("unit.hour", "小时")}</option>
  <option value="day">{t("unit.day", "天")}</option>
  <option value="week">{t("unit.week", "周")}</option>
</select>
```
- label 文案「保留天数」→「保留时间」（更准确）
- 「永久保留」逻辑：value === 0（不看单位，0 永远是永久）

## i18n（8 locale）
新增 3 扁平 key（[[locale-flat-key-convention]]）：
- `"unit.hour"`: 小时 / Hours / ساعة / Heures / Stunden / Часы / 時間 / Horas
- `"unit.day"`: 天 / Days / يوم / Jours / Tage / Дни / 日 / Días（注意：现有 `"unit.days"` 复数已存，新增单数 `"unit.day"`）
- `"unit.week"`: 周 / Weeks / أسبوع / Semaines / Wochen / Недели / 週 / Semanas

现有 label key 改文案（不改 key 名，向后兼容）：
- `"proxy.userReqRetention"`: "原始请求保留天数" → "原始请求保留时间"
- `"proxy.upstreamReqRetention"`: 同
- `"proxy.logRetention"`: "日志记录保留天数" → "日志记录保留时间"

## 关键取舍

| 取舍 | 选 | 理由 |
|---|---|---|
| 字段名是否 rename | 不 rename（保 `*_retention_days`） | serde 向后兼容，老 settings.json 无需 migration |
| serde default vs Default impl 分离 | serde default=Day（老兼容），Default impl=Hour+6（新装） | 老配置保行为，新装得 6h |
| 单位 enum 共享 vs per-field | 共享 RetentionUnit enum，字段独立 | 类型复用，字段解耦用户可分别配 |
| retention_cutoff 改 secs vs 加重载 | 改签名（只 3 caller） | 禁冗余重载，YAGNI |
| value=0 永久判定 | 看值不看单位 | 0 小时 = 0 天 = 永久，单位无关 |

## 风险
1. **serde default 与 Default impl 不一致**：老配置反序列化走 serde default（Day），新装走 Default impl（Hour）。测试覆盖两条路径。
2. **TS 老后端兼容**：前端 `?? "day"` fallback 防后端未升级时 unit 字段 undefined（升级期灰度）。
3. **现有 `"unit.days"` 复数 key**：新增 `"unit.day"` 单数，两 key 共存（复数用于其他处如保留 7 天提示），check-i18n 不会误判。
