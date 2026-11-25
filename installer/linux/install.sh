#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC=""

if [[ -f "$SCRIPT_DIR/supercli" ]]; then
  BIN_SRC="$SCRIPT_DIR/supercli"
elif [[ -f "$SCRIPT_DIR/../../target/release/supercli" ]]; then
  BIN_SRC="$SCRIPT_DIR/../../target/release/supercli"
else
  echo "Error: no se encontro el binario 'supercli'. Compila primero con 'cargo build --release'." >&2
  exit 1
fi

INSTALL_BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"

mkdir -p "$INSTALL_BIN_DIR" "$APPS_DIR"
cp "$BIN_SRC" "$INSTALL_BIN_DIR/supercli"
chmod +x "$INSTALL_BIN_DIR/supercli"

DESKTOP_ENTRY="$APPS_DIR/supercli.desktop"
cat > "$DESKTOP_ENTRY" <<EOF
[Desktop Entry]
Type=Application
Name=superCLI
Comment=Editor de codigo ligero para Agentes CLI
Exec=$INSTALL_BIN_DIR/supercli %F
Terminal=false
Categories=Development;TextEditor;IDE;
StartupNotify=true
EOF
chmod +x "$DESKTOP_ENTRY"

if [[ -d "$HOME/Desktop" ]]; then
  cp "$DESKTOP_ENTRY" "$HOME/Desktop/supercli.desktop"
  chmod +x "$HOME/Desktop/supercli.desktop"
  echo "Acceso directo creado en: $HOME/Desktop/supercli.desktop"
fi

echo "superCLI instalado en: $INSTALL_BIN_DIR/supercli"
echo "Lanzador de aplicaciones creado en: $DESKTOP_ENTRY"
echo "Asegurate de tener '$HOME/.local/bin' en tu PATH para ejecutar 'supercli .'."
