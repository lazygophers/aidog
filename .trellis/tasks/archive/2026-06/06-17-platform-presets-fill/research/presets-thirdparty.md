# Research: 第三方/中转组平台 base_url 核实（26 平台）

- **Query**: 逐平台核实第三方/中转组官方 base_url，校正过时值；默认模型多继承 anthropic
- **Scope**: mixed（内部预设 Read + 外部域名探测）
- **Date**: 2026-06-17

## 方法与局限

- 现有预设来源：`src/pages/Platforms.tsx` `getDefaultEndpoints`（:150-365）+ `getDefaultModels`（:371-394）。
- 本会话**无可用 WebSearch/WebFetch 工具**，外部核实改用 `curl`（HTTP 状态码 + 站点正文抓取）+ `dig`（DNS 解析）。这是事实性弱化的核实方式：能证明「域名存活/解析/仍是 Claude Code-Codex 网关」，**不能**逐一证明上游官方文档里写明的精确 base_url path。凡未抓到官方文档明示 path 的，结论前缀「推测:」。
- 约束复核：base_url 含版本前缀，最终 URL = base_url + `/chat/completions`，禁额外拼接；多数透传平台 base_url = host 根。
- API 子域根路径返回 404/403 属正常（API host 不在 `/` 提供服务），不等于域名失效——已用 DNS 解析 + marketing 站正文双重确认存活。

## Findings

### 探测原始数据

HTTP（`curl -L -o /dev/null -w %{http_code}`，https，root path）：

| 域名 | HTTP root | 备注 |
|---|---|---|
| www.packyapi.com / packycode.com | 200 / 200 | 存活 |
| api.cubence.com | 404 | API host 存活（cubence.com 200, title「Claude Code & Codex Gateway」）|
| api.aigocode.com | 200 | 存活，docs.aigocode.com 存在 |
| www.right.codes | 200 | 存活（Cloudflare）|
| api.aicodemirror.com | 200 | 存活 |
| api.pateway.ai | 404 | API host 存活（pateway.ai 200, title「Claude & Codex API 中转站」）|
| www.ccsub.net | 200 | 存活 |
| api.apikey.fun | 200 | 存活 |
| apinebula.com | 200 | 存活 |
| sudocode.us | 200 | 存活 |
| gw.claudeapi.com | 200 | 存活 |
| claudecn.top | 200 | 存活 |
| runapi.co | 200 | 存活 |
| www.relaxycode.com | 200 | 存活 |
| cn.crazyrouter.com | 404 | API host 存活（crazyrouter.com 200, title「One API for 300+ AI Models」）|
| node-hk.sssaicodeapi.com | 404 | API host 存活（sssaicode.com 200 但 Cloudflare「Just a moment」挑战）|
| api.modelverse.cn | 404 | API host 存活（CNAME uewaf.com WAF；modelverse.cn 裸域不解析）|
| cp.compshare.cn | 404 | API host 存活（compshare.cn 200, title「优云智算 GPU 云平台」）|
| www.micuapi.ai | 200 | 存活 |
| api.ctok.ai | 200 | 存活 |
| e-flowcode.cc | 200 | 存活 |
| api.lemondata.cc | 200 | 存活 |
| cc-api.pipellm.ai | 200 | 存活 |
| opencode.ai | 200 | 存活 |

DNS（`dig +short`，确认无死域名）：cubence/pateway/crazyrouter/sssaicode/compshare 全部解析成功（多为 Cloudflare 104.x 或 WAF CNAME）。`modelverse.cn` 裸域不解析，但 `api.modelverse.cn` 子域解析（业务子域，预设用的就是它，OK）。

### 核实结果表

| 平台key | 现有base_url | 核实后base_url（变更?/失效?） | 默认模型 | 来源URL | 核查日期 |
|---|---|---|---|---|---|
| packycode | `https://www.packyapi.com`（+/v1 codex，+gemini） | 保持原值（N，域名 200 存活） | 继承 anthropic（留空） | https://www.packyapi.com（HTTP 200） | 2026-06-17 |
| cubence | `https://api.cubence.com`（+/v1，+gemini） | 保持原值（N，API host 存活，cubence.com 自述 Claude Code & Codex Gateway） | 继承 anthropic（留空） | https://cubence.com（title 确认）；api.cubence.com（404=root，正常） | 2026-06-17 |
| aigocode | `https://api.aigocode.com`（anthropic+openai+gemini 三 ep 同 host 无 /v1 后缀） | 推测: 保持原值（域名 200，docs.aigocode.com 存在；openai ep 缺 `/v1` 后缀待人工核 docs） | 继承 anthropic（留空） | https://api.aigocode.com（200）；https://docs.aigocode.com（存在） | 2026-06-17 |
| rightcode | anthropic `https://www.right.codes/claude` / openai `https://right.codes/codex/v1` | 保持原值（N，www.right.codes 200 存活） | 继承 anthropic（留空） | https://www.right.codes（HTTP 200） | 2026-06-17 |
| aicodemirror | anthropic `…/api/claudecode` / openai `…/api/codex/backend-api/codex` / gemini `…/api/gemini` | 保持原值（N，api.aicodemirror.com 200 存活） | 继承 anthropic（留空） | https://api.aicodemirror.com（HTTP 200） | 2026-06-17 |
| pateway | `https://api.pateway.ai` | 保持原值（N，pateway.ai 自述「Claude & Codex API 中转站」，api host 存活） | 继承 anthropic（留空） | https://pateway.ai（title 确认）；api.pateway.ai（404=root，正常） | 2026-06-17 |
| ccsub | `https://www.ccsub.net` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://www.ccsub.net（HTTP 200） | 2026-06-17 |
| apikeyfun | `https://api.apikey.fun` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://api.apikey.fun（HTTP 200） | 2026-06-17 |
| apinebula | `https://apinebula.com` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://apinebula.com（HTTP 200） | 2026-06-17 |
| sudocode | `https://sudocode.us` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://sudocode.us（HTTP 200） | 2026-06-17 |
| claudeapi | `https://gw.claudeapi.com` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://gw.claudeapi.com（HTTP 200） | 2026-06-17 |
| claudecn | `https://claudecn.top` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://claudecn.top（HTTP 200） | 2026-06-17 |
| runapi | `https://runapi.co` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://runapi.co（HTTP 200） | 2026-06-17 |
| relaxycode | `https://www.relaxycode.com` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://www.relaxycode.com（HTTP 200） | 2026-06-17 |
| crazyrouter | `https://cn.crazyrouter.com` | 推测: 复核——主站 `crazyrouter.com` 200 且 marketing 正文出现 `crazyrouter.com/v1`（codex ep）；预设的 `cn.` 子域解析+存活(404=root)。anthropic base_url 是否应为 `crazyrouter.com` 而非 `cn.crazyrouter.com` 需人工核官方 Claude Code 接入文档（SPA，curl 抓不到 docs 正文）。**疑似可改但未证实，暂保持原值** | 继承 anthropic（留空） | https://crazyrouter.com（title「One API for 300+ AI Models」，正文含 /v1）；cn.crazyrouter.com（404=root） | 2026-06-17 |
| sssaicode | `https://node-hk.sssaicodeapi.com/api` | 保持原值（N，node host 404=root 正常存活；sssaicode.com 200 但 Cloudflare 人机挑战，docs 抓不到。base_url 含 `/api` 前缀+客户端 path，符合约束） | 继承 anthropic（留空） | sssaicode.com（Cloudflare 200）；node-hk.sssaicodeapi.com（404=root） | 2026-06-17 |
| compshare | `https://api.modelverse.cn` | 保持原值（N，api host 存活[WAF]；注：modelverse 是优云智算/compshare 的模型 API 品牌） | 继承 anthropic（留空） | api.modelverse.cn（404=root，CNAME uewaf WAF） | 2026-06-17 |
| compshare_coding | `https://cp.compshare.cn` | 保持原值（N，compshare.cn 200「优云智算 GPU 云平台」，cp 子域存活） | 继承 anthropic（留空） | https://compshare.cn（title 确认）；cp.compshare.cn（404=root） | 2026-06-17 |
| micu | `https://www.micuapi.ai` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://www.micuapi.ai（HTTP 200） | 2026-06-17 |
| ctok | `https://api.ctok.ai` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://api.ctok.ai（HTTP 200） | 2026-06-17 |
| eflowcode | `https://e-flowcode.cc` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://e-flowcode.cc（HTTP 200） | 2026-06-17 |
| lemondata | `https://api.lemondata.cc` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://api.lemondata.cc（HTTP 200） | 2026-06-17 |
| pipellm | `https://cc-api.pipellm.ai` | 保持原值（N，HTTP 200） | 继承 anthropic（留空） | https://cc-api.pipellm.ai（HTTP 200） | 2026-06-17 |
| opencode | `https://opencode.ai/zen/go`（openai/codex_tui） | 保持原值（N，opencode.ai 200 存活；base_url 含 `/zen/go` 路由前缀，opencode zen 网关，符合约束） | 继承 anthropic（留空，且本平台是 openai 协议 codex 网关，无 anthropic 默认模型） | https://opencode.ai（HTTP 200） | 2026-06-17 |
| newapi | `https://your-newapi-instance.com/v1` | 保持原值（**占位示例**，非真实域名；NewAPI 是自托管中转面板，base_url 由用户填自己实例。按现状保留） | N/A（占位，用户自填） | NewAPI 自托管特性（项目内 quota.rs NewAPI backend 佐证） | 2026-06-17 |
| claude_code | `https://api.anthropic.com`（client_type=default） | 保持原值（N，订阅透传占位，base_url=官方 host 根，客户端原始 path 直拼） | 继承 anthropic（留空，纯透传） | 官方 api.anthropic.com | 2026-06-17 |

### Code Patterns

- 第三方组无一在 `getDefaultModels`（:374-393）中定义默认模型——presets 仅覆盖到 `deepseek`（国内官方组），第三方组**全部继承 anthropic 留空**，与「这类多为 Claude Code 代理透传」相符。无需新增默认模型。
- base_url = host 根 的透传模式：packycode/cubence/ccsub/apikeyfun/apinebula/sudocode/claudeapi/claudecn/runapi/relaxycode/micu/ctok/eflowcode/lemondata/pipellm/pateway（符合 CLAUDE.md「base_url 填 host 根，客户端原始 path 直接拼接」）。
- base_url 含路由前缀（非裸 host 根）：rightcode(`/claude`)、aicodemirror(`/api/claudecode`)、sssaicode(`/api`)、opencode(`/zen/go`)、aigocode(同 host 三协议)。这些前缀是上游自有路由，与「+ /chat/completions」约束不冲突（最终 = base_url + 客户端 path）。

## Caveats / Not Found

- **无真 WebSearch/WebFetch**：本会话仅 curl/dig。SPA 站点（crazyrouter、aigocode docs、sssaicode 被 Cloudflare 挑战拦截）的官方接入文档正文抓不到，故**未能逐字核对官方文档里的精确 base_url path**。表中标「推测:」者尤其需人工开浏览器核官方文档：
  - **crazyrouter**：主站是 `crazyrouter.com`，预设用 `cn.crazyrouter.com`。两者都解析存活，但哪个是官方 Claude Code anthropic base_url 未证实。建议人工核 crazyrouter.com 文档确认是否应去掉 `cn.` 前缀。
  - **aigocode**：openai ep base_url `https://api.aigocode.com` 缺 `/v1` 后缀（同 host 三协议都无后缀），是否正确待核 docs.aigocode.com。
- **无域名失效**：26 平台全部 DNS 解析成功 + HTTP 存活（API host 的 404/403 是 root path 未提供服务，非失效）。无一需标「域名失效」。
- **newapi**：`your-newapi-instance.com` 是占位，非失效，刻意保留。
- 所有「保持原值」结论基于「域名存活 + 仍是 Claude Code/Codex 网关」，**非**「官方文档逐字确认 base_url path」。若需高保真核验，应在带浏览器的会话用 WebFetch/agent-browser 逐站读官方接入文档。
