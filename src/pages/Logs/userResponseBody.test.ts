import { describe, it, expect } from "vitest";
import { pickUserResponseBody } from "./userResponseBody";

describe("pickUserResponseBody", () => {
  it("user_response_body 非空时优先用它", () => {
    expect(pickUserResponseBody("{\"a\":1}", "{\"b\":2}")).toBe("{\"a\":1}");
  });

  it("user_response_body 为空且 response_body 非空时回退", () => {
    expect(pickUserResponseBody("", "{\"b\":2}")).toBe("{\"b\":2}");
  });

  it("都为空返回 null（占位由调用方给）", () => {
    expect(pickUserResponseBody("", "")).toBe(null);
    expect(pickUserResponseBody(null, undefined)).toBe(null);
  });
});
