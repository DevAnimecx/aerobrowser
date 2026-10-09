# AeroBrowser — El Navegador Personal para Agentes de IA

[![CI](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml)
[![Release](https://github.com/DevAnimecx/aerobrowser/actions/workflows/release.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/releases)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Stdio%20%7C%20HTTP%20%7C%20SSE-green)](https://modelcontextprotocol.io)

**62 herramientas MCP. Un navegador real con aislamiento. Un comando para verificar la instalación.**

AeroBrowser conecta un agente de IA con un navegador real mediante el [Model Context Protocol](https://modelcontextprotocol.io). Incluye un motor gestionado con aislamiento, llamadas agrupadas, capturas diferenciales, sesiones persistentes y configuración de agente en un solo paso.

Originalmente por [DevAnimecx/aerobrowser](https://github.com/DevAnimecx/aerobrowser) · Mantenido bajo Blackvault Technology · Creado por Adarsh Kushwah

---

## Idiomas

- [English](README.md) · [中文](README.zh.md) · [Español](README.es.md)

---

## Instalación

Los binarios se publican como GitHub Releases, con un archivo `SHA256SUMS` que los instaladores verifican.

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.sh | sh
```

```powershell
# Windows PowerShell (5.1 o superior)
irm https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.ps1 | iex; Install-AeroBrowser -Agent opencode
```

Ambos instaladores resuelven la última versión desde la API de GitHub, verifican el
hash SHA-256, confirman que el archivo descargado sea un ejecutable nativo antes de
tocar el `PATH`, y eliminan el binario si no se puede ejecutar.

¿Prefieres compilar desde el código?

```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser && cargo build --release --locked
```

---

## Verifica tu instalación

```sh
aero doctor
```

`aero doctor` comprueba el motor, el soporte de aislamiento, los puertos del
navegador y del demonio, el token de autenticación y la ruta de configuración de
cada agente conocido. Cada comprobación fallida indica la solución concreta.
Usa `--json` para obtener salida legible por máquina.

---

## Conecta tu agente

```sh
aero connect opencode          # OpenCode
aero connect claude-code       # Claude Code
aero connect cursor            # Cursor
aero connect codex             # Codex
aero connect claude-desktop    # Claude Desktop
```

 Existen adaptadores para OpenCode, Claude Code, Claude Desktop, Cursor, VS Code,
Codex, Windsurf, Cline, Roo Code, Hermes y un `aero-mcp.json` genérico. **La
generación de configuración está implementada y probada para todos ellos; ningún
cliente se ha ejecutado contra un servidor real en el entorno de validación.**
«Escribió un archivo de configuración» y «está verificado de extremo a extremo» son
cosas distintas.

---

## Qué hace AeroBrowser

### Menores idas y vueltas

`batch` ejecuta varias herramientas en un solo mensaje MCP sobre un navegador
compartido, informando el resultado de cada llamada:

```json
{"calls": [
  {"name": "navigate", "arguments": {"url": "https://ejemplo.com/login"}},
  {"name": "snapshot", "arguments": {"interactive_only": true}},
  {"name": "fill", "arguments": {"ref": "@e1_4", "value": "usuario@ejemplo.com"}},
  {"name": "click", "arguments": {"ref": "@e1_5"}}
]}
```

Usa `stop_on_error: false` para continuar tras un fallo.

### Solo los elementos que cambiaron

`snapshot { "delta": true }` devuelve los elementos añadidos o modificados y una
lista de referencias que ya no existen, en lugar de todo el árbol de accesibilidad:

```json
{"text": "+ button Iniciar sesión [ref=e4_2]\n~ changed textbox Email [ref=e1_4]\n- removed [ref=e2_7]",
 "elements": [...], "changed": [...], "removed": ["e2_7"],
 "delta": {"baseline": false, "added": 1, "changed": 1, "removed": 1}}
```

La primera llamada, un cambio de documento o un cambio de URL devuelven una
captura completa con `"baseline": false` y un motivo: una respuesta parcial sería
menos útil justo cuando el agente tiene menos contexto.

### Sesiones persistentes

`save_state` / `restore_state` exportan cookies, pestañas, almacenamiento y
posición del desplazamiento, y los aplican en un contexto nuevo antes de que el
navegador arranque. Esto **no** es un punto de control del proceso, y el estado no
se cifra en reposo.

### Un aislamiento que falla cerrado

En Linux x86_64, el motor gestionado Moli se ejecuta en un proceso dedicado con una
política de archivos de Landlock y filtrado de llamadas al sistema mediante
seccomp-BPF, aplicados antes de que arranque el navegador. cgroup v2 puede
imponer límites estrictos de memoria y de procesos, y el OOM afecta solo al
trabajador.

Cuando una política no está implementada, AeroBrowser se niega a ejecutar en lugar
de degradarse. Los perfiles `base`, `web-dev` y `test` están declarados pero **no
implementados**, y producen error. No existe un límite de salida de red a nivel de
kernel: la lista de métodos CDP de `cdp_raw` está limitada al destino actual, no es
un cortafuegos. La aplicación de Seatbelt en macOS no está validada. Windows no
tiene AppContainer ni Job Object; usa allí un navegador externo y considera el
aislamiento como responsabilidad del propio navegador.

Consulta [docs/SECURITY.md](docs/SECURITY.md) para el límite de confianza completo y
[SECURITY.md](SECURITY.md) para lo que **no** se aplica, y
[docs/ROADMAP.md](docs/ROADMAP.md) para lo que falta explícitamente.

---

## Herramientas (62)

| Grupo | Herramientas |
|---|---|
| Navegación | `navigate` `snapshot` `reload` `go_back` `go_forward` `wait_for` `scroll` |
| Interacción | `click` `double_click` `right_click` `fill` `type` `press_key` `hover` `drag` `upload_file` |
| Elementos | `get_text` `get_attribute` `get_bounding_box` `get_computed_style` `get_dom` `get_title` `get_url` `evaluate` |
| Formularios | `select_option` `check` `uncheck` `set_viewport` |
| Medios | `screenshot` `annotate_screenshot` `save_pdf` |
| Pestañas | `list_tabs` `new_tab` `switch_tab` `close_tab` `close` |
| Estado | `save_state` `restore_state` `get_cookies` `set_cookie` `clear_cookies` |
| Pruebas | `test_run` `test_assert_text` `test_assert_attr` `test_assert_url` `test_assert_visible` `test_report` |
| Red | `intercept_request` `get_network_log` `save_har` `get_console_logs` |
| Rendimiento | `get_performance` `get_memory_usage` `set_user_agent` `set_geolocation` |
| Diagnóstico | `server_status` `server_logs` `server_profile_list` `server_profile_set` `server_shutdown` |
| Protocolo | `batch` `cdp_raw` |

`aero tools` imprime el inventario en formato legible por máquina.

---

## Motores

| Motor | Plataforma | Aislamiento | Uso |
|---|---|---|---|
| **Moli** (gestionado) | Linux x86_64 | Landlock + seccomp + cgroup v2 | Páginas no confiables, producción |
| **Chrome / Edge / Chromium** (externo) | Cualquiera | El del propio navegador | Desarrollo, páginas confiables |
| **Lightpanda** | Cualquiera | No validado | Experimental; requiere `--engine-path` |

---

## Recetas de prueba

```
open https://ejemplo.com/login
wait_for #email
fill #email "usuario@ejemplo.com"
click #submit
assert_text .welcome "Bienvenido de nuevo"
assert_url "https://ejemplo.com/dashboard"
```

```sh
aero test examples/login.aero --report results.html
```

`results.html` es un informe autónomo con tema claro y oscuro, duraciones relativas
al paso más lento y detalle de errores desplegable, sin registrar argumentos —
seguro para adjuntar a una revisión.

---

## Documentación

| Documento | Contenido |
|---|---|
| [Inicio rápido](docs/QUICKSTART.md) | Primera configuración funcional |
| [Instalación](docs/INSTALL.md) | Todas las plataformas y agentes |
| [Referencia API](docs/API.md) | Las 62 herramientas, esquemas y gramática DSL |
| [Habilidad para agentes](docs/AGENT-SKILL.md) | Cómo debe conducir un agente AeroBrowser |
| [Arquitectura](docs/ARCHITECTURE.md) | Diseño del sistema |
| [Seguridad](SECURITY.md) | Límites de confianza y limitaciones conocidas |
| [Despliegue](docs/DEPLOYMENT.md) | Demonio, cgroups, transportes |
| [Hoja de ruta](docs/ROADMAP.md) | Implementado frente a pendiente, con honestidad |
| [Notas de la versión 0.5](docs/RELEASE-0.5.md) | Qué cambió |

---

## Contribuir

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-encoding.py     # rechaza texto corrupto
python3 scripts/check-docs.py         # verifica los enlaces
```

Consulta [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Licencia

Business Source License 1.1 con una Concesión de Uso Adicional. Gratuito para uso
personal, de investigación, educativo y empresarial interno por debajo de 10
millones de USD de ingresos anuales. El uso en producción por encima de ese umbral,
y ofrecer la automatización de navegador como servicio, requieren una licencia
comercial.

Consulta [LICENSE](LICENSE) para los términos completos.