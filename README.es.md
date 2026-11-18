<div align="center">

# AiDog

**Administra plataformas de IA en tu equipo y reenvía las solicitudes de los clientes a las plataformas que elijas.**

Una aplicación de escritorio con proxy local, enrutamiento por grupos, asignación de modelos, registro de solicitudes y estimación de uso.

[Documentación](https://lazygophers.github.io/aidog/en/) · [Última versión](https://github.com/lazygophers/aidog/releases/latest) · [Informar de un problema](https://github.com/lazygophers/aidog/issues)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · Español · [日本語](README.ja.md)

</div>

AiDog ejecuta un servicio proxy en tu equipo. Guarda las plataformas upstream y las claves API en AiDog y luego conecta Claude Code, Codex, pi u otros clientes compatibles al proxy local. Las solicitudes siguen enviándose al upstream seleccionado; ejecutar el servicio localmente no significa que las solicitudes permanezcan en tu equipo.

## Para quién es

- Usas varios servicios de IA y quieres gestionar las credenciales de las plataformas y los modelos en una sola aplicación.
- Quieres que distintos clientes usen grupos, asignaciones de modelos o plataformas candidatas diferentes.
- Necesitas consultar el estado de las solicitudes, el uso, el coste estimado y los resultados del enrutamiento.
- Quieres que Claude Code, Codex o pi compartan un único punto de acceso al proxy local.

## Funciones

![Vista general de AiDog](screenshots/home-light.png)

![Gestión de plataformas de IA](screenshots/platforms-light.png)

*Capturas ilustrativas con datos simulados.*

### Proxy local y enrutamiento

- Gestiona las plataformas preconfiguradas y configura plataformas personalizadas.
- Aísla las solicitudes de los clientes mediante grupos y asocia plataformas propias a cada grupo.
- Selecciona plataformas candidatas según la asignación de modelos, el protocolo del endpoint y la disponibilidad.
- Las ranuras con varios modelos admiten cadenas ordenadas; si una solicitud falla, se prueban los candidatos siguientes.
- Admite el reenvío y la adaptación de los protocolos OpenAI, Anthropic, Gemini y otros; las capacidades reales dependen del endpoint de la plataforma y del tipo de solicitud.
- Admite reglas de middleware, configuración de modelos por franjas horarias y programación de enrutamiento según el estado de salud.

### Registros y estadísticas

- Consulta solicitudes del proxy y registros independientes de pruebas, consultas de saldo y solicitudes de listas de modelos.
- Conserva o elimina el contenido sin procesar de ambos lados de una solicitud según la configuración; se ocultan los encabezados sensibles.
- Estima el coste a partir del uso de la respuesta y de los precios integrados de los modelos. Las estimaciones no sustituyen la facturación del upstream.
- Consulta saldos de plataformas, cuotas y estadísticas de uso por grupo; estas métricas usan criterios distintos y no son directamente equivalentes.

### Clientes y herramientas

- Gestiona la configuración de Claude Code, Codex y pi, y genera instrucciones de conexión para cada grupo.
- Gestiona servidores MCP, skills, notificaciones y ajustes del sistema.
- Puedes ejecutar opcionalmente un kernel headless y gestionarlo mediante una API restringida al acceso local.

## Instalación

Descarga de [GitHub Releases](https://github.com/lazygophers/aidog/releases/latest) el instalador correspondiente a tu sistema. La página de cada versión indica los paquetes, arquitecturas y requisitos disponibles por sistema.

Al iniciar AiDog, el proxy local escucha por defecto en `127.0.0.1:9890`. De forma predeterminada solo escucha en la dirección de loopback; no expongas la API de gestión a Internet.

## Inicio rápido

1. Añade una plataforma en «Plataformas de IA» e indica el protocolo, la Base URL y la clave API del upstream.
2. Crea un grupo en la página de plataformas y añade plataformas al grupo.
3. En el cliente, utiliza el comando de conexión generado para el grupo o sigue la guía de integración para configurar la dirección del proxy local y las credenciales del grupo.
4. Envía una solicitud y confirma en el registro del proxy el grupo, la plataforma upstream, el modelo y el resultado.

[Guía de instalación](https://lazygophers.github.io/aidog/en/getting-started/installation) · [Primera configuración](https://lazygophers.github.io/aidog/en/getting-started/first-config) · [Verificar la primera solicitud](https://lazygophers.github.io/aidog/en/getting-started/first-request)

## Datos y privacidad

AiDog guarda los ajustes y las bases de datos en tu equipo. Las solicitudes del proxy reenvían el contenido necesario al servicio upstream seleccionado. Que los registros conserven los cuerpos de las solicitudes y respuestas depende de la configuración; aunque se desactive el registro del contenido sin procesar, se pueden seguir guardando metadatos como el estado, el modelo y el uso. Consulta [Datos y privacidad](https://lazygophers.github.io/aidog/en/core-concepts/data-privacy).

## Ejecutar desde el código fuente

Instala primero las herramientas y dependencias del sistema indicadas en los [requisitos oficiales de Tauri](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
yarn tauri dev
```

Crear un instalador:

```bash
yarn tauri build
```

## Licencia

AiDog se distribuye bajo la licencia [GNU AGPL-3.0-or-later](LICENSE).
