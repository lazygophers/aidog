// 跨层镜像一致性（通知体系票 01）：CC_EVENT_SEMANTIC 的键 ⊆ 前端 CC_HOOK_EVENTS 全量目录，
// 且每个 CC 事件要么有语义映射要么明确透传——映射侧（Rust）改动不同步时这里红。
import { describe, it, expect } from "vitest";
import { CC_HOOK_EVENTS } from "@/components/settings/NotificationEventList";
import {
  SEMANTIC_TYPES,
  PROXY_SEMANTIC_EVENTS,
  CC_EVENT_SEMANTIC,
} from "./semantic";

describe("通知语义事件跨层镜像", () => {
  it("CC_EVENT_SEMANTIC 的键全部在 CC_HOOK_EVENTS 全量目录内", () => {
    for (const ev of Object.keys(CC_EVENT_SEMANTIC)) {
      expect(CC_HOOK_EVENTS, `事件 ${ev} 不在前端 CC_HOOK_EVENTS 目录`).toContain(ev);
    }
  });

  it("CC_HOOK_EVENTS 至少 30 项（全量目录未被裁剪）", () => {
    expect(CC_HOOK_EVENTS.length).toBeGreaterThanOrEqual(30);
  });

  it("SEMANTIC_TYPES = 3 CC 语义 + 3 proxy 语义", () => {
    expect(SEMANTIC_TYPES).toHaveLength(6);
    expect(PROXY_SEMANTIC_EVENTS).toHaveLength(3);
    for (const t of PROXY_SEMANTIC_EVENTS) {
      expect(SEMANTIC_TYPES).toContain(t);
    }
  });

  it("映射值全部是合法语义类型", () => {
    for (const v of Object.values(CC_EVENT_SEMANTIC)) {
      expect(SEMANTIC_TYPES).toContain(v);
    }
  });
});
