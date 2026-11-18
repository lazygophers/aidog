// 真渲染截图：vite dev + Playwright(channel chrome) + 路由层 mock /api/rpc。
// 用法: node shoot.mjs [pageId ...]   无参数 = 全部页面。
import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync, appendFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import * as M from "./mocks.mjs";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const OUT = `${ROOT}screenshots`;
const LOG = `${ROOT}.tmp/screenshots-redo/unmatched.log`;
const BASE = "http://localhost:1420";
mkdirSync(OUT, { recursive: true });
mkdirSync(dirname(LOG), { recursive: true });
writeFileSync(LOG, "");

const PAGES = [
  ["home", "home"],
  ["platforms", "platforms"],
  ["stats", "stats"],
  ["logs", "logs"],
  ["request-log", "request-log"],
  ["mitm-log", "mitm-log"],
  ["tool-log", "tool-log"],
  ["notifications", "notifications"],
  ["skills", "skills"],
  ["mcp", "mcp"],
  ["subagents", "subagents"],
  ["context-compress", "context-compress"],
  ["variables", "variables"],
  ["plugins", "plugins"],
  ["tools", "tools"],
  ["middleware", "middleware"],
  ["about", "about"],
  ["settings/system", "settings-system"],
  ["settings/api", "settings-api"],
  ["settings/claude", "settings-claude"],
];

const argPages = process.argv.slice(2);
const wanted = argPages.length ? PAGES.filter(([id]) => argPages.includes(id)) : PAGES;
if (!wanted.length) { console.error("no matching pages:", argPages.join(",")); process.exit(2); }

// 各命令假返回。未命中命令 → 记日志 + 返回 null。
const MOCKS = {
  settings_get: (a) => {
    if (a?.key === "theme") return { mode: M.theme.current };
    if (a?.key === "locale") return { locale: "zh-Hans" };
    if (a?.key === "extension_variables") return [{ key: "DEMO_API_BASE", value: "https://demo.example.com/v1" }, { key: "DEMO_LOG_LEVEL", value: "info" }];
    return null;
  },
  proxy_status: () => true,
  proxy_get_settings: () => ({ autostart: true, bind_lan: false, port: 9890 }),
  proxy_client_get_settings: () => ({ others_enabled: true }),
  platform_list: () => M.platforms,
  platform_error_status: () => ({}),
  platform_usage_stats: () => ({ total_requests: 600, success_count: 592, today_requests: 32, today_success: 31, total_cost: 69.93 }),
  group_list: () => M.groups,
  group_detail_list: () => [],
  group_get_platforms: () => [],
  group_usage_stats: () => ({ total_requests: 640, success_count: 628, total_input_tokens: 2700000, total_output_tokens: 540000, total_cache_tokens: 1620000, cache_rate: 30.0, recent_failures: 1, recent_total: 20, total_cost: 24.1, today_tokens: 82000, today_cost: 2.4, today_requests: 32, today_success: 31 }),
  all_group_usage_stats: () => ({
    "gk-demo0001": { total_requests: 640, success_count: 628, total_input_tokens: 2700000, total_output_tokens: 540000, total_cache_tokens: 1620000, cache_rate: 30.0, recent_failures: 1, recent_total: 20, total_cost: 24.1, today_tokens: 82000, today_cost: 2.4, today_requests: 32, today_success: 31 },
    "gk-demo0002": { total_requests: 486, success_count: 480, total_input_tokens: 2040000, total_output_tokens: 410000, total_cache_tokens: 1220000, cache_rate: 28.0, recent_failures: 0, recent_total: 20, total_cost: 19.4, today_tokens: 41000, today_cost: 1.2, today_requests: 12, today_success: 12 },
  }),
  group_last_requests: () => ({
    "gk-demo0001": { id: "demo-log-1000", status_code: 200, created_at: M.nowMs - 5 * 60000, duration_ms: 4210, blocked_reason: "", error: "" },
    "gk-demo0002": { id: "demo-log-999", status_code: 200, created_at: M.nowMs - 40 * 60000, duration_ms: 5320, blocked_reason: "", error: "" },
  }),
  group_daily_usage: () => [],
  group_latest_routing: () => null,
  stats_flow_breakdown: () => [
    { client: "claude_code", group_key: "gk-demo0001", req_model: "claude-sonnet-demo", request_class: "chat", platform_id: 1, model: "demo-sonnet-main", requests: 640, tokens: 2688000, est_cost: 24.62 },
    { client: "claude_code", group_key: "gk-demo0002", req_model: "claude-sonnet-demo", request_class: "chat", platform_id: 2, model: "demo-gpt-main", requests: 486, tokens: 2041200, est_cost: 19.4 },
    { client: "codex_tui", group_key: "gk-demo0001", req_model: "gpt-demo", request_class: "chat", platform_id: 5, model: "demo-fog-auto", requests: 322, tokens: 1352400, est_cost: 8.71 },
  ],
  stats_query: () => M.statsResult(24),
  stats_query_batch: (a) => (a?.queries ?? []).map((q) => {
    const span = (q?.end ?? 0) - (q?.start ?? 0);
    return M.statsResult(span > 48 * 3600 * 1000 ? 30 : 24);
  }),
  session_stats: () => M.sessionStats,
  all_platform_usage_stats: () => Object.fromEntries(M.platforms.map((p) => [p.id, { total_requests: 600, success_count: 592, total_input_tokens: 2500000, total_output_tokens: 500000, total_cache_tokens: 1500000, cache_rate: 30.0, recent_failures: p.status === "enabled" ? 0 : 3, recent_total: 20, total_cost: 41.82, today_tokens: 82000, today_cost: 2.4, today_requests: 32, today_success: 31 }])),
  group_detail_list_paged: (a) => {
    const offset = a?.offset ?? 0;
    const limit = a?.limit ?? 12;
    const mkDetail = (g, pids) => ({
      group: g,
      platforms: pids.map((pid, i) => {
        const p = M.platforms.find((x) => x.id === pid);
        return { platform: p, priority: i + 1, weight: 100, level_priority: 0 };
      }),
      model_mappings: [],
    });
    const all = [
      mkDetail(M.groups[0], [1, 2, 5]),
      mkDetail(M.groups[1], [1, 3]),
      mkDetail(M.groups[2], [2, 4]),
    ];
    const items = all.slice(offset, offset + limit);
    return items;
  },
  group_detail_list: () => [],
  tray_today_stats: () => ({ tokens: 82000, input_tokens: 64000, output_tokens: 18000, cache_tokens: 26000, cache_rate: 40.6, cost: 2.4, total_requests: 32 }),
  quota_snapshots: () => [],
  stats_settings_get: () => ({}),
  proxy_log_list_filtered: () => ({ items: M.logRows, has_more: false }),
  proxy_log_count: () => 25,
  proxy_log_count_filtered: () => 25,
  proxy_log_distinct_models: () => ["claude-sonnet-demo", "demo-gpt-main"],
  proxy_log_settings_get: () => ({ log_user_request: true, log_upstream_request: true, user_request_retention_days: 7, upstream_request_retention_days: 7, retention_days: 90, user_request_retention_unit: "day", upstream_request_retention_unit: "day", retention_unit: "day" }),
  request_log_list: () => M.requestLogs,
  request_log_count: () => 15,
  tool_log_list: () => ({ total: M.toolLogRows.length, rows: M.toolLogRows }),
  tool_log_retention_get: () => 30,
  mitm_status: () => false,
  mitm_bypass_list: () => Array.from({ length: 10 }, (_, i) => ({
    id: i + 1,
    group_name: i % 2 ? "演示·日常编码" : "演示·长文写作",
    host: ["demo.example.com", "api.demo-upstream.cn", "demo-relay.example.net"][i % 3],
    path: ["/v1/messages", "/v1/chat/completions", "/v1/models"][i % 3],
    status_code: i % 5 === 4 ? 403 : 200,
    req_bytes: 4000 + i * 250, resp_bytes: 12000 + i * 900,
    decrypted: i % 3 !== 1,
    created_at: M.nowMs - i * 47 * 60000,
  })),
  mitm_whitelist: () => [],
  mitm_usage_trend: () => [],
  mitm_oauth_refresh_stats: () => [],
  cc_plan_info: () => null,
  notification_settings_get: () => M.notifSettings,
  notification_inbox_list: () => M.notifications,
  notification_delivery_list: () => [],
  skills_check_env: () => M.skillsEnv,
  skills_browse_catalog: () => M.catalog,
  skills_list_installed: () => M.installedSkills,
  mcp_list: () => M.mcpServers,
  mcp_recommended_list: () => [],
  subagents_list: () => M.subagents,
  context_compress_status: () => M.ccStatus,
  context_compress_stats: () => M.ccStats,
  plugins_bundle_list: () => M.pluginBundles,
  plugins_codex_list: () => ({ installed: [], marketplaces: [] }),
  middleware_list_rules: () => M.middlewareRules,
  middleware_settings_get: () => ({}),
  middleware_budget_status: () => [],
  scheduling_settings_get: () => ({}),
  middleware: () => null,
  kernel_settings_get: () => ({ api_enabled: true, auth_token: "" }),
  app_log_settings_get: () => ({ level: "info" }),
  about_info: () => ({ app_version: "0.0.0-demo", tauri_version: "2.0.0", os: "macOS", arch: "arm64", family: "unix", profile: "release", git_commit: "demo0000", build_time: String(Math.floor(M.nowMs / 1000)) }),
  cli_check_versions: () => [
    { name: "claude", installed: true, version: "2.1.319", path: "/usr/local/bin/claude", broken: false, conflict: false },
    { name: "codex", installed: true, version: "0.9.0", path: "/usr/local/bin/codex", broken: false, conflict: false },
    { name: "pi", installed: false, version: null, path: null, broken: false, conflict: false },
  ],
  cli_check_updates: () => [
    { name: "claude", installed: true, version: "2.1.319", path: "/usr/local/bin/claude", broken: false, conflict: false },
    { name: "codex", installed: true, version: "0.9.0", path: "/usr/local/bin/codex", broken: false, conflict: false },
    { name: "pi", installed: false, version: null, path: null, broken: false, conflict: false },
  ],
  agent_update_check: () => null,
  get_auto_update_enabled: () => false,
  get_defaults_json: () => ({}),
  get_client_types_json: () => ({}),
  get_client_vocab: () => ({}),
  get_protocol_logo_path: () => null,
  get_managed_paths: () => [],
  get_last_test_result: () => null,
  settings_preset_list: () => [],
  coding_tools_settings_get: () => ({}),
  claude_output_styles_list: () => [],
  export_preview: () => null,
  backup_settings_get: () => ({}),
  display_currency_get: () => ({ code: "USD", symbol: "$", decimals: 2, rate: 1, table: [{ code: "USD", symbol: "$", decimals: 2, default_rate: 1, rate: 1 }, { code: "CNY", symbol: "¥", decimals: 2, default_rate: 7.2, rate: 7.2 }] }),
  balance_currency_map: () => ({ "1": "USD", "2": "USD", "3": "CNY", "4": "USD", "5": "USD" }),
  holiday_calendar_list: () => [],
  ccswitch_detect: () => null,
  tray_config_get: () => ({}),
  popover_config_get: () => ({}),
  popover_platform_today: () => [],
  search_sources: () => [],
  price_sync_settings_get: () => ({}),
  model_entry_list: () => [],
  model_info_snapshot: () => null,
  platform_reorder: () => null,
  variables_save: () => null,
  profile_list: () => [],
  api_call_log_list: () => [],
  proxy_timeout_get: () => ({}),
};

function pickMock(cmd, args) {
  const h = MOCKS[cmd];
  if (h) return h(args);
  appendFileSync(LOG, `${cmd} ${JSON.stringify(args ?? {})}\n`);
  return null;
}

const seen = new Set();
const browser = await chromium.launch({ channel: "chrome", headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

await page.route("**/api/rpc/**", async (route) => {
  const url = new URL(route.request().url());
  const cmd = decodeURIComponent(url.pathname.replace("/api/rpc/", ""));
  let args = {};
  try { args = route.request().postDataJSON() ?? {}; } catch {}
  seen.add(cmd);
  const data = pickMock(cmd, args);
  await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(data ?? null) });
});
await page.route("**/api/events", (route) =>
  route.fulfill({ status: 200, contentType: "text/event-stream", body: ": keep-alive\n\n" }));

async function shoot([id, name]) {
  await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
  const mode = M.theme.current;
  await page.evaluate(([m, navId]) => {
    localStorage.setItem("aidog-settings", JSON.stringify({ themeMode: m, locale: "zh-Hans" }));
    localStorage.setItem("aidog.lastNav", navId);
  }, [mode, id]);
  await page.reload({ waitUntil: "networkidle" });
  // 等首页 app 挂载（sidebar 任一项出现）
  await page.waitForSelector('[data-testid^="nav-item-"]', { timeout: 20000 });
  await page.waitForTimeout(3500);
  const file = `${OUT}/${name}-${M.theme.current}.png`;
  await page.screenshot({ path: file, fullPage: false });
  console.log(`OK ${file}`);
}

// light 先跑一遍
M.theme.current = "light";
for (const p of wanted) await shoot(p);
// dark 一遍
M.theme.current = "dark";
for (const p of wanted) await shoot(p);

await browser.close();
console.log("UNMATCHED COMMANDS:", [...seen].filter((c) => !MOCKS[c]).sort().join(", ") || "(none)");
