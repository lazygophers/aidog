# 09 — 多模态图片双向

**What to build:** 图片输入跨协议保留：OpenAI image_url(data: URL / http url) ↔ Anthropic image.source(base64/url) ↔ Gemini inlineData/fileData 双向；data URL 拆解 media_type + base64 正确还原。

**Blocked by:** 01 — 地基（Content block 承载）。

**Status:** done

- [x] fixture：三协议带图请求互转，图片 block 逐字段映射断言
- [x] data URL ↔ (media_type, base64) 拆解/重组 round-trip
- [x] 纯文本请求不回归；cargo test / make lint 全绿
