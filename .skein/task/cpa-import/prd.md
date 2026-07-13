# CPA(CLIProxyAPI)配置导入 + cpa-* 平台类型 — PRD (主入口)

## 目标
支持导入 CLIProxyAPI 的配置(config.yaml / config.json,单文件 / 压缩包 / 文件夹),解析出多 provider → 映射 aidog 平台 → **用户预览确认后才创建**(禁自动建一堆)。为 cpa 中 aidog 无对应的 provider(grok/vertex/aistudio/antigravity)新增独立 `cpa-*` 协议类型。预览阶段展示每个配置的剩余额度等信息。

**澄清**: "cpa" 非 `.cpa` 后缀的独立文件格式,是 CLIProxyAPI 项目缩写。实际配置走 config.yaml(YAML)或 config.json,见 `research/cpa-format.md`。用户原措辞"cpa 格式"= CLIProxyAPI 配置。

### 导入源
- 单文件:`.yaml`/`.yml`/`.json`
- 压缩包:`.zip`/`.tar.gz`/`.tgz`/`.tar`(解压后扫);**首版不支持 rar/7z**,UI 提示「请先解压再选文件夹」
- 文件夹:递归扫内含配置文件
- OAuth 凭据目录(auth-dir):可选第二目录,扫内 JSON 按 type 创建 OAuth 类 cpa-* 平台

### 配置识别 + 合并
- 全文件尝试解析;有 cpa provider 段结构的留,无关/解析失败跳过。
- 多文件解析出的平台**全合并,name+base_url 去重**,一次预览确认。

### 映射规则
cpa 6 provider 段 + OAuth channel → aidog 协议(见 design.md 映射表):
- `gemini-api-key` / `interactions-api-key` → `gemini`
- `codex-api-key` → `codex`
- `claude-api-key` → `anthropic`
- `openai-compatibility` → **按 name 关键词自动路由**(glm→glm, kimi→kimi, minimax→minimax, deepseek→deepseek, ...;无匹配→`openai` 兜底)
- `vertex-api-key` → `cpa-vertex`(新)
- OAuth channel: xai → `cpa-grok`(新), aistudio → `cpa-aistudio`(新), antigravity → `cpa-antigravity`(新), vertex → `cpa-vertex`(新), claude/codex/kimi → 现有

### 新协议(cpa-*)
- `cpa-grok` / `cpa-vertex` / `cpa-aistudio` / `cpa-antigravity`(4 个)
- **adapter 复用现有**(二轮调研 research/cpa-format-round2.md 确认 4 协议走原生 API,非 OpenAI 兼容):
  - cpa-grok → `openai_responses` adapter(`/responses` 同族)
  - cpa-aistudio / cpa-antigravity / cpa-vertex → `gemini` adapter(`generateContent` 同族)
  - s2 验证字段兼容;不符(antigravity `/v1internal:*` / vertex URL 含 project/location)则该协议「仅存配置,路由暂不支持」标注,不阻塞导入
- preset 默认 base_url/model 见 research/cpa-format-round2.md preset 表;**每协议(含 4 cpa-* + openai 兜底)preset MUST 配默认 model_list(对应上游 web 端模型列表)**;cpa-vertex base_url region-specific 用户预览补全,但 model_list 配 Vertex 公开模型(gemini-2.5-pro 等)不留空;不同认证方式(api-key / OAuth channel)的 provider 各为独立平台
- Rust `Protocol` 枚举加 4 变体 + serde + preset JSON + 前端 PROTOCOLS + 路由层
- OAuth token(access_token)当 api_key 填;refresh_token 丢弃(token 过期用户手补,aidog 无刷新机制)

### 预览余额展示
- 预览 UI 每平台行有「余额」列,惰性查询(用户点「全部查询余额」按钮或单行查,非自动)
- 复用 `gateway::quota::query_quota(base_url, api_key, platform_id=0)`(platform_id=0 → `persist_quota_to_db` None-guard 不落库,纯临时展示)
- 仅 9 provider 支持(DeepSeek/OpenRouter/GLM/Kimi/MiniMax/NewAPI/SiliconFlow/StepFun/Novita);cpa-* 4 协议 + 其他不支持的显「—」(CLIProxyAPI 无内置余额查询,确认)
- 并发查询 ≤5,失败/超时不阻塞预览

### 导入流程(仿 CcSwitchImport 模式)
平台添加页加「导入 CPA 配置」文字按钮 → 选源(文件/压缩/文件夹 + 可选 auth-dir) → 后端解析+合并去重+映射 → 返预览数据 → 前端预览 UI(平台多选 + 选模型 + 改名 + 冲突预览 + 惰性余额)→ 用户确认 → apply 批量创建(走现有 platform_create,非原子尽力,返 created/failed)。

## 边界
### 范围内
- 导入 config.yaml/json(单文件/压缩 zip·tgz·tar/文件夹)。
- 新增 4 cpa-* 协议(preset + Protocol 变体 + 前端,复用现有 adapter)。
- cpa → aidog 映射(6 段 + OAuth channel + openai-compat name 路由)。
- 预览 + 用户确认创建(仿 CcSwitchImport)+ 惰性余额展示。
- OAuth 凭据(auth-dir JSON,可选扫描)。

### 范围外(非目标)
- 不做 aidog → cpa 反向导出。
- 不做 OAuth token 自动刷新(token 过期手补)。
- 不做 cpa 的 management secret-key / 路由规则 / cooling 等非 provider 配置导入(仅 provider 段)。
- 不做运行时对接 CLIProxyAPI 服务(仅静态配置导入)。
- 不改造现有 aidog 协议(adapter/preset 仅加 cpa-* 新变体,不动现有)。
- 不做 rar/7z 解压(首版砍,UI 提示先解压)。
- 不新写 adapter(复用现有 openai_responses / gemini)。
- 不拆多 api-key 为多平台(取首个,后续迭代)。

### 已知约束
- preset JSON 手维护(`src-tauri/defaults/platform-presets.json`),新 cpa-* 条目手写,**禁机器生成覆盖**(CLAUDE.md 约束)。
- 压缩 zip/tgz/tar 依赖 `zip`+`tar`+`flate2` crate;YAML 解析依赖 `serde_yaml`(aidog 现无,需加)。
- cpa api_key 明文,aidog platform 也明文存 api_key — 语义一致,导入预览可掩码显示。
- 新 Rust command 需 yarn tauri dev 重启(memory `tauri-rust-command-needs-restart`)。
- adapter 兼容风险(antigravity 路径 / vertex URL 结构)→ s2 验证,不符则标注「仅存配置不路由」。

## 验收标准
- [ ] 平台添加页有「导入 CPA 配置」文字按钮;支持选单文件(yaml/json)/ 压缩包(zip/tgz/tar)/ 文件夹;rar/7z 给提示。
- [ ] 压缩包/文件夹场景:解压/递归扫所有文件,尝试解析,有 cpa provider 段的留;多文件平台 name+base_url 去重合并。
- [ ] 预览 UI 列出将创建的平台(协议/名称/base_url/模型/api_key 掩码),支持多选 + 选模型 + 改名 + 冲突预览(同名/同 base_url)+ 余额列(惰性查询,支持 provider 显余额,余显「—」)。
- [ ] 用户确认后才创建(禁导入即自动建);apply 走现有 platform_create,非原子尽力,返 created/failed 报告;同名/同 base_url 冲突默认跳过(进 failed[],禁覆盖已存)。
- [ ] 新增 4 Protocol:`cpa-grok`/`cpa-vertex`/`cpa-aistudio`/`cpa-antigravity`(Rust Protocol 枚举 + serde + preset JSON + 前端 PROTOCOLS + 路由层 adapter 接线)。
- [ ] openai-compatibility 段按 name 关键词路由(glm/kimi/minimax/deepseek 等映射;无匹配→openai)。
- [ ] OAuth 凭据(auth-dir JSON)可选扫描,按 type 创建 cpa-* OAuth 平台,token 当 api_key。
- [ ] i18n 8 语言 key 齐(check-i18n 通过)。
- [ ] 门禁:cargo clippy+test / yarn build+test+check:i18n 零回归。

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [research/cpa-format.md](research/cpa-format.md)(一轮格式)+ [research/cpa-format-round2.md](research/cpa-format-round2.md)(二轮 4 cpa-* 上游 + adapter)
- 任务/子任务/调度: task.json (`skein.py subtask list cpa-import`)
- 契约: `skein.py contract cpa-import`
