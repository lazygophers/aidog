"""共享 httpx client + 便捷抓取/解析工具。"""

from __future__ import annotations

import httpx

TIMEOUT = httpx.Timeout(30.0, connect=10.0)
HEADERS = {
    "User-Agent": "aidog-pricing-sync/0.1 (+https://github.com/lazygophers/aidog)"
}


def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(timeout=TIMEOUT, headers=HEADERS, follow_redirects=True)


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
