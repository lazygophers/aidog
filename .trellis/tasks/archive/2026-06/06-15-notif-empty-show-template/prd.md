# 通知模板为空时展示默认模板（后端 render + 前端预览）

## 需求
用户留空 `per_type[type].template` 时，应展示**内置默认模板**，而非空字符串 / 退化到英文类型名。兑现设置页占位文案承诺「留空使用内置默认模板」。范围 = **1（后端 render 兜底）+ 2（前端预览/占位）**，二者一致。

注：与已完成的 fix-notif-template-stale（模板「变更不生效」stale 问题，commit 9379b37）是不同关注点——本任务是「为空时的兜底展示」。

## 现状（事实，已核）
- 后端 `src-tauri/src/gateway/notification.rs::render`（:128-160）body 兜底链：`template > content > default_template（仅 has_project 时）> default_title`。
  - **`has_project` 门禁**：无 project var 时，空 template 退化到英文 `default_title()`（"Task Complete" 等），**不用** default_template。
  - 该门禁存在的原因：`default_template()` 含 `{project}` 占位（见下），无 project 时若直接渲染，`substitute_vars` 会把 `{project}` 当未知占位**原样保留**，弹出 "`{project} 完成`" 字面，故当初用 default_title 规避。
- 默认模板 `src-tauri/src/gateway/models.rs::default_template`（:1677）：
  - TaskComplete `"{project} 完成"` / WaitingInput `"{project} 等待用户输入"` / Error `"{project} 出错"` / Custom `"{project} 通知"`（zh 硬编码，**非 i18n**）。
- 前端 `src/components/settings/NotificationSettings.tsx`：
  - template textarea（:448-450）`value={ts.template}` + `placeholder=t("notif.templatePlaceholder","留空使用内置默认模板")`（通用文案，**未展示实际默认模板内容**）。
  - 前端**无**默认模板副本。

## 实现要点

### 1. 后端 render 兜底（notification.rs::render）
- 目标：template+content 都空时，**无论有无 project** 都渲染 `default_template`，且**不泄漏 `{project}` 字面**。
- 做法：渲染 default_template 前，确保 `{project}` 有值——当 vars 无 project（或值空）时，注入一个**品牌兜底名**（建议 `"aidog"`，或取 `app.config().product_name`；但 render 是纯函数无 app，用常量 `"aidog"` 最简且稳定）。
  - 即：空 body 兜底分支改为「取 default_template → 用带 project 兜底的 vars 做 substitute_vars」，结果如无 project 时 `"{project} 完成"` → `"aidog 完成"`，而非 `"Task Complete"` 或字面 `{project}`。
  - 保留 `default_title` 仅作 default_template 为空（Custom 等理论空）时的最末兜底，避免空串。
- **同步弹窗 title 逻辑**（dispatch :219-227）：title 当前空时用 default_title。可保持，或一并改为品牌兜底（与 body 风格一致）——**实现自行判断一致性**，若改 title 兜底，优先复用同一品牌常量。最小要求：body 不再出现空串 / `{project}` 字面 / 无谓英文退化。
- 更新/新增单测：覆盖①有 project 空模板→`{project} 完成` 用真实 project；②**无 project 空模板→`aidog 完成`（核心新行为，断言不含 `{project}` 字面、不是英文 default_title）**；③有用户模板时不受影响。

### 2. 前端预览/占位（NotificationSettings.tsx）
- 目标：template 输入框为空时，让用户看到「留空会用什么」的实际默认模板内容，而非通用占位句。
- 做法（实现选其一，倾向 a）：
  - **(a) placeholder 展示实际默认模板**：placeholder 改为该类型默认模板文本（如 `{project} 完成`），让用户灰字看到留空效果。需前端镜像 4 类型默认模板。
  - (b) textarea 下方加一行 hint：「留空将使用：{project} 完成」。
- **跨层镜像约束（重要）**：前端默认模板常量必须**逐字镜像** `models.rs::default_template`（同 Protocol 双写规约）。新增前端 const map（如 `NOTIF_DEFAULT_TEMPLATES: Record<NotifType,string>`）= `{ task_complete:"{project} 完成", waiting_input:"{project} 等待用户输入", error:"{project} 出错", custom:"{project} 通知" }`，并在**两侧各加注释指向对方文件**（models.rs:1677 ↔ 该 const），防腐化。
- i18n：默认模板本身是 zh 硬编码非 i18n（与后端一致，不要本地化模板正文）。若加 hint 文案（如「留空将使用：」前缀）需 8 locale 全补 + `yarn check:i18n` 过。placeholder 方案若直接放模板文本则无新 i18n key。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（notification 套件全过，含新增空模板兜底测试）。
- `yarn build`（tsc+vite）+ `yarn check:i18n`（若加文案 key）全过。
- 后端：无 project 空模板 → body = `aidog 完成`（非空、非 `{project}` 字面、非英文 default_title）；有 project → `<project> 完成`；有用户模板 → 原样。
- 前端：template 框为空时可见实际默认模板内容（placeholder 或 hint）。
- 前后端默认模板文本一致（镜像 + 双向注释）。

## 失败处理
- 品牌兜底名取值不确定（"aidog" vs product_name）→ 默认用常量 `"aidog"`（render 纯函数无 app handle），在注释说明；若要 product_name 需改签名传 app，回报主会话定夺，先不扩面。
- 镜像 NotifType key 与前端 api.ts 类型不符 → 以 api.ts 现有 NotifType 字面量为准，标 `需要:` 回报。
