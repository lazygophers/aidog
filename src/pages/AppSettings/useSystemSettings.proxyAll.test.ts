// useSystemSettings 的「为全部平台开启」执行体（issue #64 T02）与代理配置广播：
// 逐平台写 extra.proxy_enabled=true（保留其余键）、广播平台列表刷新事件、
// 保存代理配置后广播代理配置变更事件。hook 真渲染（renderHook），API 全部 spy。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  proxyApi, proxyLogApi, proxyTimeoutApi, builtinToolCompatGlobalApi, appLogApi,
  statsSettingsApi, autoUpdateApi, platformApi,
} from "../../services/api";
import { useSystemSettings } from "./useSystemSettings";

vi.mock("../../services/platform", () => ({ getAppVersion: () => Promise.resolve("") }));

const plats = [
  { id: 1, extra: JSON.stringify({ keep: "a" }) },
  { id: 2, extra: "" },
] as never as Awaited<ReturnType<typeof platformApi.list>>;

function spyAll() {
  vi.spyOn(proxyApi, "getSettings").mockResolvedValue({} as any);
  vi.spyOn(proxyApi, "status").mockResolvedValue(false);
  vi.spyOn(proxyApi, "getAutolaunch").mockResolvedValue(false);
  vi.spyOn(proxyApi, "getProxyClientSettings").mockResolvedValue({
    others_enabled: false, proxy_type: "socks5", host: "127.0.0.1", port: 7890,
    username: "", password: "", dns_over_proxy: true, no_proxy: "",
  });
  vi.spyOn(proxyApi, "setProxyClientSettings").mockResolvedValue(undefined as any);
  vi.spyOn(proxyLogApi, "getSettings").mockRejectedValue(new Error("skip"));
  vi.spyOn(proxyTimeoutApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(builtinToolCompatGlobalApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(appLogApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(statsSettingsApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(autoUpdateApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(platformApi, "list").mockResolvedValue(plats);
  vi.spyOn(platformApi, "update").mockResolvedValue(undefined as any);
}

describe("useSystemSettings 为全部平台开启 + 代理配置广播", () => {
  beforeEach(() => { spyAll(); vi.spyOn(console, "error").mockImplementation(() => {}); });
  afterEach(() => vi.restoreAllMocks());

  it("执行体逐平台写 extra.proxy_enabled=true（保留其余键）并广播平台列表刷新", async () => {
    const seen: string[] = [];
    const onEvt = (e: Event) => seen.push(e.type);
    window.addEventListener("aidog-platforms-changed", onEvt);
    const { result } = renderHook(() => useSystemSettings());
    await act(() => result.current.runEnableProxyForAll());
    expect(platformApi.update).toHaveBeenCalledTimes(2);
    const first = JSON.parse(vi.mocked(platformApi.update).mock.calls[0][0].extra!);
    const second = JSON.parse(vi.mocked(platformApi.update).mock.calls[1][0].extra!);
    expect(first).toMatchObject({ keep: "a", proxy_enabled: true });
    expect(second).toEqual({ proxy_enabled: true });
    expect(seen).toContain("aidog-platforms-changed");
    window.removeEventListener("aidog-platforms-changed", onEvt);
  });

  it("保存代理配置成功后广播代理配置变更事件（徽标消费方重取 host）", async () => {
    const seen: string[] = [];
    const onEvt = (e: Event) => seen.push(e.type);
    window.addEventListener("aidog-proxy-config-changed", onEvt);
    const { result } = renderHook(() => useSystemSettings());
    await act(() => result.current.handleProxyClientChange({ host: "127.0.0.1" }));
    expect(proxyApi.setProxyClientSettings).toHaveBeenCalled();
    expect(seen).toContain("aidog-proxy-config-changed");
    window.removeEventListener("aidog-proxy-config-changed", onEvt);
  });
});
