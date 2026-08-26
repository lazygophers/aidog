import { describe, it, expect } from "vitest";
import { syncResultMessage } from "./syncMessage";
import type { PriceSyncResult } from "../../services/api";

/** 测试里的 t 直接返回 fallback，断言的是文案里的数据而非翻译本身。 */
const t = (_key: string, fallback: string) => fallback;

const base: PriceSyncResult = { added: 0, updated: 0, unchanged: 0, failed: 0, total: 0, failures: [] };

describe("syncResultMessage", () => {
  it("全成功时只给计数摘要，不带失败段", () => {
    const msg = syncResultMessage({ ...base, added: 3, updated: 1, total: 4 }, t);
    expect(msg).toContain("+3 新增");
    expect(msg).toContain("~1 更新");
    expect(msg).not.toContain("保留原有数据");
  });

  it("partial 失败时逐条列出文件名，用户才知道哪些平台还是旧数据", () => {
    const msg = syncResultMessage(
      {
        ...base,
        added: 1,
        failed: 2,
        total: 3,
        failures: [
          { file: "platforms/beta/platform.json", error: "status 404" },
          { file: "platforms/beta/models/b-1.json", error: "status 404" },
        ],
      },
      t
    );
    expect(msg).toContain("2 失败");
    expect(msg).toContain("platforms/beta/platform.json");
    expect(msg).toContain("platforms/beta/models/b-1.json");
    expect(msg).toContain("保留原有数据");
  });
});
