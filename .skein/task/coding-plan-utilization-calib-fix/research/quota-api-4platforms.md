# Coding Plan 用量查询 API 调研 — bailian / qianfan / xiaomi / compshare

调研日期: 2026-07-18 (s3 research-gate)。目标: 判断 4 平台上游是否存在**可程序化查询 coding plan 套餐用量/配额/reset**的公开 REST API,产出可供 s4-s7 建 handler 或降级的结论。

参照契约形态 = `gateway/quota/coding_plan.rs` 现有 kimi/zhipu/minimax handler:
每平台需 → endpoint(method+path) / auth header / response JSON schema / 到 `CodingPlanInfo{tiers:[{name,utilization,limit,resets_at}], level}` 映射。

## 总结论

**4 平台均未找到公开的程序化用量查询 API**。全部只提供控制台网页查看剩余额度。且 4 平台的 coding plan ToS **均明文禁止**用套餐 key 做非编程工具的 API 自动化调用(禁自动化脚本/自定义后端/批量),这从政策侧进一步佐证不存在给第三方轮询的 usage 端点。→ 建议 s4-s7 **全部降级**走 custom-quota-script 兜底(用户自备脚本)或直接标「无上游 API,不建 handler」。

---

## bailian (阿里百炼)

- 结论: **无公开 API / 仅控制台网页**
- preset base_url: `https://dashscope.aliyuncs.com/apps/anthropic`(anthropic);task 指定 coding 端点 `https://coding.dashscope.aliyuncs.com/apps/anthropic`
- endpoint: N/A
- auth: N/A
- response schema: N/A
- → CodingPlanInfo 映射: **降级**。官方文档明确「Coding Plan 订阅套餐目前无法查看模型具体 token 消耗量」,用量仅在「百炼控制台 → 订阅套餐 → 套餐用量」页面手动查看剩余请求次数(按模型调用次数计,非 token)。Coding Plan 专属 key(`sk-sp-xxxxx`)与按量 key 不互通,`coding.dashscope.aliyuncs.com` 仅用于模型调用接入,不提供 usage 接口。
- 引用: https://help.aliyun.com/zh/model-studio/coding-plan ; https://developer.aliyun.com/article/1716613

## qianfan (百度千帆)

- 结论: **无公开 API / 仅控制台网页**
- preset base_url: `https://qianfan.baidubce.com/anthropic/coding`(anthropic)、`https://qianfan.baidubce.com/v2/coding`(openai)
- endpoint: N/A
- auth: N/A
- response schema: N/A
- → CodingPlanInfo 映射: **降级**。用量查询走控制台订阅管理页 `https://console.bce.baidu.com/qianfan/resource/subscribe`,可实时看各模型调用次数/token 消耗/剩余额度。额度机制与其他平台同构(每 5h 滑动窗口 / 每周一 00:00 UTC+8 重置 / 每月订阅日重置)——**若后续找到端点,tiers 可对齐 five_hour/weekly_limit**。企业版(Token Plan 企业版)有「我的订阅/用量分析」后台但仍是网页,未见公开 REST usage API。ToS 禁止套餐 key 用于自动化脚本/自定义后端。
- 引用: https://cloud.baidu.com/doc/qianfan/s/imlg0beiu ; https://cloud.baidu.com/doc/qianfan/s/ymq8wwch2 (Token Plan 企业版)

## xiaomi (小米 MiMo Token Plan)

- 结论: **无公开 API / 仅控制台网页**
- preset base_url: `https://token-plan-cn.xiaomimimo.com/anthropic`(anthropic)、`.../v1`(openai);区域锁 key(`tp-xxxxx`),另有 ams/sgp 集群
- endpoint: N/A
- auth: N/A
- response schema: N/A
- → CodingPlanInfo 映射: **降级**。官方 API 集成文档(mimo.mi.com/docs api-integration)WebFetch 确认**只讲 key 获取 + 协议接入,无任何 usage/quota/balance 查询端点**;用量在 `platform.xiaomimimo.com` 控制台「订阅详情/Plan Manage」网页查看。计费为统一 Credits 制(不同模型 1x/2x 倍率),月度重置,无 5h 限额。多个第三方接入 issue(cc-switch #2810 / 9router #1251 / pi #4082)均只涉及 endpoint/auth 配置,**无人逆向出 usage 轮询端点**。ToS 限定套餐仅编程/agent 工具用,禁非编程自动化。
- 引用: https://mimo.mi.com/docs/en-US/quick-start/faq/api-integration (WebFetch 确认无 usage 端点) ; https://codingplan.link/en/plans/xiaomimimo ; https://github.com/farion1231/cc-switch/issues/2810

## compshare (优云智算 / UCloud 优刻得)

- 结论: **无公开 API / 仅控制台网页**
- preset base_url: `https://cp.compshare.cn`(anthropic 代理主机);营销文档另给上游 `https://api.modelverse.cn`(模型 id `modelverse-<name>`)
- endpoint: N/A
- auth: N/A
- response schema: N/A
- → CodingPlanInfo 映射: **降级**。积分制计费(多维度积分消耗,聚合 16 家模型)。用量/订阅在控制台 `https://console.compshare.cn/light-gpu/model-subscription` 网页查看,额度周期耗尽等下一周期恢复。未见任何公开程序化 usage/积分余额查询端点。ToS:不适合改任意 API 中转/批量脚本/多人共享。
- 引用: https://www.compshare.cn/coding-plan ; https://www.compshare.cn/docs ; https://github.com/AIddlx/coding-plans (第三方记录,无 usage API)

---

## 对 s4-s7 的建议(供 main 裁定)

- 4 个 handler 建议**全部不建**,统一走现有 custom-quota-script 兜底路径(用户自备脚本从控制台/浏览器态取数)。
- 若未来某平台开放 usage API:qianfan/xiaomi 的 tier 结构(5h/weekly/monthly 或 Credits 总额)与现有 `QuotaTier` 契约兼容,届时可补 handler。
- 无法确认「绝对无」的部分(可能存在需登录态 cookie 的内部网页 XHR 端点,非公开文档):如需精确到内部 XHR,需登录抓包,超出只读 web 调研范围 → 标 `需要:` 由 main 决策是否投入。
