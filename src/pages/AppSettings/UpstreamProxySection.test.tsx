// UpstreamProxySection（设置页「代理配置」区，issue #64 T02）测试：
// 无启用总开关（others_enabled=false 时配置输入仍可见）、「其它请求使用代理」写回、
// 「为全部平台开启」二次确认后执行。渲染真组件，data-testid 取元素断言。
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import { UpstreamProxySection } from "./ProxyStatusSection";
import type { SystemSettings } from "./useSystemSettings";

function makeS(over: Partial<SystemSettings> = {}): SystemSettings {
  return {
    proxyClient: {
      others_enabled: false, proxy_type: "socks5", host: "", port: 7890,
      username: "", password: "", dns_over_proxy: true, no_proxy: "",
    },
    handleProxyClientChange: vi.fn(),
    proxyAllConfirm: false,
    setProxyAllConfirm: vi.fn(),
    proxyAllBusy: false,
    runEnableProxyForAll: vi.fn(),
    ...over,
  } as never as SystemSettings;
}

describe("UpstreamProxySection（代理配置）", () => {
  it("无启用总开关：others_enabled=false 时主机输入仍可见（原总开关不再折叠配置区）", () => {
    render(<UpstreamProxySection s={makeS()} />);
    expect(screen.getByTestId("proxy-host-input")).toBeTruthy();
  });

  it("主机/端口输入带占位提示 127.0.0.1 / 7890", () => {
    render(<UpstreamProxySection s={makeS()} />);
    expect((screen.getByTestId("proxy-host-input") as HTMLInputElement).placeholder).toBe("127.0.0.1");
    expect((screen.getByTestId("proxy-port-input") as HTMLInputElement).placeholder).toBe("7890");
  });

  it("「其它请求使用代理」开关写回 others_enabled", () => {
    const s = makeS();
    render(<UpstreamProxySection s={s} />);
    fireEvent.click(screen.getByTestId("others-use-proxy"));
    expect(s.handleProxyClientChange).toHaveBeenCalledWith({ others_enabled: true });
  });

  it("「为全部平台开启」：二次确认弹窗出现，确认后才执行", async () => {
    const s = makeS({ proxyAllConfirm: false });
    const { rerender } = render(<UpstreamProxySection s={s} />);
    // 点击按钮 → 打开确认弹窗（经 setProxyAllConfirm）
    fireEvent.click(screen.getByTestId("enable-proxy-all-button"));
    expect(s.setProxyAllConfirm).toHaveBeenCalledWith(true);
    // 弹窗打开（proxyAllConfirm=true）→ 确认按钮触发执行
    const s2 = makeS({ proxyAllConfirm: true });
    rerender(<UpstreamProxySection s={s2} />);
    fireEvent.click(await screen.findByTestId("enable-proxy-all-confirm"));
    expect(s2.runEnableProxyForAll).toHaveBeenCalled();
  });

  it("确认弹窗取消不执行", async () => {
    const s = makeS({ proxyAllConfirm: true });
    render(<UpstreamProxySection s={s} />);
    fireEvent.click(await screen.findByTestId("enable-proxy-all-cancel"));
    expect(s.runEnableProxyForAll).not.toHaveBeenCalled();
    expect(s.setProxyAllConfirm).toHaveBeenCalledWith(false);
  });
});
