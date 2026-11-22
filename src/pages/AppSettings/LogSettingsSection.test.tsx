// LogSettingsSection 保存失败回滚（issue #56 T05）测试：
// 真 hook（renderHook 同款 spy 环境）+ 真组件，proxyLogApi.setSettings reject 后
// 断言开关 / 数值 / 单位回到保存前的值且错误提示出现；resolve 时状态保持新值
// 且调用参数含完整 ProxyLogSettings。渲染真组件，data-testid 取元素断言。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import {
  proxyApi, proxyLogApi, proxyTimeoutApi, builtinToolCompatGlobalApi, appLogApi,
  statsSettingsApi, autoUpdateApi,
} from "../../services/api";
import { useSystemSettings } from "./useSystemSettings";
import { LogSettingsSection } from "./LogSettingsSection";

vi.mock("../../services/platform", () => ({ getAppVersion: () => Promise.resolve("") }));

// Radix Select 在 jsdom 无法开面板：换原生 select 壳（DisplayCurrencySection.test 同款），
// testid 取自 SelectTrigger 子元素上的 data-testid。
vi.mock("@/components/ui/select", () => ({
  Select: ({ value, onValueChange, children }: { value: string; onValueChange: (v: string) => void; children: unknown }) => {
    let tid: string | undefined;
    for (const c of (Array.isArray(children) ? children : [children]) as { props?: Record<string, unknown> }[]) {
      if (c && typeof c === "object" && c.props && typeof c.props["data-testid"] === "string") {
        tid = c.props["data-testid"] as string;
      }
    }
    return (
      <select data-testid={tid} value={value} onChange={(e) => onValueChange(e.target.value)}>{children}</select>
    );
  },
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => <option value={value}>{children}</option>,
}));

const fullDefaults = {
  log_user_request: true,
  log_upstream_request: true,
  user_request_retention_days: 7,
  user_request_retention_unit: "day",
  upstream_request_retention_days: 7,
  upstream_request_retention_unit: "day",
  retention_days: 90,
  retention_unit: "day",
};

function spyAll() {
  vi.spyOn(proxyApi, "getSettings").mockResolvedValue({} as never);
  vi.spyOn(proxyApi, "status").mockResolvedValue(false);
  vi.spyOn(proxyApi, "getAutolaunch").mockResolvedValue(false);
  vi.spyOn(proxyApi, "getProxyClientSettings").mockResolvedValue({
    others_enabled: false, proxy_type: "socks5", host: "127.0.0.1", port: 7890,
    username: "", password: "", dns_over_proxy: true, no_proxy: "",
  });
  vi.spyOn(proxyApi, "setProxyClientSettings").mockResolvedValue(undefined as never);
  vi.spyOn(proxyApi, "setSilentLaunch").mockResolvedValue(undefined as never);
  vi.spyOn(proxyLogApi, "getSettings").mockRejectedValue(new Error("skip"));
  vi.spyOn(proxyLogApi, "cleanupEstimate").mockResolvedValue({ overdue_rows: 0, overdue_body_bytes: 0, db_size_bytes: 0 } as never);
  vi.spyOn(proxyTimeoutApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(builtinToolCompatGlobalApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(appLogApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(statsSettingsApi, "get").mockRejectedValue(new Error("skip"));
  vi.spyOn(autoUpdateApi, "get").mockRejectedValue(new Error("skip"));
}

// 真 hook + 真组件的桥接：state 与回滚逻辑都在 useSystemSettings 里，必须走真实现。
function Harness() {
  const s = useSystemSettings();
  return <LogSettingsSection s={s} />;
}

describe("LogSettingsSection 保存失败回滚（issue #56 T05）", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    spyAll();
  });

  it("开关保存失败：状态回滚到原值且错误提示出现", async () => {
    // 默认 state：logUserReq=true（getSettings reject 走默认）。保存恒败。
    vi.spyOn(proxyLogApi, "setSettings").mockRejectedValue(new Error("boom"));
    render(<Harness />);
    const sw = screen.getByTestId("log-user-req-switch");
    await waitFor(() => expect(sw).toHaveAttribute("data-state", "checked"));
    fireEvent.click(sw);
    // 失败后回滚：开关回到原值 checked（未实现回滚时停在 unchecked，此断言先红）
    await waitFor(() => expect(screen.getByTestId("log-user-req-switch")).toHaveAttribute("data-state", "checked"));
    expect(await screen.findByTestId("log-settings-message")).toHaveTextContent("boom");
  });

  it("开关保存成功：状态保持新值，调用参数含完整 ProxyLogSettings", async () => {
    const setSettings = vi.spyOn(proxyLogApi, "setSettings").mockResolvedValue(undefined as never);
    render(<Harness />);
    const sw = screen.getByTestId("log-user-req-switch");
    await waitFor(() => expect(sw).toHaveAttribute("data-state", "checked"));
    fireEvent.click(sw);
    await waitFor(() => expect(setSettings).toHaveBeenCalledTimes(1));
    expect(setSettings).toHaveBeenCalledWith({
      log_user_request: false,
      log_upstream_request: true,
      user_request_retention_days: 7,
      user_request_retention_unit: "day",
      upstream_request_retention_days: 7,
      upstream_request_retention_unit: "day",
      retention_days: 90,
      retention_unit: "day",
    });
    // 成功不回滚：开关停在新值
    expect(screen.getByTestId("log-user-req-switch")).toHaveAttribute("data-state", "unchecked");
    expect(screen.queryByTestId("log-settings-message")).toBeNull();
  });

  it("数值+单位成对控件失败：两者一并回滚", async () => {
    vi.spyOn(proxyLogApi, "setSettings").mockRejectedValue(new Error("boom"));
    render(<Harness />);
    const num = screen.getByTestId("user-req-retention") as HTMLInputElement;
    await screen.findByTestId("user-req-retention-unit");
    fireEvent.change(num, { target: { value: "3" } });
    await waitFor(() => expect(num.value).toBe("7")); // 数值回滚
    // UnitSelect 是组件内定义的函数组件，每次渲染都会重挂载：每次交互 / 断言前重新取挂载节点。
    const unitEl = () => screen.getByTestId("user-req-retention-unit") as HTMLSelectElement;
    fireEvent.change(unitEl(), { target: { value: "week" } });
    await waitFor(() => expect(unitEl().value).toBe("day")); // 单位回滚
    expect(num.value).toBe("7");
    expect(screen.getByTestId("log-settings-message")).toHaveTextContent("boom");
  });

  it("数值保存成功：状态保持新值", async () => {
    const setSettings = vi.spyOn(proxyLogApi, "setSettings").mockResolvedValue(undefined as never);
    render(<Harness />);
    const num = screen.getByTestId("log-retention") as HTMLInputElement;
    fireEvent.change(num, { target: { value: "30" } });
    await waitFor(() => expect(setSettings).toHaveBeenCalledTimes(1));
    expect(num.value).toBe("30");
    expect(screen.queryByTestId("log-settings-message")).toBeNull();
  });
});
