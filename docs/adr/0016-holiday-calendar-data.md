# ADR 0016: 节假日日历——内置静态数据、官方年度表为准、缺年按星期回落

## 背景

peak / 时段窗口需要按「工作日 / 节假日」限定（`platform.schema.json` 的 `day_type`），
平台还需 `holiday_calendar` 字段声明所在地默认日历。节假日数据一年一变，且官方
年度表逐地区公布，无法在运行时可靠获取。

## 决策

1. **数据来源与可信分级**：只以官方来源为准——国家级法规 / 官方年度表（Finlex、
   BOE、gov.uk 等）> 官方人事部门年度表（美国州雇员带薪假日）> EURES 国家页面。
   每份日历的 `source_urls` 必须是官方 HTTPS URL；取不到官方正文的日期不写入。
2. **数据形状**：`src-tauri/defaults/holidays/<code>.json`，一地区一文件：
   `code`（= 文件名）/ `name`（8 locale）/ `extends`（可选，继承父日历，如
   `CH-*` 继承 `CH` 的 8 月 1 日）/ `source_urls` / `years.<year>.{holidays, workdays}`。
   同一日期不得同时出现在两数组；`extends` 禁环。
3. **判定语义**（`day_type` 命中顺序）：
   - `workdays` 优先于 `holidays`（调休补班日覆盖周末 / 假日）；
   - 日历缺当前年 → 按纯星期回落（周一到周五 = 工作日）；
   - 窗口带 `day_type` 时忽略 `days_of_week`（schema 层两者互斥，`not` 约束）。
4. **编译期内置**：`aidog_db/build.rs` 枚举 `defaults/holidays/` 并 `include_str!`
   全部文件，新增文件自动纳入；无远程同步、无运行时下载。
5. **门禁**：`node scripts/check-holidays.mjs`——缺当年 = error（exit 1），缺次年 =
   warning；校验 code 与文件名一致、8 locale 非空、日期格式与年份归属、去重、
   holidays ∩ workdays = ∅、`extends` 存在与无环。
6. **2027 未公布例外**：官方尚未发布 2027 年表的日历（现为 US-CO / US-LA / US-MS /
   US-TN）不写推算日期，保留 warning，官方公布后按官方年度表补写。⚠ 门禁上线当年
   1 月 1 日起，缺当年即为 error，届时仍未补写会构建失败。

## 后果

- 数据是快照不是承诺：文件带 2026/2027 不表示逐年都会更新；更新靠人工按官方
  年度表维护，`check-holidays.mjs` 只拦「当年缺失」这一底线。
- 判定只看内置数据，改数据需重新编译（桌面版随构建更新）。
