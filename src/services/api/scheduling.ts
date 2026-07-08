// scheduling.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { MiddlewareRule, CreateMiddlewareRule, UpdateMiddlewareRule, MiddlewareSettings, SchedulingBreakerSettings } from "./types";

export const middlewareApi = {
  /** 列出全部规则（后端按 priority 升序、id 升序）。 */
  listRules: () => invoke<MiddlewareRule[]>("middleware_list_rules"),
  /** 创建规则，返回新规则；后端写库后自动 reload 引擎缓存。 */
  createRule: (input: CreateMiddlewareRule) =>
    invoke<MiddlewareRule>("middleware_create_rule", { input }),
  /** 全量更新规则，返回更新后规则；写库后自动 reload。 */
  updateRule: (input: UpdateMiddlewareRule) =>
    invoke<MiddlewareRule>("middleware_update_rule", { input }),
  /** 删除规则；写库后自动 reload。 */
  deleteRule: (id: number) => invoke<void>("middleware_delete_rule", { id }),
  /** 读取中间件总设置（无配置 → 默认 enabled=true）。 */
  getSettings: () => invoke<MiddlewareSettings>("middleware_settings_get"),
  /** 保存中间件总设置。 */
  setSettings: (settings: MiddlewareSettings) =>
    invoke<void>("middleware_settings_set", { settings }),
  /**
   * 一键导入默认（内置）中间件规则（10 条）。
   * INSERT 仅补缺失项，已存在跳过（不重新启用用户禁用的内置规则）；可重复点（幂等）。
   * 返 {imported: 新增数, skipped: 已存在跳过数}。
   */
  importDefaults: () =>
    invoke<{ imported: number; skipped: number }>("middleware_import_default_rules"),
};

// ─── Scheduling & Breaker Settings ─────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs SchedulingBreakerSettings）一致。
// Platform 的 breaker_* 字段为 0 时继承本结构对应默认值（5/1800/2）。

/** 全局调度 + 熔断默认设置（settings scope=scheduling）。 */


export const schedulingApi = {
  /** 读取全局调度+熔断设置（无配置 → 默认 5/1800/2，load_balance，enabled=true）。 */
  getSettings: () => invoke<SchedulingBreakerSettings>("scheduling_settings_get"),
  /** 保存全局调度+熔断设置。 */
  setSettings: (settings: SchedulingBreakerSettings) =>
    invoke<void>("scheduling_settings_set", { settings }),
};

// ─── Notification（N1 — 系统通知模块；契约冻结，N3 消费）────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs / notification.rs）一致。

/** 通知类型（serde snake_case）。3 类型：task_complete / waiting_input / error。 */

