// ProxyEnabledSection（平台编辑页「使用代理」开关，issue #64 T03）就近测试：
// 开关写回 onChange、全局代理地址为空时「暂按直连」提示出现 / 已配置时不出现。
// 渲染真组件，data-testid 取元素断言；不锚 DOM 层级 / class / 译文文案。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../test/render";
import { ProxyEnabledSection } from "./formSections";

// 全局代理地址是否已配置：模块级可变桩（useProxyConfigured 真实现走 IPC，测试注入）
const proxyCfg = vi.hoisted(() => ({ configured: false }));
vi.mock("../../domains/platforms/proxyConfig", () => ({
  useProxyConfigured: () => proxyCfg.configured,
}));

// t stub：fallback + {{var}} 插值（i18next 同构）
const t = ((key: string, fallback?: string, opts?: Record<string, unknown>) => {
  let s = fallback ?? key;
  if (opts) for (const [k, v] of Object.entries(opts)) s = s.split(`{{${k}}}`).join(String(v));
  return s;
}) as never;

describe("ProxyEnabledSection", () => {
  beforeEach(() => { proxyCfg.configured = false; });

  it("全局代理地址为空 → 显示「暂按直连」提示", () => {
    render(<ProxyEnabledSection enabled={false} onChange={vi.fn()} t={t} />);
    expect(screen.getByTestId("proxy-unconfigured-hint")).toBeTruthy();
  });

  it("全局代理地址已配置 → 不显示提示", () => {
    proxyCfg.configured = true;
    render(<ProxyEnabledSection enabled={false} onChange={vi.fn()} t={t} />);
    expect(screen.queryByTestId("proxy-unconfigured-hint")).toBeNull();
  });

  it("开关点击写回 onChange（关→开 / 开→关）", () => {
    const onChange = vi.fn();
    const { rerender } = render(<ProxyEnabledSection enabled={false} onChange={onChange} t={t} />);
    fireEvent.click(screen.getByTestId("platform-use-proxy"));
    expect(onChange).toHaveBeenCalledWith(true);
    rerender(<ProxyEnabledSection enabled={true} onChange={onChange} t={t} />);
    fireEvent.click(screen.getByTestId("platform-use-proxy"));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it("全局代理地址为空时开关仍可操作（无 disabled）", () => {
    render(<ProxyEnabledSection enabled={false} onChange={vi.fn()} t={t} />);
    expect((screen.getByTestId("platform-use-proxy") as HTMLButtonElement).hasAttribute("disabled")).toBe(false);
  });
});
