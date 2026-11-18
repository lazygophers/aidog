// 假数据：名字 / 数字一眼是演示。所有日期相对 now，让图表有曲线。
const now = Date.now();
const H = 3600 * 1000;
const D = 24 * H;

export const theme = { current: "light" };
export const nowMs = now;

export const platforms = [
  {
    id: 1, name: "演示·云端一号", platform_type: "anthropic",
    base_url: "https://demo-cloud1.example.com",
    api_key: "sk-demo-aaaa1111",
    extra: "{}",
    models: { default: ["demo-sonnet-main", "demo-sonnet-alt"], sonnet: ["demo-sonnet-main"], opus: ["demo-opus-main"], haiku: ["demo-haiku-mini"] },
    available_models: ["demo-sonnet-main", "demo-sonnet-alt", "demo-opus-main", "demo-haiku-mini"],
    endpoints: [{ protocol: "anthropic", base_url: "https://demo-cloud1.example.com", coding_plan: false }],
    enabled: true, status: "enabled", auto_disabled_until: 0, auto_disable_strikes: 0,
    expires_at: 0, created_at: now - 90 * D, updated_at: now - 2 * H, deleted_at: 0,
    est_balance_remaining: 216.4, est_coding_plan: "", rate_limit: "",
    last_real_query_at: now - 30 * 60000, estimate_count: 3, show_in_tray: true,
    tray_display: "balance", sort_order: 0, manual_budgets: [], coding_window_cost: 0,
    balance_level: "green",
  },
  {
    id: 2, name: "演示·星桥中转", platform_type: "openai",
    base_url: "https://demo-bridge.example.com/v1",
    api_key: "sk-demo-bbbb2222",
    extra: "{}",
    models: { default: ["demo-gpt-main", "demo-gpt-mini"], gpt: ["demo-gpt-main"] },
    available_models: ["demo-gpt-main", "demo-gpt-mini"],
    endpoints: [{ protocol: "openai", base_url: "https://demo-bridge.example.com/v1", coding_plan: false }],
    enabled: true, status: "enabled", auto_disabled_until: 0, auto_disable_strikes: 0,
    expires_at: 0, created_at: now - 60 * D, updated_at: now - 5 * H, deleted_at: 0,
    est_balance_remaining: 48.02, est_coding_plan: "", rate_limit: "",
    last_real_query_at: now - 2 * H, estimate_count: 8, show_in_tray: false,
    tray_display: "balance", sort_order: 1, manual_budgets: [], coding_window_cost: 0,
    balance_level: "yellow",
  },
  {
    id: 3, name: "演示·码仓套餐", platform_type: "glm_coding",
    base_url: "https://demo-codepack.example.com/api/coding/paas/v4",
    api_key: "sk-demo-cccc3333",
    extra: "{}",
    models: { default: ["demo-code-main"] },
    available_models: ["demo-code-main"],
    endpoints: [{ protocol: "glm_coding", base_url: "https://demo-codepack.example.com/api/coding/paas/v4", coding_plan: true }],
    enabled: true, status: "enabled", auto_disabled_until: 0, auto_disable_strikes: 0,
    expires_at: 0, created_at: now - 30 * D, updated_at: now - 1 * H, deleted_at: 0,
    est_balance_remaining: 0, est_coding_plan: JSON.stringify({ tiers: [{ window: "monthly", est_utilization: 0.42 }] }), rate_limit: "",
    last_real_query_at: now - 10 * 60000, estimate_count: 12, show_in_tray: true,
    tray_display: "coding", sort_order: 2, manual_budgets: [], coding_window_cost: 12.35,
    balance_level: "neutral",
  },
  {
    id: 4, name: "演示·北岸推理", platform_type: "deepseek",
    base_url: "https://demo-northshore.example.com/v1",
    api_key: "sk-demo-dddd4444",
    extra: "{}",
    models: { default: ["demo-deep-main", "demo-deep-lite"], gpt: ["demo-deep-main"] },
    available_models: ["demo-deep-main", "demo-deep-lite"],
    endpoints: [{ protocol: "openai", base_url: "https://demo-northshore.example.com/v1", coding_plan: false }],
    enabled: false, status: "disabled", auto_disabled_until: 0, auto_disable_strikes: 0,
    expires_at: 0, created_at: now - 45 * D, updated_at: now - 26 * H, deleted_at: 0,
    est_balance_remaining: 3.18, est_coding_plan: "", rate_limit: "",
    last_real_query_at: now - 26 * H, estimate_count: 1, show_in_tray: false,
    tray_display: "balance", sort_order: 3, manual_budgets: [], coding_window_cost: 0,
    balance_level: "red",
  },
  {
    id: 5, name: "演示·雾港聚合", platform_type: "openrouter",
    base_url: "https://demo-fogport.example.com/api/v1",
    api_key: "sk-demo-eeee5555",
    extra: "{}",
    models: { default: ["demo-fog-auto"], gpt: ["demo-fog-auto"], embedding: ["demo-fog-embed"] },
    available_models: ["demo-fog-auto", "demo-fog-embed"],
    endpoints: [{ protocol: "openai", base_url: "https://demo-fogport.example.com/api/v1", coding_plan: false }],
    enabled: true, status: "enabled", auto_disabled_until: 0, auto_disable_strikes: 0,
    expires_at: 0, created_at: now - 15 * D, updated_at: now - 8 * H, deleted_at: 0,
    est_balance_remaining: 87.9, est_coding_plan: "", rate_limit: "",
    last_real_query_at: now - 4 * H, estimate_count: 2, show_in_tray: false,
    tray_display: "balance", sort_order: 4, manual_budgets: [], coding_window_cost: 0,
    balance_level: "green",
  },
];

export const groups = [
  { id: 1, name: "演示·日常编码", group_key: "gk-demo0001", routing_mode: "cost_first", auto_from_platform: "", created_at: now - 80 * D, updated_at: now - 3 * H, deleted_at: 0, request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: "anthropic", sort_order: 0, max_retries: 3, model_mappings: [], is_default: true, extra: "" },
  { id: 2, name: "演示·长文写作", group_key: "gk-demo0002", routing_mode: "health_first", auto_from_platform: "", created_at: now - 40 * D, updated_at: now - 1 * D, deleted_at: 0, request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: "anthropic", sort_order: 1, max_retries: 2, model_mappings: [], is_default: false, extra: "" },
  { id: 3, name: "演示·批量任务", group_key: "gk-demo0003", routing_mode: "cost_first", auto_from_platform: "", created_at: now - 20 * D, updated_at: now - 6 * H, deleted_at: 0, request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: "anthropic", sort_order: 2, max_retries: 1, model_mappings: [], is_default: false, extra: "" },
];

export function statsResult(hours = 24) {
  const buckets = [];
  const series = [];
  const mkBucket = (label, i) => {
    const req = 14 + Math.round(9 * Math.sin(i * 0.9)) + (i % 4) * 2;
    return {
      time_bucket: label,
      total_requests: req,
      success_count: req - (i % 5 === 2 ? 1 : 0),
      error_count: i % 5 === 2 ? 1 : 0,
      input_tokens: req * 4200,
      output_tokens: req * 830,
      cache_tokens: req * 2600,
      avg_duration_ms: 3600 + (i % 7) * 300,
      total_cost: Number((req * (0.02 + 0.004 * ((i % 5) / 5))).toFixed(4)),
    };
  };
  if (hours <= 24) {
    // hourly：24 桶，time_bucket = "YYYY-MM-DD HH:00:00"
    for (let i = 23; i >= 0; i--) {
      const t = new Date(now - i * H);
      const label = `${t.toISOString().slice(0, 10)} ${String(t.getUTCHours()).padStart(2, "0")}:00:00`;
      buckets.push(mkBucket(label, 23 - i));
    }
  } else {
    // daily：N 桶
    for (let i = hours - 1; i >= 0; i--) {
      const t = new Date(now - i * D);
      buckets.push(mkBucket(t.toISOString().slice(0, 10), hours - 1 - i));
    }
  }
  const pNames = ["演示·云端一号", "演示·星桥中转", "演示·雾港聚合"];
  const mNames = ["demo-sonnet-main", "demo-gpt-main", "demo-deep-main"];
  const gNames = ["演示·日常编码", "演示·长文写作", "演示·批量任务"];
  for (const name of pNames) series.push({ name, buckets: buckets.map((b, i) => ({ ...b, total_requests: Math.max(1, Math.round(b.total_requests / 3)) })) });
  for (const name of mNames) series.push({ name, buckets: buckets.map((b, i) => ({ ...b, total_requests: Math.max(1, Math.round(b.total_requests / 3)) })) });
  for (const name of gNames) series.push({ name, buckets: buckets.map((b, i) => ({ ...b, total_requests: Math.max(1, Math.round(b.total_requests / 3)) })) });
  const dim = [
    { name: "演示·云端一号", total_requests: 1024, success_count: 1018, input_tokens: 4300800, output_tokens: 849920, cache_tokens: 2662400, cache_rate: 31.0, avg_duration_ms: 4120, total_cost: 41.82 },
    { name: "演示·星桥中转", total_requests: 486, success_count: 480, input_tokens: 2041200, output_tokens: 403380, cache_tokens: 1263600, cache_rate: 28.0, avg_duration_ms: 5210, total_cost: 19.4 },
    { name: "演示·雾港聚合", total_requests: 322, success_count: 315, input_tokens: 1352400, output_tokens: 267260, cache_tokens: 837200, cache_rate: 30.0, avg_duration_ms: 6120, total_cost: 8.71 },
  ];
  return {
    overview: {
      total_requests: 1832, success_rate: 97.8,
      total_input_tokens: 1832 * 4200, total_output_tokens: 1832 * 830, total_cache_tokens: 1832 * 2600,
      cache_rate: 31.0, avg_duration_ms: 4120, total_cost: 69.93,
    },
    buckets,
    dimension_data: dim,
    available_models: ["demo-sonnet-main", "demo-gpt-main", "demo-deep-main", "demo-fog-auto"],
    series,
  };
}

export const sessionStats = Array.from({ length: 8 }, (_, i) => ({
  session_key: `demo-session-${i + 1}`,
  title: `演示会话 ${i + 1}：接口改造与联调`,
  requests: 12 + i * 3, input_tokens: 40000 + i * 9000, output_tokens: 8000 + i * 700, cache_tokens: 20000,
  est_cost: Number((2 + i * 0.8).toFixed(2)),
  first_created_at: now - (i + 1) * 2 * D, last_created_at: now - i * 2 * D + H,
}));

function logRow(i) {
  const models = ["demo-sonnet-main", "demo-gpt-main", "demo-deep-main", "demo-fog-auto", "demo-opus-main"];
  const protos = ["anthropic", "openai"];
  const ok = i % 7 !== 3;
  return {
    id: `demo-log-${1000 - i}`,
    group_key: i % 2 ? "gk-demo0001" : "gk-demo0002",
    model: "claude-sonnet-demo",
    actual_model: models[i % models.length],
    source_protocol: protos[i % 2],
    target_protocol: protos[(i + 1) % 2],
    platform_id: (i % 5) + 1,
    status_code: ok ? 200 : 429,
    duration_ms: 800 + ((i * 373) % 9000),
    input_tokens: 3500 + ((i * 911) % 20000),
    output_tokens: 400 + ((i * 233) % 6000),
    cache_tokens: 1200 + ((i * 71) % 8000),
    is_stream: i % 3 !== 0,
    retry_count: i % 5 === 0 ? 1 : 0,
    blocked_by: "", blocked_reason: "",
    client: i % 2 ? "claude_code" : "codex_tui",
    user_agent: "claude-cli/2.1.319 (demo)",
    request_url: "https://demo-cloud1.example.com/v1/messages",
    up_bytes: 12000 + i * 300, down_bytes: 40000 + i * 900,
    created_at: now - i * 25 * 60000,
    request_class: "chat",
    agent_type: i % 2 ? "main" : "subagent",
  };
}

export const logRows = Array.from({ length: 25 }, (_, i) => logRow(i));

export const toolLogRows = Array.from({ length: 12 }, (_, i) => ({
  id: i + 1,
  created_at: now - i * 40 * 60000,
  tool: i % 3 === 0 ? "web_search_20260301" : "web_fetch_20260301",
  trigger: i % 3 === 0 ? "gateway" : "mcp",
  group_name: i % 2 ? "演示·日常编码" : "演示·长文写作",
  platform: "演示·云端一号",
  query: i % 3 === 0 ? "demo 查询词：路由策略对比" : "https://demo.example.com/docs/page",
  result_count: i % 3 === 0 ? 6 + i : 0,
  engines: i % 3 === 0 ? "engine-a,engine-b" : "",
  duration_ms: 700 + i * 130,
  success: i % 6 !== 2,
  error: i % 6 === 2 ? "demo：引擎全部失败" : "",
  result: "{}",
}));

export const requestLogs = Array.from({ length: 15 }, (_, i) => ({
  id: `demo-req-${2000 - i}`,
  kind: i % 4 === 0 ? "test" : i % 4 === 1 ? "quota" : "fetch-models",
  group_key: "gk-demo0001",
  platform_id: (i % 5) + 1,
  status_code: i % 5 === 4 ? 401 : 200,
  duration_ms: 200 + i * 90,
  user_agent: "aidog/1.0 (demo)",
  created_at: now - i * 33 * 60000,
}));

export const notifSettings = {
  enabled: true,
  tts_enabled: true,
  tts_backend: "mac_say",
  per_type: {},
  per_event: {},
  inbox_retention_days: 14,
  channels: {},
  rules: [],
  dnd_enabled: false,
};

export const notifications = Array.from({ length: 8 }, (_, i) => ({
  id: i + 1,
  notif_type: i % 3 === 0 ? "quota_low" : i % 3 === 1 ? "proxy_started" : "platform_error",
  source: "demo",
  severity: i % 4 === 0 ? "warning" : "info",
  vars: {},
  action_key: "",
  title: `演示通知 ${i + 1}：平台「演示·云端一号」余量提醒`,
  body: "演示正文：这是一条模拟通知，用于界面示意，不代表真实事件。",
  created_at: now - i * 90 * 60000,
}));

export const mcpServers = [
  { id: 1, name: "demo-search", transport: "http", command: "", args: [], env: {}, url: "https://demo-mcp.example.com/mcp", headers: {}, enabledAgents: ["claude-code"], createdAt: now - 30 * D, updatedAt: now - 2 * D },
  { id: 2, name: "demo-files", transport: "stdio", command: "demo-files-server", args: ["--demo"], env: { DEMO_TOKEN: "***" }, url: "", headers: {}, enabledAgents: ["claude-code", "codex"], createdAt: now - 20 * D, updatedAt: now - 4 * D },
];

export const subagents = Array.from({ length: 5 }, (_, i) => {
  const domain = ["Rust 网关", "前端界面", "文档站", "运维脚本", "安全审计"][i];
  return {
    name: `demo-reviewer-${["rust", "ts", "docs", "ops", "sec"][i]}`,
    description: `演示子代理：负责${domain}的代码评审与要点摘录（示意数据）。`,
    prompt: "你是演示子代理。请按仓库规范评审改动，输出要点清单（示意数据）。",
    agents: i % 3 === 2 ? ["claude-code"] : ["claude-code", "codex"],
    skills: [],
    mcp_servers: [],
    codex_model: i % 2 ? "demo-model-fast" : "demo-model-deep",
    from: null,
    from_hash: null,
    written: {},
    bundle: null,
    status: { "claude-code": i % 4 === 3 ? "modified" : "on", codex: i % 2 ? "on" : "off" },
    capabilities: { "claude-code": ["skills", "mcpServers", "model"], codex: ["model"] },
  };
});

export const skillsEnv = { npx_available: true, node_version: "v22.11.0" };
export const catalog = Array.from({ length: 6 }, (_, i) => ({
  id: `demo-skill-${i + 1}`,
  name: `演示技能 ${i + 1}`,
  description: "演示技能目录条目：用于界面示意。",
  version: "1.0.0", author: "demo",
  installed: i < 2,
}));
export const installedSkills = {
  items: [
    { name: "演示技能·代码评审", enabled_agents: ["claude", "codex"], scope: "global", installed_path: "/Users/demo/.claude/skills/demo-review", description: "演示技能：按规范评审改动并给修复建议（示意数据）。", source: "demo/repo", source_type: "github", source_url: "https://github.com/demo/repo", skill_folder_hash: null, plugin_name: null, installed_at: "2026-10-01T00:00:00Z", updated_at: "2026-11-01T00:00:00Z" },
    { name: "演示技能·文档摘要", enabled_agents: ["claude"], scope: "global", installed_path: "/Users/demo/.claude/skills/demo-summary", description: "演示技能：抽取长文档要点生成摘要（示意数据）。", source: null, source_type: null, source_url: null, skill_folder_hash: null, plugin_name: null, installed_at: null, updated_at: null },
  ],
  stale: false,
};

export const ccStatus = { rtk_version: "demo-1.4.0", rtk_claude: true, rtk_codex: false, codex_present: true, sqz_version: "demo-2.1.0", sqz_mcp_version: "demo-2.1.0", sqz_enabled: true, sqz_missing: [] };
export const ccStats = {
  rtk: { total_commands: 420, total_input: 9120000, total_output: 5320000, total_saved: 3800000, avg_savings_pct: 41.7 },
  sqz: { totalCompressions: 900, tokensIn: 12800000, tokensOut: 8640000, tokensSaved: 4160000, avgReduction: 32.5 },
  rtk_daily: Array.from({ length: 30 }, (_, i) => ({ date: isoDay(i), commands: 10 + ((i * 7) % 18), input_tokens: 300000 + i * 9000, output_tokens: 190000 + i * 6000, saved_tokens: 110000 + i * 4000, savings_pct: 30 + (i % 9) })),
  sqz_daily: Array.from({ length: 30 }, (_, i) => ({ date: isoDay(i), saved_tokens: 90000 + ((i * 13) % 80000) })),
};
function isoDay(daysAgo) {
  const d = new Date(now - daysAgo * D);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export const pluginBundles = {
  plugins: [
    { name: "演示插件·格式化", source: "demo-market/demo-format", pick: null, agents: ["claude-code"], on: true, skills: ["演示技能·格式化"], subagents: ["demo-format-agent"], hooks: [], servers: [], left: [] },
    { name: "演示插件·检索增强", source: "demo-market/demo-rag", pick: null, agents: [], on: false, skills: [], subagents: [], hooks: [], servers: ["demo-search"], left: [{ code: "text", args: { text: "演示：该插件未携带可分发部件" } }] },
  ],
  warnings: [],
};

export const middlewareRules = Array.from({ length: 3 }, (_, i) => ({
  id: i + 1,
  name: `演示规则 ${i + 1}：${["敏感信息脱敏", "请求日志采样", "响应改写"][i]}`,
  description: "演示中间件规则：界面示意用。",
  conditions: { kind: "leaf", target: "model", field: "", match_type: "contains", pattern: "demo" },
  actions: [{ kind: "mask", params: { mode: "redact" } }],
  applies_to: { platforms: [1, 2], groups: ["gk-demo0001"], models: ["claude-sonnet-demo"] },
  priority: i + 1,
  enabled: i !== 2,
  is_builtin: i === 0,
  failed: false,
  user_modified: i === 1,
  created_at: now - 20 * D, updated_at: now - 2 * D,
}));

export const variablesPage = [
  { id: 1, key: "DEMO_API_BASE", value: "https://demo.example.com", scope: "global", description: "演示变量" },
  { id: 2, key: "DEMO_RETRY", value: "3", scope: "global", description: "演示变量" },
];
