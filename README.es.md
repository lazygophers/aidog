<div align="center">

# 🐕 AiDog

**Gateway de IA API unificado**

App de escritorio · Sin nube · 50+ plataformas en un solo lugar · Enrutamiento inteligente · Analítica de uso

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/es/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · `Español` · [日本語](README.ja.md)

</div>

---

> 📖 **Documentación completa**: <https://lazygophers.github.io/aidog/es/>

AiDog es un **gateway de IA API de escritorio** basado en Tauri. Unifica la gestión, el enrutamiento y la supervisión de peticiones a más de 50 plataformas de IA — consolidando claves API dispersas, mapeos de modelos, balanceo de carga, analítica de uso y configuración de asistentes de código en una sola app. Sin servicio backend, sin nube; todos los datos permanecen en una base SQLite local.

![AiDog — Panel principal](screenshots/dashboard.png)

## Qué problema resuelve

| Tu dolor | Cómo lo trata AiDog |
| --- | --- |
| Claves API dispersas en una docena de plataformas, difícil cambiar | **Agregación multiplataforma** — 50+ presets, gestionar cada clave en un solo lugar |
| Una plataforma cae y todo tu flujo se detiene | **Failover + balanceo de carga** — auto-reintento, circuit breaking, planificación entre plataformas |
| Claude Code / Codex / cada cliente configurado por separado | **Integración nativa de asistentes de código** — export de config en un clic, todo el tráfico por el proxy |
| Sin idea de cuánto gastas al mes ni qué plataforma se agota pronto | **Monitor de uso** — tokens + estimación de coste + saldo + cuota Coding Plan |
| Datos que no quieres en la nube ni en manos de terceros | **Puramente local** — proxy + base en tu máquina, cero exfiltración |

## Funciones principales

### 🌐 Gateway y enrutamiento
- **Agregación multiplataforma** — 50+ presets de plataforma (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter, etc.), configuración en un clic
- **Agrupación inteligente** — coincidencia por token Bearer / ruta; Failover y Load Balance
- **Mapeo de modelos** — sustitución transparente del nombre del modelo (p. ej. `claude-sonnet-4` → `deepseek-chat`)
- **Conversión de protocolos** — bidireccional entre OpenAI Chat / Completions / Responses, Anthropic y Gemini
- **Circuit breaking y planificación** — auto-desconexión de plataformas anómalas, gestión triestado, backoff exponencial, planificación inteligente dentro del grupo
- **Motor de reglas middleware** — reglas inbound/outbound: normalización, override, redacción, inyección, filtrado de palabras sensibles, detección de errores, con presets integrados

### 📊 Monitor y estadísticas
- **Monitor de uso** — estadísticas de tokens, estimación de coste (auto-sync de precios + presupuesto manual)
- **Consultas de saldo** — saldo de cada plataforma en tiempo real
- **Cuota Coding Plan** — muestra y cuenta atrás de la cuota Coding Plan DeepSeek / Kimi / GLM
- **Logs de peticiones** — tres niveles de granularidad (petición original del usuario / petición upstream / resumen), cada uno con su conmutador y retención

### 🤖 Integración de asistentes de código
- **Claude Code** — integración nativa: edición de config, import/export en un clic, scripts StatusLine, Hooks, sincronización de config por grupo
- **OpenAI Codex** — integración nativa: editor de `~/.codex/config.toml`, auto-enrutamiento de la API Responses
- **Gestión MCP** — almacenamiento centralizado en DB + activación por agente + escanear e importar + enmascarado de campos sensibles
- **Gestión Skills** — lista unificada multiplataforma basada en npx + activación por elemento
- **Notificaciones del sistema** — anuncios TTS / popup / bandeja de entrada + inyección en un clic del hook Claude Code/Codex

### 🎨 Personalización
- **Sistema de temas** — 3 ejes: 9 estilos (Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy) × 12 paletas con nombre (Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl, etc.) × modos claro/oscuro
- **Internacionalización** — 8 idiomas (incl. árabe RTL)
- **Importar y exportar** — contenedor de un solo fichero cifrado AES-256-GCM `.aidogx`, 7 ámbitos con resolución de conflictos por elemento
- **Bandeja + barra de estado** — acciones rápidas desde la bandeja del sistema + scripts de barra de estado personalizables (Python + uv)

## Instalación

### Requisitos del sistema

| SO | Versión mínima | Notas |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | Requiere WebKit2GTK |

**Descarga** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. Descarga el `.dmg` desde [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Doble clic para abrir, arrastra **AiDog** a la carpeta `Aplicaciones`
3. En el primer arranque, **clic derecho** en la app → selecciona «Abrir» (omitir Gatekeeper — la app no está firmada)

> ⚠️ Si el primer arranque muestra «no se puede verificar el desarrollador»: `Ajustes del Sistema → Privacidad y seguridad → Abrir de todos modos`.

### Windows

1. Descarga el instalador `.msi` desde [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Doble clic en el instalador y sigue las indicaciones
3. Si SmartScreen lo bloquea, haz clic en «Más información → Ejecutar de todos modos»

### Linux

```bash
# Paquete DEB
sudo dpkg -i aidog_0.1.0_amd64.deb

# O AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux primero requiere la dependencia WebKit2GTK: `sudo apt install libwebkit2gtk-4.1-dev` (Debian/Ubuntu).

### Primer arranque

Tras instalar, lanza AiDog — automáticamente:

1. Inicia el servidor proxy local (por defecto `http://127.0.0.1:9876`)
2. Crea la base SQLite local (`~/.aidog/aidog.db`)
3. Muestra la interfaz principal y te guía para añadir tu primera plataforma

## Inicio rápido (3 pasos)

### Paso 1: Añadir una plataforma

![AiDog — Añadir plataforma](screenshots/add-platform.png)

1. Haz clic en **«Plataformas»** en la navegación izquierda
2. Haz clic en **«+ Añadir plataforma»**
3. Rellena: **Nombre** (p. ej. `Mi OpenAI`), **Base URL** (p. ej. `https://api.openai.com/v1`, incluyendo el prefijo de versión `/v1`), **API Key**
4. Guardar

> 💡 La Base URL ya incluye el prefijo de versión; AiDog añade `/chat/completions` automáticamente — no hace falta montar la ruta a mano.

### Paso 2: Apuntar el cliente al proxy

En la app que consume APIs de IA, cambia la dirección de API por la dirección del proxy AiDog:

```
http://127.0.0.1:9876/proxy/v1
```

La clave API puede ser **cualquier valor** — AiDog reenvía con tu clave real configurada.

### Paso 3: Verificar

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

Una respuesta normal de IA significa que la configuración está completa. Las peticiones se enrutan, miden y registran automáticamente.

## Integración de clientes en detalle

### Claude Code

AiDog ofrece integración completa en **«Ajustes → Claude Code»** (editar modelo/permisos/sandbox/plugins/Hooks/StatusLine, import/export en un clic).

**Opción 1: Variables de entorno (lo más rápido)**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**Opción 2: Export de config en un clic**

Haz clic en «Exportar a Claude Code» en «Ajustes → Claude Code»; AiDog escribe `~/.claude.json`:

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**Aislamiento por grupo** — haz clic en «Sincronizar ajustes de grupo» para generar configs independientes por grupo (`~/.aidog/settings.<nombre-grupo>.json`); el botón «Claude» de la tarjeta de grupo copia el comando de arranque.

### OpenAI Codex

Edita `~/.codex/config.toml` (o en la pestaña «Ajustes → Codex»):

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex usa la API Responses (`/v1/responses`); AiDog la detecta y enruta automáticamente.

### Cualquier cliente compatible OpenAI / Anthropic

Apunta el `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` del cliente a `http://127.0.0.1:9876/proxy/v1` y usa cualquier valor como clave.

> 🔐 **Autenticación por grupo** — Pon el **nombre del grupo** como clave en la dirección del proxy; AiDog enruta al grupo correspondiente por token Bearer: `Authorization: Bearer <nombre_grupo>`.

![AiDog — Ajustes](screenshots/settings.png)

## Compilar desde el código fuente

```bash
# Clonar
git clone https://github.com/lazygophers/aidog.git
cd aidog

# Instalar dependencias
yarn install

# Modo dev
yarn tauri dev

# Build producción
yarn tauri build
```

**Requisitos previos** — Node.js ≥ 18, Yarn 4.x, toolchain de Rust (rustup), Tauri CLI, dependencias del sistema por SO (ver [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)).

## Stack tecnológico

| Capa | Tecnología |
| --- | --- |
| Framework de escritorio | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + proxy Axum + almacenamiento SQLite |
| Docs | Rspress (sitio en 8 idiomas) |
| Build | Yarn 4 + Vite + cargo |

## Documentación

Sitio completo de docs 👉 <https://lazygophers.github.io/aidog/es/>

| Tema | Enlace |
| --- | --- |
| Inicio rápido | [/getting-started/quick-start](https://lazygophers.github.io/aidog/es/getting-started/quick-start) |
| Guía de instalación | [/getting-started/installation](https://lazygophers.github.io/aidog/es/getting-started/installation) |
| Protocolos de plataforma | [/platforms/protocols](https://lazygophers.github.io/aidog/es/platforms/protocols) |
| Grupos y enrutamiento | [/groups/routing-rules](https://lazygophers.github.io/aidog/es/groups/routing-rules) |
| Planificación inteligente | [/groups/scheduling](https://lazygophers.github.io/aidog/es/groups/scheduling) |
| Integración Codex | [/proxy/codex-integration](https://lazygophers.github.io/aidog/es/proxy/codex-integration) |
| Reglas middleware | [/middleware](https://lazygophers.github.io/aidog/es/middleware/) |
| Estadísticas y precios | [/stats/usage-stats](https://lazygophers.github.io/aidog/es/stats/usage-stats) |
| Referencia API | [/api/api-reference](https://lazygophers.github.io/aidog/es/api/api-reference) |

## README multilingüe

| Idioma | Fichero |
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

## Agradecimientos

[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)

Gracias a la comunidad [LINUX DO](https://linux.do).

## Licencia

MIT
