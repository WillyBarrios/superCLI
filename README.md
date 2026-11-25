# superCLI

Editor de código nativo en **Rust** diseñado para consumir la **menor cantidad de RAM posible**, con **explorador de proyecto (scaffolding) a la izquierda** y **terminal integrada** optimizada para ejecutar CLIs de agentes de programación (Claude Code, Gemini CLI, Codex, Aider, OpenCode, etc.). Compatible con **Windows** y **Linux**.

## Características principales

- **Consumo mínimo de RAM:** Sin Electron ni navegador embebido (`eframe`/`egui` con OpenGL `glow`, *reactive repaint*, 0% CPU en reposo y monitor de RAM en tiempo real en la barra de estado).
- **Gestor de proyecto (panel izquierdo):** Carga perezosa (*lazy loading*), respeto automático a `.gitignore`, creación de archivos/carpetas, filtrado rápido y watcher incremental (`ReadDirectoryChangesW` en Windows / `inotify` no recursivo en Linux).
- **Terminal integrada para Agentes CLI:** ConPTY nativo en Windows y POSIX PTY en Linux (`portable-pty`), emulación VT100/ANSI/Truecolor 24-bit (`vt100`), scrollback acotado a 4,000 líneas por pestaña para evitar picos de RAM, pestañas múltiples, botones de lanzamiento rápido de agentes y posición alternable (**Abajo / Derecha**).
- **Editor de código ligero:** Buffers respaldados por `ropey` (*Rope*), resaltado sintáctico de cero memoria persistente, preservación de `LF`/`CRLF`, *undo/redo* acotado, búsqueda (`Ctrl+F`) y recarga automática cuando un agente CLI modifica un archivo en disco.

## Atajos de teclado

| Atajo | Acción |
| :--- | :--- |
| `Ctrl+P` | Búsqueda rápida de archivos (*Quick Open*) |
| `Ctrl+B` | Mostrar / ocultar gestor de proyecto (izquierda) |
| `Ctrl+J` o ``Ctrl+` `` | Mostrar / ocultar terminal integrada |
| `Ctrl+S` | Guardar archivo activo |
| `Ctrl+F` | Buscar dentro del archivo activo |
| `Ctrl+Z` / `Ctrl+Y` | Deshacer / Rehacer |
| `Ctrl+Shift+C` / `Ctrl+Shift+V` | Copiar / Pegar en la terminal |

## Compilar y ejecutar

### Windows
```powershell
cargo run --release -- C:\ruta\a\tu\proyecto
```
O ejecuta directamente el binario compilado:
```powershell
.\target\release\supercli.exe C:\ruta\a\tu\proyecto
```

### Linux (Ubuntu / Debian)
Instala las dependencias del sistema para ventana/OpenGL y diálogo de carpetas:
```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev libgtk-3-dev
cargo run --release -- /ruta/a/tu/proyecto
```
