#!/bin/bash
# Fabrique et installe Toc Toc sur ce Mac, à partir du code source.
# Usage, dans le Terminal, depuis le dossier du projet :
#   bash scripts/installer-mac.sh
# La première fois, cela prend 5 à 10 minutes (téléchargement des outils et compilation).
set -e
cd "$(dirname "$0")/.."

echo "▶︎ Vérification des outils…"
if ! xcode-select -p >/dev/null 2>&1; then
  echo "Les outils de développement d'Apple manquent : une fenêtre va s'ouvrir."
  echo "Cliquez sur « Installer », attendez la fin, puis relancez ce script."
  xcode-select --install || true
  exit 1
fi
if ! command -v node >/dev/null 2>&1; then
  echo "Node.js manque : installez la version « LTS » depuis https://nodejs.org, puis relancez ce script."
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    echo "▶︎ Installation de Rust (outil officiel rustup)…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

echo "▶︎ Préparation…"
npm install --no-fund --no-audit

echo "▶︎ Fabrication de Toc Toc (quelques minutes)…"
npx tauri build --bundles app

APP="src-tauri/target/release/bundle/macos/Toc Toc.app"
echo "▶︎ Installation dans Applications…"
osascript -e 'quit app "Toc Toc"' >/dev/null 2>&1 || true
sleep 1
rm -rf "/Applications/Toc Toc.app"
cp -R "$APP" /Applications/
# application fabriquée ici : on retire tout marquage « téléchargé d'Internet »
xattr -cr "/Applications/Toc Toc.app" || true
open "/Applications/Toc Toc.app"
echo "✅ Toc Toc est installé dans Applications et vient de s'ouvrir (icône dans la barre de menus, en haut à droite)."
