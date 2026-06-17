"""共享 httpx client + 便捷抓取/解析工具。"""

from __future__ import annotations

import os

import httpx

TIMEOUT = httpx.Timeout(30.0, connect=10.0)
HEADERS = {
    "User-Agent": "aidog-pricing-sync/0.1 (+https://github.com/lazygophers/aidog)"
}


def _proxy_from_env() -> str | None:
    """显式读 *_proxy env (保留 clash 等翻墙代理)。

    配合 trust_env=False —— 关闭 httpx 自动读 env, 避开 NO_PROXY 含裸 IPv6
    (::1 / fe80:: / fc00::) 致 httpx URLPattern → URL() → normalize_port(':')
    → InvalidURL 的坑 (httpx 0.28 对 NO_PROXY 每项建 URLPattern, IPv6 未方括号化时冒号被当 port)。
    NO_PROXY bypass 因此失效, 对 pricing-sync 无害 (只抓外部域名, 不涉 localhost)。
    """
    for var in ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"):
        v = os.environ.get(var)
        if v:
            return v
    return None


def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(
        timeout=TIMEOUT, headers=HEADERS, follow_redirects=True,
        trust_env=False, proxy=_proxy_from_env(),
    )


async def get_text(url: str, **kw) -> str:
    async with client() as c:
        r = await c.get(url, **kw)
        r.raise_for_status()
        return r.text


async def get_json(url: str, **kw):
    async with client() as c:
        r = await c.get(url, **kw)
        r.raise_for_status()
        return r.json()
