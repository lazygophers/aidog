<div align="center" dir="rtl">

# 🐕 AiDog

**بوابة موحّدة لـ AI API**

تطبيق سطح المكتب · بدون سحابة · أكثر من 50 منصة في مكان واحد · توجيه ذكي · تحليلات الاستخدام

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/ar/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-AGPL_3.0-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · `العربية` · [Español](README.es.md) · [日本語](README.ja.md)

</div>

---

> 📖 **التوثيق الكامل**: <https://lazygophers.github.io/aidog/ar/>

AiDog هو **بوابة AI API لسطح المكتب** مبنية على Tauri. يوحّد إدارة الطلبات وتوجيهها ومراقبتها عبر أكثر من 50 منصة ذكاء اصطناعي — يجمع مفاتيح API المبعثرة، وربط النماذج، وموازنة الحمل، وتحليلات الاستخدام، وإعداد مساعدات البرمجة في تطبيق واحد. بدون خدمة خلفية، بدون سحابة؛ تبقى كل البيانات في قاعدة بيانات SQLite محلية.

![AiDog — اللوحة الرئيسية](screenshots/dashboard.png)

## ما المشكلة التي يحلها

| مشكلتك | كيف يعالجها AiDog |
| --- | --- |
| مفاتيح API مبعثرة عبر عشرات المنصات، التبديل متعب | **تجميع متعدد المنصات** — أكثر من 50 إعدادًا مسبقًا، أدر كل مفتاح في مكان واحد |
| منصة واحدة تتعطل ويتوقف سير عملك بالكامل | **التبديل عند الفشل + موازنة الحمل** — إعادة محاولة تلقائية، قواطع دوائر، جدولة بين المنصات |
| Claude Code / Codex / كل عميل مضبوط على حدة | **تكامل أصلي مع مساعدات البرمجة** — تصدير الإعداد بنقرة واحدة، كل الزيارات عبر البروكسي |
| لا تعرف كم تنفق شهريًا ولا أي منصة على وشك النفاد | **مراقبة الاستخدام** — توكنز + تقدير التكلفة + الرصيد + حصة Coding Plan |
| بياناتك لا تريدها في السحابة ولا لد طرف ثالث | **محلي بحت** — البروكسي + القاعدة على جهازك، صفر تسريب |

## الميزات الأساسية

### 🌐 البوابة والتوجيه
- **تجميع متعدد المنصات** — أكثر من 50 إعدادًا مسبقًا للمنصات (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter وغيرها)، إعداد بنقرة واحدة
- **تجميع ذكي** — مطابقة الطلبات برمز Bearer / المسار؛ تبديل عند الفشل وموازنة الحمل
- **ربط النماذج** — استبدال شفاف لاسم النموذج (مثال `claude-sonnet-4` → `deepseek-chat`)
- **تحويل البروتوكول** — ثنائي الاتجاه بين OpenAI Chat / Completions / Responses و Anthropic و Gemini
- **قواطع الدوائر والجدولة** — فصل تلقائي للمنصات الشاذة، إدارة ثلاثية الحالة، تراجع أُسّي، جدولة ذكية داخل المجموعة
- **محرّك قواعد الوسائط** — قواعد داخلية/خارجية: تسوية، تجاوز، تنقيح، حقن، تصفية كلمات حساسة، كشف أخطاء، مع إعدادات مسبقة مدمجة

### 📊 المراقبة والإحصاءات
- **مراقبة الاستخدام** — إحصاءات التوكنز، تقدير التكلفة (مزامنة تلقائية للأسعار + ميزانية يدوية)
- **استعلامات الرصيد** — سحب رصيد كل منصة في الوقت الحقيقي
- **حصة Coding Plan** — عرض حصة Coding Plan لـ DeepSeek / Kimi / GLM والعد التنازلي
- **سجلّات الطلبات** — ثلاث مستويات دقة (الطلب الأصلي للمستخدم / الطلب المنبعث / الملخّص)، لكل واحد مفتاحه وفترة احتفاظه

### 🤖 تكامل مساعدات البرمجة
- **Claude Code** — تكامل أصلي: تحرير الإعداد، استيراد/تصدير بنقرة واحدة، نصوص StatusLine، Hooks، مزامنة الإعداد لكل مجموعة
- **OpenAI Codex** — تكامل أصلي: محرّر `~/.codex/config.toml`، توجيه تلقائي لـ Responses API
- **إدارة MCP** — تخزين مركزي في القاعدة + تفعيل لكل وكيل + مسح واستيراد + إخفاء الحقول الحساسة
- **إدارة Skills** — قائمة skills موحّدة عبر الأنظمة مبنية على npx + تفعيل لكل عنصر
- **إشعارات النظام** — إعلانات TTS / منبثقة / صندوق وارد + حقن hook لـ Claude Code/Codex بنقرة واحدة

### 🎨 التخصيص
- **نظام الثيمات** — 3 محاور: 9 أنماط (Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy) × 12 لوحة ألوان مُسمّاة (Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl وغيرها) × وضعي فاتح/داكن
- **التدويل** — 8 لغات (تشمل العربية RTL)
- **الاستيراد والتصدير** — حاوية ملف واحد مشفّرة AES-256-GCM ‎`.aidogx`، 7 نطاقات مع حل تعارض لكل عنصر
- **الصينية + شريط الحالة** — إجراءات سريعة من صينية النظام + نصوص شريط حالة قابلة للتخصيص (Python + uv)

## التثبيت

### متطلبات النظام

| النظام | الحد الأدنى للإصدار | ملاحظات |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | يتطلب WebKit2GTK |

**التحميل** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. نزّل `.dmg` من [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. نقر مزدوج للفتح، اسحب **AiDog** إلى مجلد `Applications`
3. عند أول تشغيل، **نقر بالزر الأيمن** على التطبيق → اختر «فتح» (تجاوز Gatekeeper — التطبيق غير موقّع)

> ⚠️ إذا ظهر عند أول تشغيل «لا يمكن التحقق من المطوّر»: `إعدادات النظام → الخصوصية والأمان → افتح على أي حال`.

### Windows

1. نزّل مثبّت `.msi` من [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. نقر مزدوج على المثبّت واتبع المطالبات
3. إذا حظر SmartScreen، انقر «مزيد من المعلومات → تشغيل على أي حال»

### Linux

```bash
# حزمة DEB
sudo dpkg -i aidog_0.1.0_amd64.deb

# أو AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> يتطلب Linux أولًا اعتمادية WebKit2GTK: `sudo apt install libwebkit2gtk-4.1-dev` (Debian/Ubuntu).

### أول تشغيل

بعد التثبيت، شغّل AiDog — وسيقوم تلقائيًا بـ:

1. تشغيل خادم البروكسي المحلي (افتراضيًا `http://127.0.0.1:9876`)
2. إنشاء قاعدة بيانات SQLite المحلية (`~/.aidog/aidog.db`)
3. إظهار الواجهة الرئيسية وإرشادك لإضافة أول منصة

## البدء السريع (3 خطوات)

### الخطوة 1: إضافة منصة

![AiDog — إضافة منصة](screenshots/add-platform.png)

1. انقر **«المنصات»** في التنقل الأيسر
2. انقر **«+ إضافة منصة»**
3. املأ: **الاسم** (مثال `My OpenAI`)، **Base URL** (مثال `https://api.openai.com/v1`، يشمل بادئة الإصدار `/v1`)، **API Key**
4. حفظ

> 💡 Base URL يشمل بادئة الإصدار؛ يضيف AiDog ‎`/chat/completions` تلقائيًا — لا حاجة لبناء المسار يدويًا.

### الخطوة 2: توجيه العميل إلى البروكسي

في التطبيق الذي يستهلك AI APIs، غيّر عنوان API إلى عنوان بروكسي AiDog:

```
http://127.0.0.1:9876/proxy/v1
```

مفتاح API يمكن أن يكون **أي قيمة** — يحوّل AiDog باستخدام مفتاحك الحقيقي المضبوط.

### الخطوة 3: التحقق

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

استجابة AI طبيعية تعني اكتمال الإعداد. الطلبات موجَّهة ومقاسة ومسجّلة تلقائيًا.

## تفاصيل تكامل العملاء

### Claude Code

يقدّم AiDog تكاملاً كاملًا في **«الإعدادات → Claude Code»** (تحرير النموذج/الصلاحيات/الصندوق الرملي/الإضافات/Hooks/StatusLine، استيراد/تصدير بنقرة واحدة).

**الخيار 1: متغيرات البيئة (الأسرع)**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**الخيار 2: تصدير الإعداد بنقرة واحدة**

انقر «صدّر إلى Claude Code» في «الإعدادات → Claude Code»؛ يكتب AiDog ‎`~/.claude.json`:

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**عزل لكل مجموعة** — انقر «مزامنة إعدادات المجموعة» لتوليد إعدادات مستقلة لكل مجموعة (‎`~/.aidog/settings.<اسم-المجموعة>.json`)؛ زر «Claude» في بطاقة المجموعة ينسخ أمر التشغيل.

### OpenAI Codex

حرّر ‎`~/.codex/config.toml` (أو داخل تبويب «الإعدادات → Codex»):

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> يستخدم Codex واجهة Responses API (‎`/v1/responses`)؛ يكتشفها AiDog ويوجّهها تلقائيًا.

### أي عميل متوافق مع OpenAI / Anthropic

وجّه ‎`base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` للعميل إلى ‎`http://127.0.0.1:9876/proxy/v1` واستخدم أي قيمة كمفتاح.

> 🔐 **مصادقة المجموعة** — ضع **اسم المجموعة** كمفتاح في عنوان البروكسي؛ يوجّه AiDog إلى المجموعة المطابقة برمز Bearer: `Authorization: Bearer <اسم_المجموعة>`.

![AiDog — الإعدادات](screenshots/settings.png)

## البناء من المصدر

```bash
# الاستنساخ
git clone https://github.com/lazygophers/aidog.git
cd aidog

# تثبيت الاعتماديات
yarn install

# وضع التطوير
yarn tauri dev

# بناء الإنتاج
yarn tauri build
```

**المتطلبات المسبقة** — Node.js ≥ 18، Yarn 4.x، سلسلة أدوات Rust (rustup)، Tauri CLI، اعتماديات النظام لكل نظام تشغيل (راجع [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)).

## حزمة التقنيات

| الطبقة | التقنية |
| --- | --- |
| إطار سطح المكتب | Tauri 2.0 |
| الواجهة الأمامية | React 19 + TypeScript + Vite |
| الواجهة الخلفية | Rust + بروكسي Axum + تخزين SQLite |
| التوثيق | Rspress (موقع بـ 8 لغات) |
| البناء | Yarn 4 + Vite + cargo |

## التوثيق

موقع التوثيق الكامل 👉 <https://lazygophers.github.io/aidog/ar/>

| الموضوع | الرابط |
| --- | --- |
| البدء السريع | [/getting-started/quick-start](https://lazygophers.github.io/aidog/ar/getting-started/quick-start) |
| دليل التثبيت | [/getting-started/installation](https://lazygophers.github.io/aidog/ar/getting-started/installation) |
| بروتوكولات المنصات | [/platforms/protocols](https://lazygophers.github.io/aidog/ar/platforms/protocols) |
| المجموعات والتوجيه | [/groups/routing-rules](https://lazygophers.github.io/aidog/ar/groups/routing-rules) |
| الجدولة الذكية | [/groups/scheduling](https://lazygophers.github.io/aidog/ar/groups/scheduling) |
| تكامل Codex | [/proxy/codex-integration](https://lazygophers.github.io/aidog/ar/proxy/codex-integration) |
| قواعد الوسائط | [/middleware](https://lazygophers.github.io/aidog/ar/middleware/) |
| إحصاءات الاستخدام والتسعير | [/stats/usage-stats](https://lazygophers.github.io/aidog/ar/stats/usage-stats) |
| مرجع API | [/api/api-reference](https://lazygophers.github.io/aidog/ar/api/api-reference) |

## README متعدد اللغات

| اللغة | الملف |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## IDE الموصى به

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## شكر وتقدير

[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)

شكراً لمجتمع [LINUX DO](https://linux.do).

## الترخيص

[GNU AGPL-3.0-or-later](LICENSE) © AiDog

يُرخص هذا المشروع تحت رخصة GNU Affero العامة الإصدار 3 أو أحدث. إذا قمت بتعديل هذا البرنامج وقدّمته كخدمة عبر الشبكة، يجب إفصاح المستخدمين عن الشفرة المصدرية المقابلة.
