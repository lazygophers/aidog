<div align="center">

# 🐕 AiDog

**Единый шлюз AI API**

Десктоп-приложение · Без облака · 50+ платформ в одном · Умная маршрутизация · Аналитика использования

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/ru/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · `Русский` · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

---

> 📖 **Полная документация**: <https://lazygophers.github.io/aidog/ru/>

AiDog — это **десктоп-шлюз AI API** на базе Tauri. Он объединяет управление, маршрутизацию и мониторинг запросов к 50+ AI-платформам — собирая разрозненные API-ключи, мэппинги моделей, балансировку нагрузки, аналитику использования и конфигурацию кодинг-ассистентов в одном приложении. Без бэкенд-сервиса, без облака; все данные остаются в локальной базе SQLite.

![AiDog — Главная панель](screenshots/dashboard.png)

## Какую проблему решает

| Ваша боль | Как AiDog её решает |
| --- | --- |
| API-ключи разбросаны по десятку платформ, болезненно переключаться | **Мультиплатформенная агрегация** — 50+ пресетов, управлять каждым ключом в одном месте |
| Одна платформа падает и весь поток встаёт | **Failover + балансировка** — авто-повтор, circuit breaking, планирование между платформами |
| Claude Code / Codex / каждый клиент настроен отдельно | **Нативная интеграция кодинг-ассистентов** — экспорт конфига в один клик, весь трафик через прокси |
| Не знаете, сколько тратите в месяц и какая платформа скоро иссякнет | **Мониторинг использования** — токены + оценка стоимости + баланс + квота Coding Plan |
| Данные не хотите в облако или третьим лицам | **Полностью локально** — прокси + база на вашей машине, ноль утечек |

## Ключевые функции

### 🌐 Шлюз и маршрутизация
- **Мультиплатформенная агрегация** — 50+ пресетов платформ (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter и т. д.), настройка в один клик
- **Умные группы** — сопоставление запросов по Bearer-токену / пути; Failover и Load Balance
- **Мэппинг моделей** — прозрачная подстановка имени модели (напр. `claude-sonnet-4` → `deepseek-chat`)
- **Конвертация протоколов** — двусторонняя между OpenAI Chat / Completions / Responses, Anthropic и Gemini
- **Circuit breaking и планирование** — авто-отключение аномальных платформ, три-state-управление, экспоненциальный backoff, умное планирование внутри группы
- **Движок middleware-правил** — входящие/исходящие правила: нормализация, переопределение, редакция, инъекция, фильтр чувствительных слов, обнаружение ошибок, со встроенными пресетами

### 📊 Мониторинг и статистика
- **Мониторинг использования** — статистика токенов, оценка стоимости (авто-синх цен + ручной бюджет)
- **Запросы баланса** — баланс каждой платформы в реальном времени
- **Квота Coding Plan** — отображение и обратный отсчёт квоты Coding Plan DeepSeek / Kimi / GLM
- **Логи запросов** — три уровня детализации (исходный запрос пользователя / upstream-запрос / сводка), у каждого свой переключатель и срок хранения

### 🤖 Интеграция кодинг-ассистентов
- **Claude Code** — нативная интеграция: редактирование конфига, импорт/экспорт в один клик, скрипты StatusLine, Hooks, синхронизация конфига по группам
- **OpenAI Codex** — нативная интеграция: редактор `~/.codex/config.toml`, авто-маршрутизация Responses API
- **Управление MCP** — централизованное хранение в БД + переключение по агентам + скан и импорт + маскировка чувствительных полей
- **Управление Skills** — единый кроссплатформенный список skills на базе npx + переключение по элементам
- **Системные уведомления** — TTS-объявления / popup / входящие + инъекция hook Claude Code/Codex в один клик

### 🎨 Персонализация
- **Система тем** — 3 оси: 9 стилей (Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy) × 12 именованных палитр (Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl и т. д.) × светлая/тёмная темы
- **Интернационализация** — 8 языков (вкл. арабский RTL)
- **Импорт и экспорт** — зашифрованный AES-256-GCM однофайловый контейнер `.aidogx`, 7 областей с разрешением конфликтов по элементам
- **Tray + статус-бар** — быстрые действия из системного трея + настраиваемые скрипты статус-бара (Python + uv)

## Установка

### Системные требования

| ОС | Минимальная версия | Примечания |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | Требуется WebKit2GTK |

**Скачать** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. Скачайте `.dmg` из [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Двойной клик для открытия, перетащите **AiDog** в папку `Программы`
3. При первом запуске **правый клик** по приложению → выберите «Открыть» (обход Gatekeeper — приложение не подписано)

> ⚠️ Если при первом запуске появляется «не удаётся проверить разработчика»: `Системные настройки → Конфиденциальность и безопасность → Открыть в любом случае`.

### Windows

1. Скачайте `.msi`-установщик из [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Двойной клик по установщику и следуйте подсказкам
3. Если SmartScreen блокирует, нажмите «Подробнее → Всё равно запустить»

### Linux

```bash
# DEB-пакет
sudo dpkg -i aidog_0.1.0_amd64.deb

# Или AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux сначала требует зависимость WebKit2GTK: `sudo apt install libwebkit2gtk-4.1-dev` (Debian/Ubuntu).

### Первый запуск

После установки запустите AiDog — он автоматически:

1. Запустит локальный прокси-сервер (по умолчанию `http://127.0.0.1:9876`)
2. Создаст локальную базу SQLite (`~/.aidog/aidog.db`)
3. Покажет главный интерфейс и проведёт вас к добавлению первой платформы

## Быстрый старт (3 шага)

### Шаг 1: Добавить платформу

![AiDog — Добавить платформу](screenshots/add-platform.png)

1. Нажмите **«Платформы»** в левой навигации
2. Нажмите **«+ Добавить платформу»**
3. Заполните: **Имя** (напр. `Мой OpenAI`), **Base URL** (напр. `https://api.openai.com/v1`, включая префикс версии `/v1`), **API Key**
4. Сохранить

> 💡 Base URL уже включает префикс версии; AiDog добавляет `/chat/completions` автоматически — путь не нужно собирать вручную.

### Шаг 2: Направить клиент на прокси

В приложении, потребляющем AI API, измените адрес API на адрес прокси AiDog:

```
http://127.0.0.1:9876/proxy/v1
```

API-ключ может быть **любым значением** — AiDog перенаправит с вашим настроенным реальным ключом.

### Шаг 3: Проверка

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

Нормальный AI-ответ означает, что настройка завершена. Запросы автоматически маршрутизируются, измеряются и логируются.

## Интеграция клиентов подробно

### Claude Code

AiDog предоставляет полную интеграцию в **«Настройки → Claude Code»** (редактирование модели/разрешений/песочницы/плагинов/Hooks/StatusLine, импорт/экспорт в один клик).

**Вариант 1: Переменные окружения (быстрее всего)**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**Вариант 2: Экспорт конфига в один клик**

Нажмите «Экспорт в Claude Code» в «Настройки → Claude Code»; AiDog запишет `~/.claude.json`:

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**Изоляция по группам** — нажмите «Синхронизировать настройки группы», чтобы сгенерировать независимые конфиги для каждой группы (`~/.aidog/settings.<имя-группы>.json`); кнопка «Claude» карточки группы копирует команду запуска.

### OpenAI Codex

Отредактируйте `~/.codex/config.toml` (или во вкладке «Настройки → Codex»):

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex использует Responses API (`/v1/responses`); AiDog автоматически её обнаруживает и маршрутизирует.

### Любой OpenAI-/Anthropic-совместимый клиент

Направьте `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` клиента на `http://127.0.0.1:9876/proxy/v1` и используйте любое значение как ключ.

> 🔐 **Групповая аутентификация** — Укажите **имя группы** как ключ в адресе прокси; AiDog маршрутизирует в соответствующую группу по Bearer-токену: `Authorization: Bearer <имя_группы>`.

![AiDog — Настройки](screenshots/settings.png)

## Сборка из исходников

```bash
# Клонировать
git clone https://github.com/lazygophers/aidog.git
cd aidog

# Установить зависимости
yarn install

# Режим разработки
yarn tauri dev

# Продакшен-сборка
yarn tauri build
```

**Предварительные требования** — Node.js ≥ 18, Yarn 4.x, Rust toolchain (rustup), Tauri CLI, системные зависимости по ОС (см. [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)).

## Технологический стек

| Слой | Технология |
| --- | --- |
| Десктоп-фреймворк | Tauri 2.0 |
| Фронтенд | React 19 + TypeScript + Vite |
| Бэкенд | Rust + Axum-прокси + SQLite-хранилище |
| Документация | Rspress (8-язычный сайт) |
| Сборка | Yarn 4 + Vite + cargo |

## Документация

Полный сайт документации 👉 <https://lazygophers.github.io/aidog/ru/>

| Тема | Ссылка |
| --- | --- |
| Быстрый старт | [/getting-started/quick-start](https://lazygophers.github.io/aidog/ru/getting-started/quick-start) |
| Руководство по установке | [/getting-started/installation](https://lazygophers.github.io/aidog/ru/getting-started/installation) |
| Протоколы платформ | [/platforms/protocols](https://lazygophers.github.io/aidog/ru/platforms/protocols) |
| Группы и маршрутизация | [/groups/routing-rules](https://lazygophers.github.io/aidog/ru/groups/routing-rules) |
| Умное планирование | [/groups/scheduling](https://lazygophers.github.io/aidog/ru/groups/scheduling) |
| Интеграция Codex | [/proxy/codex-integration](https://lazygophers.github.io/aidog/ru/proxy/codex-integration) |
| Middleware-правила | [/middleware](https://lazygophers.github.io/aidog/ru/middleware/) |
| Статистика и цены | [/stats/usage-stats](https://lazygophers.github.io/aidog/ru/stats/usage-stats) |
| Справочник API | [/api/api-reference](https://lazygophers.github.io/aidog/ru/api/api-reference) |

## Многоязычный README

| Язык | Файл |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## Рекомендуемая IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Благодарности

[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)

Спасибо сообществу [LINUX DO](https://linux.do).

## Лицензия

MIT
