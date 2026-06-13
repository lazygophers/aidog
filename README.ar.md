<div align="center">

# 🐕 AiDog

**بوابة موحدة لواجهات برمجة الذكاء الاصطناعي**

تجميع متعدد المنصات · توجيه ذكي · تحليلات الاستخدام — تطبيق سطح مكتب متعدد المنصات لإدارة المفاتيح والطلبات والإنفاق عبر جميع منصات الذكاء الاصطناعي الخاصة بك

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/ar/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **التوثيق الكامل**: <https://lazygophers.github.io/aidog/ar/>

AiDog بوابة واجهات برمجة تطبيقات للذكاء الاصطناعي على سطح المكتب مبنية على Tauri، تُوحّد إدارة وتوجيه ومراقبة الطلبات عبر أكثر من 50 منصة ذكاء اصطناعي. تجمع في تطبيق واحد مفاتيح API المبعثرة، وتعيين النماذج، وموازنة الحمل، وتحليلات الاستخدام — دون خدمة خلفية، دون سحابة، تُخزَّن كل البيانات محليًا.

## ✨ الميزات

- **تجميع متعدد المنصات** — أكثر من 50 إعدادًا مسبقًا (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen، إلخ)، إعداد بنقرة واحدة
- **تجميع ذكي** — مطابقة الطلبات برمز Bearer / المسار؛ توجيه Failover و Load Balance
- **تعيين النماذج** — استبدال شفاف لاسم النموذج (مثل `claude-sonnet-4` → `deepseek-chat`)
- **تحويل البروتوكول** — تحويل ثنائي الاتجاه بين بروتوكولات OpenAI Chat / Completions / Responses و Anthropic و Gemini
- **موازنة الحمل والتحويل** — إعادة محاولة تلقائية بين المنصات عند الفشل، قاطع الدائرة / إدارة ثلاثية الحالات / تراجع أُسي
- **مراقبة الاستخدام** — إحصاءات الرموز، تقدير التكلفة، استعلامات الرصيد لكل منصة، عرض حصة Coding Plan
- **تسجيل الطلبات** — ثلاثة مستويات من الدقة (طلب المستخدم الأصلي / الطلب الصاعد / الملخص)، لكل منهما مفتاح وفترة احتفاظ
- **محرك قواعد الوسائط** — قواعد واردة/صادرة: تسوية، إعادة كتابة، إخفاء، حقن، تصفية الكلمات الحساسة، كشف الأخطاء
- **تكامل مساعدي البرمجة** — دعم أصلي بنقرة واحدة لـ Claude Code و OpenAI Codex وغيرها من المساعدين
- **التدويل والسمات** — 8 لغات (بما فيها العربية RTL)، Liquid Glass وسمات أخرى بأوضاع فاتح/داكن

## 🚀 البدء السريع

### التنزيل والتثبيت

نزّل المثبّت لمنصتك (macOS / Windows / Linux) من [GitHub Releases](https://github.com/lazygophers/aidog/releases).

راجع [دليل التثبيت](https://lazygophers.github.io/aidog/ar/getting-started/installation).

### ثلاث خطوات

1. **إضافة منصة** — أدخل مفتاح API ونقطة نهاية إحدى منصات الذكاء الاصطناعي
2. **تهيئة الوكيل** — وجّه عنوان URL الأساسي لعميلك إلى الوكيل المحلي:
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **ابدأ الاستخدام** — تُوجَّه الطلبات وتُقاس وتُسجَّل تلقائيًا

تحقق باستخدام curl:

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 يمكن أن يكون مفتاح API في عنوان الوكيل أي قيمة — يعيد AiDog التوجيه بمفتاحك الحقيقي المُهيّأ.

📖 الدرس الكامل: [البدء السريع](https://lazygophers.github.io/aidog/ar/getting-started/quick-start).

## 🧩 الحزمة التقنية

| الطبقة | التقنية |
| --- | --- |
| إطار سطح المكتب | Tauri 2.0 |
| الواجهة الأمامية | React 19 + TypeScript + Vite |
| الواجهة الخلفية | Rust + وكيل Axum + تخزين SQLite |
| البناء | Yarn + Vite |

## 🛠️ التطوير

```bash
yarn                          # تثبيت تبعيات الواجهة الأمامية
yarn tauri dev                # تشغيل تطبيق سطح المكتب (dev)
yarn build                    # بناء الواجهة الأمامية (tsc && vite build)
cd src-tauri && cargo build   # بناء الواجهة الخلفية Rust
cd src-tauri && cargo clippy  # فحص Rust (يجب تنظيف التحذيرات)
cd src-tauri && cargo test    # اختبارات Rust
```

المتطلبات المسبقة: Node.js ≥ 18، Yarn 4.x، سلسلة أدوات Rust، Tauri CLI.

## 📚 التوثيق

موقع التوثيق الكامل: <https://lazygophers.github.io/aidog>

- [البدء السريع](https://lazygophers.github.io/aidog/ar/getting-started/quick-start)
- [بروتوكولات المنصات](https://lazygophers.github.io/aidog/ar/platforms/protocols)
- [المجموعات والتوجيه](https://lazygophers.github.io/aidog/ar/groups/routing-rules)
- [تكامل Codex](https://lazygophers.github.io/aidog/ar/proxy/codex-integration)
- [الإحصاءات والتسعير](https://lazygophers.github.io/aidog/ar/stats/usage-stats)

## 🌍 اللغات

| اللغة | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## بيئة التطوير الموصى بها

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## الترخيص

MIT
