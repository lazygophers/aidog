<div align="center">

# 🐕 AiDog

**Единый шлюз API для ИИ**

Агрегация нескольких платформ · Умная маршрутизация · Аналитика использования — кроссплатформенное десктоп-приложение для управления ключами, запросами и расходами на всех ваших ИИ-платформах

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/en/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · `Русский` · [العربية](README.ar.md) · [Español](README.es.md)

</div>

> 📖 **Полная документация**: <https://lazygophers.github.io/aidog/en/>
>
> ℹ️ Документация сайта пока не имеет русской локали — ссылки ведут на английскую версию.

AiDog — это основанный на Tauri десктоп-шлюз API для ИИ, объединяющий управление, маршрутизацию и мониторинг запросов к более чем 50 ИИ-платформам. Он собирает в одном приложении разбросанные ключи API, сопоставления моделей, балансировку нагрузки и аналитику использования — без бэкенд-сервиса, без облака, все данные хранятся локально.

## ✨ Возможности

- **Агрегация платформ** — более 50 пресетов (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen и др.), настройка в один клик
- **Умные группы** — сопоставление запросов по токену Bearer / пути; маршрутизация Failover и Load Balance
- **Сопоставление моделей** — прозрачная подстановка имени модели (напр. `claude-sonnet-4` → `deepseek-chat`)
- **Преобразование протоколов** — двустороннее преобразование между протоколами OpenAI Chat / Completions / Responses, Anthropic и Gemini
- **Балансировка и отказоустойчивость** — автоматический повтор между платформами при сбое, автоматический разрыв цепи / трёхстатусное управление / экспоненциальная задержка
- **Мониторинг использования** — статистика токенов, оценка стоимости, запросы баланса по платформам, отображение квоты Coding Plan
- **Логирование запросов** — три уровня детализации (оригинальный запрос пользователя / восходящий запрос / сводка), у каждого свой переключатель и срок хранения
- **Движок правил middleware** — входящие/исходящие правила: нормализация, переопределение, маскирование, инъекция, фильтрация чувствительных слов, обнаружение ошибок
- **Интеграция с ассистентами кода** — нативная поддержка в один клик для Claude Code, OpenAI Codex и других ассистентов
- **i18n и темы** — 7 языков (вкл. арабский RTL), Liquid Glass и другие темы в светлых/тёмных режимах

## 🚀 Быстрый старт

### Загрузка и установка

Скачайте установщик для вашей платформы (macOS / Windows / Linux) со страницы [GitHub Releases](https://github.com/lazygophers/aidog/releases).

См. [Руководство по установке](https://lazygophers.github.io/aidog/en/getting-started/installation).

### Три шага

1. **Добавить платформу** — введите ключ API и эндпоинт ИИ-платформы
2. **Настроить прокси** — укажите базовый URL API вашего клиента на локальный прокси:
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **Начать использовать** — запросы маршрутизируются, учитываются и логируются автоматически

Проверка через curl:

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 Ключ API в URL прокси может быть любым значением — AiDog переадресует с вашим настроенным реальным ключом.

📖 Полный учебник: [Быстрый старт](https://lazygophers.github.io/aidog/en/getting-started/quick-start).

## 🧩 Технологический стек

| Слой | Технология |
| --- | --- |
| Десктоп-фреймворк | Tauri 2.0 |
| Фронтенд | React 19 + TypeScript + Vite |
| Бэкенд | Rust + прокси Axum + хранилище SQLite |
| Сборка | Yarn + Vite |

## 🛠️ Разработка

```bash
yarn                          # установить зависимости фронтенда
yarn tauri dev                # запустить десктоп-приложение (dev)
yarn build                    # сборка фронтенда (tsc && vite build)
cd src-tauri && cargo build   # сборка бэкенда Rust
cd src-tauri && cargo clippy  # lint Rust (предупреждения должны быть устранены)
cd src-tauri && cargo test    # тесты Rust
```

Требования: Node.js ≥ 18, Yarn 4.x, toolchain Rust, Tauri CLI.

## 📚 Документация

Полный сайт документации: <https://lazygophers.github.io/aidog>

- [Быстрый старт](https://lazygophers.github.io/aidog/en/getting-started/quick-start)
- [Протоколы платформ](https://lazygophers.github.io/aidog/en/platforms/protocols)
- [Группы и маршрутизация](https://lazygophers.github.io/aidog/en/groups/routing-rules)
- [Интеграция Codex](https://lazygophers.github.io/aidog/en/proxy/codex-integration)
- [Статистика и цены](https://lazygophers.github.io/aidog/en/stats/usage-stats)

## 🌍 Языки

| Язык | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |

## Рекомендуемая IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Лицензия

MIT
