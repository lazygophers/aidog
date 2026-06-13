<div align="center">

# 🐕 AiDog

**Pasarela de API de IA unificada**

Agregación multiplataforma · Enrutamiento inteligente · Analítica de uso — una aplicación de escritorio multiplataforma para gestionar claves, solicitudes y gasto en todas tus plataformas de IA

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/es/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **Documentación completa**: <https://lazygophers.github.io/aidog/es/>

AiDog es una pasarela de API de IA de escritorio basada en Tauri que unifica la gestión, el enrutamiento y la supervisión de solicitudes en más de 50 plataformas de IA. Consolida en una sola aplicación las claves API dispersas, las correspondencias de modelos, el balanceo de carga y la analítica de uso — sin servicio backend, sin nube, todos los datos almacenados localmente.

## ✨ Características

- **Agregación multiplataforma** — más de 50 preajustes (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen, etc.), configuración con un clic
- **Agrupación inteligente** — coincidencia de solicitudes por token Bearer / ruta; enrutamiento Failover y Load Balance
- **Correspondencia de modelos** — sustitución transparente del nombre del modelo (p. ej. `claude-sonnet-4` → `deepseek-chat`)
- **Conversión de protocolo** — conversión bidireccional entre los protocolos OpenAI Chat / Completions / Responses, Anthropic y Gemini
- **Balanceo de carga y conmutación** — reintentos automáticos entre plataformas ante fallos, disyuntor / gestión tri-estado / backoff exponencial
- **Supervisión de uso** — estadísticas de tokens, estimación de costes, consultas de saldo por plataforma, visualización de cuota Coding Plan
- **Registro de solicitudes** — tres niveles de granularidad (solicitud original del usuario / solicitud ascendente / resumen), cada uno con su conmutador y retención
- **Motor de reglas middleware** — reglas entrantes/salientes: normalización, sobrescritura, redacción, inyección, filtrado de palabras sensibles, detección de errores
- **Integración de asistentes de código** — soporte nativo con un clic para Claude Code, OpenAI Codex y otros asistentes
- **i18n y temas** — 8 idiomas (incl. árabe RTL), Liquid Glass y otros temas con modos claro/oscuro

## 🚀 Inicio rápido

### Descarga e instalación

Descarga el instalador para tu plataforma (macOS / Windows / Linux) desde [GitHub Releases](https://github.com/lazygophers/aidog/releases).

Consulta la [Guía de instalación](https://lazygophers.github.io/aidog/es/getting-started/installation).

### Tres pasos

1. **Añadir una plataforma** — introduce la clave API y el endpoint de una plataforma de IA
2. **Configurar el proxy** — apunta la URL base de tu cliente al proxy local:
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **Empezar a usar** — las solicitudes se enrutan, miden y registran automáticamente

Verifica con curl:

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 La clave API en la URL del proxy puede ser cualquier valor — AiDog reenvía con tu clave real configurada.

📖 Tutorial completo: [Inicio rápido](https://lazygophers.github.io/aidog/es/getting-started/quick-start).

## 🧩 Pila tecnológica

| Capa | Tecnología |
| --- | --- |
| Framework de escritorio | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + proxy Axum + almacenamiento SQLite |
| Build | Yarn + Vite |

## 🛠️ Desarrollo

```bash
yarn                          # instalar dependencias frontend
yarn tauri dev                # lanzar app de escritorio (dev)
yarn build                    # build frontend (tsc && vite build)
cd src-tauri && cargo build   # build backend Rust
cd src-tauri && cargo clippy  # lint Rust (los warnings deben limpiarse)
cd src-tauri && cargo test    # tests Rust
```

Requisitos previos: Node.js ≥ 18, Yarn 4.x, toolchain de Rust, Tauri CLI.

## 📚 Documentación

Sitio de docs completo: <https://lazygophers.github.io/aidog>

- [Inicio rápido](https://lazygophers.github.io/aidog/es/getting-started/quick-start)
- [Protocolos de plataforma](https://lazygophers.github.io/aidog/es/platforms/protocols)
- [Grupos y enrutamiento](https://lazygophers.github.io/aidog/es/groups/routing-rules)
- [Integración Codex](https://lazygophers.github.io/aidog/es/proxy/codex-integration)
- [Estadísticas y precios](https://lazygophers.github.io/aidog/es/stats/usage-stats)

## 🌍 Idiomas

| Idioma | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## IDE recomendado

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Licencia

MIT
