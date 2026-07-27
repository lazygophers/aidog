import { describe, it, expect } from "vitest";
import { editReducer, EMPTY_EDIT, upsertPlatformInto } from "./editReducer";
import type { GroupDetail, Platform } from "../../services/api";

function makeDetail(overrides: Partial<GroupDetail["group"]> = {}): GroupDetail {
  return {
    group: {
      group_key: "g1",
      name: "Group 1",
      routing_mode: "roundrobin" as any,
      env_vars: [{ key: "K", value: "V" }],
      request_timeout_secs: 30,
      connect_timeout_secs: 5,
      max_retries: 3,
      ...overrides,
    } as any,
    platforms: [{ platform: { id: 1 } }, { platform: { id: 2 } }] as any,
    model_mappings: [
      {
        source_model: "claude-3",
        target_platform_id: 1,
        target_model: "gpt-4",
        request_timeout_secs: 60,
        connect_timeout_secs: 10,
      },
    ] as any,
  } as GroupDetail;
}

describe("editReducer", () => {
  it("open：按 GroupDetail 字段逐一映射到编辑态", () => {
    const detail = makeDetail();
    const next = editReducer(EMPTY_EDIT, { type: "open", detail });
    expect(next).toEqual({
      target: detail,
      name: "Group 1",
      mode: "roundrobin",
      platformIds: [1, 2],
      mappings: [{
        source_model: "claude-3",
        target_platform_id: 1,
        target_model: "gpt-4",
        request_timeout_secs: 60,
        connect_timeout_secs: 10,
      }],
      envVars: [{ key: "K", value: "V" }],
      reqTimeout: 30,
      connTimeout: 5,
      maxRetries: 3,
    });
  });

  it("open：mappings/envVars 是新对象（非原 detail 对象引用），不会互相污染", () => {
    const detail = makeDetail();
    const next = editReducer(EMPTY_EDIT, { type: "open", detail });
    expect(next.mappings[0]).not.toBe(detail.model_mappings[0]);
    expect(next.envVars[0]).not.toBe(detail.group.env_vars[0]);
  });

  it("reset：无论当前态如何，回落到 EMPTY_EDIT（含隐私默认 env）", () => {
    const dirty = editReducer(EMPTY_EDIT, { type: "open", detail: makeDetail() });
    const next = editReducer(dirty, { type: "reset" });
    expect(next).toBe(EMPTY_EDIT);
    expect(next.envVars.length).toBeGreaterThan(0);
  });

  it("patch：浅合并，未 patch 的字段保留原值", () => {
    const base = editReducer(EMPTY_EDIT, { type: "open", detail: makeDetail() });
    const next = editReducer(base, { type: "patch", patch: { name: "renamed" } });
    expect(next.name).toBe("renamed");
    expect(next.mode).toBe(base.mode);
    expect(next.platformIds).toBe(base.platformIds);
  });
});

describe("upsertPlatformInto", () => {
  const p1: Platform = { id: 1, name: "P1" } as any;
  const p2: Platform = { id: 2, name: "P2" } as any;

  it("命中 id → 原位替换该项，其余项引用不变", () => {
    const prev = [p1, p2];
    const updated: Platform = { id: 1, name: "P1-renamed" } as any;
    const next = upsertPlatformInto(prev, updated);
    expect(next).toEqual([{ id: 1, name: "P1-renamed" }, p2]);
    expect(next[1]).toBe(p2);
    expect(next).not.toBe(prev);
  });

  it("未命中 id → 追加到末尾，不改动已有项", () => {
    const prev = [p1];
    const p3: Platform = { id: 3, name: "P3" } as any;
    const next = upsertPlatformInto(prev, p3);
    expect(next).toEqual([p1, p3]);
    expect(next[0]).toBe(p1);
  });
});
