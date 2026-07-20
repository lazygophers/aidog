# 自定义 quota 脚本支持 — PRD (主入口)

## 目标

让用户为任一平台自定义余额 / coding plan / 任意 metric 查询：HTTP 模板（url/headers/body `{{var}}` 占位 + JSONPath 提取）或外部脚本（spawn + stdout JSON）。覆盖内置 host 匹配未支持的小众平台 / 自建中转 / 需本地脚本计算的余额。

**用户价值**: 内置 provider 仅覆盖 ~10 平台（host 硬编码），长尾平台余额查询无门 → custom_quota 让用户自助接入任意平台 quota。

**架构**: per-platform `extra.custom_quota` override，调度优先走 custom（跳 host 匹配）。核心逻辑下沉 aidog_core，command 壳 commands_platform。

## 边界

### 范围内
- `extra.custom_quota` 配置 schema: type=http_template|script + response_mapping（balance/coding_plan/metrics 三段可选 JSONPath）
- HTTP 模板执行器（{{var}} 正则严格替换 + http_client 复用 + JSONPath 提取）
- 脚本执行器（禁 shell + timeout 30s + stdout cap 64KiB + env_clear + stdin config + cwd temp）
- `query_custom_quota` entry（独立，query_quota 签名不动）
- command: query_custom_quota + test_custom_quota（即时测试不落库）
- cold_start custom 分支 + est_balance_remaining 落库
- 前端 CustomQuotaSection 编辑表单 + 测试按钮 + 8 locale i18n
- PlatformQuota 加 custom_metrics 扩展字段

### 范围外（非目标）
- FS sandbox（macOS Seatbelt / Linux bubblewrap）— YAGNI，残余风险声明
- uid/gid 降权 — YAGNI
- 模板条件/循环（{{#if}}/{{#each}}）— YAGNI，正则够
- custom 影响 router 候选（低余额排除路由）— 后续 task，本 task 仅查询展示
- 新 quota 配置表 — 用 extra 子字段，无 migration

### 已知约束
- JSONPath 新依赖 serde_json_path（aidog 现无 jsonpath dep）
- 脚本同 uid 跑，可读 ~/.aidog/*（设计目标仅防误配+防注入+防 OOM，非防用户自伤）
- 新 Rust command 需 `yarn tauri dev` 重启
- 配置字段跨 Rust serde + TS type + JSON 三层必须一致

## 验收标准
- [ ] extra.custom_quota schema Rust serde round-trip + TS type 同步（type/response_mapping/command/stdin 字段字面量三层一致）
- [ ] HTTP 模板: {{var}} 严格替换（未知变量报错）+ 复用 http_client + JSONPath 提取 balance/coding_plan/metrics → PlatformQuota
- [ ] 脚本: 禁 shell（argv 单元素）+ timeout 30s kill + stdout cap 64KiB + env_clear（PATH/HOME/LANG）+ stdin config + cwd temp + 退出码契约
- [ ] query_custom_quota 独立 entry，query_quota 签名不变，command 层调度 custom override 优先
- [ ] cold_start_init_tray_estimates 加 custom 分支 + est_balance_remaining 落库
- [ ] command query_custom_quota + test_custom_quota（test 不落库即时返）注册 + 重启生效
- [ ] 前端 CustomQuotaSection: type 选 + 模板/response_mapping 编辑 + 测试按钮 + 8 locale
- [ ] 失败处理复用 trellis-08（禁新错误分类）: timeout/非 0 退出/JSON parse 失败/JSONPath 无命中/模板未知变量 各路径返 PlatformQuota{success:false,error}
- [ ] cargo clippy 零新增 + cargo test 全过 + yarn build + check-i18n 零缺失

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (`skein subtask list custom-quota-script`)
